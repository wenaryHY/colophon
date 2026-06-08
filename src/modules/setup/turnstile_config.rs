use axum::{extract::State, Json};
use std::sync::Arc;

use crate::state::AppState;
use serde::Serialize;

#[derive(Serialize)]
pub struct TurnstileConfigResponse {
    /// Turnstile site key。`None` 表示未启用 Turnstile（配置值为空字符串）。
    pub site_key: Option<String>,
}

/// GET /api/v1/turnstile-config — Cloudflare Turnstile 前端配置
///
/// 返回 Turnstile site key 供前端渲染验证组件。无需认证。
/// 如果配置中 `turnstile_site_key` 为空字符串，`site_key` 字段为 `null`，
/// 前端据此判断是否需要渲染 Turnstile widget。
///
/// 注意：本端点直接返回 `TurnstileConfigResponse`，不包装 ApiResponse 信封。
///
/// # Response
/// 启用时：
/// ```json
/// {
///   "site_key": "0x4AAA..."
/// }
/// ```
/// 未启用时：
/// ```json
/// {
///   "site_key": null
/// }
/// ```
///
/// # Use Case
/// 前端登录/注册表单据此决定是否渲染 Turnstile widget。
/// 后端登录/注册接口会用 `turnstile_secret` 验证 token。
///
/// # Example
/// ```bash
/// curl http://localhost:2000/api/v1/turnstile-config
/// ```
pub async fn get_turnstile_config(
    State(state): State<Arc<AppState>>,
) -> Json<TurnstileConfigResponse> {
    let site_key = state.config.auth.turnstile_site_key.clone();
    let site_key = if site_key.is_empty() { None } else { Some(site_key) };
    Json(TurnstileConfigResponse { site_key })
}
