use crate::modules::category::domain::Category;
use crate::modules::post::domain::PublicPostSummary;
use crate::modules::tag::domain::Tag;
use crate::modules::theme::provider::TemplateDataProvider;
use crate::modules::theme::ThemeConfig;
use crate::shared::error::AppResult;
use crate::state::AppState;
use std::sync::Arc;

const DEFAULT_POST_LIMIT: i64 = 10;

#[derive(Debug, Clone)]
pub struct TemplateContext {
    pub active_theme: String,
    pub site_title: String,
    pub site_description: String,
    pub site_url: String,
    pub admin_url: String,
    pub theme_config: Option<ThemeConfig>,
    pub recent_posts: Vec<PublicPostSummary>,
    pub tags: Vec<Tag>,
    pub categories: Vec<Category>,
}

impl TemplateContext {
    pub async fn from_provider(provider: &dyn TemplateDataProvider) -> AppResult<Self> {
        let active_theme = provider.get_active_theme().await?;

        let site_title = provider.get_setting("site_title", "InkForge").await;

        let site_description = provider.get_setting("site_description", "").await;

        let site_url = provider.get_setting("site_url", "").await;

        let admin_url = provider.get_setting("admin_url", "/admin").await;

        let theme_config = provider.get_theme_config(&active_theme).await;

        let recent_posts = provider.get_recent_posts(DEFAULT_POST_LIMIT).await;

        let tags = provider.get_tags().await;

        let categories = provider.get_categories().await;

        Ok(Self {
            active_theme,
            site_title,
            site_description,
            site_url,
            admin_url,
            theme_config,
            recent_posts,
            tags,
            categories,
        })
    }

    pub async fn load(state: &Arc<AppState>) -> AppResult<Self> {
        let provider = super::provider::DbTemplateDataProvider::new(&state.pool);
        state.template_cache.get_or_load(&provider).await
    }
}
