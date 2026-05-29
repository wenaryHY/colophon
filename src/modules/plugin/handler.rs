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

pub async fn list_slots(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let slots: Vec<serde_json::Value> = state
        .plugin_manager
        .discovered_manifests()
        .into_iter()
        .flat_map(|m| {
            let plugin_id = m.plugin.id.clone();
            let admin_root = m.resources
                .as_ref()
                .and_then(|r| r.admin_root.as_deref())
                .unwrap_or("admin/")
                .to_string();
            m.slots.unwrap_or_default().into_iter().map(move |s| {
                serde_json::json!({
                    "target": s.target,
                    "label": s.label,
                    "entry": format!("/static/plugins/{}/{}{}", plugin_id, admin_root, s.entry),
                    "width": s.width,
                    "height": s.height,
                    "plugin_name": plugin_id,
                })
            })
        })
        .collect();

    Ok(Json(ApiResponse::success(serde_json::json!({
        "slots": slots,
    }))))
}
