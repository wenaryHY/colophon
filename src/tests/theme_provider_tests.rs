#[cfg(test)]
mod theme_provider_tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;

    use crate::modules::category::domain::Category;
    use crate::modules::post::domain::PublicPostSummary;
    use crate::modules::tag::domain::Tag;
    use crate::modules::theme::cache::TemplateContextCache;
    use crate::modules::theme::context::TemplateContext;
    use crate::modules::theme::provider::TemplateDataProvider;
    use crate::modules::theme::ThemeConfig;
    use crate::shared::error::AppResult;

    struct MockProvider {
        call_count: Arc<AtomicU32>,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                call_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn get_call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl TemplateDataProvider for MockProvider {
        async fn get_active_theme(&self) -> AppResult<String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok("mock-theme".to_string())
        }

        async fn get_setting(&self, _key: &str, default: &str) -> String {
            default.to_string()
        }

        async fn get_theme_config(&self, _slug: &str) -> Option<ThemeConfig> {
            None
        }

        async fn get_recent_posts(&self, _limit: i64) -> Vec<PublicPostSummary> {
            vec![]
        }

        async fn get_tags(&self) -> Vec<Tag> {
            vec![]
        }

        async fn get_categories(&self) -> Vec<Category> {
            vec![]
        }
    }

    #[tokio::test]
    async fn from_provider_populates_all_fields() {
        let provider = MockProvider::new();
        let ctx = TemplateContext::from_provider(&provider)
            .await
            .expect("from_provider should succeed");

        assert_eq!(ctx.active_theme, "mock-theme");
        assert_eq!(ctx.site_title, "Colophon");
        assert_eq!(ctx.site_description, "");
        assert_eq!(ctx.site_url, "");
        assert_eq!(ctx.admin_url, "/admin");
        assert!(ctx.theme_config.is_none());
        assert!(ctx.recent_posts.is_empty());
        assert!(ctx.tags.is_empty());
        assert!(ctx.categories.is_empty());
    }

    #[tokio::test]
    async fn cache_avoids_redundant_provider_calls() {
        let provider = MockProvider::new();
        let cache = TemplateContextCache::with_default_ttl();

        let _ = cache.get_or_load(&provider).await.unwrap();
        let _ = cache.get_or_load(&provider).await.unwrap();

        assert_eq!(provider.get_call_count(), 1);
    }

    #[tokio::test]
    async fn cache_expires_after_ttl() {
        let provider = MockProvider::new();
        let cache = TemplateContextCache::new(0);

        let _ = cache.get_or_load(&provider).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = cache.get_or_load(&provider).await.unwrap();

        assert_eq!(provider.get_call_count(), 2);
    }

    #[tokio::test]
    async fn cache_invalidate_forces_reload() {
        let provider = MockProvider::new();
        let cache = TemplateContextCache::new(60);

        let _ = cache.get_or_load(&provider).await.unwrap();
        cache.invalidate().await;
        let _ = cache.get_or_load(&provider).await.unwrap();

        assert_eq!(provider.get_call_count(), 2);
    }
}
