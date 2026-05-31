use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use axum_extra::extract::CookieJar;
use uuid::Uuid;

use crate::{
    infra::jwt,
    shared::{error::AppResult, json::AppJson, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{LoginRequest, RegisterRequest},
    repository,
    service,
};

pub async fn register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    AppJson(body): AppJson<RegisterRequest>,
) -> AppResult<axum::response::Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
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
    let (login_data, refresh_token) = service::register(state, body).await?;
    let access_token = login_data.access_token.clone();
    let refresh_cookie = build_refresh_cookie(&refresh_token, REMEMBER_ME_MAX_AGE);
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie).unwrap();
    let json = Json(ApiResponse::success(login_data));

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, refresh_header);
    let session_cookie = build_session_cookie(
        &access_token,
        expires_in_seconds,
    );
    resp_headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&session_cookie).unwrap(),
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
    tracing::info!(
        module = "auth",
        event = "login_request",
        client_request_id = %client_request_id,
        login = %body.login,
        "received login request"
    );
    let expires_in_seconds = state.config.auth.expires_in_seconds;
    let remember_me = body.remember_me.unwrap_or(false);
    let (login_data, refresh_token) = service::login(state, body).await?;
    let access_token = login_data.access_token.clone();

    let (session_max_age, refresh_max_age) = if remember_me {
        (REMEMBER_ME_MAX_AGE as i64, REMEMBER_ME_MAX_AGE)
    } else {
        (expires_in_seconds, SHORT_MAX_AGE)
    };

    let refresh_cookie = build_refresh_cookie(&refresh_token, refresh_max_age);
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie).unwrap();
    let json = Json(ApiResponse::success(login_data));

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, refresh_header);
    let session_cookie = build_session_cookie(
        &access_token,
        session_max_age,
    );
    resp_headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&session_cookie).unwrap(),
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
    if let Some(cookie) = jar.get("inkforge_refresh") {
        let token_hash = jwt::hash_token(cookie.value());
        let _ = repository::revoke_refresh_token(&state.pool, &token_hash).await;
    }

    let json = Json(ApiResponse::success(
        serde_json::json!({ "logged_out": true }),
    ));
    let clear_refresh = build_clear_refresh_cookie();
    let clear_session = build_clear_session_cookie();

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&clear_refresh).unwrap(),
    );
    resp_headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&clear_session).unwrap(),
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
        .get("inkforge_refresh")
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
    let (user_id, _expires_at, family_id, used_at) = repository::find_valid_refresh_token(&state.pool, &token_hash)
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
        &state.pool, &new_id, &user_id, &new_hash, &expires_at, &family,
    ).await?;

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
        user.role.clone(),
    )?;

    tracing::info!(
        module = "auth",
        event = "refresh_token_success",
        user_id = %user.id,
        username = %user.username,
        "access token refreshed (rotation)"
    );

    // 设置新 refresh_token cookie + 返回 access_token JSON
    let refresh_cookie = build_refresh_cookie(&new_token, REMEMBER_ME_MAX_AGE);
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie).unwrap();
    let json = Json(ApiResponse::success(serde_json::json!({
        "access_token": access_token,
    })));

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, refresh_header);
    Ok((resp_headers, json).into_response())
}

// ── Cookie helpers ──

/// 7 天，用于"记住我"
const REMEMBER_ME_MAX_AGE: u64 = 604800;
/// 15 分钟，用于未勾选"记住我"的短期会话
const SHORT_MAX_AGE: u64 = 900;

/// 构建 refresh_token 的 HttpOnly Secure SameSite=Strict cookie
fn build_refresh_cookie(token: &str, max_age_secs: u64) -> String {
    let secure = if cfg!(debug_assertions) {
        ""
    } else {
        "; Secure"
    };
    format!(
        "inkforge_refresh={token}; Path=/api/v1/auth/refresh; Max-Age={max_age_secs}; HttpOnly; SameSite=Strict{secure}"
    )
}

/// 清除 refresh_token cookie
fn build_clear_refresh_cookie() -> String {
    let secure = if cfg!(debug_assertions) { "" } else { "; Secure" };
    format!("inkforge_refresh=; Path=/api/v1/auth/refresh; Max-Age=0; HttpOnly; SameSite=Strict{secure}")
}

/// 构建 session cookie（access_token），15 分钟过期，Path=/
fn build_session_cookie(access_token: &str, max_age_seconds: i64) -> String {
    let secure = if cfg!(debug_assertions) {
        ""
    } else {
        "; Secure"
    };
    format!(
        "inkforge_session={access_token}; Path=/; Max-Age={max_age_seconds}; HttpOnly; SameSite=Strict{secure}"
    )
}

/// 清除 session cookie（access_token）
fn build_clear_session_cookie() -> String {
    let secure = if cfg!(debug_assertions) { "" } else { "; Secure" };
    format!("inkforge_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict{secure}")
}
