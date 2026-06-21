use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow, utoipa::ToSchema)]
pub struct CurrentUser {
    pub id: String,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub avatar_media_id: Option<String>,
    pub bio: Option<String>,
    pub role: String,
    pub status: String,
    pub theme_preference: String,
    pub language: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// 公开可见的作者简档（不含 email、role 等敏感字段）
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AuthorProfile {
    pub username: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_media_id: Option<String>,
}
