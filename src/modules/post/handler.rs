use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, Query, State},
    response::IntoResponse,
    Json,
};

use crate::{
    modules::{seo, theme::context::TemplateContext},
    shared::{
        auth::AdminUser,
        error::{AppError, AppResult},
        response::{ApiResponse, PaginatedResponse},
    },
    state::AppState,
};

use super::{
    dto::{AdminPostResponse, CreatePostRequest, PostQuery, PublicPostResponse, SearchQuery},
    service,
};

pub async fn list_public_posts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PostQuery>,
) -> AppResult<Json<ApiResponse<PaginatedResponse<super::domain::PublicPostSummary>>>> {
    Ok(Json(ApiResponse::success(
        service::list_public_posts(state, query).await?,
    )))
}

pub async fn get_public_post(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> AppResult<Json<ApiResponse<PublicPostResponse>>> {
    Ok(Json(ApiResponse::success(
        service::get_public_post(state, &slug).await?,
    )))
}

/// GET /api/search — FTS5 full-text search for public posts
pub async fn search_posts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<ApiResponse<PaginatedResponse<super::domain::PublicPostSummary>>>> {
    Ok(Json(ApiResponse::success(
        service::search_posts(state, query).await?,
    )))
}

pub async fn list_admin_posts(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Query(query): Query<PostQuery>,
) -> AppResult<Json<ApiResponse<PaginatedResponse<AdminPostResponse>>>> {
    Ok(Json(ApiResponse::success(
        service::list_admin_posts(state, query).await?,
    )))
}

pub async fn get_admin_post(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<AdminPostResponse>>> {
    Ok(Json(ApiResponse::success(
        service::get_admin_post(state, &id).await?,
    )))
}

pub async fn create_post(
    State(state): State<Arc<AppState>>,
    admin: AdminUser,
    Json(body): Json<CreatePostRequest>,
) -> AppResult<Json<ApiResponse<AdminPostResponse>>> {
    Ok(Json(ApiResponse::success(
        service::create_post(state, &admin.0, body).await?,
    )))
}

pub async fn update_post(
    State(state): State<Arc<AppState>>,
    admin: AdminUser,
    Path(id): Path<String>,
    Json(body): Json<super::dto::UpdatePostRequest>,
) -> AppResult<Json<ApiResponse<AdminPostResponse>>> {
    Ok(Json(ApiResponse::success(
        service::update_post(state, &admin.0, &id, body).await?,
    )))
}

pub async fn delete_post(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    Ok(Json(ApiResponse::success(
        service::delete_post(state, &id).await?,
    )))
}

/// POST /api/v1/admin/pages/upload — Upload custom HTML/ZIP for a page
pub async fn upload_custom_page(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    mut multipart: Multipart,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let mut slug: Option<String> = None;
    let mut file_data: Option<(String, Option<String>, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        AppError::Multipart(format!("multipart field error: {}", e))
    })? {
        match field.name() {
            Some("slug") => {
                slug = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("failed to read slug: {}", e))
                })?);
            }
            Some("file") => {
                let filename = field.file_name().unwrap_or("untitled").to_string();
                let ct = field.content_type().map(|s| s.to_string());
                let data = field.bytes().await.map_err(|e| {
                    AppError::BadRequest(format!("failed to read file: {}", e))
                })?.to_vec();
                file_data = Some((filename, ct, data));
            }
            _ => {}
        }
    }

    let slug = slug
        .filter(|s| !s.trim().is_empty())
        .ok_or(AppError::BadRequest("slug field is required".into()))?;
    let (filename, ct, data) = file_data
        .ok_or(AppError::BadRequest("file field is required".into()))?;

    let path = service::upload_custom_page(state, &slug, filename, ct, data).await?;
    Ok(Json(ApiResponse::success(serde_json::json!({
        "custom_html_path": path
    }))))
}

/// GET /pages/:slug — Render page based on page_render_mode
/// - "custom_html" → serve custom HTML/ZIP directly
/// - "editor" → render via theme template (like a post, but using page template)
pub async fn render_custom_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> AppResult<axum::response::Response> {
    let page = super::repository::get_page_by_slug(&state.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;

    if page.content_type != "page" || page.status != "published" || page.visibility != "public" {
        return Err(AppError::NotFound);
    }

    match page.page_render_mode.as_str() {
        "custom_html" => {
            // Serve custom HTML file
            let custom_html_path = page.custom_html_path.ok_or(AppError::NotFound)?;
            let index_path = state.upload_dir.join(&custom_html_path).join("index.html");
            if !index_path.exists() {
                return Err(AppError::NotFound);
            }
            let content = tokio::fs::read_to_string(&index_path).await?;
            let mut response = axum::response::Html(content).into_response();
            crate::shared::security::mark_response_security_profile(
                &mut response,
                crate::shared::security::SECURITY_PROFILE_CUSTOM_HTML,
            );
            Ok(response)
        }
        _ => {
            // "editor" mode — render via theme template using content_html
            let ctx = TemplateContext::load(&state).await?;
            let plugin_guard = state.plugin_manager.read().await;
            let env = crate::modules::theme::engine::build_template_engine(&ctx, &state.theme_dir, &*plugin_guard, &state.template_env_cache).await?;
            let tmpl = env
                .get_template("post.html")
                .map_err(|e| AppError::Anyhow(anyhow::anyhow!("template error: {}", e)))?;

            let description = seo::meta::extract_description(&page.content_html, "");
            let og_image = "";
            let seo_meta = seo::meta::build_post_meta_with_content_type(
                &ctx.site_title, &ctx.site_url,
                &page.title, &slug, Some(description.as_str()), &page.content_html,
                "", og_image, &page.content_type,
            );

            let json_ld = seo::meta::build_post_json_ld_with_content_type(
                &ctx.site_title, &ctx.site_url,
                &page.title, &slug, &description,
                "", None, "", &page.content_type,
            );

            let html = tmpl
                .render(minijinja::context! {
                    site_title => &ctx.site_title,
                    seo_meta => seo_meta,
                    json_ld => json_ld,
                    post => minijinja::context! {
                        title => page.title,
                        content_html => page.content_html,
                        slug => slug,
                        id => page.id,
                        published_at => "",
                        created_at => "",
                        category_name => "",
                        author_display_name => "",
                    },
                    comments => Vec::<serde_json::Value>::new(),
                    current_user => None::<serde_json::Value>,
                })
                .map_err(|e| AppError::Anyhow(anyhow::anyhow!("render error: {}", e)))?;

            let mut response = axum::response::Html(html).into_response();
            crate::shared::security::mark_response_security_profile(
                &mut response,
                crate::shared::security::SECURITY_PROFILE_THEME_HTML,
            );
            Ok(response)
        }
    }
}
