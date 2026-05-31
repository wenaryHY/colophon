use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::domain::{
    AdminPost, CommentTargetPost, PublicPostDetail, PublicPostSummary, SitemapItem,
};
use crate::modules::tag::domain::Tag;

#[allow(dead_code)]
pub async fn list_recent_public_posts(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<PublicPostSummary>, sqlx::Error> {
    sqlx::query_as::<_, PublicPostSummary>(
        "SELECT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_type,
            p.published_at,
            p.created_at,
            p.updated_at,
            u.display_name AS author_display_name,
            c.name AS category_name,
            c.id AS category_id
         FROM posts p
         JOIN users u ON u.id = p.author_id
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE p.status = 'published' AND p.visibility = 'public' AND p.deleted_at IS NULL
         ORDER BY p.pinned DESC, p.published_at DESC, p.created_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn list_for_sitemap(pool: &SqlitePool) -> Result<Vec<SitemapItem>, sqlx::Error> {
    sqlx::query_as::<_, SitemapItem>(
        "SELECT slug, content_type, published_at, updated_at
         FROM posts
         WHERE status = 'published' AND visibility = 'public' AND deleted_at IS NULL
         ORDER BY published_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// FTS5 trigram search with LIKE fallback (trigram tokenizer handles CJK better than unicode61).
/// Returns PublicPostSummary items matching the keyword, optionally filtered by category/tag.
pub async fn search_posts(
    pool: &SqlitePool,
    keyword: &str,
    category_id: Option<&str>,
    tag_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<PublicPostSummary>, sqlx::Error> {
    // First: try FTS5 trigram
    let fts_keyword = keyword.to_string();
    let fts_results = sqlx::query_as::<_, PublicPostSummary>(
        "SELECT p.id, p.title, p.slug, p.excerpt, p.content_type, p.published_at, p.created_at, p.updated_at,
                u.display_name AS author_display_name, c.name AS category_name, c.id AS category_id
         FROM posts p
         JOIN users u ON u.id = p.author_id
         LEFT JOIN categories c ON c.id = p.category_id
         JOIN posts_fts fts ON fts.rowid = p.rowid
         WHERE p.status = 'published' AND p.visibility = 'public' AND p.deleted_at IS NULL
           AND fts.posts_fts MATCH ?
           AND (? IS NULL OR p.category_id = ?)
           AND (? IS NULL OR EXISTS (
                    SELECT 1 FROM post_tags pt WHERE pt.post_id = p.id AND pt.tag_id = ?
               ))
         ORDER BY bm25(posts_fts), p.pinned DESC, p.published_at DESC, p.created_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(&fts_keyword)
    .bind(category_id)
    .bind(category_id)
    .bind(tag_id)
    .bind(tag_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    if !fts_results.is_empty() {
        return Ok(fts_results);
    }

    // FTS5 no results → fallback to LIKE
    let like_keyword = format!("%{}%", keyword);
    sqlx::query_as::<_, PublicPostSummary>(
        "SELECT p.id, p.title, p.slug, p.excerpt, p.content_type, p.published_at, p.created_at, p.updated_at,
                u.display_name AS author_display_name, c.name AS category_name, c.id AS category_id
         FROM posts p
         JOIN users u ON u.id = p.author_id
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE p.status = 'published' AND p.visibility = 'public' AND p.deleted_at IS NULL
           AND (p.title LIKE ? OR p.content_md LIKE ?)
           AND (? IS NULL OR p.category_id = ?)
           AND (? IS NULL OR EXISTS (
                    SELECT 1 FROM post_tags pt WHERE pt.post_id = p.id AND pt.tag_id = ?
               ))
         ORDER BY p.pinned DESC, p.published_at DESC, p.created_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(&like_keyword)
    .bind(&like_keyword)
    .bind(category_id)
    .bind(category_id)
    .bind(tag_id)
    .bind(tag_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_search_posts(
    pool: &SqlitePool,
    keyword: &str,
    category_id: Option<&str>,
    tag_id: Option<&str>,
) -> Result<i64, sqlx::Error> {
    // First: try FTS5 trigram count
    let fts_keyword = keyword.to_string();
    let fts_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM posts p
         JOIN posts_fts fts ON fts.rowid = p.rowid
         WHERE p.status = 'published' AND p.visibility = 'public' AND p.deleted_at IS NULL
           AND fts.posts_fts MATCH ?
           AND (? IS NULL OR p.category_id = ?)
           AND (? IS NULL OR EXISTS (
                    SELECT 1 FROM post_tags pt WHERE pt.post_id = p.id AND pt.tag_id = ?
               ))",
    )
    .bind(&fts_keyword)
    .bind(category_id)
    .bind(category_id)
    .bind(tag_id)
    .bind(tag_id)
    .fetch_one(pool)
    .await?;

    if fts_count > 0 {
        return Ok(fts_count);
    }

    // FTS5 no results → fallback to LIKE
    let like_keyword = format!("%{}%", keyword);
    sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM posts p
         WHERE p.status = 'published' AND p.visibility = 'public' AND p.deleted_at IS NULL
           AND (p.title LIKE ? OR p.content_md LIKE ?)
           AND (? IS NULL OR p.category_id = ?)
           AND (? IS NULL OR EXISTS (
                    SELECT 1 FROM post_tags pt WHERE pt.post_id = p.id AND pt.tag_id = ?
               ))",
    )
    .bind(&like_keyword)
    .bind(&like_keyword)
    .bind(category_id)
    .bind(category_id)
    .bind(tag_id)
    .bind(tag_id)
    .fetch_one(pool)
    .await
}

pub async fn list_public_posts(
    pool: &SqlitePool,
    keyword: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<PublicPostSummary>, sqlx::Error> {
    if let Some(keyword) = keyword {
        let like = format!("%{}%", keyword);
        sqlx::query_as::<_, PublicPostSummary>(
            "SELECT
                p.id,
                p.title,
                p.slug,
                p.excerpt,
                p.content_type,
                p.published_at,
                p.created_at,
                p.updated_at,
                u.display_name AS author_display_name,
                c.name AS category_name,
                c.id AS category_id
             FROM posts p
             JOIN users u ON u.id = p.author_id
             LEFT JOIN categories c ON c.id = p.category_id
             WHERE p.status = 'published' AND p.visibility = 'public'
               AND p.deleted_at IS NULL
               AND (p.title LIKE ? OR p.excerpt LIKE ? OR p.content_md LIKE ?)
             ORDER BY p.pinned DESC, p.published_at DESC, p.created_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, PublicPostSummary>(
            "SELECT
                p.id,
                p.title,
                p.slug,
                p.excerpt,
                p.content_type,
                p.published_at,
                p.created_at,
                p.updated_at,
                u.display_name AS author_display_name,
                c.name AS category_name,
                c.id AS category_id
             FROM posts p
             JOIN users u ON u.id = p.author_id
             LEFT JOIN categories c ON c.id = p.category_id
             WHERE p.status = 'published' AND p.visibility = 'public'
               AND p.deleted_at IS NULL
             ORDER BY p.pinned DESC, p.published_at DESC, p.created_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
    }
}

pub async fn count_public_posts(
    pool: &SqlitePool,
    keyword: Option<&str>,
) -> Result<i64, sqlx::Error> {
    if let Some(keyword) = keyword {
        let like = format!("%{}%", keyword);
        sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM posts
             WHERE status = 'published' AND visibility = 'public'
               AND deleted_at IS NULL
               AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(pool)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM posts WHERE status = 'published' AND visibility = 'public' AND deleted_at IS NULL",
        )
        .fetch_one(pool)
        .await
    }
}

pub async fn get_public_post_by_slug(
    pool: &SqlitePool,
    slug: &str,
) -> Result<Option<PublicPostDetail>, sqlx::Error> {
    sqlx::query_as::<_, PublicPostDetail>(
        "SELECT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_html,
            p.content_type,
            p.allow_comment,
            p.published_at,
            p.created_at,
            p.updated_at,
            u.display_name AS author_display_name,
            c.name AS category_name
         FROM posts p
         JOIN users u ON u.id = p.author_id
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE p.slug = ? AND p.status = 'published' AND p.visibility = 'public' AND p.deleted_at IS NULL
         LIMIT 1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}

pub async fn find_comment_target(
    pool: &SqlitePool,
    slug: &str,
) -> Result<Option<CommentTargetPost>, sqlx::Error> {
    sqlx::query_as::<_, CommentTargetPost>(
        "SELECT id, title, status, visibility, allow_comment FROM posts WHERE slug = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}

#[allow(dead_code)]
pub async fn list_post_tags(pool: &SqlitePool, post_id: &str) -> Result<Vec<Tag>, sqlx::Error> {
    sqlx::query_as::<_, Tag>(
        "SELECT t.*
         FROM tags t
         JOIN post_tags pt ON pt.tag_id = t.id
         WHERE pt.post_id = ? AND t.deleted_at IS NULL
         ORDER BY t.name ASC",
    )
    .bind(post_id)
    .fetch_all(pool)
    .await
}

pub async fn list_admin_posts(
    pool: &SqlitePool,
    status: Option<&str>,
    keyword: Option<&str>,
    content_type: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AdminPost>, sqlx::Error> {
    match (status, keyword, content_type) {
        (Some(status), Some(keyword), Some(ct)) => {
            let like = format!("%{}%", keyword);
            sqlx::query_as::<_, AdminPost>(
                "SELECT * FROM posts
                 WHERE deleted_at IS NULL AND status = ? AND content_type = ? AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)
                 ORDER BY pinned DESC, published_at DESC, created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(status)
            .bind(ct)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (Some(status), Some(keyword), None) => {
            let like = format!("%{}%", keyword);
            sqlx::query_as::<_, AdminPost>(
                "SELECT * FROM posts
                 WHERE deleted_at IS NULL AND status = ? AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)
                 ORDER BY pinned DESC, published_at DESC, created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(status)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (Some(status), None, Some(ct)) => {
            sqlx::query_as::<_, AdminPost>(
                "SELECT * FROM posts
                 WHERE deleted_at IS NULL AND status = ? AND content_type = ?
                 ORDER BY pinned DESC, published_at DESC, created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(status)
            .bind(ct)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (Some(status), None, None) => {
            sqlx::query_as::<_, AdminPost>(
                "SELECT * FROM posts
                 WHERE deleted_at IS NULL AND status = ?
                 ORDER BY pinned DESC, published_at DESC, created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (None, Some(keyword), Some(ct)) => {
            let like = format!("%{}%", keyword);
            sqlx::query_as::<_, AdminPost>(
                "SELECT * FROM posts
                 WHERE deleted_at IS NULL AND content_type = ? AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)
                 ORDER BY pinned DESC, published_at DESC, created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(ct)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (None, Some(keyword), None) => {
            let like = format!("%{}%", keyword);
            sqlx::query_as::<_, AdminPost>(
                "SELECT * FROM posts
                 WHERE deleted_at IS NULL AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)
                 ORDER BY pinned DESC, published_at DESC, created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (None, None, Some(ct)) => {
            sqlx::query_as::<_, AdminPost>(
                "SELECT * FROM posts
                 WHERE deleted_at IS NULL AND content_type = ?
                 ORDER BY pinned DESC, published_at DESC, created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(ct)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (None, None, None) => {
            sqlx::query_as::<_, AdminPost>(
                "SELECT * FROM posts
                 WHERE deleted_at IS NULL
                 ORDER BY pinned DESC, published_at DESC, created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    }
}

pub async fn count_admin_posts(
    pool: &SqlitePool,
    status: Option<&str>,
    keyword: Option<&str>,
    content_type: Option<&str>,
) -> Result<i64, sqlx::Error> {
    match (status, keyword, content_type) {
        (Some(status), Some(keyword), Some(ct)) => {
            let like = format!("%{}%", keyword);
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL AND status = ? AND content_type = ? AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)",
            )
            .bind(status)
            .bind(ct)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_one(pool)
            .await
        }
        (Some(status), Some(keyword), None) => {
            let like = format!("%{}%", keyword);
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL AND status = ? AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)",
            )
            .bind(status)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_one(pool)
            .await
        }
        (Some(status), None, Some(ct)) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL AND status = ? AND content_type = ?")
                .bind(status)
                .bind(ct)
                .fetch_one(pool)
                .await
        }
        (Some(status), None, None) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL AND status = ?")
                .bind(status)
                .fetch_one(pool)
                .await
        }
        (None, Some(keyword), Some(ct)) => {
            let like = format!("%{}%", keyword);
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL AND content_type = ? AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)",
            )
            .bind(ct)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_one(pool)
            .await
        }
        (None, Some(keyword), None) => {
            let like = format!("%{}%", keyword);
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL AND (title LIKE ? OR excerpt LIKE ? OR content_md LIKE ?)",
            )
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_one(pool)
            .await
        }
        (None, None, Some(ct)) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL AND content_type = ?")
                .bind(ct)
                .fetch_one(pool)
                .await
        }
        (None, None, None) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL")
                .fetch_one(pool)
                .await
        }
    }
}

pub async fn get_admin_post(pool: &SqlitePool, id: &str) -> Result<Option<AdminPost>, sqlx::Error> {
    sqlx::query_as::<_, AdminPost>("SELECT * FROM posts WHERE id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn slug_exists(
    pool: &SqlitePool,
    slug: &str,
    exclude_id: Option<&str>,
) -> Result<bool, sqlx::Error> {
    if let Some(exclude_id) = exclude_id {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE slug = ? AND id != ?)")
            .bind(slug)
            .bind(exclude_id)
            .fetch_one(pool)
            .await
    } else {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE slug = ?)")
            .bind(slug)
            .fetch_one(pool)
            .await
    }
}

pub async fn insert_post(
    pool: &SqlitePool,
    author_id: &str,
    title: &str,
    slug: &str,
    excerpt: Option<&str>,
    content_md: &str,
    content_html: &str,
    cover_media_id: Option<&str>,
    status: &str,
    visibility: &str,
    category_id: Option<&str>,
    allow_comment: bool,
    pinned: bool,
    content_type: &str,
    custom_html_path: Option<&str>,
    page_render_mode: &str,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let published_at = if status == "published" {
        Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO posts (
            id, author_id, title, slug, excerpt, content_md, content_html, cover_media_id,
            status, visibility, category_id, allow_comment, pinned, content_type,
            custom_html_path, page_render_mode, published_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(author_id)
    .bind(title)
    .bind(slug)
    .bind(excerpt)
    .bind(content_md)
    .bind(content_html)
    .bind(cover_media_id)
    .bind(status)
    .bind(visibility)
    .bind(category_id)
    .bind(if allow_comment { 1 } else { 0 })
    .bind(if pinned { 1 } else { 0 })
    .bind(content_type)
    .bind(custom_html_path)
    .bind(page_render_mode)
    .bind(published_at)
    .execute(pool)
    .await?;

    Ok(id)
}

pub async fn update_post(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    slug: &str,
    excerpt: Option<&str>,
    content_md: &str,
    content_html: &str,
    cover_media_id: Option<&str>,
    status: &str,
    visibility: &str,
    category_id: Option<&str>,
    allow_comment: bool,
    pinned: bool,
    content_type: &str,
    custom_html_path: Option<&str>,
    page_render_mode: &str,
    current_published_at: Option<&str>,
) -> Result<(), sqlx::Error> {
    // 只在从非 published 状态首次发布时才设置 published_at；
    // 若已发布则保留原时间；若切换回 draft/trashed 则清空。
    let published_at: Option<String> = if status == "published" {
        Some(
            current_published_at
                .map(|t| t.to_string())
                .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        )
    } else {
        None
    };

    sqlx::query(
        "UPDATE posts
         SET title = ?, slug = ?, excerpt = ?, content_md = ?, content_html = ?, cover_media_id = ?,
             status = ?, visibility = ?, category_id = ?, allow_comment = ?, pinned = ?,
             content_type = ?, custom_html_path = ?, page_render_mode = ?, published_at = ?,
             updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(title)
    .bind(slug)
    .bind(excerpt)
    .bind(content_md)
    .bind(content_html)
    .bind(cover_media_id)
    .bind(status)
    .bind(visibility)
    .bind(category_id)
    .bind(if allow_comment { 1 } else { 0 })
    .bind(if pinned { 1 } else { 0 })
    .bind(content_type)
    .bind(custom_html_path)
    .bind(page_render_mode)
    .bind(published_at)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn replace_tags(
    pool: &SqlitePool,
    post_id: &str,
    tag_ids: &[String],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(pool)
        .await?;
    for tag_id in tag_ids {
        sqlx::query("INSERT INTO post_tags (post_id, tag_id) VALUES (?, ?)")
            .bind(post_id)
            .bind(tag_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn delete_post(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE posts SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Get page info for custom page rendering
#[derive(Debug, Clone, FromRow)]
pub struct PageCustomHtml {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub title: String,
    pub content_type: String,
    pub custom_html_path: Option<String>,
    pub page_render_mode: String,
    #[allow(dead_code)]
    pub content_md: String,
    #[allow(dead_code)]
    pub content_html: String,
    pub status: String,
    pub visibility: String,
}

pub async fn get_page_by_slug(
    pool: &SqlitePool,
    slug: &str,
) -> Result<Option<PageCustomHtml>, sqlx::Error> {
    sqlx::query_as::<_, PageCustomHtml>(
        "SELECT id, title, content_type, custom_html_path, page_render_mode,
                content_md, content_html, status, visibility
         FROM posts WHERE slug = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}
