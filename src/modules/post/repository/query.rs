use std::collections::HashMap;

use sqlx::FromRow;

use crate::modules::post::domain::{
    AdminPost, CommentTargetPost, PublicPostDetail, PublicPostSummary, SitemapItem,
};
use crate::modules::post::post_types::{ContentType, PostStatus, Visibility};
use crate::modules::tag::domain::Tag;

pub async fn list_recent_public_posts<'e, E>(
    executor: E,
    limit: i64,
) -> Result<Vec<PublicPostSummary>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        PublicPostSummary,
        r#"
        SELECT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_type as "content_type: ContentType",
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
        LIMIT ?
        "#,
        limit
    )
    .fetch_all(executor)
    .await
}

pub async fn list_for_sitemap<'e, E>(executor: E) -> Result<Vec<SitemapItem>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        SitemapItem,
        r#"
        SELECT
            slug,
            content_type as "content_type: ContentType",
            published_at,
            updated_at
        FROM posts
        WHERE status = 'published' AND visibility = 'public' AND deleted_at IS NULL
        ORDER BY published_at DESC
        "#
    )
    .fetch_all(executor)
    .await
}

/// FTS5 trigram search with LIKE fallback (trigram tokenizer handles CJK better than unicode61).
/// Returns PublicPostSummary items matching the keyword, optionally filtered by category/tag.
pub async fn search_posts<'e, E>(
    executor: E,
    keyword: &str,
    category_id: Option<&str>,
    tag_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<PublicPostSummary>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    // First: try FTS5 trigram
    let fts_keyword = keyword.to_string();
    let fts_results = sqlx::query_as!(
        PublicPostSummary,
        r#"
        SELECT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_type as "content_type: ContentType",
            p.published_at,
            p.created_at,
            p.updated_at,
            u.display_name AS author_display_name,
            c.name AS category_name,
            c.id AS category_id
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
        LIMIT ? OFFSET ?
        "#,
        fts_keyword,
        category_id,
        category_id,
        tag_id,
        tag_id,
        limit,
        offset
    )
    .fetch_all(executor)
    .await?;

    if !fts_results.is_empty() {
        return Ok(fts_results);
    }

    // FTS5 no results → fallback to LIKE
    let like_keyword = format!("%{}%", keyword);
    sqlx::query_as!(
        PublicPostSummary,
        r#"
        SELECT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_type as "content_type: ContentType",
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
          AND (p.title LIKE ? OR p.content_md LIKE ?)
          AND (? IS NULL OR p.category_id = ?)
          AND (? IS NULL OR EXISTS (
                   SELECT 1 FROM post_tags pt WHERE pt.post_id = p.id AND pt.tag_id = ?
              ))
        ORDER BY p.pinned DESC, p.published_at DESC, p.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        like_keyword,
        like_keyword,
        category_id,
        category_id,
        tag_id,
        tag_id,
        limit,
        offset
    )
    .fetch_all(executor)
    .await
}

pub async fn count_search_posts<'e, E>(
    executor: E,
    keyword: &str,
    category_id: Option<&str>,
    tag_id: Option<&str>,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
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
    .fetch_one(executor)
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
    .fetch_one(executor)
    .await
}

pub async fn list_public_posts<'e, E>(
    executor: E,
    keyword: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<PublicPostSummary>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(keyword) = keyword {
        let like = format!("%{}%", keyword);
        sqlx::query_as!(
            PublicPostSummary,
            r#"
            SELECT
                p.id,
                p.title,
                p.slug,
                p.excerpt,
                p.content_type as "content_type: ContentType",
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
            LIMIT ? OFFSET ?
            "#,
            like,
            like,
            like,
            limit,
            offset
        )
        .fetch_all(executor)
        .await
    } else {
        sqlx::query_as!(
            PublicPostSummary,
            r#"
            SELECT
                p.id,
                p.title,
                p.slug,
                p.excerpt,
                p.content_type as "content_type: ContentType",
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
            LIMIT ? OFFSET ?
            "#,
            limit,
            offset
        )
        .fetch_all(executor)
        .await
    }
}

