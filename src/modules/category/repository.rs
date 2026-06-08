use uuid::Uuid;

use super::domain::Category;

pub async fn list_categories<'e, E>(executor: E) -> Result<Vec<Category>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        Category,
        r#"
        SELECT
            id,
            name,
            slug,
            description,
            parent_id,
            sort_order,
            created_at,
            updated_at,
            deleted_at
        FROM categories
        WHERE deleted_at IS NULL
        ORDER BY sort_order ASC, name ASC
        "#
    )
    .fetch_all(executor)
    .await
}

pub async fn get_category<'e, E>(executor: E, id: &str) -> Result<Option<Category>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        Category,
        r#"
        SELECT
            id,
            name,
            slug,
            description,
            parent_id,
            sort_order,
            created_at,
            updated_at,
            deleted_at
        FROM categories
        WHERE id = ? AND deleted_at IS NULL
        LIMIT 1
        "#,
        id
    )
    .fetch_optional(executor)
    .await
}

pub async fn category_slug_or_name_exists<'e, E>(
    executor: E,
    slug: &str,
    name: &str,
    exclude_id: Option<&str>,
) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(exclude_id) = exclude_id {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE (slug = ? OR name = ?) AND id != ? AND deleted_at IS NULL)",
        )
        .bind(slug)
        .bind(name)
        .bind(exclude_id)
        .fetch_one(executor)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE (slug = ? OR name = ?) AND deleted_at IS NULL)",
        )
            .bind(slug)
            .bind(name)
            .fetch_one(executor)
            .await
    }
}

pub async fn insert_category<'e, E>(
    executor: E,
    name: &str,
    slug: &str,
    description: Option<&str>,
    parent_id: Option<&str>,
    sort_order: i64,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO categories (id, name, slug, description, parent_id, sort_order)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(parent_id)
    .bind(sort_order)
    .execute(executor)
    .await?;
    Ok(id)
}

pub async fn update_category<'e, E>(
    executor: E,
    id: &str,
    name: &str,
    slug: &str,
    description: Option<&str>,
    parent_id: Option<&str>,
    sort_order: i64,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE categories
         SET name = ?, slug = ?, description = ?, parent_id = ?, sort_order = ?, updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(parent_id)
    .bind(sort_order)
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn delete_category<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    sqlx::query("UPDATE posts SET category_id = NULL WHERE category_id = ?")
        .bind(id)
        .execute(executor)
        .await?;
    sqlx::query(
        "UPDATE categories
         SET deleted_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ?",
    )
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}
