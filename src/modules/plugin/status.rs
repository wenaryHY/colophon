use sqlx::SqlitePool;

use crate::shared::error::AppResult;

pub async fn get_enabled_ids(pool: &SqlitePool) -> AppResult<Vec<String>> {
    let rows = sqlx::query_scalar::<_, String>("SELECT id FROM plugins WHERE enabled = 1")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn ensure_installed(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    version: &str,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO plugins (id, title, version, enabled, installed_at) VALUES (?, ?, ?, 1, ?)",
    )
    .bind(id)
    .bind(title)
    .bind(version)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<()> {
    sqlx::query("UPDATE plugins SET enabled = ? WHERE id = ?")
        .bind(enabled as i32)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
