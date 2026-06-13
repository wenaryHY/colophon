use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use axum_extra::extract::CookieJar;
use uuid::Uuid;

use crate::{
    infra::jwt,
    shared::{auth_constants, error::AppResult, json::AppJson, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{LoginRequest, RegisterRequest},
    repository, service,
};

pub async fn register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    AppJson(body): AppJson<RegisterRequest>,
) -> AppResult<axum::response::Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);

    // Turnstile 验证：配置了 secret 时强制校验，token 缺失或无效均拒绝
    if !state.config.auth.turnstile_secret.is_empty() {
        let token = body
            .turnstile_token
            .as_ref()
            .ok_or_else(|| crate::shared::error::AppError::BadRequest("请完成人机验证".into()))?;
        if !crate::shared::turnstile::verify_turnstile(token, &state.config.auth.turnstile_secret)
            .await
        {
            tracing::warn!(
                module = "auth",
                event = "register_turnstile_failed",
                client_request_id = %client_request_id,
                "Turnstile verification failed"
            );
            return Err(crate::shared::error::AppError::BadRequest(
                "验证失败，请刷新页面重试".into(),
            ));
        }
    }

    tracing::info!(
        module = "auth",
        event = "register_request",
        client_request_id = %client_request_id,
        username = %body.username,
        email = %body.email,
        has_display_name = body.display_name.as_deref().map(|value| !value.trim().is_empty()).unwrap_or(false),
        "received registration request"
    );
    let expires_in_seconds = state.config.auth.expires_in_seconds;
    let cookie_secure = state.config.cookie_secure();
    let (login_data, refresh_token) = service::register(
        state,
        body,
        expires_in_seconds,
        REGISTER_DEFAULT_REFRESH_MAX_AGE_IN_SECONDS,
    )
    .await?;
    let access_token = login_data.access_token.clone();
    let refresh_cookie = build_refresh_cookie(
        &refresh_token,
        REGISTER_DEFAULT_REFRESH_MAX_AGE_IN_SECONDS,
        cookie_secure,
    );
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie)
        .expect("JWT cookie must be ASCII-only; if this fails, check token encoding");
    let json = Json(ApiResponse::success(login_data));

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, refresh_header);
    let session_cookie = build_session_cookie(&access_token, expires_in_seconds, cookie_secure);
    resp_headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&session_cookie)
            .expect("JWT cookie must be ASCII-only; if this fails, check token encoding"),
    );
    Ok((resp_headers, json).into_response())
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    AppJson(body): AppJson<LoginRequest>,
) -> AppResult<axum::response::Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);

    // Turnstile 验证：配置了 secret 时强制校验，token 缺失或无效均拒绝
    if !state.config.auth.turnstile_secret.is_empty() {
        let token = body
            .turnstile_token
            .as_ref()
            .ok_or_else(|| crate::shared::error::AppError::BadRequest("请完成人机验证".into()))?;
        if !crate::shared::turnstile::verify_turnstile(token, &state.config.auth.turnstile_secret)
            .await
        {
            tracing::warn!(
                module = "auth",
                event = "login_turnstile_failed",
                client_request_id = %client_request_id,
                "Turnstile verification failed"
            );
            return Err(crate::shared::error::AppError::BadRequest(
                "验证失败，请刷新页面重试".into(),
            ));
        }
    }

    tracing::info!(
        module = "auth",
        event = "login_request",
        client_request_id = %client_request_id,
        login = %body.login,
        "received login request"
    );
    let expires_in_seconds = state.config.auth.expires_in_seconds;
    let remember_me = body.remember_me.unwrap_or(false);

    let (session_max_age, refresh_max_age) = if remember_me {
        (REMEMBER_ME_MAX_AGE, REMEMBER_ME_MAX_AGE)
    } else {
        (expires_in_seconds, SHORT_MAX_AGE)
    };

    let cookie_secure = state.config.cookie_secure();
    let (login_data, refresh_token) =
        service::login(state, body, session_max_age, refresh_max_age).await?;
    let access_token = login_data.access_token.clone();

    let refresh_cookie = build_refresh_cookie(&refresh_token, refresh_max_age, cookie_secure);
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie)
        .expect("JWT cookie must be ASCII-only; if this fails, check token encoding");
    let json = Json(ApiResponse::success(login_data));

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, refresh_header);
    let session_cookie = build_session_cookie(&access_token, session_max_age, cookie_secure);
    resp_headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&session_cookie)
            .expect("JWT cookie must be ASCII-only; if this fails, check token encoding"),
    );
    Ok((resp_headers, json).into_response())
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
) -> AppResult<axum::response::Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "auth",
        event = "logout_request",
        client_request_id = %client_request_id,
        "received logout request"
    );

    // 撤销 refresh token（如果存在）
    if let Some(cookie) = jar.get(auth_constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN) {
        let token_hash = jwt::hash_token(cookie.value());
        if let Err(e) = repository::revoke_refresh_token(&state.pool, &token_hash).await {
            tracing::warn!(
                module = "auth",
                event = "logout_revoke_failed",
                error = %e,
                "failed to revoke refresh token during logout"
            );
        }
    }

    let json = Json(ApiResponse::success(
        serde_json::json!({ "logged_out": true }),
    ));
    let cookie_secure = state.config.cookie_secure();
    let clear_refresh = build_clear_refresh_cookie(cookie_secure);
    let clear_session = build_clear_session_cookie(cookie_secure);

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&clear_refresh)
            .expect("JWT cookie must be ASCII-only; if this fails, check token encoding"),
    );
    resp_headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&clear_session)
            .expect("JWT cookie must be ASCII-only; if this fails, check token encoding"),
    );
    Ok((resp_headers, json).into_response())
}

pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
) -> AppResult<axum::response::Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "auth",
        event = "refresh_token_request",
        client_request_id = %client_request_id,
        "received refresh token request"
    );

    let token = jar
        .get(auth_constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN)
        .map(|c| c.value().to_string())
        .ok_or_else(|| {
            tracing::warn!(
                module = "auth",
                event = "refresh_token_missing",
                "refresh token cookie not found"
            );
            crate::shared::error::AppError::Unauthorized
        })?;

    let token_hash = jwt::hash_token(&token);
    let (user_id, _expires_at, family_id, used_at) =
        repository::find_valid_refresh_token(&state.pool, &token_hash)
            .await?
            .ok_or_else(|| {
                tracing::warn!(
                    module = "auth",
                    event = "refresh_token_invalid",
                    "refresh token not found or expired"
                );
                crate::shared::error::AppError::Unauthorized
            })?;

    // 并发保护：如果 token 已被使用（前一次 refresh 已标记），直接拒绝
    if used_at.is_some() {
        tracing::warn!(
            module = "auth",
            event = "refresh_token_reused",
            user_id = %user_id,
            "refresh token already used — possible replay or concurrent refresh"
        );
        return Err(crate::shared::error::AppError::Unauthorized);
    }

    // 标记旧 token 已使用
    repository::mark_token_used(&state.pool, &token_hash).await?;

    // 生成新的 refresh_token（同一 family）
    let new_token = jwt::generate_refresh_token();
    let new_hash = jwt::hash_token(&new_token);
    let new_id = Uuid::new_v4().to_string();
    let family = family_id.unwrap_or_else(|| new_id.clone());
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    repository::save_refresh_token(
        &state.pool,
        &new_id,
        &user_id,
        &new_hash,
        &expires_at,
        &family,
    )
    .await?;

    // 签发新 access_token
    let user = crate::modules::user::repository::find_current(&state.pool, &user_id)
        .await?
        .ok_or_else(|| {
            tracing::warn!(
                module = "auth",
                event = "refresh_token_user_not_found",
                user_id = %user_id,
                "user for refresh token not found"
            );
            crate::shared::error::AppError::Unauthorized
        })?;

    let access_token = jwt::issue_token(
        &state.config.auth.secret,
        state.config.auth.expires_in_seconds,
        user.id.clone(),
        user.username.clone(),
        user.role.parse()?,
    )?;

    tracing::info!(
        module = "auth",
        event = "refresh_token_success",
        user_id = %user.id,
        username = %user.username,
        "access token refreshed (rotation)"
    );

    // 设置新 refresh_token cookie + 返回 access_token JSON
    let cookie_secure = state.config.cookie_secure();
    let refresh_cookie = build_refresh_cookie(&new_token, REMEMBER_ME_MAX_AGE, cookie_secure);
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie)
        .expect("JWT cookie must be ASCII-only; if this fails, check token encoding");
    let json = Json(ApiResponse::success(serde_json::json!({
        "access_token": access_token,
    })));

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, refresh_header);
    // 同步更新 session cookie。JWT 内容是新签发的 15 分钟有效期，
    // 但 cookie 本身的 Max-Age 仍沿用登录时的设定（refresh 不改 cookie 存活长度）。
    let session_cookie = build_session_cookie(
        &access_token,
        state.config.auth.expires_in_seconds,
        cookie_secure,
    );
    resp_headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&session_cookie)
            .expect("JWT cookie must be ASCII-only; if this fails, check token encoding"),
    );
    Ok((resp_headers, json).into_response())
}

// ── 时间常量 ──

/// 7 天（秒），用于"记住我"场景
const REMEMBER_ME_MAX_AGE: u64 = 604800;
/// 15 分钟（秒），用于未勾选"记住我"的短期会话
const SHORT_MAX_AGE: u64 = 900;
/// 1 天（秒），注册用户 refresh cookie 默认存活时长
const REGISTER_DEFAULT_REFRESH_MAX_AGE_IN_SECONDS: u64 = 86400;

// ── Cookie helpers ──

/// 构建 refresh_token 的 HttpOnly Secure SameSite=Strict cookie
fn build_refresh_cookie(token: &str, max_age_seconds: u64, cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}={token}; Path=/api/v1/auth/refresh; Max-Age={max_age_seconds}; HttpOnly; SameSite=Strict{secure}",
        name = auth_constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN,
    )
}

/// 清除 refresh_token cookie
fn build_clear_refresh_cookie(cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}=; Path=/api/v1/auth/refresh; Max-Age=0; HttpOnly; SameSite=Strict{secure}",
        name = auth_constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN,
    )
}

/// 构建 session cookie（access_token），Path=/
fn build_session_cookie(access_token: &str, max_age_seconds: u64, cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}={access_token}; Path=/; Max-Age={max_age_seconds}; HttpOnly; SameSite=Strict{secure}",
        name = auth_constants::SESSION_COOKIE_NAME_FOR_JWT_ACCESS_TOKEN,
    )
}

/// 清除 session cookie（access_token）
fn build_clear_session_cookie(cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict{secure}",
        name = auth_constants::SESSION_COOKIE_NAME_FOR_JWT_ACCESS_TOKEN,
    )
}
