use serde::Serialize;
use uuid::Uuid;

use super::domain::Tag;

/// 标签及其文章数量（用于标签云）
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TagWithCount {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub post_count: i64,
}

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

/// 根据 slug 获取标签
pub async fn get_by_slug<'e, E>(executor: E, slug: &str) -> Result<Option<Tag>, sqlx::Error>
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
        WHERE slug = ? AND deleted_at IS NULL
        LIMIT 1
        "#,
        slug
    )
    .fetch_optional(executor)
    .await
}

/// 获取所有标签及其文章数量（用于标签云）
/// 只返回至少有一篇已发布文章的标签
pub async fn get_all_tags_with_count<'e, E>(executor: E) -> Result<Vec<TagWithCount>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        TagWithCount,
        r#"
        SELECT 
            t.id,
            t.name,
            t.slug,
            COUNT(DISTINCT CASE 
                WHEN p.status = 'published' 
                     AND p.visibility = 'public' 
                     AND p.deleted_at IS NULL 
                THEN pt.post_id 
                ELSE NULL 
            END) as post_count
        FROM tags t
        LEFT JOIN post_tags pt ON t.id = pt.tag_id
        LEFT JOIN posts p ON pt.post_id = p.id
        WHERE t.deleted_at IS NULL
        GROUP BY t.id, t.name, t.slug
        HAVING post_count > 0
        ORDER BY post_count DESC, t.name ASC
        "#
    )
    .fetch_all(executor)
    .await
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
