use serde::Serialize;
use sqlx::FromRow;

use super::post_types::{ContentType, PostStatus, Visibility};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SitemapItem {
    pub slug: String,
    pub content_type: ContentType,
    pub published_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PublicPostSummary {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content_type: ContentType,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub author_display_name: String,
    pub category_name: Option<String>,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PublicPostDetail {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content_html: String,
    pub content_type: ContentType,
    pub allow_comment: bool,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub author_display_name: String,
    pub category_name: Option<String>,
    pub cover_media_id: Option<String>,
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
    pub status: PostStatus,
    pub visibility: Visibility,
    pub category_id: Option<String>,
    pub allow_comment: bool,
    pub pinned: bool,
    pub content_type: ContentType,
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
    pub status: PostStatus,
    pub visibility: Visibility,
    pub allow_comment: bool,
}
