use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::{
    modules::post::post_types::ContentType,
    shared::{
        error::{AppError, AppResult},
    },
    state::AppState,
};

use crate::modules::theme::{
    context::TemplateContext, engine,
    dto::ArchivePageQuery,
};
use crate::modules::plugin::hook::{HookContext, HookData, PostBeforeRenderData};

pub async fn render_home(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_home",
        client_request_id = %client_request_id,
        authenticated = auth.is_some(),
        "rendering home page"
    );
    let ctx = TemplateContext::load(&state).await?;

    // 查询最近 20 篇公开文章，用于首页 SEO + 服务端渲染
    let recent_posts =
        crate::modules::post::repository::list_public_posts(&state.pool, None, 20, 0)
            .await
            .unwrap_or_default();

    // 兜底：如果数据库 site_url 为空，从 Host header 推断
    let fallback_site_url = crate::modules::seo::infer_site_url_from_host_header(&headers);
    let effective_site_url = if ctx.site_url.trim().is_empty() {
        &fallback_site_url
    } else {
        &ctx.site_url
    };

    let seo_meta = crate::modules::seo::meta::build_home_meta(
        &ctx.site_title,
        &ctx.site_description,
        effective_site_url,
        "", // seo_keywords
        "",
    );
    let json_ld = crate::modules::seo::meta::build_home_json_ld(
        &ctx.site_title,
        &ctx.site_description,
        effective_site_url,
    );

    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;
    let tmpl = env
        .get_template("index.html")
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context!(
            seo_meta => seo_meta,
            json_ld => json_ld,
            current_user => auth,
            posts => recent_posts
        ))
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

