use uuid::Uuid;

use super::domain::{MediaItem, MediaThumbnail, ThumbnailTask};

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
    sqlx::query_as::<_, MediaItem>("SELECT * FROM media WHERE id = ? AND deleted_at IS NULL LIMIT 1")
        .bind(id)
        .fetch_optional(executor)
        .await
}

pub async fn delete_media<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE media SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?")
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

/// 批量插入缩略图记录
pub async fn insert_media_thumbnails<'e, E>(
    executor: E,
    thumbnails: &[MediaThumbnail],
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    for t in thumbnails {
        sqlx::query(
            "INSERT INTO media_thumbnails (id, media_id, size_label, width, height, storage_path, public_url, size_bytes) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&t.id)
        .bind(&t.media_id)
        .bind(&t.size_label)
        .bind(t.width)
        .bind(t.height)
        .bind(&t.storage_path)
        .bind(&t.public_url)
        .bind(t.size_bytes)
        .execute(executor)
        .await?;
    }
    Ok(())
}

/// 查询单个媒体的所有缩略图（按宽度升序）
pub async fn get_thumbnails_by_media_id<'e, E>(
    executor: E,
    media_id: &str,
) -> Result<Vec<MediaThumbnail>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    sqlx::query_as::<_, MediaThumbnail>(
        "SELECT * FROM media_thumbnails WHERE media_id = ? ORDER BY width ASC"
    )
    .bind(media_id)
    .fetch_all(executor)
    .await
}

/// 批量查询多个媒体的缩略图（按 media_id 分组，每组的缩略图按宽度升序）
pub async fn get_thumbnails_by_media_ids<'e, E>(
    executor: E,
    media_ids: &[String],
) -> Result<Vec<MediaThumbnail>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    if media_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; media_ids.len()].join(",");
    let sql = format!(
        "SELECT * FROM media_thumbnails WHERE media_id IN ({}) ORDER BY media_id, width ASC",
        placeholders
    );
    let mut query = sqlx::query_as::<_, MediaThumbnail>(&sql);
    for id in media_ids {
        query = query.bind(id);
    }
    query.fetch_all(executor).await
}

/// 删除媒体的所有缩略图记录
pub async fn delete_thumbnails_by_media_id<'e, E>(
    executor: E,
    media_id: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    sqlx::query("DELETE FROM media_thumbnails WHERE media_id = ?")
        .bind(media_id)
        .execute(executor)
        .await?;
    Ok(())
}

// ── 异步缩略图任务 ──

/// 插入一个待处理的缩略图任务
pub async fn insert_thumbnail_task<'e, E>(
    executor: E,
    task: &ThumbnailTask,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    tracing::debug!(
        module = "media",
        event = "thumbnail_task_inserting",
        task_id = %task.id,
        media_id = %task.media_id,
        "executing INSERT INTO thumbnail_tasks"
    );

    let result = sqlx::query(
        "INSERT INTO thumbnail_tasks (id, media_id, status, retry_count, max_retries, last_error) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&task.id)
    .bind(&task.media_id)
    .bind(&task.status)
    .bind(task.retry_count)
    .bind(task.max_retries)
    .bind(&task.last_error)
    .execute(executor)
    .await;

    match &result {
        Ok(_) => tracing::debug!("thumbnail_task_inserted: {}", task.id),
        Err(e) => tracing::error!(error = %e, "thumbnail_task_insert_failed: {}", task.id),
    }

    result.map(|_| ())
}

/// 取出一个 pending 任务并原子性地标记为 processing
/// SQLite 是单写者，先用子查询取 id，再 UPDATE 标记，最后 SELECT 返回
pub async fn take_one_pending_thumbnail_task<'e, E>(
    executor: E,
) -> Result<Option<ThumbnailTask>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    // 两步法（兼容不支持 RETURNING 的 SQLite 版本）：
    // 1. 用子查询 UPDATE 标记一个 pending 任务为 processing
    // 2. SELECT 该任务返回给调用者
    // SQLite 单写者模式下，UPDATE 天然原子，不存在竞态

    let rows = sqlx::query(
        "UPDATE thumbnail_tasks SET status = 'processing', updated_at = datetime('now')
         WHERE id IN (
             SELECT id FROM thumbnail_tasks WHERE status = 'pending'
             ORDER BY created_at ASC LIMIT 1
         )"
    )
    .execute(executor)
    .await?;

    if rows.rows_affected() == 0 {
        return Ok(None);
    }

    let result = sqlx::query_as::<_, ThumbnailTask>(
        "SELECT * FROM thumbnail_tasks WHERE status = 'processing'
         ORDER BY created_at ASC LIMIT 1"
    )
    .fetch_optional(executor)
    .await;

    match &result {
        Ok(Some(t)) => tracing::info!(
            module = "media",
            event = "thumbnail_task_taken",
            task_id = %t.id,
            media_id = %t.media_id,
            "took pending thumbnail task"
        ),
        Ok(None) => {
            // 不做记录——每分钟会有高频空轮询，避免日志洪水
        }
        Err(e) => tracing::error!(
            module = "media",
            event = "thumbnail_task_take_failed",
            error = %e,
            "failed to take pending thumbnail task"
        ),
    }

    result
}

/// 标记任务为完成，记录原图尺寸
pub async fn mark_thumbnail_task_done<'e, E>(
    executor: E,
    task_id: &str,
    width: u32,
    height: u32,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    sqlx::query(
        "UPDATE thumbnail_tasks SET status = 'done', width = ?, height = ?, updated_at = datetime('now') WHERE id = ?"
    )
    .bind(width as i64)
    .bind(height as i64)
    .bind(task_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// 标记任务为失败
/// retry=true 时重置为 pending（递增 retry_count）；retry=false 时标记为 failed
pub async fn mark_thumbnail_task_failed<'e, E>(
    executor: E,
    task_id: &str,
    error_message: &str,
    retry: bool,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    if retry {
        sqlx::query(
            "UPDATE thumbnail_tasks SET status = 'pending', retry_count = retry_count + 1, last_error = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(error_message)
        .bind(task_id)
        .execute(executor)
        .await?;
    } else {
        sqlx::query(
            "UPDATE thumbnail_tasks SET status = 'failed', last_error = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(error_message)
        .bind(task_id)
        .execute(executor)
        .await?;
    }
    Ok(())
}

/// 统计 pending 状态的任务数（用于攻击防御）
pub async fn count_pending_thumbnail_tasks<'e, E>(
    executor: E,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM thumbnail_tasks WHERE status = 'pending'"
    )
    .fetch_one(executor)
    .await
}
