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
    post_types::{ContentType, PostStatus, Visibility},
    service,
};

/// GET /api/v1/posts — 列出公开文章
///
/// 返回 `status=published` 且 `visibility=public` 的文章列表。
/// 按 `pinned DESC, published_at DESC` 排序（置顶文章在前）。
/// 无需认证。
///
/// # Query Parameters
/// - `keyword` (optional): 标题/内容关键词模糊匹配（SQL LIKE，非全文搜索）
///   - 注意：全文搜索请用 `/api/v1/search` 端点
/// - `page` (optional, default: 1): 页码，从 1 开始
/// - `page_size` (optional, default: 10, max: 100): 每页数量
///
/// # Response
/// 返回 `ApiResponse<PaginatedResponse<PublicPostSummary>>`，每个 item 包含：
/// - `id`: 文章 ID
/// - `title`: 标题
/// - `slug`: URL slug
/// - `excerpt`: 摘要
/// - `author_display_name`: 作者显示名
/// - `category_name`: 分类名
/// - `published_at`: 发布时间（ISO 8601）
///
/// # Example
/// ```bash
/// # 列出所有文章
/// curl "http://localhost:2000/api/v1/posts"
///
/// # 关键词过滤 + 分页
/// curl "http://localhost:2000/api/v1/posts?keyword=rust&page=2&page_size=20"
/// ```
pub async fn list_public_posts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PostQuery>,
) -> AppResult<Json<ApiResponse<PaginatedResponse<super::domain::PublicPostSummary>>>> {
    Ok(Json(ApiResponse::success(
        service::list_public_posts(state, query).await?,
    )))
}

/// GET /api/v1/posts/:slug — 获取单篇公开文章详情
///
/// 根据 slug 获取文章完整内容（包含 HTML 渲染后的正文）。
/// 仅返回 `status=published` 且 `visibility=public` 的文章。
/// 无需认证。
///
/// # Path Parameters
/// - `slug`: 文章 URL slug（唯一标识符）
///
/// # Response
/// 返回 `ApiResponse<PublicPostResponse>`，包含：
/// - `post`: 文章完整信息（包括 `content_html` 渲染后的正文）
/// - `tags`: 文章关联的标签列表
///
/// # Errors
/// - 404: 文章不存在、未发布或非公开
///
/// # Example
/// ```bash
/// curl "http://localhost:2000/api/v1/posts/my-first-post"
/// ```
pub async fn get_public_post(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> AppResult<Json<ApiResponse<PublicPostResponse>>> {
    Ok(Json(ApiResponse::success(
        service::get_public_post(state, &slug).await?,
    )))
}

/// GET /api/v1/search — 全文搜索公开文章
///
/// 基于 SQLite FTS5 的全文检索，支持中英文。
/// 仅搜索 `status=published` 且 `visibility=public` 的文章。
/// 无需认证。
///
/// # Query Parameters
/// - `keyword` (required): 搜索关键词，必填
/// - `category_id` (optional): 按分类 ID 过滤
/// - `tag_id` (optional): 按标签 ID 过滤
/// - `page` (optional, default: 1): 页码，从 1 开始
/// - `page_size` (optional, default: 10, max: 100): 每页数量
///
/// # Response
/// ```json
/// {
///   "code": 0,
///   "message": "ok",
///   "data": {
///     "items": [
///       {
///         "id": "...",
///         "title": "...",
///         "slug": "...",
///         "excerpt": "...",
///         "author_display_name": "...",
///         "category_name": "...",
///         "published_at": "2024-01-01T12:00:00Z"
///       }
///     ],
///     "pagination": {
///       "page": 1,
///       "page_size": 10,
///       "total": 42
///     }
///   },
///   "request_id": "..."
/// }
/// ```
///
/// # Example
/// ```bash
/// curl "http://localhost:2000/api/v1/search?keyword=rust&page=1&page_size=20"
/// curl "http://localhost:2000/api/v1/search?keyword=性能优化&category_id=tech"
/// ```
pub async fn search_posts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<ApiResponse<PaginatedResponse<super::domain::PublicPostSummary>>>> {
    Ok(Json(ApiResponse::success(
        service::search_posts(state, query).await?,
    )))
}

/// GET /api/v1/admin/posts — 列出后台文章（管理员）
///
/// 返回所有状态的文章（包括草稿、已发布、私有），用于后台管理。
/// 需要 Admin 权限（通过 `AdminUser` 提取器验证）。
///
/// # Query Parameters
/// - `keyword` (optional): 标题/内容关键词模糊匹配
/// - `status` (optional): 按状态过滤（`draft` | `published`）
/// - `content_type` (optional): 按类型过滤（`post` | `page`）
/// - `page` (optional, default: 1): 页码
/// - `page_size` (optional, default: 10, max: 100): 每页数量
///
/// # Response
/// 返回 `ApiResponse<PaginatedResponse<AdminPostResponse>>`，每个 item 包含：
/// - `post`: 文章完整信息（包括状态、可见性等后台字段）
/// - `tags`: 关联标签列表
///
/// # Authentication
/// 需要携带有效的 session cookie（Admin 角色）。
///
/// # Example
/// ```bash
/// # 列出所有草稿
/// curl -b cookies.txt "http://localhost:2000/api/v1/admin/posts?status=draft"
///
/// # 列出所有页面类型
/// curl -b cookies.txt "http://localhost:2000/api/v1/admin/posts?content_type=page"
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/admin/posts",
    tag = "admin.posts",
    params(PostQuery),
    responses(
        (status = 200, description = "文章列表", body = ApiResponse<PaginatedResponse<AdminPostResponse>>),
        (status = 401, description = "未认证"),
        (status = 403, description = "无管理员权限"),
    ),
    security(("jwt" = []))
)]
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

