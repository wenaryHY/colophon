use serde::Deserialize;

use crate::shared::pagination::PaginationQuery;

#[derive(Debug, Deserialize)]
pub struct MediaQuery {
    #[serde(flatten)]
    pub pagination: PaginationQuery,
    pub kind: Option<String>,
    pub keyword: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenameMediaRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryRequest {
    pub category: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMediaCategoryRequest {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMediaCategoryCrudRequest {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: Option<i64>,
}
