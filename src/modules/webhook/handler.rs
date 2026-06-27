use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::{
    shared::{
        auth::AdminUser, error::AppResult, pagination::PaginationQuery, response::ApiResponse,
    },
    state::AppState,
};

use super::{
    domain::Webhook,
    dto::{CreateWebhookRequest, UpdateWebhookRequest},
    service,
};

/// 投递记录查询参数
#[derive(Debug, Deserialize)]
pub struct DeliveryQuery {
    #[serde(flatten)]
    pub pagination: PaginationQuery,
}

/// GET /admin/webhooks — 列出所有 webhook
pub async fn list_webhooks(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> AppResult<Json<ApiResponse<Vec<Webhook>>>> {
    Ok(Json(ApiResponse::success(
        service::list_webhooks(state).await?,
    )))
}

/// POST /admin/webhooks — 创建 webhook
pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Json(body): Json<CreateWebhookRequest>,
) -> AppResult<Json<ApiResponse<Webhook>>> {
    Ok(Json(ApiResponse::success(
        service::create_webhook(state, body).await?,
    )))
}

/// GET /admin/webhooks/:id — 获取单个 webhook
pub async fn get_webhook(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<Webhook>>> {
    Ok(Json(ApiResponse::success(
        service::get_webhook(state, &id).await?,
    )))
}

/// PATCH /admin/webhooks/:id — 更新 webhook
pub async fn update_webhook(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateWebhookRequest>,
) -> AppResult<Json<ApiResponse<Webhook>>> {
    Ok(Json(ApiResponse::success(
        service::update_webhook(state, &id, body).await?,
    )))
}

/// DELETE /admin/webhooks/:id — 删除 webhook
pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    Ok(Json(ApiResponse::success(
        service::delete_webhook(state, &id).await?,
    )))
}

/// GET /admin/webhooks/:id/deliveries — 获取投递记录
pub async fn list_deliveries(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
    Query(query): Query<DeliveryQuery>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let (page, page_size, _offset) = query.pagination.normalized(20, 100);
    let (deliveries, total) = service::list_deliveries(state, &id, page, page_size).await?;
    Ok(Json(ApiResponse::success(serde_json::json!({
        "items": deliveries,
        "total": total,
        "page": page,
        "page_size": page_size,
    }))))
}
