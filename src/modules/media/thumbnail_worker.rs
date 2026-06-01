use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};

use crate::{
    shared::error::AppResult,
    state::AppState,
};

use super::{
    domain::{MediaThumbnail, ThumbnailTask},
    repository,
    thumbnail::{generate_thumbnails, ThumbnailGenerationConfig},
};

/// 缩略图生成的目标宽度（不放大，所以大图生成多个尺寸，小图只生成比它小的）
const THUMBNAIL_TARGET_WIDTHS: &[u32] = &[400, 800, 1200];

/// 后台轮询间隔（秒）
const WORKER_POLL_INTERVAL_SECS: u64 = 2;

/// 启动后台缩略图 worker
/// 应在 `serve()` 中、路由构建后、监听前调用
pub fn start_thumbnail_worker(state: Arc<AppState>) {
    let concurrency = state.config.media.thumbnail.concurrency as usize;
    let semaphore = Arc::new(Semaphore::new(concurrency));

    tokio::spawn(async move {
        tracing::info!(
            module = "media",
            event = "thumbnail_worker_started",
            concurrency = concurrency,
            "thumbnail worker started"
        );

        loop {
            sleep(Duration::from_secs(WORKER_POLL_INTERVAL_SECS)).await;

            if !state.config.media.thumbnail.enabled {
                continue;
            }

            // 取出一个 pending 任务（内部原子 UPDATE + SELECT）
            let task = match repository::take_one_pending_thumbnail_task(&state.pool).await {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(e) => {
                    tracing::error!(
                        module = "media",
                        event = "thumbnail_worker_db_error",
                        error = %e,
                        "failed to take pending thumbnail task"
                    );
                    continue;
                }
            };

            let state = Arc::clone(&state);
            let semaphore = Arc::clone(&semaphore);

            // spawn 独立 tokio task，不受 worker 循环阻塞
            tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();

                let start = std::time::Instant::now();
                let result = process_one_thumbnail_task(&state, &task).await;
                let elapsed_ms = start.elapsed().as_millis();

                match result {
                    Ok((w, h)) => {
                        let _ = repository::mark_thumbnail_task_done(
                            &state.pool,
                            &task.id,
                            w,
                            h,
                        )
                        .await;
                        tracing::info!(
                            module = "media",
                            event = "thumbnail_task_completed",
                            media_id = %task.media_id,
                            task_id = %task.id,
                            width = w,
                            height = h,
                            elapsed_ms = elapsed_ms,
                        );
                    }
                    Err(e) => {
                        let should_retry = task.retry_count < task.max_retries;
                        let _ = repository::mark_thumbnail_task_failed(
                            &state.pool,
                            &task.id,
                            &e.to_string(),
                            should_retry,
                        )
                        .await;
                        tracing::warn!(
                            module = "media",
                            event = "thumbnail_task_failed",
                            media_id = %task.media_id,
                            task_id = %task.id,
                            retry_count = task.retry_count,
                            should_retry = should_retry,
                            error = %e,
                            elapsed_ms = elapsed_ms,
                        );
                    }
                }
            });
        }
    });
}

/// 处理单个缩略图任务：解码原图 → 生成缩略图 → 写入记录
async fn process_one_thumbnail_task(
    state: &AppState,
    task: &ThumbnailTask,
) -> AppResult<(u32, u32)> {
    tracing::info!(
        module = "media",
        event = "thumbnail_task_processing_start",
        task_id = %task.id,
        media_id = %task.media_id,
        "starting thumbnail processing"
    );

    // 1. 从 media 表获取源文件信息
    let media = repository::get_media(&state.pool, &task.media_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!(
            "media_id={} not found (may have been deleted)", task.media_id
        ))?;

    let source_path = state.upload_dir.join(&media.storage_path);
    let output_dir = state.upload_dir.join("thumb");

    // 2. spawn_blocking 生成缩略图（避免阻塞 async runtime）
    let media_id = task.media_id.clone();
    let config = ThumbnailGenerationConfig {
        widths: THUMBNAIL_TARGET_WIDTHS.to_vec(),
        keep_original: true,
    };

    tracing::info!(
        module = "media",
        event = "thumbnail_task_spawning_blocking",
        task_id = %task.id,
        "entering spawn_blocking for thumbnail generation"
    );

    let result = tokio::task::spawn_blocking(move || {
        generate_thumbnails(&source_path, &output_dir, &media_id, &config)
    })
    .await
    .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {}", e))??;

    let (orig_w, orig_h, thumbs) = result;

    // 3. 写入缩略图记录到 media_thumbnails 表
    let thumb_records: Vec<MediaThumbnail> = thumbs
        .into_iter()
        .map(|t| MediaThumbnail {
            id: uuid::Uuid::new_v4().to_string(),
            media_id: task.media_id.clone(),
            size_label: t.size_label,
            width: t.width as i64,
            height: t.height as i64,
            storage_path: t.storage_path,
            public_url: t.public_url,
            size_bytes: t.size_bytes,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .collect();

    if !thumb_records.is_empty() {
        repository::insert_media_thumbnails(&state.pool, &thumb_records).await?;
    }

    Ok((orig_w, orig_h))
}
