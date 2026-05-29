use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SitemapItem {
    pub slug: String,
    pub content_type: String,
    pub published_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PublicPostSummary {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content_type: String,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub author_display_name: String,
    pub category_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PublicPostDetail {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content_html: String,
    pub content_type: String,
    pub allow_comment: i64,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub author_display_name: String,
    pub category_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AdminPost {
    pub id: String,
    pub author_id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content_md: String,
    pub content_html: String,
    pub cover_media_id: Option<String>,
    pub status: String,
    pub visibility: String,
    pub category_id: Option<String>,
    pub allow_comment: i64,
    pub pinned: i64,
    pub content_type: String,
    pub custom_html_path: Option<String>,
    pub page_render_mode: String,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct CommentTargetPost {
    pub id: String,
    pub title: String,
    pub status: String,
    pub visibility: String,
    pub allow_comment: i64,
}
