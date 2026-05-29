use std::sync::Arc;

use pulldown_cmark::{html, Options, Parser};
use slug::slugify;

use crate::{
    modules::plugin::hook::{
        HookContext, HookData, PostAfterPublishData, PostAfterSaveData, PostBeforeSaveData,
    },
    shared::{
        auth::AuthUser,
        error::{AppError, AppResult},
        pagination::PaginationQuery,
        response::PaginatedResponse,
    },
    state::AppState,
};

use super::{
    domain::{AdminPost, PublicPostSummary},
    dto::{
        AdminPostResponse, CreatePostRequest, PostQuery, PublicPostResponse, SearchQuery,
        UpdatePostRequest,
    },
    repository,
};

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(markdown, options);
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    ammonia::clean(&html_out)
}

fn sanitize_html(html: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder.add_tags(&["span", "mark"]);
    builder.add_tag_attributes("span", &["style"]);
    builder.add_tag_attributes("mark", &["style"]);
    ammonia::clean(html)
}

fn normalize_status(value: Option<&str>) -> AppResult<String> {
    let status = value.unwrap_or("draft");
    if !matches!(status, "draft" | "published" | "trashed") {
        return Err(AppError::BadRequest("invalid post status".into()));
    }
    Ok(status.to_string())
}

fn normalize_visibility(value: Option<&str>) -> AppResult<String> {
    let visibility = value.unwrap_or("public");
    if !matches!(visibility, "public" | "private") {
        return Err(AppError::BadRequest("invalid visibility".into()));
    }
    Ok(visibility.to_string())
}

fn normalize_content_type(value: Option<&str>) -> AppResult<String> {
    let ct = value.unwrap_or("post");
    if !matches!(ct, "post" | "page") {
        return Err(AppError::BadRequest("invalid content_type, must be 'post' or 'page'".into()));
    }
    Ok(ct.to_string())
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
        .ok_or(AppError::NotFound)?;
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
        query.status.as_deref(),
        query.keyword.as_deref(),
        query.content_type.as_deref(),
        page_size,
        offset,
    )
    .await?;
    let total = repository::count_admin_posts(
        &state.pool,
        query.status.as_deref(),
        query.keyword.as_deref(),
        query.content_type.as_deref(),
    )
    .await?;

    let mut items = Vec::with_capacity(posts.len());
    for post in posts {
        items.push(attach_admin_post(state.as_ref(), post).await?);
    }

    Ok(PaginatedResponse::new(items, page, page_size, total))
}

