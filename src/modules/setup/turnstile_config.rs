use axum::{extract::State, Json};
use std::sync::Arc;

use crate::state::AppState;
use serde::Serialize;

#[derive(Serialize)]
pub struct TurnstileConfigResponse {
    /// Turnstile site key。null 表示不启用 Turnstile widget。
    pub site_key: Option<String>,
}

/// 公开路由：返回 Turnstile site key 配置。
/// 无需认证——前端在渲染登录页之前需要知道是否启用 Turnstile。
pub async fn get_turnstile_config(
    State(state): State<Arc<AppState>>,
) -> Json<TurnstileConfigResponse> {
    let site_key = state.config.auth.turnstile_site_key.clone();
    let site_key = if site_key.is_empty() { None } else { Some(site_key) };
    Json(TurnstileConfigResponse { site_key })
}
