//! WebP 转换后台工作器。
//!
//! 通过 bounded channel 接收转换任务，在 spawn_blocking 中执行 CPU 密集的图片处理。
//! Semaphore 控制并发数，防止内存叠加。
//!
//! 架构：
//!   mpsc channel → run_conversion_loop → process_one_job
//!     ├─ acquire semaphore permit（限流）
//!     ├─ spawn_blocking（CPU 密集：读文件 → 解码 → 缩放 → 编码）
//!     ├─ atomic write（异步 IO）
//!     └─ DB update（异步）
//!
//! 关闭时：CancellationToken 触发 → 排空队列中已投递的任务 → 退出

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing;

use super::converter;
use super::repository;
use crate::bootstrap::config::MediaConfig;

/// 转换任务
#[derive(Debug, Clone)]
pub struct ConversionJob {
    pub media_id: String,
    pub source_path: PathBuf,
    pub mime_type: String,
    pub file_size: u64,
}

/// 启动 WebP 转换 worker，返回 sender 和 JoinHandle。
///
/// sender 用于投递任务（存入 `AppState.converter_send`）。
/// handle 用于关闭时等待排空。
pub fn start_webp_worker(
    pool: SqlitePool,
    upload_dir: PathBuf,
    config: &MediaConfig,
    shutdown_token: CancellationToken,
) -> (mpsc::Sender<ConversionJob>, tokio::task::JoinHandle<()>) {
    let max_concurrent = config.webp_max_concurrent.max(1);
    let channel_cap = max_concurrent * 20;
    let (tx, rx) = mpsc::channel::<ConversionJob>(channel_cap);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let max_edge = config.webp_max_edge;
    let quality = config.webp_quality as f32;

    let handle = tokio::spawn(run_conversion_loop(
        rx,
        semaphore,
        pool,
        upload_dir,
        max_edge,
        quality,
        shutdown_token,
    ));

    (tx, handle)
}

async fn run_conversion_loop(
    mut rx: mpsc::Receiver<ConversionJob>,
    semaphore: Arc<Semaphore>,
    pool: SqlitePool,
    upload_dir: PathBuf,
    max_edge: u32,
    quality: f32,
    shutdown_token: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                tracing::info!(
                    module = "webp_worker",
                    "shutdown signal received, draining queue"
                );
                // 排空队列中已投递的任务
                while let Ok(job) = rx.try_recv() {
                    process_one_job(
                        &semaphore, &pool, &upload_dir, job, max_edge, quality
                    ).await;
                }
                tracing::info!(module = "webp_worker", "shutdown complete");
                break;
            }
            job = rx.recv() => {
                let Some(job) = job else { break; }; // channel closed
                process_one_job(
                    &semaphore, &pool, &upload_dir, job, max_edge, quality
                ).await;
            }
            _ = tokio::time::sleep(Duration::from_secs(300)) => {
                // 每 5 分钟重新扫描 pending 记录（补偿可能因 channel 满而丢失的消息）
                if let Ok(pending) = repository::list_pending_conversions(&pool).await {
                    for item in pending {
                        let job = ConversionJob {
                            media_id: item.id,
                            source_path: upload_dir.join(&item.storage_path),
                            mime_type: item.mime_type.clone(),
                            file_size: item.size_bytes as u64,
                        };
                        process_one_job(&semaphore, &pool, &upload_dir, job, max_edge, quality).await;
                    }
                }
            }
        }
    }
}

async fn process_one_job(
    semaphore: &Semaphore,
    pool: &SqlitePool,
    upload_dir: &PathBuf,
    job: ConversionJob,
    max_edge: u32,
    quality: f32,
) {
    // 获取信号量——阻塞直到有空余并发槽位
    let _permit = semaphore.acquire().await.expect("semaphore closed");

    // 防御性检查：如果上游遗漏了跳过条件，此处兜底并清除 pending
    if converter::should_skip_conversion(&job.mime_type, job.file_size) {
        let _ = sqlx::query(
            "UPDATE media SET conversion_status = '', conversion_retries = 0, conversion_error = NULL WHERE id = ?"
        )
        .bind(&job.media_id)
        .execute(pool)
        .await;
        tracing::debug!(media_id = %job.media_id, mime = %job.mime_type, "skipped conversion (defensive check)");
        return;
    }

    let media_dir = upload_dir.join("media");

    // Step 1: CPU 密集工作放在 spawn_blocking 中
    //   - 读原图文件
    //   - 解码 JPEG/PNG → 缩放 → 编码 WebP
    //   - 返回 WebP 字节数据
    let cpu_result = {
        let source_path = job.source_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>, anyhow::Error> {
            let input = std::fs::read(&source_path)
                .map_err(|e| anyhow::anyhow!("读取原图失败: {}", e))?;

            let webp_data = converter::convert_to_webp(&input, max_edge, quality)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            Ok(webp_data)
        })
        .await
    };

    let webp_data = match cpu_result {
        Ok(Ok(data)) => data,
        Ok(Err(e)) => {
            // 转换失败：更新 DB 状态
            let _ = sqlx::query(
                "UPDATE media SET conversion_status = 'failed', conversion_retries = conversion_retries + 1, \
                 conversion_error = ?, updated_at = datetime('now') WHERE id = ?"
            )
            .bind(format!("{}", e))
            .bind(&job.media_id)
            .execute(pool)
            .await;
            tracing::warn!(
                module = "webp_worker",
                media_id = %job.media_id,
                error = %e,
                "conversion failed"
            );
            return;
        }
        Err(join_err) => {
            tracing::error!(
                module = "webp_worker",
                media_id = %job.media_id,
                error = %join_err,
                "spawn_blocking panicked"
            );
            return;
        }
    };

    let webp_len = webp_data.len();

    // Step 2: 异步原子写入 — WebP 独立存储，保留原文件
    let original_name = job
        .source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let webp_name = format!("{}.webp", original_name);
    let temp_path = media_dir.join(format!("{}.webp.tmp", original_name));
    let final_path = media_dir.join(&webp_name);

    match converter::atomic_write_webp(&temp_path, &final_path, &webp_data).await {
        Ok(()) => {
            // WebP 文件已独立保存，原文件 untouched
        }
        Err(e) => {
            tracing::error!(
                module = "webp_worker",
                media_id = %job.media_id,
                error = %e,
                "atomic write failed"
            );
            let _ = sqlx::query(
                "UPDATE media SET conversion_status = 'failed', conversion_retries = conversion_retries + 1, \
                 conversion_error = ?, updated_at = datetime('now') WHERE id = ?"
            )
            .bind(format!("{}", e))
            .bind(&job.media_id)
            .execute(pool)
            .await;
            return;
        }
    }

    // Step 3: 更新 DB —— 标记已转换（保留原 mime_type/size_bytes，由 handler 按协商选择返回）
    if let Err(e) = sqlx::query(
        "UPDATE media SET conversion_status = 'converted', conversion_retries = 0, \
         conversion_error = NULL, updated_at = datetime('now') WHERE id = ?"
    )
    .bind(&job.media_id)
    .execute(pool)
    .await
    {
        tracing::error!(
            module = "webp_worker",
            media_id = %job.media_id,
            error = %e,
            "failed to update DB after conversion"
        );
    } else {
        tracing::info!(
            module = "webp_worker",
            media_id = %job.media_id,
            bytes = webp_len,
            "converted to WebP"
        );
    }
}
