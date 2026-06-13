use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MediaItem {
    pub id: String,
    pub uploader_id: String,
    pub kind: String,
    pub mime_type: String,
    pub original_name: String,
    pub stored_name: String,
    pub storage_path: String,
    pub public_url: String,
    pub size_bytes: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub alt_text: Option<String>,
    /// 文件分类，可为 null
    pub category: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}
