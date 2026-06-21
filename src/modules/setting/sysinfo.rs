use axum::{extract::State, Json};
use std::sync::Arc;
use sysinfo::System;

use crate::shared::response::ApiResponse;
use crate::state::AppState;

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct SysInfoResponse {
    pub uptime_seconds: u64,
    pub memory_rss_mb: f64,
    pub database_size_mb: f64,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/sysinfo",
    tag = "admin.system",
    responses(
        (status = 200, description = "系统资源信息"),
        (status = 401, description = "未认证"),
        (status = 403, description = "无管理员权限"),
    ),
    security(("jwt" = []))
)]
pub async fn sysinfo(
    State(state): State<Arc<AppState>>,
    _admin: crate::shared::auth::AdminUser,
) -> Json<ApiResponse<SysInfoResponse>> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let uptime = System::uptime();

    let memory = sys
        .process(sysinfo::Pid::from(std::process::id() as usize))
        .map(|p| p.memory() as f64 / 1_048_576.0)
        .unwrap_or(0.0);

    let db_size = tokio::fs::metadata(&state.db_path)
        .await
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0);

    Json(ApiResponse::success(SysInfoResponse {
        uptime_seconds: uptime,
        memory_rss_mb: memory,
        database_size_mb: db_size,
    }))
}
