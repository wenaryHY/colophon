use crate::modules::setting::dto::SettingItem;

pub async fn list<'e, E>(executor: E) -> Result<Vec<SettingItem>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    #[derive(sqlx::FromRow)]
    struct SettingRow {
        key: String,
        value: String,
    }
    
    sqlx::query_as!(
        SettingRow,
        r#"
        SELECT key, value
        FROM settings
        ORDER BY key ASC
        "#
    )
    .fetch_all(executor)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|row| SettingItem { key: row.key, value: row.value })
            .collect()
    })
}

pub async fn get_optional_string<'e, E>(
    executor: E,
    key: &str,
) -> Result<Option<String>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(executor)
        .await
}

pub async fn get_string<'e, E>(
    executor: E,
    key: &str,
    default: &str,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    Ok(get_optional_string(executor, key)
        .await?
        .unwrap_or_else(|| default.to_string()))
}

pub async fn get_bool<'e, E>(executor: E, key: &str, default: bool) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let default_str = if default { "true" } else { "false" };
    let value = get_string(executor, key, default_str).await?;
    Ok(value == "true")
}

pub async fn upsert<'e, E>(executor: E, key: &str, value: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at)
         VALUES (?, ?, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
    )
    .bind(key)
    .bind(value)
    .execute(executor)
    .await?;
    Ok(())
}