pub async fn get_admin_post(state: Arc<AppState>, id: &str) -> AppResult<AdminPostResponse> {
    let post = repository::get_admin_post(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
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
    let mut content_type = normalize_content_type(body.content_type.as_deref())?;
    let is_page = content_type == "page";
    let page_render_mode = normalize_page_render_mode(body.page_render_mode.as_deref(), is_page);

    // Both content_md and custom_html_path are preserved independently.
    // page_render_mode determines which one is used for front-end rendering.
    let content_md = body
        .content_md
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    let mut slug = body
        .slug
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| slugify(&body.title));

    let status = normalize_status(body.status.as_deref())?;
    let visibility = normalize_visibility(body.visibility.as_deref())?;
    let mut content_html = body.content_html
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
            content_type: content_type.clone(),
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
        content_type = data.content_type.clone();
    }

    if repository::slug_exists(&state.pool, &slug, None).await? {
        return Err(AppError::Conflict("post slug already exists".into()));
    }

    let id = repository::insert_post(
        &state.pool,
        &auth.id,
        &title,
        &slug,
        excerpt.as_deref(),
        &content_md,
        &content_html,
        body.cover_media_id.as_deref(),
        &status,
        &visibility,
        category_id.as_deref(),
        body.allow_comment.unwrap_or(content_type == "post"),
        body.pinned.unwrap_or(false),
        &content_type,
        body.custom_html_path.as_deref(),
        &page_render_mode,
    )
    .await?;

    // Pages don't have tags
    if content_type == "post" && (has_original_tags || !tags.is_empty()) {
        repository::replace_tags(&state.pool, &id, &tags).await?;
    }

    // =============== Hook: post.after_save (Action) ===============
    let post = repository::get_admin_post(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let old_status = "".to_string();
    let after_save_ctx = HookContext {
        hook_name: "post.after_save".into(),
        data: HookData::PostAfterSave(PostAfterSaveData {
            post_id: id.clone(),
            title: post.title.clone(),
            slug: post.slug.clone(),
            is_new: true,
            status: post.status.clone(),
            old_status: Some(old_status.clone()),
        }),
    };
    hook_registry
        .dispatch_action("post.after_save", after_save_ctx)
        .await;

    // =============== Hook: post.after_publish (Action) ===============
    if post.status == "published" {
        let publish_ctx = HookContext {
            hook_name: "post.after_publish".into(),
            data: HookData::PostAfterPublish(PostAfterPublishData {
                post_id: id.clone(),
                title: post.title.clone(),
                slug: post.slug.clone(),
                old_status: old_status,
                new_status: "published".to_string(),
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
        .ok_or(AppError::NotFound)?;

    let old_status = current.status.clone();

    let mut content_type = normalize_content_type(
        body.content_type
            .as_deref()
            .or(Some(&current.content_type)),
    )?;

    let is_page = content_type == "page";
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
    // If content_md is provided, update it (and re-render content_html).
    // If not provided, keep existing values.
    let content_md = body.content_md.unwrap_or(current.content_md.clone());
    let mut content_html = body.content_html
        .filter(|h| !h.trim().is_empty())
        .map(|h| sanitize_html(&h))
        .unwrap_or_else(|| markdown_to_html(&content_md));

    let mut title = body.title.unwrap_or(current.title.clone());
    let mut slug = body.slug.unwrap_or(current.slug.clone());
    let mut excerpt = body.excerpt.or(current.excerpt.clone());
    let cover_media_id = body.cover_media_id.or(current.cover_media_id.clone());
    let status = normalize_status(body.status.as_deref().or(Some(&current.status)))?;
    let visibility =
        normalize_visibility(body.visibility.as_deref().or(Some(&current.visibility)))?;
    let mut category_id = body.category_id.or(current.category_id.clone());
    let allow_comment = body.allow_comment.unwrap_or(current.allow_comment == 1);
    let pinned = body.pinned.unwrap_or(current.pinned == 1);
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
            content_type: content_type.clone(),
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
        content_type = data.content_type.clone();
    }

    if repository::slug_exists(&state.pool, &slug, Some(id)).await? {
        return Err(AppError::Conflict("post slug already exists".into()));
    }

    repository::update_post(
        &state.pool,
        id,
        &title,
        &slug,
        excerpt.as_deref(),
        &content_md,
        &content_html,
        cover_media_id.as_deref(),
        &status,
        &visibility,
        category_id.as_deref(),
        allow_comment,
        pinned,
        &content_type,
        custom_html_path,
        &page_render_mode,
        current.published_at.as_deref(),
    )
    .await?;

    // Pages don't have tags; only update tags for posts
    if content_type == "post" && (has_original_tags || !tags.is_empty()) {
        repository::replace_tags(&state.pool, id, &tags).await?;
    }

    // =============== Hook: post.after_save (Action) ===============
    let post = repository::get_admin_post(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let after_save_ctx = HookContext {
        hook_name: "post.after_save".into(),
        data: HookData::PostAfterSave(PostAfterSaveData {
            post_id: id.to_string(),
            title: post.title.clone(),
            slug: post.slug.clone(),
            is_new: false,
            status: post.status.clone(),
            old_status: Some(old_status.clone()),
        }),
    };
    hook_registry
        .dispatch_action("post.after_save", after_save_ctx)
        .await;

    // =============== Hook: post.after_publish (Action) ===============
    if post.status == "published" && old_status != "published" {
        let publish_ctx = HookContext {
            hook_name: "post.after_publish".into(),
            data: HookData::PostAfterPublish(PostAfterPublishData {
                post_id: id.to_string(),
                title: post.title.clone(),
                slug: post.slug.clone(),
                old_status,
                new_status: "published".to_string(),
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
    Ok(serde_json::json!({ "deleted": true }))
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
