use std::sync::Arc;

use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{shared::error::AppError, state::AppState};

use super::service;

pub async fn start_trash_scheduler(state: Arc<AppState>) -> Result<(), AppError> {
    // 获取配置，默认凌晨3点执行
    let hour_str =
        crate::modules::setting::repository::get_string(&state.pool, "trash_cleanup_hour", "3")
            .await
            .unwrap_or_else(|_| "3".into());
    let minute_str =
        crate::modules::setting::repository::get_string(&state.pool, "trash_cleanup_minute", "0")
            .await
            .unwrap_or_else(|_| "0".into());

    let hour = hour_str.parse::<u32>().unwrap_or(3).clamp(0, 23);
    let minute = minute_str.parse::<u32>().unwrap_or(0).clamp(0, 59);

    let cron = format!("0 {} {} * * * *", minute, hour);
    let mut scheduler = JobScheduler::new()
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("create trash scheduler failed: {e}")))?;

    let cloned = state.clone();
    let job = Job::new_async(cron.as_str(), move |_id, _lock| {
        let state = cloned.clone();
        Box::pin(async move {
            match service::purge_expired(state).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("auto-purged {} expired trash items", count);
                    }
                }
                Err(err) => {
                    tracing::error!(error = ?err, "scheduled trash purge execution failed");
                }
            }
        })
    })
    .map_err(|e| AppError::Anyhow(anyhow::anyhow!("create trash job failed: {e}")))?;

    scheduler
        .add(job)
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("register trash job failed: {e}")))?;
    scheduler
        .start()
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("start trash scheduler failed: {e}")))?;

    let cancel_token = state.shutdown_token.clone();
    let handle = tokio::spawn(async move {
        cancel_token.cancelled().await;
        tracing::info!(module = "trash_scheduler", "shutdown signal received");
        if let Err(e) = scheduler.shutdown().await {
            tracing::warn!(module = "trash_scheduler", error = %e, "scheduler shutdown error");
        } else {
            tracing::info!(module = "trash_scheduler", "stopped gracefully");
        }
    });

    *state.trash_scheduler_handle.lock().await = Some(handle);

    tracing::info!(cron = %cron, "trash cleanup scheduler started");
    Ok(())
}
