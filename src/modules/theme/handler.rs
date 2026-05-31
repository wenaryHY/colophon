use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Form, Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};

use crate::{
    shared::{
        auth::AdminUser,
        error::{AppError, AppResult},
        response::ApiResponse,
    },
    state::AppState,
};

use crate::modules::plugin::hook::{HookContext, HookData, PostBeforeRenderData};
use super::{context::TemplateContext, domain::ThemeSummary, engine, service::ThemeService, ThemeConfig};

pub async fn active_theme(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    let slug = service.list_themes(&state.pool).await?.1;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "slug": slug }),
    )))
}

pub async fn list_themes(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
) -> AppResult<Json<ApiResponse<Vec<ThemeSummary>>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    let (manifests, active_slug) = service.list_themes(&state.pool).await?;
    let summaries = manifests
        .into_iter()
        .map(|manifest| ThemeSummary {
            active: manifest.slug == active_slug,
            manifest,
        })
        .collect();
    Ok(Json(ApiResponse::success(summaries)))
}

pub async fn activate_theme(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(slug): Path<String>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    service.activate_theme(&state.pool, &slug).await?;
    state.invalidate_all_caches().await;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "activated": slug }),
    )))
}

pub async fn get_theme_detail(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(slug): Path<String>,
) -> AppResult<Json<ApiResponse<super::dto::ThemeDetailResponse>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    let (manifest, config) = service.get_theme_detail(&state.pool, &slug).await?;
    let schema = manifest.config.clone();
    Ok(Json(ApiResponse::success(
        super::dto::ThemeDetailResponse {
            manifest,
            config,
            schema,
        },
    )))
}

pub async fn save_theme_config(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Path(slug): Path<String>,
    Json(req): Json<super::dto::SaveThemeConfigRequest>,
) -> AppResult<Json<ApiResponse<serde_json::Value>>> {
    let service = ThemeService::new(state.theme_dir.clone());
    service
        .save_theme_config(&state.pool, &slug, &req.config)
        .await?;
    state.invalidate_all_caches().await;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "saved": slug }),
    )))
}

pub async fn upload_theme_archive(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    mut multipart: Multipart,
) -> AppResult<Json<ApiResponse<super::dto::ThemeUploadResponse>>> {
    let mut theme_data: Option<Vec<u8>> = None;

    // 提取上传的文件
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            theme_data = Some(field.bytes().await?.to_vec());
            break;
        }
    }

    let theme_data = theme_data.ok_or(crate::shared::error::AppError::BadRequest(
        "No file uploaded".to_string(),
    ))?;

    // 解析 zip 包
    let cursor = std::io::Cursor::new(theme_data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|_| crate::shared::error::AppError::BadRequest("Invalid zip file".to_string()))?;

    // 查找 theme.toml
    let mut manifest_content = String::new();
    {
        let mut theme_toml = archive.by_name("theme.toml").map_err(|_| {
            crate::shared::error::AppError::BadRequest(
                "theme.toml not found in archive".to_string(),
            )
        })?;
        std::io::Read::read_to_string(&mut theme_toml, &mut manifest_content)
            .map_err(|e| crate::shared::error::AppError::Io(e))?;
    }

    // 解析 manifest
    let manifest: super::ThemeManifest = toml::from_str(&manifest_content).map_err(|e| {
        crate::shared::error::AppError::BadRequest(format!("Failed to parse theme.toml: {}", e))
    })?;

    // 提取主题到 themes 目录
    let theme_dir = state.theme_dir.join(&manifest.slug);
    if theme_dir.exists() {
        std::fs::remove_dir_all(&theme_dir).map_err(|e| crate::shared::error::AppError::Io(e))?;
    }
    std::fs::create_dir_all(&theme_dir).map_err(|e| crate::shared::error::AppError::Io(e))?;

    let extract_result = (|| -> AppResult<()> {
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                crate::shared::error::AppError::Anyhow(anyhow::anyhow!("Failed to read archive: {}", e))
            })?;
            let entry_path = file
                .enclosed_name()
                .ok_or_else(|| AppError::BadRequest("ZIP contains invalid path entry".to_string()))?
                .to_path_buf();
            let outpath = theme_dir.join(entry_path);

            if file.is_dir() {
                std::fs::create_dir_all(&outpath).map_err(crate::shared::error::AppError::Io)?;
                continue;
            }
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(crate::shared::error::AppError::Io)?;
            }
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(crate::shared::error::AppError::Io)?;
            std::io::copy(&mut file, &mut outfile).map_err(crate::shared::error::AppError::Io)?;
        }
        Ok(())
    })();
    if let Err(err) = extract_result {
        let _ = std::fs::remove_dir_all(&theme_dir);
        return Err(err);
    }

    Ok(Json(ApiResponse::success(
        super::dto::ThemeUploadResponse {
            slug: manifest.slug.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            message: "主题已上传".to_string(),
        },
    )))
}

