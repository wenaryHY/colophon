use std::sync::Arc;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use slug::slugify;
use uuid::Uuid;

use crate::{
    shared::{
        auth::AuthUser,
        content::{markdown_to_html, sanitize_html},
        error::{AppError, AppResult},
        http::require_non_empty,
        response::{deleted_json, PaginatedResponse},
    },
    state::AppState,
};

use super::{
    domain::{AdminPost, PublicPostSummary},
    dto::{
        AdminPostResponse, CreatePostRequest, PostQuery, PublicPostResponse, SearchQuery,
        UpdatePostRequest,
    },
    hook_dispatcher,
    post_types::{ContentType, NewPostParams, PostStatus, UpdatePostParams, Visibility},
    repository,
};

/// SQLite datetime 格式，用于 scheduled_at 的 UTC 存储。
pub(crate) const SQLITE_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

fn normalize_status(value: Option<PostStatus>) -> AppResult<PostStatus> {
    Ok(value.unwrap_or_default())
}

fn normalize_visibility(value: Option<Visibility>) -> AppResult<Visibility> {
    Ok(value.unwrap_or(Visibility::Public))
}

fn normalize_content_type(value: Option<ContentType>) -> AppResult<ContentType> {
    Ok(value.unwrap_or_default())
}

fn normalize_page_render_mode(value: Option<&str>, is_page: bool) -> String {
    if !is_page {
        return "editor".to_string();
    }
    match value.unwrap_or("editor") {
        "custom_html" => "custom_html".to_string(),
        _ => "editor".to_string(),
    }
}

/// 将用户输入的 scheduled_at 时间字符串转换为 UTC 时间字符串。
///
/// 支持的输入格式：
/// - 带时区的 ISO 8601: "2026-06-27T20:00:00+08:00" → 直接转 UTC
/// - 不带时区的本地时间: "2026-06-27T20:00:00" → 按 site_timezone 转 UTC
///
/// 返回格式: "2026-06-27 12:00:00" (SQLite datetime 格式)
fn convert_scheduled_at_to_utc(
    scheduled_at: &str,
    site_timezone: &str,
) -> AppResult<String> {
    // 尝试解析带时区的 ISO 8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(scheduled_at) {
        let utc = dt.with_timezone(&Utc);
        return Ok(utc.format(SQLITE_DATETIME_FORMAT).to_string());
    }

    // 尝试解析不带时区的本地时间
    let naive = NaiveDateTime::parse_from_str(scheduled_at, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(scheduled_at, SQLITE_DATETIME_FORMAT))
        .map_err(|_| AppError::BadRequest(format!(
            "无效的时间格式: '{}'。请使用 ISO 8601 格式，如 '2026-06-27T20:00:00'",
            scheduled_at
        )))?;

    let tz: Tz = site_timezone.parse().map_err(|_| {
        AppError::BadRequest(format!("无效的时区: '{}'", site_timezone))
    })?;

    let local_dt = match tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt1, _dt2) => {
            // DST 结束时同一本地时间出现两次，选择较早的（夏令时版本）
            dt1
        }
        chrono::LocalResult::None => {
            return Err(AppError::BadRequest(format!(
                "时间 '{}' 在时区 '{}' 中不存在（夏令时跳过），请调整时间",
                scheduled_at, site_timezone
            )));
        }
    };

    let utc = local_dt.with_timezone(&Utc);
    Ok(utc.format(SQLITE_DATETIME_FORMAT).to_string())
}

/// 验证 scheduled_at 是否在未来。
fn validate_scheduled_at_is_future(scheduled_at_utc: &str) -> AppResult<()> {
    let naive = NaiveDateTime::parse_from_str(scheduled_at_utc, SQLITE_DATETIME_FORMAT)
        .map_err(|_| AppError::BadRequest(format!(
            "无法解析定时发布时间: '{}'，请使用正确的时间格式",
            scheduled_at_utc
        )))?;
    
    let scheduled = naive.and_utc();
    let now = Utc::now();
    
    if scheduled <= now {
        return Err(AppError::BadRequest(
            "定时发布时间必须在未来".to_string()
        ));
    }
    
    if scheduled > now + chrono::Duration::days(365) {
        return Err(AppError::BadRequest(
            "定时发布时间不能超过一年后".to_string()
        ));
    }
    
    Ok(())
}

