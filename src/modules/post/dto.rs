use serde::{Deserialize, Serialize};

use crate::modules::tag::domain::Tag;

#[derive(Debug, Deserialize)]
pub struct PostQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub keyword: Option<String>,
    pub status: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub keyword: String,
    pub category_id: Option<String>,
    pub tag_id: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub content_md: Option<String>,
    pub cover_media_id: Option<String>,
    pub status: Option<String>,
    pub visibility: Option<String>,
    pub category_id: Option<String>,
    pub tag_ids: Option<Vec<String>>,
    pub allow_comment: Option<bool>,
    pub pinned: Option<bool>,
    pub content_type: Option<String>,
    pub custom_html_path: Option<String>,
    pub page_render_mode: Option<String>,
    pub content_html: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostRequest {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub content_md: Option<String>,
    pub cover_media_id: Option<String>,
    pub status: Option<String>,
    pub visibility: Option<String>,
    pub category_id: Option<String>,
    pub tag_ids: Option<Vec<String>>,
    pub allow_comment: Option<bool>,
    pub pinned: Option<bool>,
    pub content_type: Option<String>,
    pub custom_html_path: Option<String>,
    pub page_render_mode: Option<String>,
    pub content_html: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PublicPostResponse {
    #[serde(flatten)]
    pub post: super::domain::PublicPostDetail,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Serialize)]
pub struct AdminPostResponse {
    #[serde(flatten)]
    pub post: super::domain::AdminPost,
    pub tags: Vec<Tag>,
}
