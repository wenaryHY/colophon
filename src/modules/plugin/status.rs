use crate::shared::error::AppResult;

pub async fn get_enabled_ids<'e, E>(executor: E) -> AppResult<Vec<String>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query_scalar::<_, String>("SELECT id FROM plugins WHERE enabled = 1")
        .fetch_all(executor)
        .await?;
    Ok(rows)
}

pub async fn ensure_installed<'e, E>(
    executor: E,
    id: &str,
    title: &str,
    version: &str,
) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO plugins (id, title, version, enabled, installed_at) VALUES (?, ?, ?, 1, ?)",
    )
    .bind(id)
    .bind(title)
    .bind(version)
    .bind(&now)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn set_enabled<'e, E>(executor: E, id: &str, enabled: bool) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE plugins SET enabled = ? WHERE id = ?")
        .bind(enabled as i32)
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}
