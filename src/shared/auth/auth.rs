use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts},
    http::{request::Parts, HeaderMap},
};
use axum_extra::{
    extract::CookieJar,
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use sha2::{Digest, Sha256};
use std::convert::Infallible;

use super::constants as auth_constants;
use super::role::Role;
use crate::shared::error::AppError;
use crate::state::AppState;

// Re-export hash and jwt functions for convenience
pub use crate::infra::hash::*;
pub use crate::infra::jwt::*;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthUser {
    pub id: String,
    #[allow(dead_code)]
    pub username: String,
    pub role: Role,
}

impl AuthUser {
    /// 面向后续 NGAC 演进的鉴权插槽。目前使用 Role 枚举的方法调用，
    /// 后期可以在此通过 Graph / Policy 彻底改造判断逻辑而无需修改多处 Handler。
    ///
    /// ## 支持的动作
    /// - `"admin:access"` — 委托给 `self.role.can_access_admin()`
    /// - 其他 — deny-by-default
    #[allow(dead_code)]
    pub fn has_permission(&self, action: &str) -> bool {
        match action {
            "admin:access" => self.role.can_access_admin(),
            _ => false, // deny-by-default
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthUser);

pub fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    CookieJar::from_headers(headers)
        .get(auth_constants::SESSION_COOKIE_NAME_FOR_JWT_ACCESS_TOKEN)
        .map(|cookie| cookie.value().to_string())
}

/// 从 X-API-Key header 提取并 hash，查询 DB 验证
/// API Key 权限由创建时指定的 permissions 字段决定：
/// - read_only → Role::Member（仅 GET 路由）
/// - read_write → Role::Admin（全权限，包括 /api/v1/admin/*）
async fn authenticate_via_api_key(
    api_key_plaintext: &str,
    app_state: &Arc<AppState>,
) -> Result<Option<AuthUser>, AppError> {
    let key_hash = hex::encode(Sha256::digest(api_key_plaintext.as_bytes()));

    let result = crate::modules::api_key::repository::find_api_key_with_user_by_hash(
        &app_state.pool,
        &key_hash,
    )
    .await;

    match result {
        Ok(Some(row)) => {
            // 检查是否过期
            if let Some(ref expires_at) = row.api_key_expires_at {
                let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
                if expires_at < &now {
                    return Ok(None);
                }
            }

            // 更新 last_used_at（best-effort，失败不影响认证）
            let _ = crate::modules::api_key::repository::update_api_key_last_used_at(
                &app_state.pool,
                &row.api_key_id,
            )
            .await;

            tracing::debug!(
                module = "shared_auth",
                event = "auth_api_key_success",
                path = "",
                user_id = %row.user_id,
                username = %row.username,
                "authenticated via API key"
            );

            // API Key 的 permissions 字段决定该 Key 的权限范围：
            // - "read_only" → Role::Member（无法访问管理后台）
            // - "read_write" → Role::Admin（可访问管理后台）
            let role = row.permissions.parse::<Role>()?;

            Ok(Some(AuthUser {
                id: row.user_id,
                username: row.username,
                role,
            }))
        }
        Ok(None) => Ok(None),
        Err(e) => {
            tracing::error!(
                module = "shared_auth",
                event = "auth_api_key_db_error",
                error = ?e,
                "database error during API key lookup"
            );
            Err(AppError::Sqlx(e))
        }
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = Arc::<AppState>::from_ref(state);

        // ── 1. 尝试 X-API-Key header ──
        if let Some(api_key) = parts
            .headers
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
        {
            if let Some(auth_user) = authenticate_via_api_key(api_key, &app_state).await? {
                return Ok(auth_user);
            }
        }

        // ── 2. 尝试 Bearer Token ──
        let auth_header =
            <TypedHeader<Authorization<Bearer>> as FromRequestParts<S>>::from_request_parts(
                parts, state,
            )
            .await
            .ok();

        let token = if let Some(TypedHeader(Authorization(bearer))) = auth_header {
            bearer.token().to_string()
        } else if let Some(cookie_token) = session_token_from_headers(&parts.headers) {
            cookie_token
        } else {
            tracing::debug!(
                module = "shared_auth",
                event = "auth_missing_credentials",
                path = %parts.uri.path(),
                "no credentials found on request"
            );
            return Err(AppError::Unauthorized);
        };

        let claims = match decode_token(&token, &app_state.config.auth.secret) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    module = "shared_auth",
                    event = "auth_token_decode_failed",
                    path = %parts.uri.path(),
                    error = ?e,
                    "token decode failed"
                );
                return Err(AppError::Unauthorized);
            }
        };

        // ── token_version 自救校验：比对 JWT 中的 version 与 DB 当前值 ──
        let current_version: i32 =
            sqlx::query_scalar("SELECT token_version FROM users WHERE id = ?")
                .bind(&claims.sub)
                .fetch_optional(&app_state.pool)
                .await
                .map_err(|e| {
                    tracing::error!(
                        module = "shared_auth",
                        event = "auth_token_version_query_failed",
                        path = %parts.uri.path(),
                        user_id = %claims.sub,
                        error = ?e,
                        "token_version query failed"
                    );
                    AppError::Sqlx(e)
                })?
                .unwrap_or(1);

        if current_version != claims.token_version {
            tracing::warn!(
                module = "shared_auth",
                event = "auth_token_version_mismatch",
                path = %parts.uri.path(),
                user_id = %claims.sub,
                jwt_version = claims.token_version,
                current_version = current_version,
                "token version mismatch — token has been logically revoked"
            );
            return Err(AppError::Unauthorized);
        }

        tracing::debug!(
            module = "shared_auth",
            event = "auth_success",
            path = %parts.uri.path(),
            user_id = %claims.sub,
            username = %claims.username,
            role = %claims.role,
            "authentication succeeded"
        );

        Ok(Self {
            id: claims.sub,
            username: claims.username,
            role: claims.role,
        })
    }
}

