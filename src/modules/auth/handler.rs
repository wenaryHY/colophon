use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use axum_extra::extract::CookieJar;

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
    let (payload, refresh_token) = service::register(state, body).await?;
    let access_cookie = build_session_cookie("inkforge_session", &payload.token, ACCESS_TOKEN_MAX_AGE);
    let refresh_cookie = build_session_cookie("inkforge_refresh", &refresh_token, REFRESH_TOKEN_MAX_AGE);
    let access_header = axum::http::HeaderValue::from_str(&access_cookie).unwrap();
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie).unwrap();
    let json = Json(ApiResponse::success(payload));

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, access_header);
    resp_headers.append(axum::http::header::SET_COOKIE, refresh_header);
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
    let (payload, refresh_token) = service::login(state, body).await?;
    let access_cookie = build_session_cookie("inkforge_session", &payload.token, ACCESS_TOKEN_MAX_AGE);
    let refresh_cookie = build_session_cookie("inkforge_refresh", &refresh_token, REFRESH_TOKEN_MAX_AGE);
    let access_header = axum::http::HeaderValue::from_str(&access_cookie).unwrap();
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie).unwrap();
    let json = Json(ApiResponse::success(payload));

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, access_header);
    resp_headers.append(axum::http::header::SET_COOKIE, refresh_header);
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
        // revoke 失败不阻塞登出流程
        let _ = repository::revoke_refresh_token(&state.pool, &token_hash).await;
    }

    let json = Json(ApiResponse::success(
        serde_json::json!({ "logged_out": true }),
    ));
    let clear_session = "inkforge_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax";
    let clear_refresh = "inkforge_refresh=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax";

    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_static(clear_session),
    );
    resp_headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_static(clear_refresh),
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
    let (user_id, _expires_at) = repository::find_valid_refresh_token(&state.pool, &token_hash)
        .await?
        .ok_or_else(|| {
            tracing::warn!(
                module = "auth",
                event = "refresh_token_invalid",
                "refresh token not found or expired"
            );
            crate::shared::error::AppError::Unauthorized
        })?;

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

    let access_cookie = build_session_cookie("inkforge_session", &access_token, ACCESS_TOKEN_MAX_AGE);

    tracing::info!(
        module = "auth",
        event = "refresh_token_success",
        user_id = %user.id,
        username = %user.username,
        "access token refreshed"
    );

    let resp_header =
        axum::http::HeaderValue::from_str(&access_cookie).unwrap();
    let mut resp_headers = axum::http::HeaderMap::new();
    resp_headers.insert(axum::http::header::SET_COOKIE, resp_header);

    Ok((
        resp_headers,
        Json(ApiResponse::success(serde_json::json!({ "ok": true }))),
    )
        .into_response())
}

// ── Cookie helpers ──

/// access_token 15min
const ACCESS_TOKEN_MAX_AGE: u32 = 900;
/// refresh_token 7 天
const REFRESH_TOKEN_MAX_AGE: u32 = 604800;

fn build_session_cookie(name: &str, token: &str, max_age: u32) -> String {
    format!(
        "{name}={token}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax"
    )
}