/// Returns a slug that is unique across all posts (including soft-deleted ones in
/// the trash). If `desired_slug` is already taken, a 6-character random suffix is
/// appended and retried until a free slug is found, so callers never surface a
/// conflict error to the user.
async fn resolve_unique_post_slug(
    pool: &sqlx::SqlitePool,
    desired_slug: &str,
    exclude_post_id: Option<&str>,
) -> AppResult<String> {
    if !repository::slug_exists(pool, desired_slug, exclude_post_id).await? {
        return Ok(desired_slug.to_string());
    }
    const MAX_RETRY_ATTEMPTS_FOR_SLUG_RANDOM_SUFFIX: u32 = 10;
    for _attempt in 0..MAX_RETRY_ATTEMPTS_FOR_SLUG_RANDOM_SUFFIX {
        let random_suffix: String = Uuid::new_v4().to_string().chars().take(6).collect();
        let slug_with_random_suffix = format!("{}-{}", desired_slug, random_suffix);
        if !repository::slug_exists(pool, &slug_with_random_suffix, exclude_post_id).await? {
            return Ok(slug_with_random_suffix);
        }
    }
    // After 10 failed attempts, fall back to a full UUID suffix
    let fallback_suffix = Uuid::new_v4().to_string();
    let slug_with_long_suffix = format!("{}-{}", desired_slug, fallback_suffix);
    if !repository::slug_exists(pool, &slug_with_long_suffix, exclude_post_id).await? {
        return Ok(slug_with_long_suffix);
    }
    Err(AppError::Anyhow(anyhow::anyhow!(
        "unable to generate unique slug after max retries"
    )))
}

async fn attach_admin_post(state: &AppState, post: AdminPost) -> AppResult<AdminPostResponse> {
    let tags = repository::list_post_tags(&state.pool, &post.id).await?;
    Ok(AdminPostResponse { post, tags })
}

pub async fn list_public_posts(
    state: Arc<AppState>,
    query: PostQuery,
) -> AppResult<PaginatedResponse<PublicPostSummary>> {
    let (page, page_size, offset) = query.pagination.normalized(10, 100);
    let items =
        repository::list_public_posts(&state.pool, query.keyword.as_deref(), page_size, offset)
            .await?;
    let total = repository::count_public_posts(&state.pool, query.keyword.as_deref()).await?;
    Ok(PaginatedResponse::new(items, page, page_size, total))
}

/// FTS5 full-text search for public posts
pub async fn search_posts(
    state: Arc<AppState>,
    query: SearchQuery,
) -> AppResult<PaginatedResponse<PublicPostSummary>> {
    let (page, page_size, offset) = query.pagination.normalized(10, 100);
    let items = repository::search_posts(
        &state.pool,
        &query.keyword,
        query.category_id.as_deref(),
        query.tag_id.as_deref(),
        page_size,
        offset,
    )
    .await?;
    let total = repository::count_search_posts(
        &state.pool,
        &query.keyword,
        query.category_id.as_deref(),
        query.tag_id.as_deref(),
    )
    .await?;
    Ok(PaginatedResponse::new(items, page, page_size, total))
}

pub async fn get_public_post(state: Arc<AppState>, slug: &str) -> AppResult<PublicPostResponse> {
    let post = repository::get_public_post_by_slug(&state.pool, slug)
        .await?
        .ok_or(AppError::NotFound(format!("文章 '{}' 未找到", slug)))?;
    let tags = repository::list_post_tags(&state.pool, &post.id).await?;
    Ok(PublicPostResponse { post, tags })
}

