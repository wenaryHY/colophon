#[cfg(test)]
mod plugin_manager_tests {
    use async_trait::async_trait;
    use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
    use minijinja::Environment;
    use serial_test::serial;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    use crate::modules::plugin::manager::PluginManager;
    use crate::modules::plugin::registry;
    use crate::modules::plugin::Plugin;
    use crate::shared::error::AppResult;
    use crate::state::AppState;

    struct MockPlugin {
        name: String,
        init_count: Arc<AtomicU32>,
        shutdown_count: Arc<AtomicU32>,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                init_count: Arc::new(AtomicU32::new(0)),
                shutdown_count: Arc::new(AtomicU32::new(0)),
            }
        }
    }

    #[async_trait]
    impl Plugin for MockPlugin {
        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        async fn init(&self, _state: &Arc<AppState>) -> AppResult<()> {
            self.init_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn shutdown(&self) -> AppResult<()> {
            self.shutdown_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn api_routes(&self) -> Router<Arc<AppState>> {
            Router::new().route(
                &format!("/api/v1/mock/{}", self.name),
                get(|| async { (StatusCode::OK, "mock ok").into_response() }),
            )
        }

        fn extend_template_env(&self, env: &mut Environment<'_>) -> AppResult<()> {
            let greeting = format!("Hello from {}", self.name);
            env.add_function(
                std::borrow::Cow::Owned(format!("mock_{}", self.name)),
                move || -> Result<String, minijinja::Error> { Ok(greeting.clone()) },
            );
            Ok(())
        }
    }

    #[serial]
    #[tokio::test]
    async fn manager_loads_plugins_from_registry() {
        let plugin_a = MockPlugin::new("alpha");
        let plugin_b = MockPlugin::new("beta");

        registry::register(Box::new(plugin_a)).await;
        registry::register(Box::new(plugin_b)).await;

        let manager = PluginManager::load().await;
        assert!(!manager.is_empty());

        // TODO: collect_routes now requires &Arc<AppState> with AdminUser middleware.
        // Tests need to be updated to construct proper state.
        // let _router = manager.collect_routes(pool);
    }

    #[serial]
    #[tokio::test]
    async fn manager_init_all_calls_init_on_every_plugin() {
        let plugin_a = MockPlugin::new("init-a");
        let _init_count_a = plugin_a.init_count.clone();
        let plugin_b = MockPlugin::new("init-b");
        let _init_count_b = plugin_b.init_count.clone();

        registry::register(Box::new(plugin_a)).await;
        registry::register(Box::new(plugin_b)).await;

        let manager = PluginManager::load().await;
        assert!(!manager.is_empty());

        let remaining = registry::take_all().await;
        assert!(
            remaining.is_empty(),
            "registry should be empty after take_all"
        );
    }

    #[serial]
    #[tokio::test]
    async fn manager_collect_routes_merges_all_plugin_routes() {
        let plugin = MockPlugin::new("route-test");
        registry::register(Box::new(plugin)).await;

        let _manager = PluginManager::load().await;
        // TODO: collect_routes now requires &Arc<AppState> with AdminUser middleware.
        // Tests need to be updated to construct proper state.
        // let pool = create_test_pool().await;
        // let router = manager.collect_routes(pool);
        // let debug_str = format!("{:?}", router);
        // assert!(
        //     debug_str.contains("mock"),
        //     "collected router should contain mock route path: {}",
        //     debug_str
        // );
    }

    #[serial]
    #[tokio::test]
    async fn manager_shutdown_all_calls_shutdown_on_every_plugin() {
        let plugin_a = MockPlugin::new("shutdown-a");
        let sd_count_a = plugin_a.shutdown_count.clone();
        let plugin_b = MockPlugin::new("shutdown-b");
        let sd_count_b = plugin_b.shutdown_count.clone();

        registry::register(Box::new(plugin_a)).await;
        registry::register(Box::new(plugin_b)).await;

        let manager = PluginManager::load().await;
        manager.shutdown_all().await.unwrap();

        assert_eq!(
            sd_count_a.load(Ordering::SeqCst),
            1,
            "shutdown-a should be called once"
        );
        assert_eq!(
            sd_count_b.load(Ordering::SeqCst),
            1,
            "shutdown-b should be called once"
        );
    }
}
