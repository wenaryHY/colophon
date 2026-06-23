// TODO: Wave 3.2 — rewrite for Wasm plugin manager
#[cfg(test)]
mod plugin_manager_tests {
    use crate::modules::plugin::manager::PluginManager;

    #[serial_test::serial]
    #[tokio::test]
    #[ignore]
    async fn manager_loads_plugins_from_registry() {
        // TODO: Wave 3.2 — registry removed, reimplement for Wasm
        let manager = PluginManager::load().await;
        assert!(manager.is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    #[ignore]
    async fn manager_init_all_calls_init_on_every_plugin() {
        // TODO: Wave 3.2 — Plugin trait removed, reimplement for Wasm
    }

    #[serial_test::serial]
    #[tokio::test]
    #[ignore]
    async fn manager_collect_routes_merges_all_plugin_routes() {
        // TODO: Wave 3.2 — Plugin trait removed, reimplement for Wasm
    }

    #[serial_test::serial]
    #[tokio::test]
    #[ignore]
    async fn manager_shutdown_all_calls_shutdown_on_every_plugin() {
        // TODO: Wave 3.2 — Plugin trait removed, reimplement for Wasm
        let manager = PluginManager::load().await;
        assert!(manager.shutdown_all().await.is_ok());
    }
}