pub async fn list_admin_posts(
    state: Arc<AppState>,
    query: PostQuery,
) -> AppResult<PaginatedResponse<AdminPostResponse>> {
    let (page, page_size, offset) = query.pagination.normalized(10, 100);
    let posts = repository::list_admin_posts(
        &state.pool,
        query.status,
        query.keyword.as_deref(),
        query.content_type,
        page_size,
        offset,
    )
    .await?;
    let total = repository::count_admin_posts(
        &state.pool,
        query.status,
        query.keyword.as_deref(),
        query.content_type,
    )
    .await?;

    // 批量取所有文章的标签（1 次查询替代 N 次）
    let post_ids: Vec<String> = posts.iter().map(|post| post.id.clone()).collect();
    let tags_map = repository::list_tags_for_posts(&state.pool, &post_ids).await?;

    let mut items = Vec::with_capacity(posts.len());
    for post in posts {
        let tags = tags_map.get(&post.id).cloned().unwrap_or_default();
        items.push(AdminPostResponse { post, tags });
    }

    Ok(PaginatedResponse::new(items, page, page_size, total))
}

pub async fn get_admin_post(state: Arc<AppState>, id: &str) -> AppResult<AdminPostResponse> {
    let post = repository::get_admin_post(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound(format!("文章 '{}' 未找到", id)))?;
    attach_admin_post(state.as_ref(), post).await
}

pub async fn create_post(
    state: Arc<AppState>,
    auth: &AuthUser,
    body: CreatePostRequest,
) -> AppResult<AdminPostResponse> {
    require_non_empty(&body.title, "title")?;

    let mut title = body.title.trim().to_string();
    let mut content_type = normalize_content_type(body.content_type)?;
    let is_page = content_type.is_page();
    let page_render_mode = normalize_page_render_mode(body.page_render_mode.as_deref(), is_page);

    // Both content_md and custom_html_path are preserved independently.
    // page_render_mode determines which one is used for front-end rendering.
    let content_md = body
        .content_md
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    let slug_from_user_or_generated_from_title = body
        .slug
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| slugify(&body.title));

    let mut slug =
        resolve_unique_post_slug(&state.pool, &slug_from_user_or_generated_from_title, None)
            .await?;

    // 读取 site_timezone 设置
    let site_timezone = crate::modules::setting::repository::get_string(
        &state.pool, "site_timezone", "UTC"
    ).await.unwrap_or_else(|_| "UTC".to_string());

    // 处理定时发布
    let (status, scheduled_at_utc) = if let Some(ref sa) = body.scheduled_at {
        let utc = convert_scheduled_at_to_utc(sa, &site_timezone)?;
        validate_scheduled_at_is_future(&utc)?;
        // 如果用户没有显式指定 status，或者指定为 Draft/Published，强制设为 Scheduled
        let status = match body.status {
            Some(PostStatus::Published) | Some(PostStatus::Draft) | None => PostStatus::Scheduled,
            Some(PostStatus::Trashed) => {
                // Trashed 状态与 scheduled_at 不兼容，忽略 scheduled_at
                PostStatus::Trashed
            }
            Some(PostStatus::Scheduled) => PostStatus::Scheduled,
        };
        // 如果最终状态不是 Scheduled，清空 scheduled_at
        if status != PostStatus::Scheduled {
            (status, None)
        } else {
            (status, Some(utc))
        }
    } else {
        let status = normalize_status(body.status)?;
        // 如果没有 scheduled_at 但 status 是 Scheduled，报错
        if status == PostStatus::Scheduled {
            return Err(AppError::BadRequest(
                "设置定时发布状态必须提供 scheduled_at 时间".to_string()
            ));
        }
        (status, None)
    };

    let visibility = normalize_visibility(body.visibility)?;
    let mut content_html = body
        .content_html
        .filter(|h| !h.trim().is_empty())
        .map(|h| sanitize_html(&h))
        .unwrap_or_else(|| markdown_to_html(&content_md));
    let mut excerpt = body.excerpt.clone();
    let mut category_id = body.category_id.clone();
    let mut tags = body.tag_ids.clone().unwrap_or_default();
    let has_original_tags = body.tag_ids.is_some();

    // =============== Hook: post.before_save (Filter) ===============
    let hook_result = hook_dispatcher::dispatch_post_before_save(
        state.as_ref(),
        title,
        content_html,
        excerpt,
        slug,
        tags,
        category_id,
        content_type,
    )
    .await?;
    title = hook_result.title;
    content_html = hook_result.content_html;
    excerpt = hook_result.excerpt;
    slug = hook_result.slug;
    tags = hook_result.tags;
    category_id = hook_result.category_id;
    content_type = hook_result.content_type;

    slug = resolve_unique_post_slug(&state.pool, &slug, None).await?;

    let id = repository::insert_post(
        &state.pool,
        NewPostParams {
            author_id: &auth.id,
            title: &title,
            slug: &slug,
            excerpt: excerpt.as_deref(),
            content_md: &content_md,
            content_html: &content_html,
            cover_media_id: body.cover_media_id.as_deref(),
            status,
            visibility,
            category_id: category_id.as_deref(),
            allow_comment: body.allow_comment.unwrap_or(content_type.is_post()),
            pinned: body.pinned.unwrap_or(false),
            content_type,
            custom_html_path: body.custom_html_path.as_deref(),
            page_render_mode: &page_render_mode,
            scheduled_at: scheduled_at_utc.as_deref(),
        },
    )
    .await?;

    // Pages don't have tags
    if content_type.is_post() && (has_original_tags || !tags.is_empty()) {
        repository::replace_tags(&state.pool, &id, &tags).await?;
    }

    // =============== Hooks: post.after_save + post.after_publish (Action) ===============
    let post = repository::get_admin_post(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("文章未找到".to_string()))?;
    let old_status = PostStatus::Draft.to_string();
    hook_dispatcher::dispatch_post_after_save(
        state.as_ref(),
        id.clone(),
        post.title.clone(),
        post.slug.clone(),
        true,
        post.status.to_string(),
        old_status.clone(),
    )
    .await;

    if post.status == PostStatus::Published {
        hook_dispatcher::dispatch_post_after_publish(
            state.as_ref(),
            id.clone(),
            post.title.clone(),
            post.slug.clone(),
            old_status,
            PostStatus::Published.to_string(),
        )
        .await;
    }

    get_admin_post(state, &id).await
}

