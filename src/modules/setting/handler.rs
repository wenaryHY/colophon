use std::collections::HashMap;
use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{
    shared::{auth::AdminUser, error::AppResult, response::ApiResponse},
    state::AppState,
};

use super::{
    dto::{SettingItem, UpdateSettingRequest},
    service,
};

pub async fn list_settings(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> AppResult<Json<ApiResponse<Vec<SettingItem>>>> {
    Ok(Json(ApiResponse::success(
        service::list_settings(state).await?,
    )))
}

pub async fn update_setting(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Json(body): Json<UpdateSettingRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    Ok(Json(ApiResponse::success(
        service::update_setting(state, body).await?,
    )))
}

#[derive(serde::Deserialize)]
pub struct BatchUpdateSettingsRequest {
    pub settings: HashMap<String, String>,
}

pub async fn update_settings_batch(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Json(body): Json<BatchUpdateSettingsRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    for (key, value) in &body.settings {
        let req = UpdateSettingRequest {
            key: key.clone(),
            value: value.clone(),
        };
        super::service::update_setting(state.clone(), req).await?;
    }
    state.invalidate_all_caches().await;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "updated": true }),
    )))
}
