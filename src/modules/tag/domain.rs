use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow, utoipa::ToSchema)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}