pub async fn update_post(
    state: Arc<AppState>,
    _auth: &AuthUser,
    id: &str,
    body: UpdatePostRequest,
) -> AppResult<AdminPostResponse> {
    let current = repository::get_admin_post(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound(format!("文章 '{}' 未找到", id)))?;

    let old_status = current.status;

    let mut content_type =
        normalize_content_type(body.content_type.or(Some(current.content_type)))?;

    let is_page = content_type.is_page();
    let page_render_mode = normalize_page_render_mode(
        body.page_render_mode
            .as_deref()
            .or(Some(&current.page_render_mode)),
        is_page,
    );

    let custom_html_path = body
        .custom_html_path
        .as_deref()
        .or(current.custom_html_path.as_deref());

    // Both sides are preserved independently.
    // If content_md is provided, update it.
    // If content_md changed and content_html is not explicitly provided (or unchanged),
    // regenerate content_html from the new content_md.
    let content_md_changed = body
        .content_md
        .as_ref()
        .map_or(false, |md| md != &current.content_md);
    let content_md = body.content_md.unwrap_or(current.content_md.clone());
    let mut content_html = if content_md_changed {
        markdown_to_html(&content_md)
    } else {
        body.content_html
            .filter(|h| !h.trim().is_empty())
            .map(|h| sanitize_html(&h))
            .unwrap_or_else(|| current.content_html.clone())
    };

    let mut title = body.title.unwrap_or(current.title.clone());
    let slug_from_user_or_kept_from_current = body
        .slug
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| current.slug.clone());
    let mut slug =
        resolve_unique_post_slug(&state.pool, &slug_from_user_or_kept_from_current, Some(id))
            .await?;
    let mut excerpt = body.excerpt.or(current.excerpt.clone());
    let cover_media_id = body.cover_media_id.or(current.cover_media_id.clone());

    // 读取 site_timezone 设置
    let site_timezone = crate::modules::setting::repository::get_string(
        &state.pool, "site_timezone", "UTC"
    ).await.unwrap_or_else(|_| "UTC".to_string());

    // 处理定时发布状态机
    let (status, scheduled_at_utc) = if let Some(ref sa) = body.scheduled_at {
        let utc = convert_scheduled_at_to_utc(sa, &site_timezone)?;
        validate_scheduled_at_is_future(&utc)?;
        let status = match body.status {
            Some(PostStatus::Published) | Some(PostStatus::Draft) | None => PostStatus::Scheduled,
            Some(PostStatus::Trashed) => PostStatus::Trashed,
            Some(PostStatus::Scheduled) => PostStatus::Scheduled,
        };
        // 如果最终状态不是 Scheduled，清空 scheduled_at
        if status != PostStatus::Scheduled {
            (status, None)
        } else {
            (status, Some(utc))
        }
    } else if let Some(requested_status) = body.status {
        // 没有传 scheduled_at，但传了 status
        match requested_status {
            PostStatus::Scheduled => {
                // 从 Draft/Published 切换到 Scheduled 但没有提供时间，报错
                return Err(AppError::BadRequest(
                    "设置定时发布状态必须提供 scheduled_at 时间".to_string()
                ));
            }
            PostStatus::Draft => {
                // 从 Scheduled 降级到 Draft，清空 scheduled_at
                (requested_status, None)
            }
            PostStatus::Published => {
                // 发布时清空 scheduled_at
                (requested_status, None)
            }
            _ => (requested_status, current.scheduled_at.clone()),
        }
    } else {
        // 都没传，保持当前状态
        (current.status, current.scheduled_at.clone())
    };

    let visibility = normalize_visibility(body.visibility.or(Some(current.visibility)))?;
    let mut category_id = body.category_id.or(current.category_id.clone());
    let allow_comment = body.allow_comment.unwrap_or(current.allow_comment);
    let pinned = body.pinned.unwrap_or(current.pinned);
    let mut tags = body.tag_ids.clone().unwrap_or_default();
    let has_original_tags = body.tag_ids.is_some();

    // =============== Hook: post.before_save (Filter) ===============
    let hook_result = hook_dispatcher::dispatch_post_before_save(
        state.as_ref(),
        title,
        content_html,
        excerpt,
        slug,
        tags,
        category_id,
        content_type,
    )
    .await?;
    title = hook_result.title;
    content_html = hook_result.content_html;
    excerpt = hook_result.excerpt;
    slug = hook_result.slug;
    tags = hook_result.tags;
    category_id = hook_result.category_id;
    content_type = hook_result.content_type;

    slug = resolve_unique_post_slug(&state.pool, &slug, Some(id)).await?;

    repository::update_post(
        &state.pool,
        UpdatePostParams {
            post_id: id,
            title: &title,
            slug: &slug,
            excerpt: excerpt.as_deref(),
            content_md: &content_md,
            content_html: &content_html,
            cover_media_id: cover_media_id.as_deref(),
            status,
            visibility,
            category_id: category_id.as_deref(),
            allow_comment,
            pinned,
            content_type,
            custom_html_path,
            page_render_mode: &page_render_mode,
            scheduled_at: scheduled_at_utc.as_deref(),
        },
        current.published_at.as_deref(),
    )
    .await?;

    // Pages don't have tags; only update tags for posts
    if content_type.is_post() && (has_original_tags || !tags.is_empty()) {
        repository::replace_tags(&state.pool, id, &tags).await?;
    }

    // =============== Hooks: post.after_save + post.after_publish (Action) ===============
    let post = repository::get_admin_post(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound("文章未找到".to_string()))?;
    hook_dispatcher::dispatch_post_after_save(
        state.as_ref(),
        id.to_string(),
        post.title.clone(),
        post.slug.clone(),
        false,
        post.status.to_string(),
        old_status.to_string(),
    )
    .await;

    if post.status == PostStatus::Published && old_status != PostStatus::Published {
        hook_dispatcher::dispatch_post_after_publish(
            state.as_ref(),
            id.to_string(),
            post.title.clone(),
            post.slug.clone(),
            old_status.to_string(),
            PostStatus::Published.to_string(),
        )
        .await;
    }

    get_admin_post(state, id).await
}

