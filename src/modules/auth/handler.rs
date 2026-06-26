use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use axum_extra::extract::CookieJar;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    infra::jwt,
    shared::{auth::cookie::*, error::AppResult, json::AppJson, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{LoginRequest, LoginResponseData, RegisterRequest},
    repository, service,
};

pub async fn register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    AppJson(body): AppJson<RegisterRequest>,
) -> AppResult<axum::response::Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);

    crate::shared::turnstile::verify_turnstile_from_request(
        &headers,
        &state.config.auth.turnstile_secret,
        &state.config.auth.turnstile_site_key,
        "register",
    )
    .await?;

    tracing::info!(
        module = "auth",
        event = "register_request",
        client_request_id = %client_request_id,
        username = %body.username,
        email_hash = %hex::encode(&Sha256::digest(body.email.as_bytes())[..4]),
        has_display_name = body.display_name.as_deref().map(|value| !value.trim().is_empty()).unwrap_or(false),
        "received registration request"
    );
    let expires_in_seconds = state.config.auth.expires_in_seconds;
    let cookie_secure = state.config.cookie_secure();
    let (login_data, refresh_token) = service::register(
        state,
        body,
        expires_in_seconds,
        REGISTER_DEFAULT_REFRESH_MAX_AGE_SECONDS,
    )
    .await?;
    let access_token = login_data.access_token.clone();
    let refresh_cookie = build_refresh_cookie(
        &refresh_token,
        REGISTER_DEFAULT_REFRESH_MAX_AGE_SECONDS,
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

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "登录成功", body = ApiResponse<LoginResponseData>),
        (status = 400, description = "验证失败或参数错误"),
        (status = 429, description = "触发限流"),
    )
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    AppJson(body): AppJson<LoginRequest>,
) -> AppResult<axum::response::Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);

    crate::shared::turnstile::verify_turnstile_from_request(
        &headers,
        &state.config.auth.turnstile_secret,
        &state.config.auth.turnstile_site_key,
        "login",
    )
    .await?;

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
        (REMEMBER_ME_MAX_AGE_SECONDS, REMEMBER_ME_MAX_AGE_SECONDS)
    } else {
        (expires_in_seconds, SHORT_MAX_AGE_SECONDS)
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
    if let Some(cookie) = jar.get(crate::shared::auth::constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN) {
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
        .get(crate::shared::auth::constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN)
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
    // M-7: 从配置读取 refresh token 过期时间，而非硬编码 7 天
    let refresh_ttl_seconds = state
        .config
        .auth
        .refresh_token_ttl_seconds
        .unwrap_or(604800);
    let expires_at =
        (chrono::Utc::now() + chrono::Duration::seconds(refresh_ttl_seconds as i64))
            .to_rfc3339();

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
    let user = crate::modules::user::service::get_current_user(&state, &user_id).await?;

    let token_version = crate::modules::user::service::get_token_version(&state, &user.id).await?;

    let access_token = jwt::issue_token(
        &state.config.auth.secret,
        state.config.auth.expires_in_seconds,
        user.id.clone(),
        user.username.clone(),
        user.role.parse()?,
        token_version,
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
    let refresh_cookie = build_refresh_cookie(&new_token, REMEMBER_ME_MAX_AGE_SECONDS, cookie_secure);
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