// --- 前台主题渲染 Handlers ---

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

    let seo_meta = crate::modules::seo::meta::build_home_meta(
        &ctx.site_title,
        &ctx.site_description,
        &ctx.site_url,
        "",  // seo_keywords
        "",
    );
    let json_ld = crate::modules::seo::meta::build_home_json_ld(
        &ctx.site_title,
        &ctx.site_description,
        &ctx.site_url,
    );

    let plugin_guard = state.plugin_manager.read().await;
    let env = engine::build_template_engine(&ctx, &state.theme_dir, &*plugin_guard, &state.template_env_cache).await?;
    let tmpl = env
        .get_template("index.html")
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    let rendered = tmpl
        .render(minijinja::context!(
            seo_meta => seo_meta,
            json_ld => json_ld,
            current_user => auth
        ))
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
        if p.content_type == "page" && p.page_render_mode == "custom_html" {
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
    if post.is_none() {
        tracing::warn!(
            module = "theme",
            event = "render_post_not_found",
            client_request_id = %client_request_id,
            slug = %slug,
            "public post not found"
        );
        return Ok((StatusCode::NOT_FOUND, "Not Found").into_response());
    }
    let p = post.unwrap();

    let ctx = TemplateContext::load(&state).await?;

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
        .map(|id| format!("{}/uploads/{}", ctx.site_url, id))
        .unwrap_or_default();

    let seo_meta = crate::modules::seo::meta::build_post_meta_with_content_type(
        &ctx.site_title,
        &ctx.site_url,
        &p.title,
        &p.slug,
        p.excerpt.as_deref(),
        &p.content_html,
        "",  // seo_keywords
        &og_image,
        &p.content_type,
    );

    let json_ld = crate::modules::seo::meta::build_post_json_ld_with_content_type(
        &ctx.site_title,
        &ctx.site_url,
        &p.title,
        &p.slug,
        p.excerpt.as_deref().unwrap_or(""),
        &p.author_display_name,
        p.published_at.as_deref(),
        &p.updated_at,
        &p.content_type,
    );

    let comments = crate::modules::comment::repository::list_approved_for_post(&state.pool, &p.id)
        .await
        .unwrap_or_default();

    let plugin_guard = state.plugin_manager.read().await;
    let env = engine::build_template_engine(&ctx, &state.theme_dir, &*plugin_guard, &state.template_env_cache).await?;
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
    if file_path.contains("..") || file_path.contains('\\') || file_path.starts_with('/') {
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
        Ok(d) => ([(header::CONTENT_TYPE, mime)], d).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

pub async fn serve_upload_static(
    State(state): State<Arc<AppState>>,
    Path(file_path): Path<String>,
) -> impl IntoResponse {
    if file_path.contains("..") || file_path.contains('\\') || file_path.starts_with('/') {
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

    match tokio::fs::read(&full_path).await {
        Ok(d) => ([(header::CONTENT_TYPE, mime)], d).into_response(),
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
                return ([(header::CONTENT_TYPE, "image/svg+xml")], placeholder).into_response();
            }

            (StatusCode::NOT_FOUND, "404 Not Found").into_response()
        }
    }
}

pub async fn serve_plugin_static(
    State(state): State<Arc<AppState>>,
    Path((plugin_slug, file_path)): Path<(String, String)>,
) -> impl IntoResponse {
    if file_path.contains("..") || file_path.contains('\\') || file_path.starts_with('/') {
        return (StatusCode::FORBIDDEN, "403 Forbidden").into_response();
    }

    // 检查插件是否启用
    let enabled = match crate::modules::plugin::status::get_enabled_ids(&state.pool).await {
        Ok(ids) => ids,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    };
    if !enabled.contains(&plugin_slug) {
        return (StatusCode::NOT_FOUND, "404 Not Found").into_response();
    }

    let plugins_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let full_path = plugins_dir.join(&plugin_slug).join("static").join(&file_path);

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
        Ok(d) => ([(header::CONTENT_TYPE, mime)], d).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

/// 裸 HTML 预览：纯 Markdown→HTML，不经过 MiniJinja 渲染
pub async fn preview_content(
    State(_state): State<Arc<AppState>>,
    _admin: AdminUser,
    Form(req): Form<super::dto::PreviewContentRequest>,
) -> Result<Response, AppError> {
    let content = req.content.trim().to_string();
    if content.is_empty() {
        return Err(AppError::BadRequest("content must not be empty".into()));
    }
    if content.len() > 1_048_576 {
        return Err(AppError::BadRequest("content exceeds 1MB limit".into()));
    }

    let html = crate::modules::post::service::markdown_to_html(&content);
    let mut response = Html(html).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_PREVIEW,
    );
    Ok(response)
}

/// 主题渲染预览：Markdown→HTML→MiniJinja 渲染为完整的主题页面
pub async fn preview_theme(
    State(state): State<Arc<AppState>>,
    _admin: AdminUser,
    Form(req): Form<super::dto::PreviewThemeRequest>,
) -> Result<Response, AppError> {
    let content = req.content.trim().to_string();
    if content.is_empty() {
        return Err(AppError::BadRequest("content must not be empty".into()));
    }
    if content.len() > 1_048_576 {
        return Err(AppError::BadRequest("content exceeds 1MB limit".into()));
    }
    let content_type = match req.content_type.as_str() {
        "post" | "page" => req.content_type.clone(),
        other => return Err(AppError::BadRequest(
            format!("invalid content_type '{}', must be 'post' or 'page'", other)
        )),
    };

    // Markdown → HTML
    let content_html = crate::modules::post::service::markdown_to_html(&content);

    // 加载 TemplateContext
    let mut ctx = TemplateContext::load(&state).await?;

    // 覆写主题 slug（如果指定）
    if let Some(ref slug) = req.theme_slug {
        validateThemeSlugIsInstalledAndSafeForPreviewRendering(slug, &state.theme_dir)?;
        ctx.active_theme = slug.clone();
    }

    // 覆写主题配置（如果指定）
    if let Some(ref cfg_str) = req.theme_config {
        if let Ok(cfg) = serde_json::from_str::<ThemeConfig>(cfg_str) {
            // SAFETY: 对所有字符串值进行 HTML 转义，防止模板中 | safe 导致 XSS
            let sanitized = sanitizeThemeConfigStringValuesForPreventTemplateInjection(&cfg);
            ctx.theme_config = Some(sanitized);
        }
    }

    // 构造虚拟 post（不存库）
    let now = chrono::Utc::now().to_rfc3339();
    let fake_post = crate::modules::post::domain::PublicPostDetail {
        id: "_preview_".into(),
        title: "(Preview)".into(),
        slug: "_preview_".into(),
        excerpt: None,
        content_html,
        content_type: content_type.clone(),
        allow_comment: 0,
        published_at: None,
        created_at: now.clone(),
        updated_at: now,
        author_display_name: "(Preview)".into(),
        category_name: None,
        cover_media_id: None,
    };

    // 构建模板引擎
    let plugin_guard = state.plugin_manager.read().await;
    let env = engine::build_template_engine(
        &ctx, &state.theme_dir, &*plugin_guard, &state.template_env_cache
    ).await?;

    // 选择模板
    let template_name = if content_type == "page" && env.get_template("page.html").is_ok() {
        "page.html"
    } else {
        "post.html"
    };
    let tmpl = env.get_template(template_name)
        .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Template error: {}", e)))?;

    // 渲染
    let rendered = tmpl.render(minijinja::context! {
        post => fake_post,
        seo_meta => "",
        json_ld => "",
        image => "",
        comments => Vec::<()>::new(),
        current_user => _admin.0,
        plugins => serde_json::Value::Object(Default::default()),
        post_excerpt => "",
    })
    .map_err(|e| AppError::Anyhow(anyhow::anyhow!("Render error: {}", e)))?;

    let mut response = Html(rendered).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_PREVIEW,
    );
    Ok(response)
}

