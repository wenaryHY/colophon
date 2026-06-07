use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};

use crate::{
    shared::{auth_constants, error::AppResult, json::AppJson, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{SetupInitializeRequest, SetupStatusResponse},
    service,
};

/// 7 天，setup 初始化必然对应管理员，属于"记住我"场景
const SETUP_REFRESH_MAX_AGE: u64 = 604800;

pub async fn status(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<ApiResponse<SetupStatusResponse>>> {
    Ok(Json(ApiResponse::success(service::get_status(state).await?)))
}

pub async fn initialize(
    State(state): State<Arc<AppState>>,
    AppJson(body): AppJson<SetupInitializeRequest>,
) -> AppResult<axum::response::Response> {
    let (payload, refresh_token) = service::initialize(state.clone(), body).await?;

    let cookie_secure = state.config.cookie_secure();
    let refresh_cookie = build_refresh_cookie(&refresh_token, cookie_secure);
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie).unwrap();
    let session_cookie =
        build_session_cookie(&payload.token, state.config.auth.expires_in_seconds, cookie_secure);
    let session_header = axum::http::HeaderValue::from_str(&session_cookie).unwrap();

    let json = Json(ApiResponse::success(payload));

    let mut headers = HeaderMap::new();
    headers.insert(axum::http::header::SET_COOKIE, refresh_header);
    headers.append(axum::http::header::SET_COOKIE, session_header);
    Ok((headers, json).into_response())
}

fn build_session_cookie(token: &str, max_age_seconds: u64, cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}={token}; Path=/; Max-Age={max_age_seconds}; HttpOnly; SameSite=Strict{secure}",
        name = auth_constants::SESSION_COOKIE_NAME_FOR_JWT_ACCESS_TOKEN,
    )
}

fn build_refresh_cookie(token: &str, cookie_secure: bool) -> String {
    let secure = if cookie_secure { "; Secure" } else { "" };
    format!(
        "{name}={token}; Path=/api/v1/auth/refresh; Max-Age={SETUP_REFRESH_MAX_AGE}; HttpOnly; SameSite=Strict{secure}",
        name = auth_constants::REFRESH_COOKIE_NAME_FOR_OAUTH2_REFRESH_TOKEN,
    )
}