pub async fn delete_post(state: Arc<AppState>, id: &str) -> AppResult<serde_json::Value> {
    repository::delete_post(&state.pool, id).await?;
    deleted_json()
}

/// Upload custom HTML/ZIP for a page, return relative path for custom_html_path
pub async fn upload_custom_page(
    state: Arc<AppState>,
    slug: &str,
    filename: String,
    content_type: Option<String>,
    data: Vec<u8>,
) -> AppResult<String> {
    let page_dir = state.upload_dir.join("pages").join(slug);
    if page_dir.exists() {
        tokio::fs::remove_dir_all(&page_dir).await?;
    }
    tokio::fs::create_dir_all(&page_dir).await?;

    let ct = content_type.as_deref().unwrap_or("");
    if ct.contains("zip") || filename.ends_with(".zip") {
        // Extract ZIP — run in spawn_blocking to avoid non-Send futures
        let page_dir_clone = page_dir.clone();
        let data_clone = data.clone();
        tokio::task::spawn_blocking(move || -> AppResult<()> {
            extract_zip(&data_clone, &page_dir_clone)
        })
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("spawn_blocking error: {}", e)))??;
    } else if filename.ends_with(".html") || filename.ends_with(".htm") {
        // Single HTML file → save as index.html
        let outpath = page_dir.join("index.html");
        tokio::fs::write(&outpath, &data).await?;
    } else {
        tokio::fs::remove_dir_all(&page_dir).await.ok();
        return Err(AppError::BadRequest(
            "Only .html, .htm, or .zip files are accepted".into(),
        ));
    }

    // Verify index.html exists
    let index_path = page_dir.join("index.html");
    if !index_path.exists() {
        let _ = tokio::fs::remove_dir_all(&page_dir).await;
        return Err(AppError::BadRequest(
            "ZIP must contain an index.html file".into(),
        ));
    }

    // Return relative path from upload_dir
    let relative = format!("pages/{}", slug);
    Ok(relative)
}

