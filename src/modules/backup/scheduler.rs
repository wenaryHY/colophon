use std::{str::FromStr, sync::Arc};

use chrono::Utc;
use cron::Schedule;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{shared::error::AppError, state::AppState};

use super::{
    domain::{BackupProvider, BackupScheduleFrequency},
    repository, service,
};

/// Start the backup scheduler based on DB config.
/// Stores the scheduler handle in `state.backup_scheduler` for lifecycle management.
pub async fn start_backup_scheduler(state: Arc<AppState>) -> Result<(), AppError> {
    let schedule = repository::get_or_create_schedule(&state.pool).await?;
    if !schedule.enabled {
        tracing::info!("backup scheduler disabled, skipping startup");
        return Ok(());
    }

    let frequency = BackupScheduleFrequency::from_str(&schedule.frequency)
        .ok_or_else(|| AppError::BadRequest("invalid backup frequency".into()))?;
    let provider = BackupProvider::from_str(&schedule.provider)
        .ok_or_else(|| AppError::BadRequest("invalid backup provider".into()))?;

    let schedule_hour = schedule.hour as u32;
    let schedule_minute = schedule.minute as u32;
    let cron = frequency.cron_expression(schedule_hour, schedule_minute);
    let scheduler = JobScheduler::new()
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("create scheduler failed: {e}")))?;

    let cloned = state.clone();
    let job = Job::new_async(cron.as_str(), move |_id, _lock| {
        let state = cloned.clone();
        Box::pin(async move {
            match service::create_backup(state.clone(), provider).await {
                Ok(_) => {
                    // Update last_run_at and compute next_run_at from cron expression
                    if let Err(err) = update_run_times_after_backup(
                        &state, &frequency, schedule_hour, schedule_minute,
                    )
                    .await
                    {
                        tracing::error!(error = ?err, "failed to update schedule run times after backup");
                    }
                }
                Err(err) => {
                    tracing::error!(error = ?err, "scheduled backup execution failed");
                }
            }
        })
    })
    .map_err(|e| AppError::Anyhow(anyhow::anyhow!("create backup job failed: {e}")))?;

    scheduler
        .add(job)
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("register backup job failed: {e}")))?;
    scheduler
        .start()
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("start backup scheduler failed: {e}")))?;

    tracing::info!(cron = %cron, "backup scheduler started");

    // Store handle for dynamic stop/restart (replaces mem::forget)
    *state.backup_scheduler.lock().await = Some(scheduler);
    Ok(())
}

/// Stop the running backup scheduler (if any) and start a new one from current DB config.
/// Called by `update_schedule` when the user changes backup schedule settings.
pub async fn restart_backup_scheduler(state: Arc<AppState>) -> Result<(), AppError> {
    stop_backup_scheduler(&state).await;
    start_backup_scheduler(state).await
}

/// Gracefully stop the running backup scheduler.
async fn stop_backup_scheduler(state: &AppState) {
    let mut guard = state.backup_scheduler.lock().await;
    if let Some(mut scheduler) = guard.take() {
        if let Err(err) = scheduler.shutdown().await {
            tracing::warn!(error = ?err, "failed to shutdown old backup scheduler");
        } else {
            tracing::info!("old backup scheduler stopped");
        }
    }
}

/// Update `last_run_at` (now) and `next_run_at` (computed from cron expression) after a successful backup.
async fn update_run_times_after_backup(
    state: &AppState,
    frequency: &BackupScheduleFrequency,
    hour: u32,
    minute: u32,
) -> Result<(), AppError> {
    let now = Utc::now();
    let (utc_hour, utc_minute) = service::local_time_to_utc_for_cron(hour, minute);
    let cron_expr = frequency.cron_expression(utc_hour, utc_minute);
    let next = calculate_next_run_at_from_cron_expression(&cron_expr);
    repository::update_schedule_run_time(
        &state.pool,
        &now.to_rfc3339(),
        &next.to_rfc3339(),
    )
    .await?;
    Ok(())
}

/// 基于 cron 表达式计算下一次触发时间。
/// 如果 cron 表达式无效，fallback 到 now + 1 小时。
fn calculate_next_run_at_from_cron_expression(cron_expression: &str) -> chrono::DateTime<Utc> {
    Schedule::from_str(cron_expression)
        .ok()
        .and_then(|schedule| schedule.upcoming(Utc).next())
        .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1))
}