pub async fn count_public_posts<'e, E>(
    executor: E,
    keyword: Option<&str>,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
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
        .fetch_one(executor)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM posts WHERE status = 'published' AND visibility = 'public' AND deleted_at IS NULL",
        )
        .fetch_one(executor)
        .await
    }
}

pub async fn get_public_post_by_slug<'e, E>(
    executor: E,
    slug: &str,
) -> Result<Option<PublicPostDetail>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        PublicPostDetail,
        r#"
        SELECT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_html,
            p.content_type as "content_type: ContentType",
            p.allow_comment as "allow_comment: bool",
            p.published_at,
            p.created_at,
            p.updated_at,
            u.display_name AS author_display_name,
            c.name AS category_name,
            p.cover_media_id
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE p.slug = ? AND p.status = 'published' AND p.visibility = 'public' AND p.deleted_at IS NULL
        LIMIT 1
        "#,
        slug
    )
    .fetch_optional(executor)
    .await
}

pub async fn find_comment_target<'e, E>(
    executor: E,
    slug: &str,
) -> Result<Option<CommentTargetPost>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        CommentTargetPost,
        r#"
        SELECT
            id,
            title,
            status as "status: PostStatus",
            visibility as "visibility: Visibility",
            allow_comment as "allow_comment: bool"
        FROM posts
        WHERE slug = ? AND deleted_at IS NULL
        LIMIT 1
        "#,
        slug
    )
    .fetch_optional(executor)
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

/// 批量查询多篇文章的标签。一次 IN 查询替代 N 次单独查询。
/// 返回 HashMap<post_id, Vec<Tag>>，无标签的文章 key 不存在。
pub async fn list_tags_for_posts<'e, E>(
    executor: E,
    post_ids: &[String],
) -> Result<HashMap<String, Vec<Tag>>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite> + Copy,
{
    if post_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders: Vec<String> = post_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "SELECT pt.post_id, t.id, t.name FROM tags t \
         JOIN post_tags pt ON pt.tag_id = t.id \
         WHERE pt.post_id IN ({})",
        placeholders.join(",")
    );

    let mut query = sqlx::query_as::<_, (String, String, String)>(&sql);
    for id in post_ids {
        query = query.bind(id);
    }

    let rows = query.fetch_all(executor).await?;
    let mut map: HashMap<String, Vec<Tag>> = HashMap::new();
    for (post_id, tag_id, tag_name) in rows {
        map.entry(post_id).or_default().push(Tag {
            id: tag_id,
            name: tag_name,
            slug: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        });
    }
    Ok(map)
}

/// 管理后台文章列表——动态构建 WHERE 条件，替代 8-arm match。
pub async fn list_admin_posts<'e, E>(
    executor: E,
    status: Option<PostStatus>,
    keyword: Option<&str>,
    content_type: Option<ContentType>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AdminPost>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let mut builder =
        sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM posts WHERE deleted_at IS NULL");

    if let Some(s) = status {
        builder.push(" AND status = ").push_bind(s);
    }
    if let Some(kw) = keyword {
        let pattern = format!("%{}%", kw);
        builder
            .push(" AND (title LIKE ")
            .push_bind(pattern.clone())
            .push(" OR excerpt LIKE ")
            .push_bind(pattern.clone())
            .push(" OR content_md LIKE ")
            .push_bind(pattern)
            .push(")");
    }
    if let Some(ct) = content_type {
        builder.push(" AND content_type = ").push_bind(ct);
    }

    builder.push(" ORDER BY pinned DESC, published_at DESC, created_at DESC");
    builder.push(" LIMIT ").push_bind(limit);
    builder.push(" OFFSET ").push_bind(offset);

    builder
        .build_query_as::<AdminPost>()
        .fetch_all(executor)
        .await
}

