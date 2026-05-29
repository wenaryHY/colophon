use std::collections::HashSet;
use std::sync::Arc;

use axum::Router;
use minijinja::Environment;

use crate::shared::error::AppResult;
use crate::state::AppState;

use super::asset::PluginAsset;
use super::hook_registry::HookRegistry;
use super::loader::DiscoveredPlugin;
use super::manifest::PluginManifest;
use super::registry;
use super::Plugin;

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    hook_registry: Arc<HookRegistry>,
    manifests: Vec<PluginManifest>,
}

impl PluginManager {
    pub async fn load() -> Self {
        let plugins = registry::take_all().await;
        tracing::info!(
            module = "plugin",
            count = plugins.len(),
            "PluginManager loaded {} plugin(s)",
            plugins.len()
        );
        Self {
            plugins,
            hook_registry: Arc::new(HookRegistry::new()),
            manifests: vec![],
        }
    }

    pub async fn load_with(discovered: Vec<DiscoveredPlugin>) -> Self {
        let manifests: Vec<PluginManifest> = discovered.iter().map(|d| d.manifest.clone()).collect();
        let all_plugins = registry::take_all().await;
        let discovered_ids: HashSet<String> = discovered
            .iter()
            .map(|d| d.manifest.plugin.id.clone())
            .collect();
        let plugins: Vec<Box<dyn Plugin>> = all_plugins
            .into_iter()
            .filter(|p| discovered_ids.contains(p.name()))
            .collect();
        tracing::info!(
            module = "plugin",
            discovered = discovered.len(),
            loaded = plugins.len(),
            "PluginManager loaded {}/{} discovered plugin(s)",
            plugins.len(),
            discovered.len()
        );
        Self {
            plugins,
            hook_registry: Arc::new(HookRegistry::new()),
            manifests,
        }
    }

    pub async fn init_all(&self, state: &Arc<AppState>) -> AppResult<()> {
        for plugin in &self.plugins {
            let hooks = plugin.hooks();
            if !hooks.is_empty() {
                self.hook_registry.register(plugin.name(), hooks).await;
            }
            tracing::info!(
                module = "plugin",
                plugin = plugin.name(),
                version = plugin.version(),
                "initializing plugin"
            );
            plugin.init(state).await?;
        }
        Ok(())
    }

    pub fn hook_registry(&self) -> &Arc<HookRegistry> {
        &self.hook_registry
    }

    pub fn discovered_manifests(&self) -> Vec<PluginManifest> {
        self.manifests.clone()
    }

    pub async fn shutdown_all(&self) -> AppResult<()> {
        for plugin in &self.plugins {
            tracing::info!(
                module = "plugin",
                plugin = plugin.name(),
                "shutting down plugin"
            );
            plugin.shutdown().await?;
        }
        Ok(())
    }

    pub fn collect_routes(&self) -> Router<Arc<AppState>> {
        let mut router = Router::new();
        for plugin in &self.plugins {
            let plugin_routes = plugin.api_routes();
            router = router.merge(plugin_routes);
        }
        router
    }

    pub fn extend_template_env(&self, env: &mut Environment<'_>) -> AppResult<()> {
        for plugin in &self.plugins {
            plugin.extend_template_env(env)?;
        }
        Ok(())
    }

    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.name().to_string()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn collect_assets(&self) -> Vec<PluginAsset> {
        let mut assets = Vec::new();
        for plugin in &self.plugins {
            assets.extend(plugin.frontend_assets());
        }
        assets
    }

    pub fn render_asset_html(&self, placement: &str) -> String {
        self.collect_assets()
            .iter()
            .filter(|a| match placement {
                "head" => matches!(a.placement, super::asset::AssetPlacement::Head),
                "body" => matches!(a.placement, super::asset::AssetPlacement::Body),
                _ => false,
            })
            .map(|a| a.render_html())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
