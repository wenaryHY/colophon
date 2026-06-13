use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::{
    shared::{auth::AdminUser, error::AppResult, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{ApiKeyListItem, CreateApiKeyRequest, CreateApiKeyResponse, UpdateApiKeyRequest},
    repository, service,
};

/// GET /api/v1/admin/api-keys — 列出当前用户的所有 API Key
pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    admin: AdminUser,
) -> AppResult<Json<ApiResponse<Vec<ApiKeyListItem>>>> {
    let keys = repository::list_api_keys_by_user_id(&state.pool, &admin.0.id).await?;
    let items: Vec<ApiKeyListItem> = keys
        .into_iter()
        .map(|k| ApiKeyListItem {
            id: k.id,
            name: k.name,
            key_prefix: k.key_prefix,
            permissions: k.permissions,
            last_used_at: k.last_used_at,
            expires_at: k.expires_at,
            created_at: k.created_at,
        })
        .collect();
    Ok(Json(ApiResponse::success(items)))
}

/// POST /api/v1/admin/api-keys — 创建新的 API Key
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    admin: AdminUser,
    Json(body): Json<CreateApiKeyRequest>,
) -> AppResult<Json<ApiResponse<CreateApiKeyResponse>>> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(crate::shared::error::AppError::BadRequest(
            "api key name is required".into(),
        ));
    }

    let permissions = if body.permissions.trim().is_empty() {
        "read_only"
    } else {
        body.permissions.trim()
    };

    let (full_key, key_prefix, key_hash) = service::generate_api_key_and_hash();
    let expires_at = body.expires_at.as_deref().filter(|s| !s.trim().is_empty());

    let id = repository::insert_api_key(
        &state.pool,
        &admin.0.id,
        name,
        &key_prefix,
        &key_hash,
        permissions,
        expires_at,
    )
    .await?;

    let created = repository::get_api_key_by_id(&state.pool, &id)
        .await?
        .ok_or(crate::shared::error::AppError::NotFound)?;

    Ok(Json(ApiResponse::success(CreateApiKeyResponse {
        id: created.id,
        name: created.name,
        key_prefix: created.key_prefix,
        full_key,
        permissions: created.permissions,
        expires_at: created.expires_at,
        created_at: created.created_at,
    })))
}

/// PATCH /api/v1/admin/api-keys/:id — 更新 API Key 名称
pub async fn update_api_key(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateApiKeyRequest>,
) -> AppResult<Json<ApiResponse<ApiKeyListItem>>> {
    let _existing = repository::get_api_key_by_id(&state.pool, &id)
        .await?
        .ok_or(crate::shared::error::AppError::NotFound)?;

    if let Some(name) = body.name.as_deref() {
        let name = name.trim();
        if name.is_empty() {
            return Err(crate::shared::error::AppError::BadRequest(
                "api key name cannot be empty".into(),
            ));
        }
        repository::update_api_key_name(&state.pool, &id, name).await?;
    }

    let updated = repository::get_api_key_by_id(&state.pool, &id)
        .await?
        .ok_or(crate::shared::error::AppError::NotFound)?;

    Ok(Json(ApiResponse::success(ApiKeyListItem {
        id: updated.id,
        name: updated.name,
        key_prefix: updated.key_prefix,
        permissions: updated.permissions,
        last_used_at: updated.last_used_at,
        expires_at: updated.expires_at,
        created_at: updated.created_at,
    })))
}

/// DELETE /api/v1/admin/api-keys/:id — 撤销 API Key
pub async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    repository::delete_api_key(&state.pool, &id).await?;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "revoked": true }),
    )))
}