/// 管理后台文章计数——与 list_admin_posts 共享相同的动态 WHERE 构建模式。
pub async fn count_admin_posts<'e, E>(
    executor: E,
    status: Option<PostStatus>,
    keyword: Option<&str>,
    content_type: Option<ContentType>,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT COUNT(*) FROM posts WHERE deleted_at IS NULL",
    );

    if let Some(s) = status {
        builder.push(" AND status = ").push_bind(s);
    }
    if let Some(kw) = keyword {
        let pattern = format!("%{}%", kw);
        builder
            .push(" AND (title LIKE ")
            .push_bind(pattern.clone())
            .push(" OR excerpt LIKE ")
            .push_bind(pattern.clone())
            .push(" OR content_md LIKE ")
            .push_bind(pattern)
            .push(")");
    }
    if let Some(ct) = content_type {
        builder.push(" AND content_type = ").push_bind(ct);
    }

    builder
        .build_query_scalar::<i64>()
        .fetch_one(executor)
        .await
}

pub async fn get_admin_post<'e, E>(executor: E, id: &str) -> Result<Option<AdminPost>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        AdminPost,
        r#"
        SELECT
            id,
            author_id,
            title,
            slug,
            excerpt,
            content_md,
            content_html,
            cover_media_id,
            status as "status: PostStatus",
            visibility as "visibility: Visibility",
            category_id,
            allow_comment as "allow_comment: bool",
            pinned as "pinned: bool",
            content_type as "content_type: ContentType",
            custom_html_path,
            page_render_mode,
            published_at,
            created_at,
            updated_at,
            deleted_at
        FROM posts
        WHERE id = ?
        LIMIT 1
        "#,
        id
    )
    .fetch_optional(executor)
    .await
}

pub async fn slug_exists<'e, E>(
    executor: E,
    slug: &str,
    exclude_id: Option<&str>,
) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    if let Some(exclude_id) = exclude_id {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE slug = ? AND id != ?)")
            .bind(slug)
            .bind(exclude_id)
            .fetch_one(executor)
            .await
    } else {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE slug = ?)")
            .bind(slug)
            .fetch_one(executor)
            .await
    }
}

/// Get page info for custom page rendering
#[derive(Debug, Clone, FromRow)]
pub struct PageCustomHtml {
    pub id: String,
    pub title: String,
    pub content_type: ContentType,
    pub custom_html_path: Option<String>,
    pub page_render_mode: String,
    pub content_md: String,
    pub content_html: String,
    pub status: PostStatus,
    pub visibility: Visibility,
}

pub async fn get_page_by_slug<'e, E>(
    executor: E,
    slug: &str,
) -> Result<Option<PageCustomHtml>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as!(
        PageCustomHtml,
        r#"
        SELECT
            id,
            title,
            content_type as "content_type: ContentType",
            custom_html_path,
            page_render_mode,
            content_md,
            content_html,
            status as "status: PostStatus",
            visibility as "visibility: Visibility"
        FROM posts
        WHERE slug = ? AND deleted_at IS NULL
        LIMIT 1
        "#,
        slug
    )
    .fetch_optional(executor)
    .await
}

/// 按标签 slug 查询已发布文章（分页）
pub async fn list_posts_by_tag_slug<'e, E>(
    executor: E,
    tag_slug: &str,
    page: u32,
    page_size: u32,
) -> Result<Vec<PublicPostSummary>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    
    sqlx::query_as!(
        PublicPostSummary,
        r#"
        SELECT DISTINCT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_type as "content_type: ContentType",
            p.published_at,
            p.created_at,
            p.updated_at,
            u.display_name AS author_display_name,
            c.name AS category_name,
            c.id AS category_id
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        INNER JOIN post_tags pt ON p.id = pt.post_id
        INNER JOIN tags t ON pt.tag_id = t.id
        WHERE t.slug = ?
          AND p.status = 'published'
          AND p.visibility = 'public'
          AND p.deleted_at IS NULL
          AND p.content_type = 'post'
        ORDER BY p.pinned DESC, p.published_at DESC, p.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        tag_slug,
        page_size,
        offset
    )
    .fetch_all(executor)
    .await
}