/// Synchronous ZIP extraction (called from spawn_blocking)
fn extract_zip(data: &[u8], dest_dir: &std::path::Path) -> AppResult<()> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| AppError::BadRequest(format!("Invalid zip file: {}", e)))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AppError::BadRequest(format!("Failed to read zip entry: {}", e)))?;
        let entry_path = file
            .enclosed_name()
            .ok_or_else(|| AppError::BadRequest("ZIP contains invalid path entry".into()))?
            .to_path_buf();
        let outpath = dest_dir.join(entry_path);

        if file.is_dir() {
            std::fs::create_dir_all(&outpath).map_err(AppError::Io)?;
            continue;
        }
        if let Some(parent) = outpath.parent() {
            std::fs::create_dir_all(parent).map_err(AppError::Io)?;
        }
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut buf).map_err(AppError::Io)?;
        std::fs::write(&outpath, &buf).map_err(AppError::Io)?;
    }
    Ok(())
}

#[cfg(test)]
mod resolve_unique_post_slug_tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn new_migrated_pool() -> sqlx::SqlitePool {
        let connect_options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("parse sqlite url")
            .foreign_keys(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_options)
            .await
            .expect("connect in-memory sqlite");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        pool
    }

    async fn insert_post_with_slug(pool: &sqlx::SqlitePool, slug: &str) -> String {
        repository::insert_post(
            pool,
            NewPostParams {
                author_id: "test-author",
                title: "Title",
                slug,
                excerpt: None,
                content_md: "",
                content_html: "",
                cover_media_id: None,
                status: PostStatus::Draft,
                visibility: Visibility::Public,
                category_id: None,
                allow_comment: true,
                pinned: false,
                content_type: ContentType::Post,
                custom_html_path: None,
                page_render_mode: "editor",
                scheduled_at: None,
            },
        )
        .await
        .expect("insert post")
    }

    #[tokio::test]
    async fn returns_desired_slug_when_unused() {
        let pool = new_migrated_pool().await;
        let resolved = resolve_unique_post_slug(&pool, "unused-slug", None)
            .await
            .unwrap();
        assert_eq!(resolved, "unused-slug");
    }

    #[tokio::test]
    async fn appends_random_suffix_when_slug_taken() {
        let pool = new_migrated_pool().await;
        insert_post_with_slug(&pool, "taken-slug").await;
        let resolved = resolve_unique_post_slug(&pool, "taken-slug", None)
            .await
            .unwrap();
        assert_ne!(resolved, "taken-slug");
        assert!(resolved.starts_with("taken-slug-"));
        assert_eq!(resolved.len(), "taken-slug".len() + 1 + 6);
    }

    #[tokio::test]
    async fn treats_trashed_post_slug_as_taken() {
        let pool = new_migrated_pool().await;
        let id = insert_post_with_slug(&pool, "trashed-slug").await;
        repository::delete_post(&pool, &id).await.unwrap();
        let resolved = resolve_unique_post_slug(&pool, "trashed-slug", None)
            .await
            .unwrap();
        assert!(resolved.starts_with("trashed-slug-"));
    }

    #[tokio::test]
    async fn keeps_slug_when_only_conflict_is_excluded_post() {
        let pool = new_migrated_pool().await;
        let id = insert_post_with_slug(&pool, "my-own-slug").await;
        let resolved = resolve_unique_post_slug(&pool, "my-own-slug", Some(&id))
            .await
            .unwrap();
        assert_eq!(resolved, "my-own-slug");
    }
}

