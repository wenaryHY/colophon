use std::sync::Arc;

use axum::Router;
use minijinja::Environment;

use crate::shared::error::AppResult;
use crate::state::AppState;

use super::action_registry::ActionRegistry;
use super::asset::PluginAsset;
use super::hook_registry::HookRegistry;
use super::loader::DiscoveredPlugin;
use super::manifest::PluginManifest;

/// PluginManager — 存根实现。
///
/// Wave 3.2 将用 Wasm sandbox 替代此处的原生插件逻辑。
pub struct PluginManager {
    hook_registry: Arc<HookRegistry>,
    manifests: Vec<PluginManifest>,
}

impl PluginManager {
    pub async fn load() -> Self {
        Self {
            hook_registry: Arc::new(HookRegistry::new()),
            manifests: vec![],
        }
    }

    pub async fn load_with(discovered: Vec<DiscoveredPlugin>) -> Self {
        let manifests: Vec<PluginManifest> =
            discovered.into_iter().map(|d| d.manifest).collect();
        Self {
            hook_registry: Arc::new(HookRegistry::new()),
            manifests,
        }
    }

    pub async fn init_all(&self, _state: &Arc<AppState>) -> AppResult<()> {
        Ok(())
    }

    pub fn hook_registry(&self) -> &Arc<HookRegistry> {
        &self.hook_registry
    }

    pub fn action_registry(&self) -> &Arc<ActionRegistry> {
        &self.hook_registry.action_registry
    }

    pub fn discovered_manifests(&self) -> Vec<PluginManifest> {
        self.manifests.clone()
    }

    pub async fn shutdown_all(&self) -> AppResult<()> {
        Ok(())
    }

    pub fn collect_routes(&self, _state: &Arc<AppState>) -> Router<Arc<AppState>> {
        Router::new()
    }

    pub fn extend_template_env(&self, _env: &mut Environment<'_>) -> AppResult<()> {
        Ok(())
    }

    pub fn plugin_names(&self) -> Vec<String> {
        vec![]
    }

    pub fn is_empty(&self) -> bool {
        true
    }

    pub fn collect_assets(&self) -> Vec<PluginAsset> {
        vec![]
    }

    pub fn render_asset_html(&self, _placement: &str) -> String {
        String::new()
    }
}