/// 验证主题 slug 是否为合法且已安装的主题标识符
#[allow(non_snake_case)]
fn validateThemeSlugIsInstalledAndSafeForPreviewRendering(slug: &str, theme_dir: &std::path::Path) -> Result<(), AppError> {
    if slug.contains("..") || slug.contains('/') || slug.contains('\\') {
        return Err(AppError::BadRequest("invalid theme_slug".into()));
    }
    let manifest_path = theme_dir.join(slug).join("theme.toml");
    if !manifest_path.exists() || !manifest_path.is_file() {
        return Err(AppError::BadRequest(format!(
            "theme '{}' not found or not installed",
            slug
        )));
    }
    Ok(())
}

/// 递归转义 ThemeConfig 中所有字符串值的 HTML 特殊字符，防止模板注入
#[allow(non_snake_case)]
fn sanitizeThemeConfigStringValuesForPreventTemplateInjection(
    config: &HashMap<String, serde_json::Value>
) -> HashMap<String, serde_json::Value> {
    config.iter().map(|(k, v)| {
        let sanitized = match v {
            serde_json::Value::String(s) => {
                serde_json::Value::String(escapeHtmlSpecialCharacters(s))
            }
            other => other.clone(),
        };
        (k.clone(), sanitized)
    }).collect()
}

