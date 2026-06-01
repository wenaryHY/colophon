use crate::shared::error::AppResult;

/// 查询所有已软删除的文章
pub async fn list_trashed_posts<'e, E>(executor: E) -> AppResult<Vec<(String, String, Option<String>, String)>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, title, slug, deleted_at FROM posts WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// 查询所有已软删除的分类
pub async fn list_trashed_categories<'e, E>(executor: E) -> AppResult<Vec<(String, String, Option<String>, String)>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, name, slug, deleted_at FROM categories WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// 查询所有已软删除的标签
pub async fn list_trashed_tags<'e, E>(executor: E) -> AppResult<Vec<(String, String, Option<String>, String)>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, name, slug, deleted_at FROM tags WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// 查询所有已软删除的媒体
pub async fn list_trashed_media<'e, E>(executor: E) -> AppResult<Vec<(String, String, Option<String>, String)>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, original_name, mime_type, deleted_at FROM media WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// 查询所有已软删除的媒体分类
pub async fn list_trashed_media_categories<'e, E>(executor: E) -> AppResult<Vec<(String, String, Option<String>, String)>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, name, slug, deleted_at FROM media_categories WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// 查询所有已软删除的评论
pub async fn list_trashed_comments<'e, E>(executor: E) -> AppResult<Vec<(String, String, Option<String>, String)>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, substr(content, 1, 60), NULL, deleted_at FROM comments WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

/// 恢复（清除 deleted_at）
pub async fn restore_item<'e, E>(executor: E, item_type: &str, id: &str) -> AppResult<bool>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let sql = match item_type {
        "post" => "UPDATE posts SET deleted_at = NULL, updated_at = datetime('now') WHERE id = ? AND deleted_at IS NOT NULL",
        "category" => "UPDATE categories SET deleted_at = NULL, updated_at = datetime('now') WHERE id = ? AND deleted_at IS NOT NULL",
        "tag" => "UPDATE tags SET deleted_at = NULL, updated_at = datetime('now') WHERE id = ? AND deleted_at IS NOT NULL",
        "media" => "UPDATE media SET deleted_at = NULL, updated_at = datetime('now') WHERE id = ? AND deleted_at IS NOT NULL",
        "media_category" => "UPDATE media_categories SET deleted_at = NULL, updated_at = datetime('now') WHERE id = ? AND deleted_at IS NOT NULL",
        "comment" => "UPDATE comments SET deleted_at = NULL, updated_at = datetime('now') WHERE id = ? AND deleted_at IS NOT NULL",
        _ => return Ok(false),
    };
    let result = sqlx::query(sql).bind(id).execute(executor).await?;
    Ok(result.rows_affected() > 0)
}

/// 永久删除（物理 DELETE）
pub async fn purge_item<'e, E>(executor: E, item_type: &str, id: &str) -> AppResult<bool>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    let sql = match item_type {
        "post" => {
            // 先删除关联的 post_tags
            sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
                .bind(id)
                .execute(executor)
                .await?;
            "DELETE FROM posts WHERE id = ? AND deleted_at IS NOT NULL"
        }
        "category" => "DELETE FROM categories WHERE id = ? AND deleted_at IS NOT NULL",
        "tag" => {
            sqlx::query("DELETE FROM post_tags WHERE tag_id = ?")
                .bind(id)
                .execute(executor)
                .await?;
            "DELETE FROM tags WHERE id = ? AND deleted_at IS NOT NULL"
        }
        "media" => "DELETE FROM media WHERE id = ? AND deleted_at IS NOT NULL",
        "media_category" => "DELETE FROM media_categories WHERE id = ? AND deleted_at IS NOT NULL",
        "comment" => "DELETE FROM comments WHERE id = ? AND deleted_at IS NOT NULL",
        _ => return Ok(false),
    };
    let result = sqlx::query(sql).bind(id).execute(executor).await?;
    Ok(result.rows_affected() > 0)
}

/// 清理超过指定天数的已删除数据
pub async fn purge_expired<'e, E>(executor: E, retention_days: i64) -> AppResult<i64>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    let threshold = format!("-{} days", retention_days);
    let tables = [
        ("post_tags", "post_id", "posts"),
        ("post_tags", "tag_id", "tags"),
    ];
    // 先清理关联表中引用已过期软删除的记录
    for (junction_table, fk_col, parent_table) in &tables {
        let sql = format!(
            "DELETE FROM {junction} WHERE {fk} IN (SELECT id FROM {parent} WHERE deleted_at IS NOT NULL AND deleted_at < datetime('now', ?))",
            junction = junction_table,
            fk = fk_col,
            parent = parent_table,
        );
        sqlx::query(&sql).bind(&threshold).execute(executor).await?;
    }

    let mut total_purged: i64 = 0;
    let entity_tables = ["posts", "categories", "tags", "media", "media_categories", "comments"];
    for table in &entity_tables {
        let sql = format!(
            "DELETE FROM {} WHERE deleted_at IS NOT NULL AND deleted_at < datetime('now', ?)",
            table
        );
        let result = sqlx::query(&sql).bind(&threshold).execute(executor).await?;
        total_purged += result.rows_affected() as i64;
    }
    Ok(total_purged)
}
