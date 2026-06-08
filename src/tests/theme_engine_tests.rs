#[cfg(test)]
mod theme_engine_tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use minijinja::Environment;

    use crate::modules::plugin::manager::PluginManager;
    use crate::modules::theme::context::TemplateContext;
    use crate::modules::theme::engine::build_template_engine;
    use crate::state::AssetManifest;

    fn make_context() -> TemplateContext {
        TemplateContext {
            active_theme: "default".to_string(),
            site_title: "Test Site".to_string(),
            site_description: "A test description".to_string(),
            site_url: "https://example.com".to_string(),
            admin_url: "/admin".to_string(),
            theme_config: None,
            recent_posts: vec![],
            tags: vec![],
            categories: vec![],
        }
    }

    fn theme_dir() -> PathBuf {
        let raw = Path::new(env!("CARGO_MANIFEST_DIR")).join("themes");
        std::fs::canonicalize(&raw).unwrap_or(raw)
    }

    async fn plugin_manager() -> PluginManager {
        PluginManager::load().await
    }

    fn empty_env_cache() -> Arc<RwLock<HashMap<String, Environment<'static>>>> {
        Arc::new(RwLock::new(HashMap::new()))
    }

    fn test_asset_manifest() -> Arc<AssetManifest> {
        // 测试环境中使用实际生成的 manifest
        Arc::new(AssetManifest::load())
    }

    #[tokio::test]
    async fn build_engine_succeeds_with_valid_theme_dir() {
        let ctx = make_context();
        let cache = empty_env_cache();
        let manifest = test_asset_manifest();
        let result = build_template_engine(&ctx, &theme_dir(), &plugin_manager().await, &cache, &manifest).await;
        assert!(
            result.is_ok(),
            "engine should build successfully: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn build_engine_sets_globals() {
        let ctx = make_context();
        let cache = empty_env_cache();
        let manifest = test_asset_manifest();
        let env = build_template_engine(&ctx, &theme_dir(), &plugin_manager().await, &cache, &manifest).await.unwrap();

        let title = env
            .render_str("{{ site_title }}", minijinja::context!())
            .unwrap();
        assert_eq!(title, "Test Site");

        let url = env
            .render_str("{{ site_url }}", minijinja::context!())
            .unwrap();
        assert_eq!(url, "https://example.com");

        let desc = env
            .render_str("{{ site_description }}", minijinja::context!())
            .unwrap();
        assert_eq!(desc, "A test description");

        let admin = env
            .render_str("{{ admin_url }}", minijinja::context!())
            .unwrap();
        assert_eq!(admin, "/admin");
    }

    #[tokio::test]
    async fn build_engine_renders_index_template() {
        let ctx = make_context();
        let cache = empty_env_cache();
        let manifest = test_asset_manifest();
        let env = build_template_engine(&ctx, &theme_dir(), &plugin_manager().await, &cache, &manifest).await.unwrap();
        let template = env.get_template("index.html");
        assert!(
            template.is_ok(),
            "index.html should load: {:?}",
            template.err()
        );
    }

    #[tokio::test]
    async fn build_engine_get_recent_posts_returns_empty_vec() {
        let ctx = make_context();
        let cache = empty_env_cache();
        let manifest = test_asset_manifest();
        let env = build_template_engine(&ctx, &theme_dir(), &plugin_manager().await, &cache, &manifest).await.unwrap();
        let result = env.render_str("{{ get_recent_posts() }}", minijinja::context!());
        assert!(
            result.is_ok(),
            "get_recent_posts() should be callable: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn build_engine_theme_assets_url_generates_correct_path() {
        let ctx = make_context();
        let cache = empty_env_cache();
        let manifest = test_asset_manifest();
        let env = build_template_engine(&ctx, &theme_dir(), &plugin_manager().await, &cache, &manifest).await.unwrap();
        let result = env
            .render_str(
                "{{ theme_assets_url('css/style.css') }}",
                minijinja::context!(),
            )
            .unwrap();
        // 现在应该包含 ?v=hash 后缀
        assert!(result.starts_with("/static/themes/default/css/style.css"));
    }

    #[tokio::test]
    async fn build_engine_rejects_path_traversal_in_loader() {
        let ctx = make_context();
        let cache = empty_env_cache();
        let manifest = test_asset_manifest();
        let env = build_template_engine(&ctx, &theme_dir(), &plugin_manager().await, &cache, &manifest).await.unwrap();
        let result = env.get_template("../../Cargo.toml");
        assert!(result.is_err(), "path traversal should be rejected");
    }
}
