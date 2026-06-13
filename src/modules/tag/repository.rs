use uuid::Uuid;

use super::domain::Tag;

pub async fn list_tags<'e, E>(executor: E) -> Result<Vec<Tag>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        Tag,
        r#"
        SELECT
            id,
            name,
            slug,
            created_at,
            updated_at,
            deleted_at
        FROM tags
        WHERE deleted_at IS NULL
        ORDER BY name ASC
        "#
    )
    .fetch_all(executor)
    .await
}

pub async fn get_tag<'e, E>(executor: E, id: &str) -> Result<Option<Tag>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        Tag,
        r#"
        SELECT
            id,
            name,
            slug,
            created_at,
            updated_at,
            deleted_at
        FROM tags
        WHERE id = ? AND deleted_at IS NULL
        LIMIT 1
        "#,
        id
    )
    .fetch_optional(executor)
    .await
}

pub async fn tag_slug_or_name_exists<'e, E>(
    executor: E,
    slug: &str,
    name: &str,
    exclude_id: Option<&str>,
) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(id) = exclude_id {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tags WHERE (slug = ? OR name = ?) AND id != ? AND deleted_at IS NULL)",
        )
        .bind(slug)
        .bind(name)
        .bind(id)
        .fetch_one(executor)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tags WHERE (slug = ? OR name = ?) AND deleted_at IS NULL)",
        )
        .bind(slug)
        .bind(name)
        .fetch_one(executor)
        .await
    }
}

pub async fn insert_tag<'e, E>(executor: E, name: &str, slug: &str) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO tags (id, name, slug) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(slug)
        .execute(executor)
        .await?;
    Ok(id)
}

pub async fn update_tag<'e, E>(
    executor: E,
    id: &str,
    name: Option<&str>,
    slug: Option<&str>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(name) = name {
        if let Some(slug) = slug {
            sqlx::query(
                "UPDATE tags SET name = ?, slug = ?, updated_at = datetime('now') WHERE id = ?",
            )
            .bind(name)
            .bind(slug)
            .bind(id)
            .execute(executor)
            .await?;
        } else {
            sqlx::query("UPDATE tags SET name = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(name)
                .bind(id)
                .execute(executor)
                .await?;
        }
    } else if let Some(slug) = slug {
        sqlx::query("UPDATE tags SET slug = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(slug)
            .bind(id)
            .execute(executor)
            .await?;
    }
    Ok(())
}

pub async fn delete_tag<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE tags SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn list_post_tags<'e, E>(executor: E, post_id: &str) -> Result<Vec<Tag>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        Tag,
        r#"
        SELECT
            t.id,
            t.name,
            t.slug,
            t.created_at,
            t.updated_at,
            t.deleted_at
        FROM tags t
        JOIN post_tags pt ON pt.tag_id = t.id
        WHERE pt.post_id = ? AND t.deleted_at IS NULL
        ORDER BY t.name ASC
        "#,
        post_id
    )
    .fetch_all(executor)
    .await
}

#[allow(dead_code)]
pub async fn replace_post_tags<'e, E>(
    executor: E,
    post_id: &str,
    tag_ids: &[String],
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(executor)
        .await?;

    for tag_id in tag_ids {
        sqlx::query("INSERT INTO post_tags (post_id, tag_id) VALUES (?, ?)")
            .bind(post_id)
            .bind(tag_id)
            .execute(executor)
            .await?;
    }

    Ok(())
}
