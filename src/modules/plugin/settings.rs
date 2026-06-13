use crate::shared::error::AppResult;
use std::collections::HashMap;

pub async fn get_all<'e, E>(executor: E, plugin_name: &str) -> AppResult<HashMap<String, String>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    #[derive(sqlx::FromRow)]
    struct PluginSettingRow {
        key: String,
        value: String,
    }

    let rows = sqlx::query_as!(
        PluginSettingRow,
        r#"
        SELECT key, value
        FROM plugin_settings
        WHERE plugin_name = ?
        "#,
        plugin_name
    )
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().map(|r| (r.key, r.value)).collect())
}

pub async fn set<'e, E>(executor: E, plugin_name: &str, key: &str, value: &str) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO plugin_settings (plugin_name, key, value, updated_at) VALUES (?, ?, ?, ?)
         ON CONFLICT(plugin_name, key) DO UPDATE SET value = ?, updated_at = ?",
    )
    .bind(plugin_name)
    .bind(key)
    .bind(value)
    .bind(&now)
    .bind(value)
    .bind(&now)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn delete_all<'e, E>(executor: E, plugin_name: &str) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("DELETE FROM plugin_settings WHERE plugin_name = ?")
        .bind(plugin_name)
        .execute(executor)
        .await?;
    Ok(())
}
