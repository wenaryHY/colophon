use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::shared::auth::AdminUser;
use crate::shared::error::AppResult;
use crate::shared::response::ApiResponse;
use crate::state::AppState;

use super::settings;
use super::status;
use super::manager::PluginManager;

#[derive(serde::Deserialize)]
pub struct UpdateSettingsRequest {
    pub settings: HashMap<String, String>,
}

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    Path(plugin_name): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let enabled_ids = status::get_enabled_ids(&state.pool).await?;
    if !enabled_ids.contains(&plugin_name) {
        return Ok(Json(ApiResponse::success(serde_json::json!({
            "plugin_name": plugin_name,
            "settings": [],
            "values": {},
        }))));
    }

    let values = settings::get_all(&state.pool, &plugin_name).await?;

    let setting_defs = state
        .plugin_manager
        .read()
        .await
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
    let enabled_ids = status::get_enabled_ids(&state.pool).await?;
    if !enabled_ids.contains(&plugin_name) {
        return Ok(Json(ApiResponse::success(serde_json::json!({
            "updated": false,
            "reason": "plugin_disabled",
        }))));
    }

    let setting_defs: HashMap<String, super::manifest::SettingDef> = state
        .plugin_manager
        .read()
        .await
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
    let enabled_ids = status::get_enabled_ids(&state.pool).await?;
    let enabled_set: HashSet<String> = enabled_ids.into_iter().collect();
    let slots: Vec<serde_json::Value> = state
        .plugin_manager
        .read()
        .await
        .discovered_manifests()
        .into_iter()
        .filter(|m| enabled_set.contains(&m.plugin.id))
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

pub async fn list_plugins(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let manifests = state.plugin_manager.read().await.discovered_manifests();
    let enabled_ids: Vec<String> = status::get_enabled_ids(&state.pool).await?;

    let plugins: Vec<serde_json::Value> = manifests.into_iter().map(|m| {
        serde_json::json!({
            "id": m.plugin.id,
            "title": m.plugin.title,
            "version": m.plugin.version,
            "description": m.plugin.description,
            "author": m.plugin.author,
            "enabled": enabled_ids.contains(&m.plugin.id),
            "has_settings": m.settings.is_some(),
            "has_admin": m.admin.as_ref().map(|a| a.enabled.unwrap_or(false)).unwrap_or(false),
        })
    }).collect();

    Ok(Json(ApiResponse::success(serde_json::json!({ "plugins": plugins }))))
}

pub async fn toggle_plugin(
    State(state): State<Arc<AppState>>,
    Path(plugin_name): Path<String>,
    _admin: AdminUser,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let enabled_ids = status::get_enabled_ids(&state.pool).await?;
    let currently_enabled = enabled_ids.contains(&plugin_name);
    let new_enabled = !currently_enabled;

    status::set_enabled(&state.pool, &plugin_name, new_enabled).await?;

    if !new_enabled {
        let hook_registry = state.plugin_manager.read().await.hook_registry().clone();
        hook_registry.unregister_all(&plugin_name).await;
    }

    // ── 重建 PluginManager ──
    // 1. 重新注册所有插件到全局 registry（含启用和禁用的）
    crate::register_all().await;

    // 2. 重新发现启用插件（反映新的启用/禁用状态）
    let loader = super::loader::PluginLoader::new(
        std::path::PathBuf::from("plugins"),
        env!("CARGO_PKG_VERSION"),
    );
    let discovered = loader.discover(&state.pool).await?;

    // 3. 构建新 PluginManager
    let new_manager = PluginManager::load_with(discovered).await;

    // 4. 初始化新插件
    new_manager.init_all(&state).await?;

    // 5. 替换
    {
        let mut guard = state.plugin_manager.write().await;
        *guard = new_manager;
    }

    tracing::info!(
        module = "plugin",
        plugin = plugin_name,
        enabled = new_enabled,
        "plugin toggled and PluginManager rebuilt"
    );

    Ok(Json(ApiResponse::success(serde_json::json!({
        "plugin_name": plugin_name,
        "enabled": new_enabled,
    }))))
}
