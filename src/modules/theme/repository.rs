use crate::modules::theme::ThemeConfig;
use crate::shared::error::AppResult;
use uuid::Uuid;

pub async fn save_config<'e, E>(
    executor: E,
    theme_slug: &str,
    config: &ThemeConfig,
) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    let config_json = serde_json::to_string(config)?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT OR REPLACE INTO theme_configs (id, theme_slug, config_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(theme_slug)
    .bind(&config_json)
    .bind(&now)
    .bind(&now)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn get_config<'e, E>(executor: E, theme_slug: &str) -> AppResult<Option<ThemeConfig>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let record = sqlx::query_as::<_, (String,)>(
        "SELECT config_json FROM theme_configs WHERE theme_slug = ? LIMIT 1",
    )
    .bind(theme_slug)
    .fetch_optional(executor)
    .await?;

    match record {
        Some((json_str,)) => match serde_json::from_str(&json_str) {
            Ok(config) => Ok(Some(config)),
            Err(e) => {
                tracing::warn!(
                    module = "theme",
                    theme_slug = %theme_slug,
                    error = %e,
                    "failed to deserialize theme config JSON, returning None"
                );
                Ok(None)
            }
        },
        None => Ok(None),
    }
}

pub async fn set_active_theme<'e, E>(executor: E, theme_slug: &str) -> AppResult<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, ?)")
        .bind("active_theme")
        .bind(theme_slug)
        .bind(&now)
        .execute(executor)
        .await?;

    Ok(())
}

pub async fn get_active_theme<'e, E>(executor: E) -> AppResult<String>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let result = sqlx::query_as::<_, (String,)>(
        "SELECT value FROM settings WHERE key = 'active_theme' LIMIT 1",
    )
    .fetch_optional(executor)
    .await?;

    Ok(result
        .map(|(v,)| v)
        .unwrap_or_else(|| "default".to_string()))
}

#[allow(dead_code)]
pub async fn config_exists<'e, E>(executor: E, theme_slug: &str) -> AppResult<bool>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM theme_configs WHERE theme_slug = ? LIMIT 1",
    )
    .bind(theme_slug)
    .fetch_one(executor)
    .await?;

    Ok(result > 0)
}
