use std::{path::PathBuf, sync::Arc};

use crate::{
    shared::{
        auth::AuthUser,
        error::{AppError, AppResult},
        response::{deleted_json, PaginatedResponse},
    },
    state::AppState,
};

use super::{domain::MediaItem, dto::MediaQuery, repository};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_jpg() {
        let (kind, mime) = classify_file("jpg").unwrap();
        assert_eq!(kind, "image");
        assert_eq!(mime, "image/jpeg");
    }

    #[test]
    fn classify_jpeg_alias() {
        let (kind, mime) = classify_file("jpeg").unwrap();
        assert_eq!(kind, "image");
        assert_eq!(mime, "image/jpeg");
    }

    #[test]
    fn classify_png() {
        let (kind, mime) = classify_file("png").unwrap();
        assert_eq!(kind, "image");
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn classify_webp() {
        let (kind, mime) = classify_file("webp").unwrap();
        assert_eq!(kind, "image");
        assert_eq!(mime, "image/webp");
    }

    #[test]
    fn classify_gif() {
        let (kind, mime) = classify_file("gif").unwrap();
        assert_eq!(kind, "image");
        assert_eq!(mime, "image/gif");
    }

    #[test]
    fn classify_mp3() {
        let (kind, mime) = classify_file("mp3").unwrap();
        assert_eq!(kind, "audio");
        assert_eq!(mime, "audio/mpeg");
    }

    #[test]
    fn classify_m4a() {
        let (kind, mime) = classify_file("m4a").unwrap();
        assert_eq!(kind, "audio");
        assert_eq!(mime, "audio/mp4");
    }

    #[test]
    fn classify_unsupported_returns_none() {
        assert!(classify_file("exe").is_none());
        assert!(classify_file("pdf").is_none());
        assert!(classify_file("").is_none());
    }

    #[test]
    fn allowed_mime_types_contains_expected() {
        assert!(ALLOWED_MIME_TYPES.contains(&"image/jpeg"));
        assert!(ALLOWED_MIME_TYPES.contains(&"image/png"));
        assert!(ALLOWED_MIME_TYPES.contains(&"audio/mpeg"));
        assert!(!ALLOWED_MIME_TYPES.contains(&"application/pdf"));
    }
}

pub async fn list_media(
    state: Arc<AppState>,
    query: MediaQuery,
) -> AppResult<PaginatedResponse<MediaItem>> {
    let (page, page_size, offset) = query.pagination.normalized(20, 100);
    let kind = query.kind.as_deref();
    let keyword = query.keyword.as_deref().filter(|k| !k.trim().is_empty());
    let category = query.category.as_deref().filter(|k| !k.trim().is_empty());
    let items =
        repository::list_media(&state.pool, kind, keyword, category, page_size, offset).await?;
    let total = repository::count_media(&state.pool, kind, keyword, category).await?;

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

    let stored_name = format!("{}.{}", uuid::Uuid::new_v4(), ext);
    let relative_path = PathBuf::from("media").join(&stored_name);
    let absolute_path = state.upload_dir.join(&relative_path);
    if let Some(parent) = absolute_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&absolute_path, &bytes).await?;

    let storage_path = relative_path.to_string_lossy().replace('\\', "/");
    let public_url = format!("/uploads/{}", storage_path);

    // 插入媒体记录
    let media_id = repository::insert_media(
        &state.pool,
        &auth.id,
        kind,
        &mime_type,
        &original_name,
        &stored_name,
        &storage_path,
        &public_url,
        bytes.len() as i64,
        &resolved_category,
    )
    .await?;

    // ── 异步触发 WebP 转换 ──
    // 通过 bounded channel 投递给后台 worker，不阻塞上传响应
    // 跳过条件统一由 converter::should_skip_conversion 判定，
    // 避免 service 与 worker 判断不一致导致记录永久卡在 pending
    if state.config.media.webp_enabled
        && !super::converter::should_skip_conversion(&mime_type, bytes.len() as u64)
    {
        tracing::debug!(
            media_id = %media_id,
            mime_type = %mime_type,
            "triggering webp conversion for uploaded image"
        );

        // 先标记为 pending
        if let Err(e) = repository::mark_media_pending(&state.pool, &media_id).await {
            tracing::warn!(media_id = %media_id, error = %e, "failed to mark media as pending");
        } else if let Some(tx) = state.converter_send.lock().await.as_ref() {
            let job = crate::modules::media::worker::ConversionJob {
                media_id: media_id.clone(),
                source_path: absolute_path.clone(),
                mime_type: mime_type.clone(),
                file_size: bytes.len() as u64,
            };
            // 非阻塞发送：如果 channel 满了则丢弃（worker 会在启动时扫描 pending 重新入队）
            let _ = tx.try_send(job);

            tracing::info!(
                media_id = %media_id,
                mime_type = %mime_type,
                file_size = bytes.len(),
                "webp conversion job submitted"
            );
        }
    }

    repository::get_media(&state.pool, &media_id)
        .await?
        .ok_or(AppError::NotFound("媒体文件未找到".to_string()))
}

pub async fn get_media_item(state: &AppState, id: &str) -> AppResult<MediaItem> {
    repository::get_media(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound(format!("媒体文件 '{}' 未找到", id)))
}

pub async fn delete_media(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    let media = repository::get_media(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound(format!("媒体文件 '{}' 未找到", id)))?;

    let absolute_path = state.upload_dir.join(&media.storage_path);
    if absolute_path.exists() {
        tokio::fs::remove_file(&absolute_path).await?;
    }
    repository::delete_media(&state.pool, id).await?;
    deleted_json()
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
        .ok_or(AppError::NotFound(format!("媒体文件 '{}' 未找到", id)))?;
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
        .ok_or(AppError::NotFound(format!("媒体文件 '{}' 未找到", id)))?;
    repository::update_media_category(&state.pool, id, category).await?;
    Ok(serde_json::json!({ "updated": true }))
}
