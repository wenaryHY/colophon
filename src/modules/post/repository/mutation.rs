use uuid::Uuid;

use crate::modules::post::post_types::{NewPostParams, PostStatus, UpdatePostParams};
use crate::modules::post::service::SQLITE_DATETIME_FORMAT;

pub async fn insert_post<'e, E>(
    executor: E,
    params: NewPostParams<'_>,
) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    assert_scheduled_has_scheduled_at(params.status, params.scheduled_at)?;

    let id = Uuid::new_v4().to_string();
    let published_at = if params.status == PostStatus::Published {
        Some(chrono::Utc::now().format(SQLITE_DATETIME_FORMAT).to_string())
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO posts (
            id, author_id, title, slug, excerpt, content_md, content_html, cover_media_id,
            status, visibility, category_id, allow_comment, pinned, content_type,
            custom_html_path, page_render_mode, published_at, scheduled_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(params.author_id)
    .bind(params.title)
    .bind(params.slug)
    .bind(params.excerpt)
    .bind(params.content_md)
    .bind(params.content_html)
    .bind(params.cover_media_id)
    .bind(params.status)
    .bind(params.visibility)
    .bind(params.category_id)
    .bind(params.allow_comment)
    .bind(params.pinned)
    .bind(params.content_type)
    .bind(params.custom_html_path)
    .bind(params.page_render_mode)
    .bind(published_at)
    .bind(params.scheduled_at)
    .execute(executor)
    .await?;

    Ok(id)
}

pub async fn update_post<'e, E>(
    executor: E,
    params: UpdatePostParams<'_>,
    current_published_at: Option<&str>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    assert_scheduled_has_scheduled_at(params.status, params.scheduled_at)?;

    // 只在从非 published 状态首次发布时才设置 published_at；
    // 若已发布则保留原时间；若切换回 draft/trashed 则清空。
    let published_at: Option<String> = if params.status == PostStatus::Published {
        Some(
            current_published_at
                .map(|t| t.to_string())
                .unwrap_or_else(|| chrono::Utc::now().format(SQLITE_DATETIME_FORMAT).to_string()),
        )
    } else {
        None
    };

    // scheduled_at 逻辑：
    // - 如果状态是 Scheduled，保留传入的 scheduled_at
    // - 如果状态不是 Scheduled（切回 Draft/Trashed），清空 scheduled_at
    let scheduled_at: Option<&str> = if params.status == PostStatus::Scheduled {
        params.scheduled_at
    } else {
        None
    };

    sqlx::query(
        "UPDATE posts
         SET title = ?, slug = ?, excerpt = ?, content_md = ?, content_html = ?, cover_media_id = ?,
             status = ?, visibility = ?, category_id = ?, allow_comment = ?, pinned = ?,
             content_type = ?, custom_html_path = ?, page_render_mode = ?, published_at = ?,
             scheduled_at = ?, updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(params.title)
    .bind(params.slug)
    .bind(params.excerpt)
    .bind(params.content_md)
    .bind(params.content_html)
    .bind(params.cover_media_id)
    .bind(params.status)
    .bind(params.visibility)
    .bind(params.category_id)
    .bind(params.allow_comment)
    .bind(params.pinned)
    .bind(params.content_type)
    .bind(params.custom_html_path)
    .bind(params.page_render_mode)
    .bind(published_at)
    .bind(scheduled_at)
    .bind(params.post_id)
    .execute(executor)
    .await?;

    Ok(())
}

pub async fn replace_tags<'e, E>(
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

pub async fn delete_post<'e, E>(executor: E, id: &str) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "UPDATE posts SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(executor)
    .await?;
    Ok(())
}

/// 验证 Scheduled 状态必须有 scheduled_at。
/// 在 insert_post 和 update_post 入口处调用，防止数据库约束泄漏到业务层。
fn assert_scheduled_has_scheduled_at(
    status: PostStatus,
    scheduled_at: Option<&str>,
) -> Result<(), sqlx::Error> {
    if status == PostStatus::Scheduled && scheduled_at.is_none() {
        return Err(sqlx::Error::Protocol(
            "Scheduled status requires scheduled_at".into(),
        ));
    }
    Ok(())
}

/// 原子发布所有到期的定时文章。
/// 返回被发布的文章 ID 列表。
pub async fn publish_scheduled_posts<'e, E>(
    executor: E,
) -> Result<Vec<String>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let ids: Vec<String> = sqlx::query_scalar(
        "UPDATE posts 
         SET status = 'published', published_at = scheduled_at, updated_at = datetime('now')
         WHERE status = 'scheduled' AND scheduled_at <= datetime('now') AND deleted_at IS NULL
         RETURNING id",
    )
    .fetch_all(executor)
    .await?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_scheduled_has_scheduled_at_ok_when_present() {
        let result = assert_scheduled_has_scheduled_at(
            PostStatus::Scheduled,
            Some("2026-12-01 10:00:00"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn assert_scheduled_has_scheduled_at_err_when_none() {
        let result = assert_scheduled_has_scheduled_at(PostStatus::Scheduled, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("scheduled_at"));
    }

    #[test]
    fn assert_scheduled_has_scheduled_at_ok_for_non_scheduled_status() {
        // Draft 不需要 scheduled_at
        let result = assert_scheduled_has_scheduled_at(PostStatus::Draft, None);
        assert!(result.is_ok());

        // Published 不需要 scheduled_at
        let result = assert_scheduled_has_scheduled_at(PostStatus::Published, None);
        assert!(result.is_ok());

        // Trashed 不需要 scheduled_at
        let result = assert_scheduled_has_scheduled_at(PostStatus::Trashed, None);
        assert!(result.is_ok());
    }
}