#[allow(non_snake_case)]
fn escapeHtmlSpecialCharacters(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#x27;")
}

/// 新标签页预览页面（空壳 HTML + 内嵌 JS）
pub async fn preview_page(
    _admin: AdminUser,
) -> Result<Html<String>, AppError> {
    let html = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>InkForge 预览</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { background: #f5f5f5; display: flex; justify-content: center; min-height: 100vh; }
        .loading { text-align: center; padding: 100px 20px; color: #999; }
        .loading .spinner { width: 40px; height: 40px; border: 3px solid #e0e0e0; border-top-color: #333; border-radius: 50%; animation: spin 0.8s linear infinite; margin: 0 auto 16px; }
        @keyframes spin { to { transform: rotate(360deg); } }
        .error { text-align: center; padding: 100px 20px; color: #c62828; display: none; }
        .preview-container { width: 100%; background: #fff; min-height: 100vh; display: none; }
        iframe { width: 100%; height: 100vh; border: none; }
    </style>
</head>
<body>
    <div class="loading" id="loading">
        <div class="spinner"></div>
        <p>正在加载预览...</p>
    </div>
    <div class="error" id="error"></div>
    <div class="preview-container" id="preview"></div>

    <script>
    (async function() {
        try {
            // 从 sessionStorage 读取预览参数
            const raw = sessionStorage.getItem('inkforge-preview-params');
            if (!raw) throw new Error('No preview params found');
            const params = JSON.parse(raw);

            // 根据模式请求不同端点
            let url, method, body;
            if (params.mode === 'theme') {
                url = '/api/v1/preview/theme';
                body = new URLSearchParams();
                body.append('content', params.content);
                body.append('content_type', params.content_type || 'post');
                if (params.theme_slug) body.append('theme_slug', params.theme_slug);
                if (params.theme_config) body.append('theme_config', params.theme_config);
            } else {
                url = '/api/v1/preview/content';
                body = new URLSearchParams();
                body.append('content', params.content);
                body.append('content_type', params.content_type || 'post');
            }

            const resp = await fetch(url, {
                method: 'POST',
                headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
                body: body.toString(),
                credentials: 'include',
            });

            if (!resp.ok) {
                const text = await resp.text();
                throw new Error(text || 'HTTP ' + resp.status);
            }

            const html = await resp.text();
            document.getElementById('loading').style.display = 'none';
            const preview = document.getElementById('preview');
            preview.style.display = 'block';
            preview.innerHTML = html;
        } catch (err) {
            document.getElementById('loading').style.display = 'none';
            const error = document.getElementById('error');
            error.style.display = 'block';
            const heading = document.createElement('h3');
            heading.textContent = '加载失败';
            const para = document.createElement('p');
            para.textContent = err.message || 'Unknown error';
            error.appendChild(heading);
            error.appendChild(para);
        }
    })();
    </script>
</body>
</html>"#;
    Ok(Html(html.to_string()))
}
