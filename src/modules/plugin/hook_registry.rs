use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

use super::action_registry::ActionRegistry;
use super::hook::{Hook, HookContext, HookType};
use crate::shared::error::AppResult;

pub struct HookRegistry {
    hooks: Arc<RwLock<HashMap<String, Vec<Hook>>>>,
    pub action_registry: Arc<ActionRegistry>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(RwLock::new(HashMap::new())),
            action_registry: Arc::new(ActionRegistry::new()),
        }
    }

    pub async fn register(&self, plugin_name: &str, hooks: Vec<Hook>) {
        let mut guard = self.hooks.write().await;
        for hook in hooks {
            let entry = guard.entry(hook.name.clone()).or_default();
            entry.push(hook);
            entry.sort_by_key(|h| (h.priority, h.plugin_name.clone()));
        }
        let count = guard.len();
        tracing::info!(
            module = "hook",
            plugin = plugin_name,
            count = count,
            "registered hooks for plugin"
        );
    }

    pub async fn unregister_all(&self, plugin_name: &str) {
        let mut guard = self.hooks.write().await;
        for hooks in guard.values_mut() {
            hooks.retain(|h| h.plugin_name != plugin_name);
        }
        guard.retain(|_, v| !v.is_empty());
        tracing::info!(
            module = "hook",
            plugin = plugin_name,
            "unregistered all hooks for plugin"
        );
    }

    pub async fn dispatch_filter(&self, name: &str, ctx: &mut HookContext) -> AppResult<()> {
        let hooks = {
            let guard = self.hooks.read().await;
            guard.get(name).cloned().unwrap_or_default()
        };

        for hook in &hooks {
            if !matches!(hook.hook_type, HookType::Filter) {
                continue;
            }
            hook.handler.run(ctx).await.map_err(|e| {
                tracing::error!(
                    module = "hook",
                    hook = name,
                    plugin = hook.plugin_name,
                    error = %e,
                    "filter hook failed"
                );
                e
            })?;
        }
        Ok(())
    }

    pub async fn dispatch_filter_best_effort(&self, name: &str, ctx: &mut HookContext) {
        let hooks = {
            let guard = self.hooks.read().await;
            guard.get(name).cloned().unwrap_or_default()
        };

        for hook in &hooks {
            if !matches!(hook.hook_type, HookType::Filter) {
                continue;
            }
            if let Err(e) = hook.handler.run(ctx).await {
                tracing::warn!(
                    module = "hook",
                    hook = name,
                    plugin = hook.plugin_name,
                    error = %e,
                    "filter hook failed, skipping plugin"
                );
            }
        }
    }

    pub async fn dispatch_action(&self, name: &str, ctx: HookContext) {
        let hooks = {
            let guard = self.hooks.read().await;
            guard.get(name).cloned().unwrap_or_default()
        };

        let ctx = Arc::new(ctx);
        for hook in &hooks {
            if !matches!(hook.hook_type, HookType::Action) {
                continue;
            }
            let ctx = ctx.clone();
            let handler = hook.handler.clone();
            let plugin_name = hook.plugin_name.clone();
            let hook_name = name.to_string();
            let action_registry = self.action_registry.clone();
            let action_id = action_registry.track(&hook_name, &plugin_name).await;

            tokio::spawn(async move {
                action_registry.mark_running(&action_id).await;
                let mut action_ctx = (*ctx).clone();
                match timeout(Duration::from_secs(5), handler.run(&mut action_ctx)).await {
                    Ok(Ok(())) => {
                        action_registry.mark_done(&action_id).await;
                    }
                    Ok(Err(e)) => {
                        action_registry.mark_failed(&action_id, &e.to_string()).await;
                        tracing::error!(
                            module = "hook",
                            hook = hook_name,
                            plugin = plugin_name,
                            action_id = %action_id,
                            error = %e,
                            "action hook failed"
                        );
                    }
                    Err(_) => {
                        action_registry.mark_timeout(&action_id).await;
                        tracing::warn!(
                            module = "hook",
                            hook = hook_name,
                            plugin = plugin_name,
                            action_id = %action_id,
                            "action hook timed out after 5s"
                        );
                    }
                }
            });
        }
    }
}