/// 标签列表页：/tags
pub async fn render_tags_list(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_tags_list",
        client_request_id = %client_request_id,
        authenticated = auth.is_some(),
        "rendering tags list page"
    );

    let tags = crate::modules::tag::repository::get_all_tags_with_count(&state.pool)
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;

    let ctx = TemplateContext::load(&state).await?;

    let fallback_site_url = crate::modules::seo::infer_site_url_from_host_header(&headers);
    let effective_site_url = if ctx.site_url.trim().is_empty() {
        &fallback_site_url
    } else {
        &ctx.site_url
    };

    let page_title = format!("标签 - {}", ctx.site_title);
    let seo_description = format!("浏览所有标签，共 {} 个", tags.len());
    let seo_meta = crate::modules::seo::meta::build_home_meta(
        &page_title,
        &seo_description,
        effective_site_url,
        "",
        "",
    );
    let json_ld = crate::modules::seo::meta::build_home_json_ld(
        &page_title,
        &seo_description,
        effective_site_url,
    );

    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;

    let template_name = if env.get_template("tags.html").is_ok() {
        "tags.html"
    } else {
        tracing::warn!(
            module = "theme",
            event = "tags_list_template_not_found",
            "tags.html not found, falling back to index.html"
        );
        "index.html"
    };

    let tmpl = env
        .get_template(template_name)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {
            tags => tags,
            posts => Vec::<minijinja::Value>::new(),
            seo_meta => seo_meta,
            json_ld => json_ld,
            current_user => auth,
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

/// 分类列表页：/categories
pub async fn render_categories_list(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_categories_list",
        client_request_id = %client_request_id,
        authenticated = auth.is_some(),
        "rendering categories list page"
    );

    let categories =
        crate::modules::category::repository::get_all_categories_with_count(&state.pool)
            .await
            .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;

    let ctx = TemplateContext::load(&state).await?;

    let fallback_site_url = crate::modules::seo::infer_site_url_from_host_header(&headers);
    let effective_site_url = if ctx.site_url.trim().is_empty() {
        &fallback_site_url
    } else {
        &ctx.site_url
    };

    let page_title = format!("分类 - {}", ctx.site_title);
    let seo_description = format!("浏览所有分类，共 {} 个", categories.len());
    let seo_meta = crate::modules::seo::meta::build_home_meta(
        &page_title,
        &seo_description,
        effective_site_url,
        "",
        "",
    );
    let json_ld = crate::modules::seo::meta::build_home_json_ld(
        &page_title,
        &seo_description,
        effective_site_url,
    );

    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;

    let template_name = if env.get_template("categories.html").is_ok() {
        "categories.html"
    } else {
        tracing::warn!(
            module = "theme",
            event = "categories_list_template_not_found",
            "categories.html not found, falling back to index.html"
        );
        "index.html"
    };

    let tmpl = env
        .get_template(template_name)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {
            categories => categories,
            posts => Vec::<minijinja::Value>::new(),
            seo_meta => seo_meta,
            json_ld => json_ld,
            current_user => auth,
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

/// 搜索页：/search?keyword=xxx&page=1&page_size=20
///
/// 复用 `search_posts()` / `count_search_posts()` 仓库函数实现 FTS5 全文搜索，
/// 支持关键词搜索和分页。模板优先选择 search.html，不存在时回退 index.html。
pub async fn render_search(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
    Query(query): Query<SearchPageQuery>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_search",
        client_request_id = %client_request_id,
        keyword = ?query.keyword,
        authenticated = auth.is_some(),
        "rendering search page"
    );

    let keyword = query.keyword.trim().to_string();
    let (page_i64, page_size_i64, offset) = query.pagination.normalized(20, 100);
    let page = page_i64 as u32;
    let page_size = page_size_i64 as u32;

    let (posts, total) = if !keyword.is_empty() {
        let posts = crate::modules::post::repository::search_posts(
            &state.pool,
            &keyword,
            None,
            None,
            page_size_i64,
            offset,
        )
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;
        let total = crate::modules::post::repository::count_search_posts(
            &state.pool,
            &keyword,
            None,
            None,
        )
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;
        (posts, total)
    } else {
        (Vec::new(), 0i64)
    };

    let total_u32 = if total > 0 { total as u32 } else { 0u32 };
    let total_pages = if total > 0 {
        ((total as f64) / (page_size_i64 as f64)).ceil() as u32
    } else {
        0u32
    };

    // 加载模板上下文，失败时降级返回 404 HTML（与标签/分类归档保持一致）
    let ctx = match TemplateContext::load(&state).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                module = "theme",
                event = "search_template_context_load_failed",
                error = %e,
                "failed to load template context, falling back to 404"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
    };

    // 兜底：如果数据库 site_url 为空，从 Host header 推断
    let fallback_site_url = crate::modules::seo::infer_site_url_from_host_header(&headers);
    let effective_site_url = if ctx.site_url.trim().is_empty() {
        &fallback_site_url
    } else {
        &ctx.site_url
    };

    // 构建搜索页 SEO meta
    let page_title = if keyword.is_empty() {
        format!("搜索 - {}", ctx.site_title)
    } else {
        format!("搜索: {} - {}", keyword, ctx.site_title)
    };
    let seo_description = if keyword.is_empty() {
        format!("在 {} 中搜索文章", ctx.site_title)
    } else {
        format!("搜索「{}」的结果，共 {} 篇", keyword, total)
    };
    let seo_meta = crate::modules::seo::meta::build_home_meta(
        &page_title,
        &seo_description,
        effective_site_url,
        "",
        "",
    );
    let json_ld = crate::modules::seo::meta::build_home_json_ld(
        &page_title,
        &seo_description,
        effective_site_url,
    );

    // 构建模板引擎
    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;

    // 模板优先 search.html，不存在时回退 index.html（加 warn 日志）
    let template_name = if env.get_template("search.html").is_ok() {
        "search.html"
    } else {
        tracing::warn!(
            module = "theme",
            event = "search_template_not_found",
            "search.html not found, falling back to index.html"
        );
        "index.html"
    };

    let tmpl = env
        .get_template(template_name)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {
            keyword => keyword,
            posts => posts,
            page => page,
            page_size => page_size,
            total => total_u32,
            total_pages => total_pages,
            seo_meta => seo_meta,
            json_ld => json_ld,
            current_user => auth,
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

/// 搜索页查询参数
#[derive(Debug, Deserialize)]
pub struct SearchPageQuery {
    #[serde(default)]
    pub keyword: String,
    #[serde(flatten)]
    pub pagination: crate::shared::pagination::PaginationQuery,
}

/// 作者归档页：/author/{username}
pub async fn render_author_archive(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
    Path(username): Path<String>,
    Query(query): Query<ArchivePageQuery>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_author_archive",
        client_request_id = %client_request_id,
        username = %username,
        authenticated = auth.is_some(),
        "rendering author archive page"
    );

    // 1. 查询作者公开信息
    let author = match crate::modules::user::repository::find_public_by_username(
        &state.pool,
        &username,
    )
    .await
    {
        Ok(Some(a)) => a,
        Ok(None) => {
            tracing::warn!(
                module = "theme",
                event = "render_author_archive_not_found",
                username = %username,
                "author not found"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
        Err(e) => {
            tracing::error!(
                module = "theme",
                event = "author_query_error",
                username = %username,
                error = %e,
                "database error when querying author"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
    };

    // 2. 分页参数（默认第 1 页，每页 20 条）
    let page = query.page;
    let page_size = 20u32;

    // 3. 查询该作者的文章列表
    let posts = crate::modules::post::repository::list_posts_by_author_username(
        &state.pool,
        &username,
        page,
        page_size,
    )
    .await
    .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;

    // 4. 统计总数
    let total =
        crate::modules::post::repository::count_posts_by_author_username(&state.pool, &username)
            .await
            .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;

    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;

    // 5. 加载模板上下文，失败时降级返回 404 HTML
    let ctx = match TemplateContext::load(&state).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                module = "theme",
                event = "author_archive_template_context_load_failed",
                username = %username,
                error = %e,
                "failed to load template context"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
    };

    // 6. 兜底：如果数据库 site_url 为空，从 Host header 推断
    let fallback_site_url = crate::modules::seo::infer_site_url_from_host_header(&headers);
    let effective_site_url = if ctx.site_url.trim().is_empty() {
        &fallback_site_url
    } else {
        &ctx.site_url
    };

    // 7. SEO meta
    let page_title = format!("{} - 作者归档 | {}", author.display_name, ctx.site_title);
    let fallback_bio = format!("{} 发布的文章，共 {} 篇", author.display_name, total);
    let description = author.bio.as_deref().unwrap_or(&fallback_bio);
    let seo_meta = crate::modules::seo::meta::build_home_meta(
        &page_title,
        description,
        effective_site_url,
        "",
        "",
    );
    let json_ld = crate::modules::seo::meta::build_home_json_ld(
        &page_title,
        description,
        effective_site_url,
    );

    // 8. 构建模板引擎
    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;

    // 9. 选择模板：优先 author.html，回退到 index.html
    let template_name = if env.get_template("author.html").is_ok() {
        "author.html"
    } else {
        tracing::warn!(
            module = "theme",
            event = "author_template_not_found",
            username = %username,
            "author.html not found, falling back to index.html"
        );
        "index.html"
    };

    let tmpl = env
        .get_template(template_name)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    // 10. 渲染模板
    let rendered = tmpl
        .render(minijinja::context! {
            author => author,
            posts => posts,
            page => page,
            page_size => page_size,
            total => total,
            total_pages => total_pages,
            seo_meta => seo_meta,
            json_ld => json_ld,
            current_user => auth,
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

pub async fn render_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_post",
        client_request_id = %client_request_id,
        slug = %slug,
        authenticated = auth.is_some(),
        "rendering public post"
    );

    // ── Check if this is a page with custom_html render mode → redirect to /pages/:slug ──
    let page_info = crate::modules::post::repository::get_page_by_slug(&state.pool, &slug).await?;
    if let Some(ref p) = page_info {
        if p.content_type == ContentType::Page && p.page_render_mode == "custom_html" {
            tracing::info!(
                module = "theme",
                event = "redirect_page_to_custom",
                slug = %slug,
                "redirecting /posts/{} to /pages/{}", slug, slug
            );
            return Ok(Redirect::temporary(&format!("/pages/{}", slug)).into_response());
        }
    }

    let post =
        crate::modules::post::repository::get_public_post_by_slug(&state.pool, &slug).await?;
    let p = match post {
        Some(p) => p,
        None => {
            tracing::warn!(
                module = "theme",
                event = "render_post_not_found",
                client_request_id = %client_request_id,
                slug = %slug,
                "public post not found"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
    };

    let ctx = TemplateContext::load(&state).await?;

    // 兜底：如果数据库 site_url 为空，从 Host header 推断
    let fallback_site_url = crate::modules::seo::infer_site_url_from_host_header(&headers);
    let effective_site_url = if ctx.site_url.trim().is_empty() {
        &fallback_site_url
    } else {
        &ctx.site_url
    };

    let hook_registry = state.plugin_manager.read().await.hook_registry().clone();
    let mut render_ctx = HookContext {
        hook_name: "post.before_render".into(),
        data: HookData::PostBeforeRender(PostBeforeRenderData {
            post_id: p.id.clone(),
            title: p.title.clone(),
            slug: p.slug.clone(),
            content_html: p.content_html.clone(),
            extra: std::collections::HashMap::new(),
        }),
    };
    hook_registry
        .dispatch_filter_best_effort("post.before_render", &mut render_ctx)
        .await;
    let plugin_extra = if let HookData::PostBeforeRender(ref data) = render_ctx.data {
        data.extra.clone()
    } else {
        std::collections::HashMap::new()
    };

    let og_image = p
        .cover_media_id
        .as_ref()
        .map(|id| format!("{}/uploads/{}", effective_site_url, id))
        .unwrap_or_default();

    let seo_meta = crate::modules::seo::meta::build_post_meta_with_content_type(
        &ctx.site_title,
        effective_site_url,
        &p.title,
        &p.slug,
        p.excerpt.as_deref(),
        &p.content_html,
        "", // seo_keywords
        &og_image,
        p.content_type,
    );

    let json_ld = crate::modules::seo::meta::build_post_json_ld_with_content_type(
        &ctx.site_title,
        effective_site_url,
        &p.title,
        &p.slug,
        p.excerpt.as_deref().unwrap_or(""),
        &p.author_display_name,
        p.published_at.as_deref(),
        &p.updated_at,
        p.content_type,
    );

    let comments = crate::modules::comment::repository::list_approved_for_post(&state.pool, &p.id)
        .await
        .unwrap_or_default();

    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let env = engine::build_template_engine(
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
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {
            post => p,
            seo_meta => seo_meta,
            json_ld => json_ld,
            image => og_image,
            comments => comments,
            current_user => auth,
            plugins => plugin_extra,
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

pub async fn serve_active_static(
    State(state): State<Arc<AppState>>,
    Path((theme_slug, file_path)): Path<(String, String)>,
) -> impl IntoResponse {
    // L-6: 白名单校验替代黑名单
    if !is_safe_static_path(&file_path) {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }

    // ── Security: validate theme_slug is a legitimate installed theme ──
    let theme_manifest_path = state.theme_dir.join(&theme_slug).join("theme.toml");
    if !theme_manifest_path.exists() || !theme_manifest_path.is_file() {
        tracing::warn!(
            module = "theme",
            event = "static_theme_slug_invalid",
            theme_slug = %theme_slug,
            "requested static file for non-existent theme slug"
        );
        return (StatusCode::NOT_FOUND, "404 Not Found").into_response();
    }

    let full_path = state
        .theme_dir
        .join(&theme_slug)
        .join("static")
        .join(&file_path);

    let ext = full_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let mime = match ext {
        "css" => "text/css",
        "js" => "application/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    };

    match tokio::fs::read(&full_path).await {
        Ok(d) => {
            let mut resp = (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                d,
            )
                .into_response();
            apply_svg_sandbox_csp_if_svg(&mut resp);
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

pub async fn serve_upload_static(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(file_path): Path<String>,
) -> impl IntoResponse {
    // L-6: 白名单校验替代黑名单
    if !is_safe_static_path(&file_path) {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }

    let full_path = state.upload_dir.join(&file_path);
    let ext = full_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let mime = match ext {
        "css" => "text/css",
        "js" => "application/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
            "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        _ => "application/octet-stream",
    };

    // ── 内容协商：浏览器支持 WebP 且存在 .webp 版本 → 返回 WebP ──
    let accept_header = headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()).unwrap_or("");
    let supports_webp = accept_header.contains("image/webp");
    let webp_path_str = format!("{}.webp", full_path.display());
    let webp_exists = supports_webp && tokio::fs::metadata(&webp_path_str).await.is_ok();

    if webp_exists {
        match tokio::fs::read(&webp_path_str).await {
            Ok(d) => {
                return (
                    [
                        (header::CONTENT_TYPE, "image/webp"),
                        (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                    ],
                    d,
                )
                    .into_response();
            }
            Err(_) => {
                // WebP 文件读取失败，回退到原文件
            }
        }
    }

    match tokio::fs::read(&full_path).await {
        Ok(d) => {
            let mut resp = (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                d,
            )
                .into_response();
            apply_svg_sandbox_csp_if_svg(&mut resp);
            resp
        }
        Err(_) => {
            let is_image = matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp");
            if is_image {
                tracing::warn!(
                    module = "theme",
                    event = "upload_static_not_found_fallback",
                    file_path = %file_path,
                    "upload static file missing, returning placeholder image"
                );
                let placeholder = r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360"><rect width="640" height="360" fill="#f3f4f6"/><g fill="none" stroke="#d1d5db" stroke-width="2"><rect x="180" y="92" width="280" height="176" rx="12"/><path d="M210 236l72-74 52 52 44-40 52 62"/></g><circle cx="262" cy="150" r="16" fill="#d1d5db"/><text x="320" y="300" font-size="18" font-family="sans-serif" text-anchor="middle" fill="#6b7280">Media Not Found</text></svg>"##;
                let mut resp = ([(header::CONTENT_TYPE, "image/svg+xml")], placeholder).into_response();
                apply_svg_sandbox_csp_if_svg(&mut resp);
                return resp;
            }

            (StatusCode::NOT_FOUND, "404 Not Found").into_response()
        }
    }
}

pub async fn serve_plugin_static(
    State(state): State<Arc<AppState>>,
    Path((plugin_slug, file_path)): Path<(String, String)>,
) -> impl IntoResponse {
    // L-6: 白名单校验替代黑名单
    if !is_safe_static_path(&file_path) {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }

    // 检查插件是否启用
    let Ok(enabled) = crate::modules::plugin::status::get_enabled_ids(&state.pool).await else {
        return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
    };
    if !enabled.contains(&plugin_slug) {
        return (StatusCode::NOT_FOUND, "404 Not Found").into_response();
    }

    let plugins_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let full_path = plugins_dir
        .join(&plugin_slug)
        .join("static")
        .join(&file_path);

    let ext = full_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let mime = match ext {
        "css" => "text/css",
        "js" => "application/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    };

    match tokio::fs::read(&full_path).await {
        Ok(d) => {
            let mut resp = (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                d,
            )
                .into_response();
            apply_svg_sandbox_csp_if_svg(&mut resp);
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

pub async fn serve_global_static(
    State(state): State<Arc<AppState>>,
    Path(file_path): Path<String>,
) -> impl IntoResponse {
    // L-6: 白名单校验替代黑名单
    if !is_safe_static_path(&file_path) {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }

    let full_path = state.static_dir.join(&file_path);

    let ext = full_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let mime = match ext {
        "css" => "text/css",
        "js" => "application/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    };

    match tokio::fs::read(&full_path).await {
        Ok(d) => {
            let mut resp = (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                d,
            )
                .into_response();
            apply_svg_sandbox_csp_if_svg(&mut resp);
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

/// 标签归档页：/tags/{slug}
pub async fn render_tag_archive(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
    Path(slug): Path<String>,
    Query(query): Query<ArchivePageQuery>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_tag_archive",
        client_request_id = %client_request_id,
        slug = %slug,
        authenticated = auth.is_some(),
        "rendering tag archive page"
    );

    // 1. 获取标签信息
    let tag = match crate::modules::tag::repository::get_by_slug(&state.pool, &slug).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            tracing::warn!(
                module = "theme",
                event = "render_tag_archive_not_found",
                slug = %slug,
                "tag not found"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
        Err(e) => {
            tracing::error!(
                module = "theme",
                event = "tag_query_error",
                slug = %slug,
                error = %e,
                "database error when querying tag"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
    };

    // 2. 分页参数（默认第 1 页，每页 20 条）
    let page = query.page;
    let page_size = 20u32;

    // 3. 查询该标签下的文章列表
    let posts = crate::modules::post::repository::list_posts_by_tag_slug(
        &state.pool,
        &slug,
        page,
        page_size,
    )
    .await
    .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;

    // 4. 统计总数
    let total = crate::modules::post::repository::count_posts_by_tag_slug(&state.pool, &slug)
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;

    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;

    // 5. 加载模板上下文
    let ctx = match TemplateContext::load(&state).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                module = "theme",
                event = "tag_archive_template_context_load_failed",
                slug = %slug,
                error = %e,
                "failed to load template context"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
    };

    // 6. 兜底：如果数据库 site_url 为空，从 Host header 推断
    let fallback_site_url = crate::modules::seo::infer_site_url_from_host_header(&headers);
    let effective_site_url = if ctx.site_url.trim().is_empty() {
        &fallback_site_url
    } else {
        &ctx.site_url
    };

    // 7. SEO meta
    let page_title = format!("{} - {}", tag.name, ctx.site_title);
    let seo_meta = crate::modules::seo::meta::build_home_meta(
        &page_title,
        &format!("标签 {} 下的所有文章", tag.name),
        effective_site_url,
        "",
        "",
    );
    let json_ld = crate::modules::seo::meta::build_home_json_ld(
        &page_title,
        &format!("标签 {} 下的所有文章", tag.name),
        effective_site_url,
    );

    // 8. 构建模板引擎
    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;

    // 9. 选择模板：优先 tag.html，回退到 index.html
    let template_name = if env.get_template("tag.html").is_ok() {
        "tag.html"
    } else {
        tracing::warn!(
            module = "theme",
            event = "tag_template_not_found",
            slug = %slug,
            "tag.html not found, falling back to index.html"
        );
        "index.html"
    };

    let tmpl = env
        .get_template(template_name)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    // 10. 渲染模板
    let rendered = tmpl
        .render(minijinja::context! {
            tag => tag,
            posts => posts,
            page => page,
            page_size => page_size,
            total => total,
            total_pages => total_pages,
            seo_meta => seo_meta,
            json_ld => json_ld,
            current_user => auth,
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

/// 分类归档页：/categories/{slug}
pub async fn render_category_archive(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    auth: Option<crate::shared::auth::AuthUser>,
    Path(slug): Path<String>,
    Query(query): Query<ArchivePageQuery>,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_category_archive",
        client_request_id = %client_request_id,
        slug = %slug,
        authenticated = auth.is_some(),
        "rendering category archive page"
    );

    // 1. 获取分类信息
    let category = match crate::modules::category::repository::get_by_slug(&state.pool, &slug).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::warn!(
                module = "theme",
                event = "render_category_archive_not_found",
                slug = %slug,
                "category not found"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
        Err(e) => {
            tracing::error!(
                module = "theme",
                event = "category_query_error",
                slug = %slug,
                error = %e,
                "database error when querying category"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
    };

    // 2. 分页参数（默认第 1 页，每页 20 条）
    let page = query.page;
    let page_size = 20u32;

    // 3. 查询该分类下的文章列表
    let posts = crate::modules::post::repository::list_posts_by_category_slug(
        &state.pool,
        &slug,
        page,
        page_size,
    )
    .await
    .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;

    // 4. 统计总数
    let total = crate::modules::post::repository::count_posts_by_category_slug(&state.pool, &slug)
        .await
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Database error: {}", e)))?;

    let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;

    // 5. 加载模板上下文
    let ctx = match TemplateContext::load(&state).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                module = "theme",
                event = "category_archive_template_context_load_failed",
                slug = %slug,
                error = %e,
                "failed to load template context"
            );
            return Ok(render_404_page(&state, &headers).await);
        }
    };

    // 6. 兜底：如果数据库 site_url 为空，从 Host header 推断
    let fallback_site_url = crate::modules::seo::infer_site_url_from_host_header(&headers);
    let effective_site_url = if ctx.site_url.trim().is_empty() {
        &fallback_site_url
    } else {
        &ctx.site_url
    };

    // 7. SEO meta
    let page_title = format!("{} - {}", category.name, ctx.site_title);
    let page_description = category
        .description
        .clone()
        .unwrap_or_else(|| format!("分类 {} 下的所有文章", category.name));
    let seo_meta = crate::modules::seo::meta::build_home_meta(
        &page_title,
        &page_description,
        effective_site_url,
        "",
        "",
    );
    let json_ld = crate::modules::seo::meta::build_home_json_ld(
        &page_title,
        &page_description,
        effective_site_url,
    );

    // 8. 构建模板引擎
    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;

    // 9. 选择模板：优先 category.html，回退到 index.html
    let template_name = if env.get_template("category.html").is_ok() {
        "category.html"
    } else {
        tracing::warn!(
            module = "theme",
            event = "category_template_not_found",
            slug = %slug,
            "category.html not found, falling back to index.html"
        );
        "index.html"
    };

    let tmpl = env
        .get_template(template_name)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    // 10. 渲染模板
    let rendered = tmpl
        .render(minijinja::context! {
            category => category,
            posts => posts,
            page => page,
            page_size => page_size,
            total => total,
            total_pages => total_pages,
            seo_meta => seo_meta,
            json_ld => json_ld,
            current_user => auth,
        })
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

/// Cookie 政策页面：/cookie-policy
///
/// 渲染主题模板 `cookie-policy.html`，纯静态内容页面，无需认证。
/// 模板中使用 `site_title`、`current_lang` 等 TemplateContext 内置变量。
pub async fn render_cookie_policy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let client_request_id =
        crate::shared::request_id::extract_or_generate_client_request_id(&headers);
    tracing::info!(
        module = "theme",
        event = "render_cookie_policy",
        client_request_id = %client_request_id,
        "rendering cookie policy page"
    );

    let ctx = TemplateContext::load(&state).await?;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(&headers);
    let plugin_guard = state.plugin_manager.read().await;
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;
    let tmpl = env
        .get_template("cookie-policy.html")
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context! {})
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_THEME_HTML,
    );
    Ok(response)
}

/// catch-all 回退路由：未匹配到任何路径时渲染 404 页面
pub async fn fallback_404(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    render_404_page(&state, &headers).await
}

/// 渲染 404 错误页面
async fn render_404_page(
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Response {
    match try_render_error_template(state, headers, "404.html").await {
        Ok(html) => {
            let mut response = (StatusCode::NOT_FOUND, Html(html)).into_response();
            crate::shared::security::mark_response_security_profile(
                &mut response,
                crate::shared::security::SECURITY_PROFILE_THEME_HTML,
            );
            response
        }
        Err(e) => {
            tracing::error!(
                module = "theme",
                event = "render_404_template_failed",
                error = %e,
                "Failed to render 404.html, falling back to plain text"
            );
            (StatusCode::NOT_FOUND, "404 - 页面未找到").into_response()
        }
    }
}

/// 尝试渲染错误模板（404.html 或 500.html）
async fn try_render_error_template(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    template_name: &str,
) -> AppResult<String> {
    let ctx = TemplateContext::load(state).await?;
    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::resolve_language_from_headers(headers);
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(&current_lang),
    )
    .await?;
    
    let tmpl = env
        .get_template(template_name)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;
    
    let rendered = tmpl
        .render(minijinja::context! {})
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;
    
    Ok(rendered)
}

/// L-6: 白名单校验静态文件路径安全性
///
/// 规则：
/// 1. 非空，不以 `/` 或 `\` 开头
/// 2. 每个字符只允许：字母、数字、`-`、`_`、`.`、`/`
/// 3. 路径组件中不允许 `.` 或 `..`
/// 4. 使用 depth 追踪确保不会遍历到 base 之外
fn is_safe_static_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    // 拒绝绝对路径
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }

    // 白名单：只允许安全字符
    if !path.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/') {
        return false;
    }

    // 按组件检查，不允许 . 或 .. 组件，且 depth 不能为负
    let mut depth: i32 = 0;
    for component in path.split('/') {
        if component == "." || component == "" {
            continue;
        }
        if component == ".." {
            depth -= 1;
            if depth < 0 {
                return false;
            }
        } else {
            depth += 1;
        }
    }

    // 至少要有一个有效文件组件
    depth > 0
}

/// M-4: 对 SVG 响应添加 Content-Security-Policy: sandbox header
/// 防止浏览器执行 SVG 内嵌的 JavaScript
fn apply_svg_sandbox_csp_if_svg(response: &mut Response) {
    let is_svg = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("image/svg+xml"))
        .unwrap_or(false);

    if is_svg {
        response.headers_mut().insert(
            "content-security-policy",
            header::HeaderValue::from_static("sandbox"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L-6: 路径遍历攻击 — 常规 `..` 应被拒绝
    #[test]
    fn security_fix_l6_rejects_dot_dot_traversal() {
        assert!(!is_safe_static_path("../etc/passwd"));
        assert!(!is_safe_static_path("foo/../../../etc/passwd"));
    }

    /// L-6: 路径遍历攻击 — 混合编码应被拒绝
    #[test]
    fn security_fix_l6_rejects_backslash_traversal() {
        assert!(!is_safe_static_path("foo\\..\\..\\etc\\passwd"));
        assert!(!is_safe_static_path("..\\etc\\passwd"));
    }

    /// L-6: 路径遍历攻击 — 绝对路径应被拒绝
    #[test]
    fn security_fix_l6_rejects_absolute_path() {
        assert!(!is_safe_static_path("/etc/passwd"));
        assert!(!is_safe_static_path("\\windows\\system32"));
    }

    /// L-6: 路径遍历攻击 — 隐藏文件应被拒绝（. 开头的组件）
    #[test]
    fn security_fix_l6_rejects_hidden_file_access() {
        // `.hidden` 本身不是 `.` 或 `..`，但只包含点+字母 → 应该允许（合法文件名）
        // 但 `.` 单独组件应被跳过（当前目录引用）
        // 这里测试空路径和纯点
        assert!(!is_safe_static_path(""));
        assert!(!is_safe_static_path("."));
        assert!(!is_safe_static_path(".."));
    }

    /// L-6: 合法路径应被允许
    #[test]
    fn security_fix_l6_allows_valid_path() {
        assert!(is_safe_static_path("css/style.css"));
        assert!(is_safe_static_path("js/app.min.js"));
        assert!(is_safe_static_path("images/logo.png"));
        assert!(is_safe_static_path("fonts/roboto-regular.woff2"));
    }

    /// L-6: 路径遍历攻击 — URL 编码形式的点号（双重编码防御）
    #[test]
    fn security_fix_l6_rejects_percent_encoded_traversal() {
        // %2e 是 URL 编码的 `.`，但 Path extractor 已解码
        // 确保解码后的 `..` 被拦截
        assert!(!is_safe_static_path("%2e%2e/etc/passwd"));
        assert!(!is_safe_static_path("%2e%2e%2fetc%2fpasswd"));
    }

    /// M-4: SVG 响应必须包含 Content-Security-Policy: sandbox header
    /// 防止浏览器执行 SVG 内嵌的 JavaScript
    #[test]
    fn security_fix_m4_svg_response_has_csp_sandbox() {
        use axum::response::IntoResponse;

        // 模拟 serve_active_static 中 SVG 文件的响应逻辑
        let mime = "image/svg+xml";
        let data = b"<svg xmlns='http://www.w3.org/2000/svg'><rect/></svg>";
        let mut response = (
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            data.to_vec(),
        )
            .into_response();

        // 应用 M-4 修复：SVG 响应需要 sandbox CSP
        apply_svg_sandbox_csp_if_svg(&mut response);

        let headers = response.headers();
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap().to_str().unwrap(),
            "image/svg+xml",
            "Content-Type should be image/svg+xml"
        );
        assert!(
            headers
                .get("content-security-policy")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("sandbox"))
                .unwrap_or(false),
            "SVG response must include Content-Security-Policy with sandbox directive"
        );
    }

    /// M-4: 非 SVG 响应不应被添加 sandbox CSP
    #[test]
    fn security_fix_m4_non_svg_response_no_sandbox() {
        use axum::response::IntoResponse;

        let mime = "image/png";
        let data = b"\x89PNG\r\n";
        let mut response = (
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            data.to_vec(),
        )
            .into_response();

        apply_svg_sandbox_csp_if_svg(&mut response);

        let headers = response.headers();
        // 非 SVG 不应有 sandbox CSP
        let has_sandbox = headers
            .get("content-security-policy")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.contains("sandbox"))
            .unwrap_or(false);
        assert!(
            !has_sandbox,
            "Non-SVG response should not have sandbox CSP"
        );
    }
}
