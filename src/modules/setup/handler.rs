use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};

use crate::{
    shared::{error::AppResult, json::AppJson, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{SetupInitializeRequest, SetupStatusResponse},
    service,
};


pub async fn status(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<ApiResponse<SetupStatusResponse>>> {
    Ok(Json(ApiResponse::success(service::get_status(state).await?)))
}

pub async fn initialize(
    State(state): State<Arc<AppState>>,
    AppJson(body): AppJson<SetupInitializeRequest>,
) -> AppResult<axum::response::Response> {
    let payload = service::initialize(state, body).await?;
    let cookie_val = build_session_cookie(&payload.token);
    let cookie_header = axum::http::HeaderValue::from_str(&cookie_val).unwrap();
    let json = Json(ApiResponse::success(payload));

    let mut headers = HeaderMap::new();
    headers.insert(axum::http::header::SET_COOKIE, cookie_header);
    Ok((headers, json).into_response())
}

fn build_session_cookie(token: &str) -> String {
    let secure = if cfg!(debug_assertions) { "" } else { "; Secure" };
    format!(
        "inkforge_session={token}; Path=/; Max-Age=900; HttpOnly; SameSite=Strict{secure}"
    )
}
