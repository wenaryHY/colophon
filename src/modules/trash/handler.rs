use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::{
    shared::{auth::AdminUser, error::AppResult, response::ApiResponse},
    state::AppState,
};

use super::service;

#[derive(Deserialize)]
pub struct TrashQuery {
    #[serde(rename = "type")]
    pub item_type: Option<String>,
}

/// GET /api/admin/trash
pub async fn list_trash(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Query(query): Query<TrashQuery>,
) -> AppResult<Json<ApiResponse<Vec<super::domain::TrashItem>>>> {
    let items = service::list_trash(state, query.item_type.as_deref()).await?;
    Ok(Json(ApiResponse::success(items)))
}

/// POST /api/admin/trash/:type/:id/restore
pub async fn restore_item(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path((item_type, id)): Path<(String, String)>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    service::restore_item(state, &item_type, &id).await?;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "restored": true }),
    )))
}

/// DELETE /api/admin/trash/:type/:id
pub async fn purge_item(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path((item_type, id)): Path<(String, String)>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    service::purge_item(state, &item_type, &id).await?;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "purged": true }),
    )))
}

/// POST /api/admin/trash/purge-expired
pub async fn purge_expired(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let count = service::purge_expired(state).await?;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "purged_count": count }),
    )))
}
