use sqlx::SqlitePool;
use std::collections::HashMap;
use crate::shared::error::AppResult;

pub async fn get_all(pool: &SqlitePool, plugin_name: &str) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value FROM plugin_settings WHERE plugin_name = ?"
    )
    .bind(plugin_name)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

pub async fn set(pool: &SqlitePool, plugin_name: &str, key: &str, value: &str) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO plugin_settings (plugin_name, key, value, updated_at) VALUES (?, ?, ?, ?)
         ON CONFLICT(plugin_name, key) DO UPDATE SET value = ?, updated_at = ?"
    )
    .bind(plugin_name)
    .bind(key)
    .bind(value)
    .bind(&now)
    .bind(value)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_all(pool: &SqlitePool, plugin_name: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM plugin_settings WHERE plugin_name = ?")
        .bind(plugin_name)
        .execute(pool)
        .await?;
    Ok(())
}