/// POST /api/v1/admin/posts — 创建新文章（管理员）
///
/// 创建新文章或页面。需要 Admin 权限。
///
/// # Request Body
/// `CreatePostRequest` JSON 对象：
/// - `title` (required): 文章标题
/// - `slug` (optional): URL slug，留空自动生成
/// - `excerpt` (optional): 摘要
/// - `content_md` (optional): Markdown 原文
/// - `cover_media_id` (optional): 封面图 media ID
/// - `status` (optional, default: draft): 状态（`draft` | `published`）
/// - `visibility` (optional, default: public): 可见性（`public` | `private`）
/// - `category_id` (optional): 分类 ID
/// - `tag_ids` (optional): 标签 ID 数组
/// - `allow_comment` (optional, default: true): 是否允许评论
/// - `pinned` (optional, default: false): 是否置顶
/// - `content_type` (optional, default: post): 内容类型（`post` | `page`）
///
/// # Response
/// 返回 `ApiResponse<AdminPostResponse>`，包含新创建的文章完整信息。
///
/// # Example
/// ```bash
/// curl -X POST -b cookies.txt \
///   -H "Content-Type: application/json" \
///   -d '{"title":"我的第一篇文章","content_md":"# Hello\n\n这是正文"}' \
///   http://localhost:2000/api/v1/admin/posts
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/admin/posts",
    tag = "admin.posts",
    request_body = CreatePostRequest,
    responses(
        (status = 201, description = "文章创建成功", body = ApiResponse<AdminPostResponse>),
        (status = 400, description = "参数错误"),
        (status = 401, description = "未认证"),
        (status = 403, description = "无管理员权限"),
    ),
    security(("jwt" = []))
)]
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

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(format!("multipart field error: {}", e)))?
    {
        match field.name() {
            Some("slug") => {
                slug =
                    Some(field.text().await.map_err(|e| {
                        AppError::BadRequest(format!("failed to read slug: {}", e))
                    })?);
            }
            Some("file") => {
                let filename = field.file_name().unwrap_or("untitled").to_string();
                let ct = field.content_type().map(|s| s.to_string());
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("failed to read file: {}", e)))?
                    .to_vec();
                file_data = Some((filename, ct, data));
            }
            _ => {}
        }
    }

    let slug = slug
        .filter(|s| !s.trim().is_empty())
        .ok_or(AppError::BadRequest("slug field is required".into()))?;
    let (filename, ct, data) =
        file_data.ok_or(AppError::BadRequest("file field is required".into()))?;

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
    headers: axum::http::HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<axum::response::Response> {
    let page = super::repository::get_page_by_slug(&state.pool, &slug)
        .await?
        .ok_or(AppError::NotFound(format!("页面 '{}' 未找到", slug)))?;

    if page.content_type != ContentType::Page
        || page.status != PostStatus::Published
        || page.visibility != Visibility::Public
    {
        return Err(AppError::NotFound("页面未找到或不可访问".to_string()));
    }

    match page.page_render_mode.as_str() {
        "custom_html" => {
            // Serve custom HTML file
            let custom_html_path = page.custom_html_path.ok_or(AppError::NotFound("自定义HTML路径未设置".to_string()))?;
            let index_path = state.upload_dir.join(&custom_html_path).join("index.html");
            if !index_path.exists() {
                return Err(AppError::NotFound("自定义HTML文件不存在".to_string()));
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
            let current_lang =
                crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
            let plugin_guard = state.plugin_manager.read().await;
            let env = crate::modules::theme::engine::build_template_engine(
                &ctx,
                &state.theme_dir,
                &*plugin_guard,
                &state.template_env_cache,
                &state.asset_manifest,
                Some(&current_lang),
            )
            .await?;
            let tmpl = env
                .get_template("post.html")
                .map_err(|e| AppError::Anyhow(anyhow::anyhow!("template error: {}", e)))?;

            let description = seo::meta::extract_description(&page.content_html, "");
            let og_image = "";
            let seo_meta = seo::meta::build_post_meta_with_content_type(
                &ctx.site_title,
                &ctx.site_url,
                &page.title,
                &slug,
                Some(description.as_str()),
                &page.content_html,
                "",
                og_image,
                page.content_type,
            );

            let json_ld = seo::meta::build_post_json_ld_with_content_type(
                &ctx.site_title,
                &ctx.site_url,
                &page.title,
                &slug,
                &description,
                "",
                None,
                "",
                page.content_type,
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
