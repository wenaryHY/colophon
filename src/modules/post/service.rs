use std::sync::Arc;

use slug::slugify;
use uuid::Uuid;

use crate::{
    modules::plugin::hook::{
        HookContext, HookData, PostAfterPublishData, PostAfterSaveData, PostBeforeSaveData,
    },
    shared::{
        auth::AuthUser,
        content::{markdown_to_html, sanitize_html},
        error::{AppError, AppResult},
        pagination::PaginationQuery,
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
    post_types::{ContentType, NewPostParams, PostStatus, UpdatePostParams, Visibility},
    repository,
};

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
    let pagination = PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    };
    let (page, page_size, offset) = pagination.normalized(10, 100);
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
    let pagination = PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    };
    let (page, page_size, offset) = pagination.normalized(10, 100);
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
    let pagination = PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    };
    let (page, page_size, offset) = pagination.normalized(10, 100);
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
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }

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

    let status = normalize_status(body.status)?;
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
    let hook_registry = state.plugin_manager.read().await.hook_registry().clone();
    let mut save_ctx = HookContext {
        hook_name: "post.before_save".into(),
        data: HookData::PostBeforeSave(PostBeforeSaveData {
            title: title.clone(),
            content_html: content_html.clone(),
            excerpt: excerpt.clone(),
            slug: slug.clone(),
            tags: tags.clone(),
            category_id: category_id.clone(),
            content_type: content_type.to_string(),
            request_ip: None,
            user_agent: None,
        }),
    };
    hook_registry
        .dispatch_filter("post.before_save", &mut save_ctx)
        .await?;

    // Extract potentially modified fields from the filter
    if let HookData::PostBeforeSave(ref data) = save_ctx.data {
        title = data.title.clone();
        content_html = data.content_html.clone();
        excerpt = data.excerpt.clone();
        slug = data.slug.clone();
        tags = data.tags.clone();
        category_id = data.category_id.clone();
        content_type = data.content_type.parse()?;
    }

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
        },
    )
    .await?;

    // Pages don't have tags
    if content_type.is_post() && (has_original_tags || !tags.is_empty()) {
        repository::replace_tags(&state.pool, &id, &tags).await?;
    }

    // =============== Hook: post.after_save (Action) ===============
    let post = repository::get_admin_post(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("文章未找到".to_string()))?;
    let old_status = PostStatus::Draft.to_string();
    let after_save_ctx = HookContext {
        hook_name: "post.after_save".into(),
        data: HookData::PostAfterSave(PostAfterSaveData {
            post_id: id.clone(),
            title: post.title.clone(),
            slug: post.slug.clone(),
            is_new: true,
            status: post.status.to_string(),
            old_status: Some(old_status.clone()),
        }),
    };
    hook_registry
        .dispatch_action("post.after_save", after_save_ctx)
        .await;

    // =============== Hook: post.after_publish (Action) ===============
    if post.status == PostStatus::Published {
        let publish_ctx = HookContext {
            hook_name: "post.after_publish".into(),
            data: HookData::PostAfterPublish(PostAfterPublishData {
                post_id: id.clone(),
                title: post.title.clone(),
                slug: post.slug.clone(),
                old_status,
                new_status: PostStatus::Published.to_string(),
            }),
        };
        hook_registry
            .dispatch_action("post.after_publish", publish_ctx)
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
    let status = normalize_status(body.status.or(Some(current.status)))?;
    let visibility = normalize_visibility(body.visibility.or(Some(current.visibility)))?;
    let mut category_id = body.category_id.or(current.category_id.clone());
    let allow_comment = body.allow_comment.unwrap_or(current.allow_comment);
    let pinned = body.pinned.unwrap_or(current.pinned);
    let mut tags = body.tag_ids.clone().unwrap_or_default();
    let has_original_tags = body.tag_ids.is_some();

    // =============== Hook: post.before_save (Filter) ===============
    let hook_registry = state.plugin_manager.read().await.hook_registry().clone();
    let mut save_ctx = HookContext {
        hook_name: "post.before_save".into(),
        data: HookData::PostBeforeSave(PostBeforeSaveData {
            title: title.clone(),
            content_html: content_html.clone(),
            excerpt: excerpt.clone(),
            slug: slug.clone(),
            tags: tags.clone(),
            category_id: category_id.clone(),
            content_type: content_type.to_string(),
            request_ip: None,
            user_agent: None,
        }),
    };
    hook_registry
        .dispatch_filter("post.before_save", &mut save_ctx)
        .await?;

    // Extract potentially modified fields from the filter
    if let HookData::PostBeforeSave(ref data) = save_ctx.data {
        title = data.title.clone();
        content_html = data.content_html.clone();
        excerpt = data.excerpt.clone();
        slug = data.slug.clone();
        tags = data.tags.clone();
        category_id = data.category_id.clone();
        content_type = data.content_type.parse()?;
    }

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
        },
        current.published_at.as_deref(),
    )
    .await?;

    // Pages don't have tags; only update tags for posts
    if content_type.is_post() && (has_original_tags || !tags.is_empty()) {
        repository::replace_tags(&state.pool, id, &tags).await?;
    }

    // =============== Hook: post.after_save (Action) ===============
    let post = repository::get_admin_post(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound("文章未找到".to_string()))?;
    let after_save_ctx = HookContext {
        hook_name: "post.after_save".into(),
        data: HookData::PostAfterSave(PostAfterSaveData {
            post_id: id.to_string(),
            title: post.title.clone(),
            slug: post.slug.clone(),
            is_new: false,
            status: post.status.to_string(),
            old_status: Some(old_status.to_string()),
        }),
    };
    hook_registry
        .dispatch_action("post.after_save", after_save_ctx)
        .await;

    // =============== Hook: post.after_publish (Action) ===============
    if post.status == PostStatus::Published && old_status != PostStatus::Published {
        let publish_ctx = HookContext {
            hook_name: "post.after_publish".into(),
            data: HookData::PostAfterPublish(PostAfterPublishData {
                post_id: id.to_string(),
                title: post.title.clone(),
                slug: post.slug.clone(),
                old_status: old_status.to_string(),
                new_status: PostStatus::Published.to_string(),
            }),
        };
        hook_registry
            .dispatch_action("post.after_publish", publish_ctx)
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
