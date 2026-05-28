use crate::modules::category::domain::Category;
use crate::modules::post::domain::PublicPostSummary;
use crate::modules::tag::domain::Tag;
use crate::modules::theme::ThemeConfig;
use crate::shared::error::AppResult;
use crate::state::AppState;
use std::sync::Arc;

const DEFAULT_POST_LIMIT: i64 = 10;

/// 预提取的模板上下文数据。
/// handler 层在异步上下文中一次性查询完所有数据，
/// 通过此结构体传入同步的 MiniJinja 渲染管道，
/// 避免在模板闭包中执行 `block_in_place + block_on`。
#[derive(Debug)]
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
    /// 在异步上下文中一次性预提取所有模板所需数据。
    pub async fn load(state: &Arc<AppState>) -> AppResult<Self> {
        let active_theme = super::repository::get_active_theme(&state.pool).await?;

        let site_title = crate::modules::setting::repository::get_string(
            &state.pool, "site_title", "InkForge",
        ).await.unwrap_or_else(|_| "InkForge".to_string());

        let site_description = crate::modules::setting::repository::get_string(
            &state.pool, "site_description", "",
        ).await.unwrap_or_default();

        let site_url = crate::modules::setting::repository::get_string(
            &state.pool, "site_url", "",
        ).await.unwrap_or_default();

        let admin_url = crate::modules::setting::repository::get_string(
            &state.pool, "admin_url", "/admin",
        ).await.unwrap_or_else(|_| "/admin".to_string());

        let theme_config = super::repository::get_config(&state.pool, &active_theme)
            .await
            .unwrap_or_default();

        // 预提取模板函数所需数据（默认 limit=10）
        let recent_posts = crate::modules::post::repository::list_recent_public_posts(
            &state.pool, DEFAULT_POST_LIMIT,
        ).await.unwrap_or_default();

        let tags = crate::modules::tag::repository::list_tags(&state.pool)
            .await.unwrap_or_default();

        let categories = crate::modules::category::repository::list_categories(&state.pool)
            .await.unwrap_or_default();

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
}
