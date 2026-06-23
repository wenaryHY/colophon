use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use minijinja::Environment;
use tokio::sync::RwLock;

use crate::shared::error::AppResult;
use crate::state::AppState;

use super::action_registry::ActionRegistry;
use super::asset::PluginAsset;
use super::hook::Hook;
use super::hook_registry::HookRegistry;
use super::loader::DiscoveredPlugin;
use super::manifest::{HookListenDef, PluginManifest};
use super::sandbox::{WasmHookHandler, WasmRuntime};

/// PluginManager — 基于 Extism Wasm 运行时的插件管理器。
///
/// Manifest 在 init_all 时编译并常驻缓存，每次 Hook 触发时创建短生命周期的
/// extism::Plugin 实例，调用完毕立即丢弃（RAII）。
pub struct PluginManager {
    hook_registry: Arc<HookRegistry>,
    manifests: Vec<PluginManifest>,
    wasm_runtime: Arc<RwLock<WasmRuntime>>,
    /// plugin_id -> 插件目录路径，用于 init_all 时定位 plugin.wasm
    plugin_dirs: HashMap<String, PathBuf>,
}

impl PluginManager {
    /// 创建空的 PluginManager（无插件）
    pub async fn load() -> Self {
        Self {
            hook_registry: Arc::new(HookRegistry::new()),
            manifests: vec![],
            wasm_runtime: Arc::new(RwLock::new(WasmRuntime::new())),
            plugin_dirs: HashMap::new(),
        }
    }

    /// 从发现的插件列表创建 PluginManager，暂不编译 Wasm
    pub async fn load_with(discovered: Vec<DiscoveredPlugin>) -> Self {
        let manifests: Vec<PluginManifest> = discovered
            .iter()
            .map(|d| d.manifest.clone())
            .collect();
        let plugin_dirs: HashMap<String, PathBuf> = discovered
            .into_iter()
            .map(|d| (d.manifest.plugin.id.clone(), d.dir_path))
            .collect();
        Self {
            hook_registry: Arc::new(HookRegistry::new()),
            manifests,
            wasm_runtime: Arc::new(RwLock::new(WasmRuntime::new())),
            plugin_dirs,
        }
    }

    /// 初始化所有已发现的插件：编译 Wasm 模块并注册 Hook。
    ///
    /// 对于每个发现的插件：
    /// 1. 定位 {dir_path}/plugin.wasm
    /// 2. 编译 Manifest 并缓存到 wasm_runtime
    /// 3. 从 manifest.hooks.listen 生成 Hook 条目
    /// 4. 调用 hook_registry.register 注册
    pub async fn init_all(&self, _state: &Arc<AppState>) -> AppResult<()> {
        for manifest in &self.manifests {
            let plugin_id = &manifest.plugin.id;

            let dir_path = match self.plugin_dirs.get(plugin_id) {
                Some(p) => p.clone(),
                None => {
                    tracing::warn!(
                        module = "plugin",
                        plugin = %plugin_id,
                        "plugin dir not found in plugin_dirs map"
                    );
                    continue;
                }
            };

            let wasm_path = dir_path.join("plugin.wasm");

            // 编译 Wasm 模块（仅当未缓存且文件存在时）
            {
                let mut runtime = self.wasm_runtime.write().await;
                if !runtime.has_module(plugin_id) {
                    if wasm_path.exists() {
                        if let Err(e) = runtime.load_module(plugin_id, &wasm_path) {
                            tracing::error!(
                                module = "plugin",
                                plugin = %plugin_id,
                                error = %e,
                                "failed to compile wasm module"
                            );
                            continue;
                        }
                        tracing::info!(
                            module = "plugin",
                            plugin = %plugin_id,
                            wasm = %wasm_path.display(),
                            "wasm module compiled and cached"
                        );
                    } else {
                        tracing::warn!(
                            module = "plugin",
                            plugin = %plugin_id,
                            path = %wasm_path.display(),
                            "plugin.wasm not found, skipping"
                        );
                        continue;
                    }
                }
            }

            // 注册 Hook（从 manifest.hooks.listen 读取）
            let listen_defs: Vec<HookListenDef> = manifest
                .hooks
                .as_ref()
                .and_then(|h| h.listen.clone())
                .unwrap_or_default();

            if listen_defs.is_empty() {
                tracing::debug!(
                    module = "plugin",
                    plugin = %plugin_id,
                    "no hooks.listen entries, skipping hook registration"
                );
                continue;
            }

            let handler = Arc::new(WasmHookHandler {
                plugin_id: plugin_id.clone(),
                wasm_runtime: self.wasm_runtime.clone(),
            });

            let mut hook_entries = Vec::with_capacity(listen_defs.len());
            for def in &listen_defs {
                match def.hook_type.as_str() {
                    "filter" => {
                        hook_entries.push(Hook::new_filter(
                            &def.event,
                            0,
                            plugin_id,
                            handler.clone(),
                        ));
                    }
                    "action" => {
                        hook_entries.push(Hook::new_action(
                            &def.event,
                            0,
                            plugin_id,
                            handler.clone(),
                        ));
                    }
                    other => {
                        tracing::warn!(
                            module = "plugin",
                            plugin = %plugin_id,
                            event = %def.event,
                            hook_type = %other,
                            "unknown hook_type, expected 'filter' or 'action'"
                        );
                    }
                }
            }

            if !hook_entries.is_empty() {
                self.hook_registry.register(plugin_id, hook_entries).await;
                tracing::info!(
                    module = "plugin",
                    plugin = %plugin_id,
                    hooks = listen_defs.len(),
                    "wasm hooks registered"
                );
            }
        }

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

    /// No-op：Wasm 实例在 Plugin drop 时自动释放，无需显式关闭
    pub async fn shutdown_all(&self) -> AppResult<()> {
        Ok(())
    }

    pub fn collect_routes(&self, _state: &Arc<AppState>) -> Router<Arc<AppState>> {
        Router::new()
    }

    pub fn extend_template_env(&self, _env: &mut Environment<'_>) -> AppResult<()> {
        Ok(())
    }

    /// 返回所有已编译 Wasm 模块的插件 ID 列表
    pub fn plugin_names(&self) -> Vec<String> {
        self.manifests
            .iter()
            .map(|m| m.plugin.id.clone())
            .collect()
    }

    /// 检查 wasm_runtime 是否为空且无 manifest
    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }

    pub fn collect_assets(&self) -> Vec<PluginAsset> {
        vec![]
    }

    pub fn render_asset_html(&self, _placement: &str) -> String {
        String::new()
    }
}
