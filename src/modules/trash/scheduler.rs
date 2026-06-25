use std::sync::Arc;

use chrono_tz::Tz;
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

    // 读取站点时区配置，转换为 UTC 时间用于 cron 表达式
    let tz_str = crate::modules::setting::repository::get_string(&state.pool, "site_timezone", "UTC")
        .await
        .unwrap_or_else(|_| "UTC".into());
    let tz: Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);
    let (utc_hour, utc_minute) = crate::modules::backup::service::local_time_to_utc_for_cron(hour, minute, tz);
    let cron = format!("0 {} {} * * * *", utc_minute, utc_hour);
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

    tracing::info!(
        cron = %cron,
        local_time = %format!("{:02}:{:02}", hour, minute),
        utc_time = %format!("{:02}:{:02}", utc_hour, utc_minute),
        tz = %tz.name(),
        "trash cleanup scheduler started"
    );
    Ok(())
}
