use serde::Serialize;
use uuid::Uuid;

use super::domain::{AdminCommentItem, CommentItem};
use crate::modules::post::post_types::ContentType;

/// 用于 WebSocket 事件广播的最小评论数据
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CommentEventData {
    pub id: String,
    pub post_id: String,
    pub author_name: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

pub async fn list_approved_for_post<'e, E>(
    executor: E,
    post_id: &str,
) -> Result<Vec<CommentItem>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        CommentItem,
        r#"
        SELECT
            c.id,
            c.post_id,
            c.user_id,
            u.username,
            u.display_name,
            c.content,
            c.status,
            c.parent_id,
            c.created_at,
            c.updated_at
        FROM comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.post_id = ? AND c.status = 'approved'
        ORDER BY c.created_at ASC
        "#,
        post_id
    )
    .fetch_all(executor)
    .await
}

pub async fn list_by_user<'e, E>(
    executor: E,
    user_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<AdminCommentItem>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        AdminCommentItem,
        r#"
        SELECT
            c.id,
            c.post_id,
            c.user_id,
            u.username,
            u.display_name,
            c.content,
            c.status,
            c.parent_id,
            c.created_at,
            c.updated_at,
            p.title AS post_title,
            p.slug AS post_slug,
            p.content_type AS "post_content_type: ContentType"
        FROM comments c
        JOIN users u ON u.id = c.user_id
        JOIN posts p ON p.id = c.post_id
        WHERE c.user_id = ?
        ORDER BY c.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        user_id,
        limit,
        offset
    )
    .fetch_all(executor)
    .await
}

pub async fn count_by_user<'e, E>(executor: E, user_id: &str) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(executor)
        .await
}

pub async fn count_approved_by_user<'e, E>(executor: E, user_id: &str) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE user_id = ? AND status = 'approved'")
        .bind(user_id)
        .fetch_one(executor)
        .await
}

pub async fn insert_comment<'e, E>(
    executor: E,
    post_id: &str,
    user_id: &str,
    content: &str,
    parent_id: Option<&str>,
    status: &str,
) -> Result<(String, String), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO comments (id, post_id, user_id, content, status, parent_id)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(post_id)
    .bind(user_id)
    .bind(content)
    .bind(status)
    .bind(parent_id)
    .execute(executor)
    .await?;

    // 取回实际插入的 created_at（SQLite datetime('now') 格式）
    let created_at: String = sqlx::query_scalar("SELECT created_at FROM comments WHERE id = ?")
        .bind(&id)
        .fetch_one(executor)
        .await?;

    Ok((id, created_at))
}

pub async fn list_admin<'e, E>(
    executor: E,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AdminCommentItem>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(status) = status {
        sqlx::query_as!(
            AdminCommentItem,
            r#"
            SELECT
                c.id,
                c.post_id,
                c.user_id,
                u.username,
                u.display_name,
                c.content,
                c.status,
                c.parent_id,
                c.created_at,
                c.updated_at,
                p.title AS post_title,
                p.slug AS post_slug,
                p.content_type AS "post_content_type: ContentType"
            FROM comments c
            JOIN users u ON u.id = c.user_id
            JOIN posts p ON p.id = c.post_id
            WHERE c.status = ?
            ORDER BY c.created_at DESC
            LIMIT ? OFFSET ?
            "#,
            status,
            limit,
            offset
        )
        .fetch_all(executor)
        .await
    } else {
        sqlx::query_as!(
            AdminCommentItem,
            r#"
            SELECT
                c.id,
                c.post_id,
                c.user_id,
                u.username,
                u.display_name,
                c.content,
                c.status,
                c.parent_id,
                c.created_at,
                c.updated_at,
                p.title AS post_title,
                p.slug AS post_slug,
                p.content_type AS "post_content_type: ContentType"
            FROM comments c
            JOIN users u ON u.id = c.user_id
            JOIN posts p ON p.id = c.post_id
            WHERE c.status != 'deleted'
            ORDER BY c.created_at DESC
            LIMIT ? OFFSET ?
            "#,
            limit,
            offset
        )
        .fetch_all(executor)
        .await
    }
}

pub async fn count_admin<'e, E>(executor: E, status: Option<&str>) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(status) = status {
        sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE status = ?")
            .bind(status)
            .fetch_one(executor)
            .await
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM comments WHERE status != 'deleted'")
            .fetch_one(executor)
            .await
    }
}

pub async fn update_status<'e, E>(executor: E, id: &str, status: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE comments SET status = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn soft_delete_owned<'e, E>(
    executor: E,
    id: &str,
    user_id: &str,
) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let result = sqlx::query(
        "UPDATE comments
         SET status = 'deleted', deleted_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn soft_delete_admin<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE comments
         SET status = 'deleted', deleted_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn restore_deleted_admin<'e, E>(executor: E, id: &str) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let result = sqlx::query(
        "UPDATE comments
         SET status = 'pending', deleted_at = NULL, updated_at = datetime('now')
         WHERE id = ? AND status = 'deleted'",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn purge_admin<'e, E>(executor: E, id: &str) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let result = sqlx::query("DELETE FROM comments WHERE id = ?")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// 查询单条评论用于 WS 事件广播（包含作者显示名）
pub async fn find_by_id_for_event<'e, E>(
    executor: E,
    id: &str,
) -> Result<Option<CommentEventData>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        CommentEventData,
        r#"
        SELECT
            c.id,
            c.post_id,
            COALESCE(u.display_name, u.username) AS author_name,
            c.content,
            c.status,
            c.created_at
        FROM comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.id = ?
        "#,
        id
    )
    .fetch_optional(executor)
    .await
}
