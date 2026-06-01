use serde::{Deserialize, Serialize};
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
    /// 缩略图列表（不从数据库映射，由 service 层查询后填充）
    #[sqlx(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnails: Option<Vec<MediaThumbnail>>,
}

/// 缩略图记录（与 media_thumbnails 表对应）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MediaThumbnail {
    pub id: String,
    pub media_id: String,
    pub size_label: String,
    pub width: i64,
    pub height: i64,
    pub storage_path: String,
    pub public_url: String,
    pub size_bytes: i64,
    pub created_at: String,
}

/// 异步缩略图任务（与 thumbnail_tasks 表对应）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ThumbnailTask {
    pub id: String,
    pub media_id: String,
    pub status: String,
    pub retry_count: i64,
    pub max_retries: i64,
    pub last_error: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}
