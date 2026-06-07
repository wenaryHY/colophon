use crate::modules::post::post_types::ContentType;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CommentItem {
    pub id: String,
    pub post_id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub content: String,
    pub status: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AdminCommentItem {
    pub id: String,
    pub post_id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub content: String,
    pub status: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub post_title: String,
    pub post_slug: String,
    pub post_content_type: ContentType,
}
