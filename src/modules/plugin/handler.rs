use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::shared::error::AppResult;
use crate::shared::response::ApiResponse;
use crate::state::AppState;

use super::settings;

#[derive(serde::Deserialize)]
pub struct UpdateSettingsRequest {
    pub settings: HashMap<String, String>,
}

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    Path(plugin_name): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let values = settings::get_all(&state.pool, &plugin_name).await?;

    let setting_defs = state
        .plugin_manager
        .discovered_manifests()
        .into_iter()
        .find(|m| m.plugin.id == plugin_name)
        .and_then(|m| m.settings.clone())
        .unwrap_or_default();

    Ok(Json(ApiResponse::success(serde_json::json!({
        "plugin_name": plugin_name,
        "settings": setting_defs,
        "values": values,
    }))))
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Path(plugin_name): Path<String>,
    Json(body): Json<UpdateSettingsRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let setting_defs: HashMap<String, super::manifest::SettingDef> = state
        .plugin_manager
        .discovered_manifests()
        .into_iter()
        .find(|m| m.plugin.id == plugin_name)
        .and_then(|m| m.settings.clone())
        .map(|defs| defs.into_iter().map(|d| (d.key.clone(), d)).collect())
        .unwrap_or_default();

    for (key, value) in &body.settings {
        if !setting_defs.contains_key(key) {
            continue;
        }
        settings::set(&state.pool, &plugin_name, key, value).await?;
    }

    tracing::info!(
        module = "plugin",
        plugin = plugin_name,
        "updated plugin settings"
    );
    Ok(Json(ApiResponse::success(serde_json::json!({
        "updated": true,
    }))))
}
