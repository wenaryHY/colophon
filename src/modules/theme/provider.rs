use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::modules::category::domain::Category;
use crate::modules::post::domain::PublicPostSummary;
use crate::modules::tag::domain::Tag;
use crate::modules::theme::ThemeConfig;
use crate::shared::error::AppResult;

#[async_trait]
pub trait TemplateDataProvider: Send + Sync {
    async fn get_active_theme(&self) -> AppResult<String>;
    async fn get_setting(&self, key: &str, default: &str) -> String;
    async fn get_theme_config(&self, slug: &str) -> Option<ThemeConfig>;
    async fn get_recent_posts(&self, limit: i64) -> Vec<PublicPostSummary>;
    async fn get_tags(&self) -> Vec<Tag>;
    async fn get_categories(&self) -> Vec<Category>;
}

pub struct DbTemplateDataProvider<'a> {
    pool: &'a SqlitePool,
}

impl<'a> DbTemplateDataProvider<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl<'a> TemplateDataProvider for DbTemplateDataProvider<'a> {
    async fn get_active_theme(&self) -> AppResult<String> {
        super::repository::get_active_theme(self.pool).await
    }

    async fn get_setting(&self, key: &str, default: &str) -> String {
        crate::modules::setting::repository::get_string(self.pool, key, default)
            .await
            .unwrap_or_else(|_| default.to_string())
    }

    async fn get_theme_config(&self, slug: &str) -> Option<ThemeConfig> {
        super::repository::get_config(self.pool, slug)
            .await
            .unwrap_or_default()
    }

    async fn get_recent_posts(&self, limit: i64) -> Vec<PublicPostSummary> {
        crate::modules::post::repository::list_recent_public_posts(self.pool, limit)
            .await
            .unwrap_or_default()
    }

    async fn get_tags(&self) -> Vec<Tag> {
        crate::modules::tag::repository::list_tags(self.pool)
            .await
            .unwrap_or_default()
    }

    async fn get_categories(&self) -> Vec<Category> {
        crate::modules::category::repository::list_categories(self.pool)
            .await
            .unwrap_or_default()
    }
}