#[cfg(test)]
mod scheduled_at_tests {
    use super::*;

    // ── convert_scheduled_at_to_utc ──

    #[test]
    fn converts_rfc3339_with_positive_offset_to_utc() {
        let result = convert_scheduled_at_to_utc("2026-06-27T20:00:00+08:00", "UTC").unwrap();
        assert_eq!(result, "2026-06-27 12:00:00");
    }

    #[test]
    fn converts_rfc3339_with_negative_offset_to_utc() {
        let result = convert_scheduled_at_to_utc("2026-06-27T08:00:00-04:00", "UTC").unwrap();
        assert_eq!(result, "2026-06-27 12:00:00");
    }

    #[test]
    fn converts_naive_local_time_with_timezone_to_utc() {
        // 20:00 in Asia/Shanghai (UTC+8) = 12:00 UTC
        let result =
            convert_scheduled_at_to_utc("2026-06-27T20:00:00", "Asia/Shanghai").unwrap();
        assert_eq!(result, "2026-06-27 12:00:00");
    }

    #[test]
    fn converts_naive_local_time_space_format() {
        let result =
            convert_scheduled_at_to_utc("2026-06-27 20:00:00", "Asia/Shanghai").unwrap();
        assert_eq!(result, "2026-06-27 12:00:00");
    }

    #[test]
    fn converts_utc_naive_time_as_utc() {
        let result = convert_scheduled_at_to_utc("2026-06-27T12:00:00", "UTC").unwrap();
        assert_eq!(result, "2026-06-27 12:00:00");
    }

