use serde::Deserialize;

use crate::shared::pagination::PaginationQuery;

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommentQuery {
    #[serde(flatten)]
    pub pagination: PaginationQuery,
    pub status: Option<String>,
}
