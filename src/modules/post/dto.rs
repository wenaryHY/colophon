use serde::{Deserialize, Serialize};

use crate::{
    modules::tag::domain::Tag,
    shared::pagination::PaginationQuery,
};

use super::post_types::{ContentType, PostStatus, Visibility};

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct PostQuery {
    #[serde(flatten)]
    pub pagination: PaginationQuery,
    pub keyword: Option<String>,
    pub status: Option<PostStatus>,
    pub content_type: Option<ContentType>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SearchQuery {
    pub keyword: String,
    pub category_id: Option<String>,
    pub tag_id: Option<String>,
    #[serde(flatten)]
    pub pagination: PaginationQuery,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreatePostRequest {
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub content_md: Option<String>,
    pub cover_media_id: Option<String>,
    pub status: Option<PostStatus>,
    pub visibility: Option<Visibility>,
    pub category_id: Option<String>,
    pub tag_ids: Option<Vec<String>>,
    pub allow_comment: Option<bool>,
    pub pinned: Option<bool>,
    pub content_type: Option<ContentType>,
    pub custom_html_path: Option<String>,
    pub page_render_mode: Option<String>,
    pub content_html: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdatePostRequest {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub content_md: Option<String>,
    pub cover_media_id: Option<String>,
    pub status: Option<PostStatus>,
    pub visibility: Option<Visibility>,
    pub category_id: Option<String>,
    pub tag_ids: Option<Vec<String>>,
    pub allow_comment: Option<bool>,
    pub pinned: Option<bool>,
    pub content_type: Option<ContentType>,
    pub custom_html_path: Option<String>,
    pub page_render_mode: Option<String>,
    pub content_html: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublicPostResponse {
    #[serde(flatten)]
    pub post: super::domain::PublicPostDetail,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AdminPostResponse {
    #[serde(flatten)]
    pub post: super::domain::AdminPost,
    pub tags: Vec<Tag>,
}
