use sqlx::SqlitePool;
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
            conversion_status,
            conversion_retries,
            conversion_error,
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

/// 将媒体记录标记为 pending 等待 WebP 转换
pub async fn mark_media_pending(pool: &SqlitePool, media_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE media SET conversion_status = 'pending' WHERE id = ?")
        .bind(media_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 查询所有 pending 状态的媒体记录（用于服务启动时重新入队）
pub async fn list_pending_conversions(pool: &SqlitePool) -> Result<Vec<MediaItem>, sqlx::Error> {
    sqlx::query_as::<_, MediaItem>(
        "SELECT * FROM media WHERE conversion_status = 'pending' AND conversion_retries < 3 AND deleted_at IS NULL",
    )
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("failed to create in-memory database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("failed to run migrations");
        pool
    }

    /// 插入一个测试用户，满足 media 表的 uploader_id 外键约束
    async fn insert_test_user(pool: &SqlitePool, user_id: &str) {
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, role, status, display_name) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(format!("user_{}", user_id))
        .bind(format!("{}@test.com", user_id))
        .bind("hash")
        .bind("admin")
        .bind("active")
        .bind("Test User")
        .execute(pool)
        .await
        .expect("failed to insert test user");
    }

    /// 插入一条测试媒体记录并返回其 id
    async fn insert_test_media(pool: &SqlitePool, uploader_id: &str) -> String {
        insert_media(
            pool,
            uploader_id,
            "image",
            "image/jpeg",
            "test.jpg",
            "test_stored.jpg",
            "media/test_stored.jpg",
            "/uploads/test_stored.jpg",
            1024,
            "general",
        )
        .await
        .expect("failed to insert test media")
    }

    #[tokio::test]
    async fn test_mark_media_pending_updates_status() {
        let pool = setup_test_db().await;
        let user_id = "test-user-1";
        insert_test_user(&pool, user_id).await;

        let media_id = insert_test_media(&pool, user_id).await;

        // 确认初始状态为 ''（默认值）
        let before = get_media(&pool, &media_id).await.unwrap().unwrap();
        assert_eq!(before.conversion_status, "");

        // 标记为 pending
        mark_media_pending(&pool, &media_id).await.unwrap();

        // 验证状态已更新
        let after = get_media(&pool, &media_id).await.unwrap().unwrap();
        assert_eq!(after.conversion_status, "pending");
    }

    #[tokio::test]
    async fn test_list_pending_conversions_only_returns_pending() {
        let pool = setup_test_db().await;
        let user_id = "test-user-2";
        insert_test_user(&pool, user_id).await;

        // 插入 4 条媒体记录
        let id_pending = insert_test_media(&pool, user_id).await;
        let id_converted = insert_test_media(&pool, user_id).await;
        let id_retries_exceeded = insert_test_media(&pool, user_id).await;
        let id_soft_deleted = insert_test_media(&pool, user_id).await;

        // 手动设置转换状态
        sqlx::query("UPDATE media SET conversion_status = 'pending' WHERE id = ?")
            .bind(&id_pending)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE media SET conversion_status = 'converted' WHERE id = ?")
            .bind(&id_converted)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE media SET conversion_status = 'failed', conversion_retries = 3 WHERE id = ?",
        )
        .bind(&id_retries_exceeded)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE media SET conversion_status = 'pending', deleted_at = datetime('now') WHERE id = ?",
        )
        .bind(&id_soft_deleted)
        .execute(&pool)
        .await
        .unwrap();

        // 验证 list_pending_conversions 只返回符合条件的一条记录
        let pending = list_pending_conversions(&pool).await.unwrap();
        assert_eq!(
            pending.len(),
            1,
            "only one pending record should be returned"
        );
        assert_eq!(pending[0].id, id_pending);
        assert_eq!(pending[0].conversion_status, "pending");
    }
}
