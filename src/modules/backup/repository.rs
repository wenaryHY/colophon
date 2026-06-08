use chrono::Utc;
use uuid::Uuid;

use super::domain::{Backup, BackupSchedule};

pub async fn create_backup<'e, E>(
    executor: E,
    provider: &str,
    size: i64,
    manifest_hash: &str,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO backups (id, created_at, size, provider, status, manifest_hash)
         VALUES (?, ?, ?, ?, 'completed', ?)",
    )
    .bind(&id)
    .bind(&now)
    .bind(size)
    .bind(provider)
    .bind(manifest_hash)
    .execute(executor)
    .await?;

    Ok(id)
}

pub async fn list_backups<'e, E>(executor: E) -> Result<Vec<Backup>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        Backup,
        r#"
        SELECT
            id as "id!",
            created_at as "created_at!",
            size as "size!",
            provider as "provider!",
            status as "status!",
            manifest_hash as "manifest_hash!",
            error_message
        FROM backups
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(executor)
    .await
}

pub async fn get_backup<'e, E>(executor: E, id: &str) -> Result<Option<Backup>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        Backup,
        r#"
        SELECT
            id as "id!",
            created_at as "created_at!",
            size as "size!",
            provider as "provider!",
            status as "status!",
            manifest_hash as "manifest_hash!",
            error_message
        FROM backups
        WHERE id = ?
        "#,
        id
    )
    .fetch_optional(executor)
    .await
}

pub async fn delete_backup<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("DELETE FROM backups WHERE id = ?")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn update_backup_status<'e, E>(
    executor: E,
    id: &str,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE backups SET status = ?, error_message = ? WHERE id = ?")
        .bind(status)
        .bind(error_message)
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn get_or_create_schedule<'e, E>(executor: E) -> Result<BackupSchedule, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    let existing = sqlx::query_as!(
        BackupSchedule,
        r#"
        SELECT
            id as "id!",
            enabled as "enabled!: bool",
            frequency as "frequency!",
            hour as "hour!: i32",
            minute as "minute!: i32",
            provider as "provider!",
            last_run_at,
            next_run_at,
            created_at as "created_at!",
            updated_at as "updated_at!"
        FROM backup_schedules
        LIMIT 1
        "#
    )
    .fetch_optional(executor)
    .await?;

    if let Some(schedule) = existing {
        return Ok(schedule);
    }

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO backup_schedules (id, enabled, frequency, hour, minute, provider, created_at, updated_at)
         VALUES (?, 0, 'daily', 2, 0, 'local', ?, ?)",
    )
    .bind(&id)
    .bind(&now)
    .bind(&now)
    .execute(executor)
    .await?;

    Ok(BackupSchedule {
        id,
        enabled: false,
        frequency: "daily".to_string(),
        hour: 2,
        minute: 0,
        provider: "local".to_string(),
        last_run_at: None,
        next_run_at: None,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn update_schedule<'e, E>(
    executor: E,
    enabled: bool,
    frequency: &str,
    hour: i32,
    minute: i32,
    provider: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE backup_schedules
         SET enabled = ?, frequency = ?, hour = ?, minute = ?, provider = ?, updated_at = ?
         WHERE id = (SELECT id FROM backup_schedules LIMIT 1)",
    )
    .bind(enabled)
    .bind(frequency)
    .bind(hour)
    .bind(minute)
    .bind(provider)
    .bind(&now)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn update_schedule_run_time<'e, E>(
    executor: E,
    last_run_at: &str,
    next_run_at: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE backup_schedules
         SET last_run_at = ?, next_run_at = ?
         WHERE id = (SELECT id FROM backup_schedules LIMIT 1)",
    )
    .bind(last_run_at)
    .bind(next_run_at)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn set_next_run_at<'e, E>(executor: E, next_run_at: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE backup_schedules
         SET next_run_at = ?
         WHERE id = (SELECT id FROM backup_schedules LIMIT 1)",
    )
    .bind(next_run_at)
    .execute(executor)
    .await?;

    Ok(())
}