// 为 AuthUser 提供 OptionalFromRequestParts，使 Option<AuthUser>
// 在 axum 0.8 的 handler 签名中可用（认证失败返回 None 而非错误）
impl<S> OptionalFromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <AuthUser as FromRequestParts<S>>::from_request_parts(parts, state).await {
            Ok(user) => Ok(Some(user)),
            Err(_) => Ok(None),
        }
    }
}

impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    Arc<AppState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = <AuthUser as FromRequestParts<S>>::from_request_parts(parts, state).await?;
        if !user.role.can_access_admin() {
            tracing::warn!(
                module = "shared_auth",
                event = "admin_access_denied",
                path = %parts.uri.path(),
                user_id = %user.id,
                username = %user.username,
                role = %user.role,
                "admin access denied"
            );
            return Err(AppError::Forbidden);
        }
        tracing::debug!(
            module = "shared_auth",
            event = "admin_auth_success",
            path = %parts.uri.path(),
            admin_id = %user.id,
            username = %user.username,
            "admin authentication succeeded"
        );
        Ok(Self(user))
    }
}

/// N-4 / M-6: 密码复杂度验证 — 至少 1 个大写 + 1 个小写 + 1 个数字
///
/// 注册和修改密码共用此规则，保持一致性。
pub fn validate_password_complexity(password: &str) -> Result<(), AppError> {
    let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_uppercase || !has_lowercase || !has_digit {
        return Err(AppError::BadRequest(
            "password must contain at least one uppercase letter, one lowercase letter, and one digit".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Role;
    use super::*;

    fn make_user(role: Role) -> AuthUser {
        AuthUser {
            id: "u1".into(),
            username: "testuser".into(),
            role,
        }
    }

    #[test]
    fn admin_has_admin_access() {
        assert!(make_user(Role::Admin).has_permission("admin:access"));
    }

    #[test]
    fn member_denied_admin_access() {
        assert!(!make_user(Role::Member).has_permission("admin:access"));
    }

    #[test]
    fn unknown_permission_denied_by_default() {
        assert!(!make_user(Role::Admin).has_permission("unknown:action"));
    }

    #[test]
    fn session_token_from_headers_returns_none_without_cookie() {
        let headers = HeaderMap::new();
        assert!(session_token_from_headers(&headers).is_none());
    }
}