/// 统计标签下的文章总数
pub async fn count_posts_by_tag_slug<'e, E>(
    executor: E,
    tag_slug: &str,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(DISTINCT p.id) as "count!"
        FROM posts p
        INNER JOIN post_tags pt ON p.id = pt.post_id
        INNER JOIN tags t ON pt.tag_id = t.id
        WHERE t.slug = ?
          AND p.status = 'published'
          AND p.visibility = 'public'
          AND p.deleted_at IS NULL
          AND p.content_type = 'post'
        "#,
        tag_slug
    )
    .fetch_one(executor)
    .await?;
    
    Ok(count as i64)
}

/// 按分类 slug 查询已发布文章（分页）
pub async fn list_posts_by_category_slug<'e, E>(
    executor: E,
    category_slug: &str,
    page: u32,
    page_size: u32,
) -> Result<Vec<PublicPostSummary>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    
    sqlx::query_as!(
        PublicPostSummary,
        r#"
        SELECT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_type as "content_type: ContentType",
            p.published_at,
            p.created_at,
            p.updated_at,
            u.display_name AS author_display_name,
            c.name AS category_name,
            c.id AS category_id
        FROM posts p
        JOIN users u ON u.id = p.author_id
        INNER JOIN categories c ON p.category_id = c.id
        WHERE c.slug = ?
          AND p.status = 'published'
          AND p.visibility = 'public'
          AND p.deleted_at IS NULL
          AND p.content_type = 'post'
        ORDER BY p.pinned DESC, p.published_at DESC, p.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        category_slug,
        page_size,
        offset
    )
    .fetch_all(executor)
    .await
}

/// 统计分类下的文章总数
pub async fn count_posts_by_category_slug<'e, E>(
    executor: E,
    category_slug: &str,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM posts p
        INNER JOIN categories c ON p.category_id = c.id
        WHERE c.slug = ?
          AND p.status = 'published'
          AND p.visibility = 'public'
          AND p.deleted_at IS NULL
          AND p.content_type = 'post'
        "#,
        category_slug
    )
    .fetch_one(executor)
    .await?;
    
    Ok(count as i64)
}

/// 按作者 username 查询已发布文章（分页）
pub async fn list_posts_by_author_username<'e, E>(
    executor: E,
    username: &str,
    page: u32,
    page_size: u32,
) -> Result<Vec<PublicPostSummary>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let offset = (page.saturating_sub(1)).saturating_mul(page_size);

    sqlx::query_as!(
        PublicPostSummary,
        r#"
        SELECT
            p.id,
            p.title,
            p.slug,
            p.excerpt,
            p.content_type as "content_type: ContentType",
            p.published_at,
            p.created_at,
            p.updated_at,
            u.display_name AS author_display_name,
            c.name AS category_name,
            c.id AS category_id
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE u.username = ?
          AND p.status = 'published'
          AND p.visibility = 'public'
          AND p.deleted_at IS NULL
          AND p.content_type = 'post'
        ORDER BY p.pinned DESC, p.published_at DESC, p.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        username,
        page_size,
        offset
    )
    .fetch_all(executor)
    .await
}

/// 统计作者的文章总数
pub async fn count_posts_by_author_username<'e, E>(
    executor: E,
    username: &str,
) -> Result<i64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM posts p
        INNER JOIN users u ON p.author_id = u.id
        WHERE u.username = ?
          AND p.status = 'published'
          AND p.visibility = 'public'
          AND p.deleted_at IS NULL
          AND p.content_type = 'post'
        "#,
        username
    )
    .fetch_one(executor)
    .await?;

    Ok(count as i64)
}
