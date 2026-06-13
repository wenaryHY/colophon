use uuid::Uuid;

use super::domain::MediaItem;

pub async fn list_media<'e, E>(
    executor: E,
    kind: Option<&str>,
    keyword: Option<&str>,
    category: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<MediaItem>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let like = keyword.map(|k| format!("%{}%", k));

    let mut sql = String::from("SELECT * FROM media WHERE deleted_at IS NULL");
    if kind.is_some() {
        sql.push_str(" AND kind = ?");
    }
    if like.is_some() {
        sql.push_str(" AND original_name LIKE ?");
    }
    if category.is_some() {
        sql.push_str(" AND category = ?");
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

    let mut q = sqlx::query_as::<_, MediaItem>(&sql);
    if let Some(k) = kind {
        q = q.bind(k);
    }
    if let Some(l) = like {
        q = q.bind(l);
    }
    if let Some(c) = category {
        q = q.bind(c);
    }
    q = q.bind(limit).bind(offset);
    q.fetch_all(executor).await
}

pub async fn count_media<'e, E>(
    executor: E,
    kind: Option<&str>,
    keyword: Option<&str>,
    category: Option<&str>,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let like = keyword.map(|k| format!("%{}%", k));

    let mut sql = String::from("SELECT COUNT(*) FROM media WHERE deleted_at IS NULL");
    if kind.is_some() {
        sql.push_str(" AND kind = ?");
    }
    if like.is_some() {
        sql.push_str(" AND original_name LIKE ?");
    }
    if category.is_some() {
        sql.push_str(" AND category = ?");
    }

    let mut q = sqlx::query_scalar::<_, i64>(&sql);
    if let Some(k) = kind {
        q = q.bind(k);
    }
    if let Some(l) = like {
        q = q.bind(l);
    }
    if let Some(c) = category {
        q = q.bind(c);
    }
    q.fetch_one(executor).await
}

pub async fn insert_media<'e, E>(
    executor: E,
    uploader_id: &str,
    kind: &str,
    mime_type: &str,
    original_name: &str,
    stored_name: &str,
    storage_path: &str,
    public_url: &str,
    size_bytes: i64,
    category: &str,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO media (
            id, uploader_id, kind, mime_type, original_name, stored_name, storage_path, public_url, size_bytes, category
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(uploader_id)
    .bind(kind)
    .bind(mime_type)
    .bind(original_name)
    .bind(stored_name)
    .bind(storage_path)
    .bind(public_url)
    .bind(size_bytes)
    .bind(category)
    .execute(executor)
    .await?;
    Ok(id)
}

pub async fn get_media<'e, E>(executor: E, id: &str) -> Result<Option<MediaItem>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        MediaItem,
        r#"
        SELECT
            id,
            uploader_id,
            kind,
            mime_type,
            original_name,
            stored_name,
            storage_path,
            public_url,
            size_bytes,
            width,
            height,
            duration_seconds,
            alt_text,
            category,
            created_at,
            updated_at,
            deleted_at
        FROM media
        WHERE id = ? AND deleted_at IS NULL
        LIMIT 1
        "#,
        id
    )
    .fetch_optional(executor)
    .await
}

pub async fn delete_media<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE media SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn rename_media<'e, E>(executor: E, id: &str, new_name: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE media SET original_name = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(new_name)
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn update_media_category<'e, E>(
    executor: E,
    id: &str,
    category: Option<&str>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE media SET category = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(category)
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}
