use uuid::Uuid;

use super::domain::Label;

pub async fn list_labels<'e, E>(executor: E) -> Result<Vec<Label>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, Label>(
        r#"
        SELECT
            id,
            name,
            color,
            created_at,
            updated_at,
            deleted_at
        FROM labels
        WHERE deleted_at IS NULL
        ORDER BY name ASC
        "#,
    )
    .fetch_all(executor)
    .await
}

pub async fn get_label<'e, E>(executor: E, id: &str) -> Result<Option<Label>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, Label>(
        r#"
        SELECT
            id,
            name,
            color,
            created_at,
            updated_at,
            deleted_at
        FROM labels
        WHERE id = ? AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn insert_label<'e, E>(
    executor: E,
    name: &str,
    color: &str,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO labels (id, name, color) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(color)
        .execute(executor)
        .await?;
    Ok(id)
}

pub async fn update_label<'e, E>(
    executor: E,
    id: &str,
    name: Option<&str>,
    color: Option<&str>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(name) = name {
        if let Some(color) = color {
            sqlx::query(
                "UPDATE labels SET name = ?, color = ?, updated_at = datetime('now') WHERE id = ?",
            )
            .bind(name)
            .bind(color)
            .bind(id)
            .execute(executor)
            .await?;
        } else {
            sqlx::query("UPDATE labels SET name = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(name)
                .bind(id)
                .execute(executor)
                .await?;
        }
    } else if let Some(color) = color {
        sqlx::query("UPDATE labels SET color = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(color)
            .bind(id)
            .execute(executor)
            .await?;
    }
    Ok(())
}

pub async fn delete_label<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE labels SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}
