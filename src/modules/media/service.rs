use std::{path::PathBuf, sync::Arc};

use crate::{
    shared::{
        auth::AuthUser,
        error::{AppError, AppResult},
        pagination::PaginationQuery,
        response::PaginatedResponse,
    },
    state::AppState,
};

use super::{
    domain::{MediaItem, MediaThumbnail, ThumbnailTask},
    dto::MediaQuery,
    repository,
};

const ALLOWED_MIME_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
    "audio/mpeg",
    "audio/ogg",
    "audio/wav",
    "audio/mp4",
];

/// 最多允许堆积的 pending 缩略图任务数（攻击防御）
/// 结合 max_upload_size_mb=10，单次攻击最多消耗 50×10MB=500MB 磁盘
const MAX_PENDING_THUMBNAIL_TASKS: i64 = 50;

fn classify_file(ext: &str) -> Option<(&'static str, &'static str)> {
    match ext {
        "jpg" | "jpeg" => Some(("image", "image/jpeg")),
        "png" => Some(("image", "image/png")),
        "webp" => Some(("image", "image/webp")),
        "gif" => Some(("image", "image/gif")),
        "mp3" => Some(("audio", "audio/mpeg")),
        "ogg" => Some(("audio", "audio/ogg")),
        "wav" => Some(("audio", "audio/wav")),
        "m4a" => Some(("audio", "audio/mp4")),
        _ => None,
    }
}

pub async fn list_media(
    state: Arc<AppState>,
    query: MediaQuery,
) -> AppResult<PaginatedResponse<MediaItem>> {
    let pagination = PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    };
    let (page, page_size, offset) = pagination.normalized(20, 100);
    let kind = query.kind.as_deref();
    let keyword = query.keyword.as_deref().filter(|k| !k.trim().is_empty());
    let category = query.category.as_deref().filter(|k| !k.trim().is_empty());
    let mut items =
        repository::list_media(&state.pool, kind, keyword, category, page_size, offset).await?;
    let total = repository::count_media(&state.pool, kind, keyword, category).await?;

    // 批量查询所有媒体的缩略图
    if !items.is_empty() {
        let media_ids: Vec<String> = items.iter().map(|m| m.id.clone()).collect();
        let all_thumbs = repository::get_thumbnails_by_media_ids(&state.pool, &media_ids).await?;
        // 按 media_id 分组
        use std::collections::HashMap;
        let mut thumb_map: HashMap<String, Vec<MediaThumbnail>> = HashMap::new();
        for thumb in all_thumbs {
            thumb_map.entry(thumb.media_id.clone()).or_default().push(thumb);
        }
        for item in &mut items {
            item.thumbnails = thumb_map.remove(&item.id);
        }
    }

    Ok(PaginatedResponse::new(items, page, page_size, total))
}