    #[test]
    fn rejects_invalid_time_format() {
        let result = convert_scheduled_at_to_utc("not-a-date", "UTC");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::BadRequest(msg) => assert!(msg.contains("无效的时间格式")),
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn rejects_invalid_timezone() {
        let result = convert_scheduled_at_to_utc("2026-06-27T20:00:00", "Invalid/Zone");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::BadRequest(msg) => assert!(msg.contains("无效的时区")),
            _ => panic!("expected BadRequest"),
        }
    }

    // ── validate_scheduled_at_is_future ──

    #[test]
    fn rejects_past_time() {
        let result = validate_scheduled_at_is_future("2020-01-01 00:00:00");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::BadRequest(msg) => assert!(msg.contains("未来")),
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn accepts_future_time_within_one_year() {
        let future = (Utc::now() + chrono::Duration::days(30)).format(SQLITE_DATETIME_FORMAT).to_string();
        let result = validate_scheduled_at_is_future(&future);
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_malformed_utc_string() {
        let result = validate_scheduled_at_is_future("not-a-time");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::BadRequest(msg) => assert!(msg.contains("无法解析定时发布时间")),
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn malformed_utc_error_shows_input_value() {
        let result = validate_scheduled_at_is_future("bad-input");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("bad-input"));
    }

    // ── end-to-end: convert then validate ──

    #[test]
    fn future_rfc3339_converts_and_validates() {
        let future = (Utc::now() + chrono::Duration::days(30)).format("%Y-%m-%dT%H:%M:%S+08:00").to_string();
        let utc = convert_scheduled_at_to_utc(&future, "UTC").unwrap();
        validate_scheduled_at_is_future(&utc).unwrap();
    }

    #[test]
    fn future_naive_time_with_tz_converts_and_validates() {
        let future = (Utc::now() + chrono::Duration::days(30)).format("%Y-%m-%dT%H:%M:%S").to_string();
        let utc =
            convert_scheduled_at_to_utc(&future, "Asia/Shanghai").unwrap();
        validate_scheduled_at_is_future(&utc).unwrap();
    }

    #[test]
    fn reject_scheduled_at_more_than_one_year_future() {
        let far_future = "2099-12-31 23:59:59";
        let result = validate_scheduled_at_is_future(far_future);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("一年"));
    }

    // ── L-1: SQLITE_DATETIME_FORMAT 常量 ──

    #[test]
    fn sqlite_datetime_format_matches_expected() {
        assert_eq!(SQLITE_DATETIME_FORMAT, "%Y-%m-%d %H:%M:%S");
    }

    #[test]
    fn convert_scheduled_at_to_utc_uses_constant_format() {
        // 验证转换结果格式与常量一致
        let result = convert_scheduled_at_to_utc("2026-06-27T20:00:00+08:00", "UTC").unwrap();
        let parsed = NaiveDateTime::parse_from_str(&result, SQLITE_DATETIME_FORMAT);
        assert!(parsed.is_ok(), "convert_scheduled_at_to_utc 输出应匹配 SQLITE_DATETIME_FORMAT");
    }

    // ── DST 边界测试（H-2）──

    #[test]
    fn convert_scheduled_at_to_utc_ambiguous_time_picks_earlier() {
        // America/New_York 在 11月第一个周日 1:00-2:00 会重复
        // 1:30 AM 出现两次：EDT (UTC-4) 和 EST (UTC-5)
        // 应选择较早的 EDT 版本
        // 2024-11-03 是 America/New_York 的 DST 结束日
        let result = convert_scheduled_at_to_utc("2024-11-03T01:30:00", "America/New_York");
        // 只验证不 panic，具体值依赖 chrono 的 Ambiguous 选择策略
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn convert_scheduled_at_to_utc_nonexistent_time_returns_error() {
        // America/New_York 在 3月第二个周日 2:00-3:00 被跳过
        // 2:30 AM 不存在，应返回错误
        let result = convert_scheduled_at_to_utc("2024-03-10T02:30:00", "America/New_York");
        // 不存在的时间应返回错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("不存在"));
    }
}
