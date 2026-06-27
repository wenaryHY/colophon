use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};

use crate::{
    shared::{auth::cookie::*, error::AppResult, json::AppJson, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{SetupInitializeRequest, SetupStatusResponse},
    service,
};

pub async fn status(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<ApiResponse<SetupStatusResponse>>> {
    Ok(Json(ApiResponse::success(
        service::get_status(state).await?,
    )))
}

pub async fn initialize(
    State(state): State<Arc<AppState>>,
    AppJson(body): AppJson<SetupInitializeRequest>,
) -> AppResult<axum::response::Response> {
    let (payload, refresh_token) = service::initialize(state.clone(), body).await?;

    let cookie_secure = state.config.cookie_secure();
    let refresh_cookie =
        build_refresh_cookie(&refresh_token, REMEMBER_ME_MAX_AGE_SECONDS, cookie_secure);
    let refresh_header = axum::http::HeaderValue::from_str(&refresh_cookie)
        .expect("JWT cookie must be ASCII-only; if this fails, check token encoding");
    let session_cookie = build_session_cookie(
        &payload.token,
        state.config.auth.expires_in_seconds,
        cookie_secure,
    );
    let session_header = axum::http::HeaderValue::from_str(&session_cookie)
        .expect("JWT cookie must be ASCII-only; if this fails, check token encoding");

    let json = Json(ApiResponse::success(payload));

    let mut headers = HeaderMap::new();
    headers.insert(axum::http::header::SET_COOKIE, refresh_header);
    headers.append(axum::http::header::SET_COOKIE, session_header);
    Ok((headers, json).into_response())
}