/// 直接接收原始文件数据的媒体上传（供手动解析 multipart 的 handler 调用）
pub async fn upload_media_raw(
    state: Arc<AppState>,
    auth: &AuthUser,
    original_name: String,
    content_type: Option<String>,
    data: Vec<u8>,
    category: Option<String>,
) -> AppResult<MediaItem> {
    let ext = std::path::Path::new(&original_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .ok_or_else(|| AppError::BadRequest("file extension is required".into()))?;
    let (kind, expected_mime) =
        classify_file(&ext).ok_or_else(|| AppError::BadRequest("unsupported media type".into()))?;
    let mime_type = content_type
        .filter(|ct| !ct.is_empty())
        .or_else(|| mime_guess::from_ext(&ext).first().map(|m| m.to_string()))
        .unwrap_or_else(|| expected_mime.to_string());

    if !ALLOWED_MIME_TYPES.contains(&mime_type.as_str()) {
        return Err(AppError::BadRequest("mime type is not allowed".into()));
    }

    if mime_type != expected_mime {
        return Err(AppError::BadRequest(
            "mime type does not match file extension".into(),
        ));
    }

    let bytes = data;
    let max_bytes = state.config.storage.max_upload_size_mb * 1024 * 1024;
    if bytes.len() as u64 > max_bytes {
        return Err(AppError::BadRequest(format!(
            "file exceeds max size of {} MB",
            state.config.storage.max_upload_size_mb
        )));
    }

    let resolved_category =
        super::category::ensure_category_exists_or_resolve(&state, category.as_deref(), &ext)
            .await?;

    // 确保 thumb 子目录存在（worker 写入缩略图时需要）
    let _thumb_dir = state.upload_dir.join("thumb");
    tokio::fs::create_dir_all(&_thumb_dir).await.ok();

    let stored_name = format!("{}.{}", uuid::Uuid::new_v4(), ext);
    let relative_path = PathBuf::from("media").join(&stored_name);
    let absolute_path = state.upload_dir.join(&relative_path);
    if let Some(parent) = absolute_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&absolute_path, &bytes).await?;

    let storage_path = relative_path.to_string_lossy().replace('\\', "/");
    let public_url = format!("/uploads/{}", storage_path);

    // 先生成媒体 ID，以便缩略图关联
    let media_id = uuid::Uuid::new_v4().to_string();

    // 异步缩略图：图片类型（非 GIF）且缩略图功能启用时，创建后台任务
    let thumbnails: Vec<MediaThumbnail> = Vec::new();
    let is_image = kind == "image";
    let is_gif = mime_type.contains("gif");

    // 插入媒体记录（使用预生成的 ID）
    let storage_path_for_db = storage_path.clone();
    let public_url_for_db = public_url.clone();
    sqlx::query(
        "INSERT INTO media (
            id, uploader_id, kind, mime_type, original_name, stored_name, storage_path, public_url, size_bytes, category
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&media_id)
    .bind(&auth.id)
    .bind(kind)
    .bind(&mime_type)
    .bind(&original_name)
    .bind(&stored_name)
    .bind(&storage_path_for_db)
    .bind(&public_url_for_db)
    .bind(bytes.len() as i64)
    .bind(&resolved_category)
    .execute(&state.pool)
    .await?;

    // 插入缩略图记录
    if !thumbnails.is_empty() {
        repository::insert_media_thumbnails(&state.pool, &thumbnails).await?;
    }

    let mut media = repository::get_media(&state.pool, &media_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // 异步缩略图：在 media 记录插入后创建任务，确保 FK 约束满足
    if is_image && !is_gif && state.config.media.thumbnail.enabled {
        // 防御：pending 任务数超限则拒绝上传（HTTP 429）
        let pending_count =
            repository::count_pending_thumbnail_tasks(&state.pool).await?;

        tracing::info!(
            module = "media",
            event = "thumbnail_task_about_to_create",
            media_id = %media_id,
            pending_count = pending_count,
            file_size_bytes = bytes.len(),
            "creating thumbnail task"
        );

        if pending_count >= MAX_PENDING_THUMBNAIL_TASKS {
            tracing::warn!(
                module = "media",
                event = "thumbnail_task_rejected_queue_full",
                media_id = %media_id,
                pending_count = pending_count,
                "thumbnail task rejected: queue full"
            );
            return Err(AppError::TooManyRequests(
                "too many pending thumbnail tasks, try again later".into(),
            ));
        }

        let task = ThumbnailTask {
            id: uuid::Uuid::new_v4().to_string(),
            media_id: media_id.clone(),
            status: "pending".to_string(),
            retry_count: 0,
            max_retries: 1,
            last_error: None,
            width: None,
            height: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        let insert_result = repository::insert_thumbnail_task(&state.pool, &task).await;
        match &insert_result {
            Ok(_) => {
                tracing::info!(
                    module = "media",
                    event = "thumbnail_task_created",
                    media_id = %media_id,
                    task_id = %task.id,
                    "thumbnail task created successfully"
                );
            }
            Err(e) => {
                tracing::error!(
                    module = "media",
                    event = "thumbnail_task_create_failed",
                    media_id = %media_id,
                    task_id = %task.id,
                    error = %e,
                    "failed to create thumbnail task"
                );
            }
        }
        insert_result?;
    }

    media.thumbnails = Some(thumbnails);
    Ok(media)
}

pub async fn delete_media(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    let media = repository::get_media(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    // 删除缩略图文件
    let thumbs = repository::get_thumbnails_by_media_id(&state.pool, id).await?;
    for thumb in &thumbs {
        let thumb_absolute = state.upload_dir.join(&thumb.storage_path);
        if thumb_absolute.exists() {
            tokio::fs::remove_file(&thumb_absolute).await.ok();
        }
    }
    // 删除缩略图数据库记录
    repository::delete_thumbnails_by_media_id(&state.pool, id).await?;

    let absolute_path = state.upload_dir.join(&media.storage_path);
    if absolute_path.exists() {
        tokio::fs::remove_file(&absolute_path).await?;
    }
    repository::delete_media(&state.pool, id).await?;
    Ok(serde_json::json!({ "deleted": true }))
}

pub async fn rename_media(
    state: Arc<AppState>,
    id: &str,
    new_name: &str,
) -> AppResult<serde_json::Value> {
    if new_name.trim().is_empty() {
        return Err(AppError::BadRequest("文件名不能为空".into()));
    }
    repository::get_media(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    repository::rename_media(&state.pool, id, new_name.trim()).await?;
    Ok(serde_json::json!({ "renamed": true }))
}

pub async fn update_category(
    state: Arc<AppState>,
    id: &str,
    category: Option<&str>,
) -> AppResult<serde_json::Value> {
    repository::get_media(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    repository::update_media_category(&state.pool, id, category).await?;
    Ok(serde_json::json!({ "updated": true }))
}
