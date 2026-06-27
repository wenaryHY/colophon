use std::sync::Arc;

use axum::{
    Extension,
    extract::{Form, State},
    response::{Html, IntoResponse, Response},
};

use crate::{
    shared::{
        auth::AdminUser,
        error::AppError,
        role::Role,
    },
    state::AppState,
};

use crate::modules::theme::{
    context::TemplateContext, engine, ThemeConfig,
};

/// 在 HTML 中注入 CSP meta 标签，用于 iframe srcdoc 场景
/// HTTP CSP 响应头在 srcdoc 中不生效，必须通过 meta 标签传递
#[allow(non_snake_case)]
fn injectCspMetaTagIntoHtmlForSrcdocProtection(html: &str) -> String {
    let csp_meta = format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"{}\">",
        crate::shared::security::PREVIEW_CSP_TEMPLATE
    );
    if let Some(pos) = html.find("<html") {
        let (before, after) = html.split_at(pos);
        format!("{}{}{}", before, csp_meta, after)
    } else {
        format!("{}{}", csp_meta, html)
    }
}

/// 裸 HTML 预览：纯 Markdown→HTML，不经过 MiniJinja 渲染
pub async fn preview_content(
    State(_state): State<Arc<AppState>>,
    _admin: AdminUser,
    Form(req): Form<crate::modules::theme::dto::PreviewContentRequest>,
) -> Result<Response, AppError> {
    let content = req.content.trim().to_string();
    if content.is_empty() {
        return Err(AppError::BadRequest("content must not be empty".into()));
    }
    if content.len() > 1_048_576 {
        return Err(AppError::BadRequest("content exceeds 1MB limit".into()));
    }

    let html = crate::shared::content::markdown_to_html(&content);
    // HTTP CSP 头在 iframe srcdoc 中不生效，通过 meta 标签注入 CSP
    let secured_html = injectCspMetaTagIntoHtmlForSrcdocProtection(&html);
    let mut response = Html(secured_html).into_response();
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
    Form(req): Form<crate::modules::theme::dto::PreviewThemeRequest>,
) -> Result<Response, AppError> {
    let content = req.content.trim().to_string();
    if content.is_empty() {
        return Err(AppError::BadRequest("content must not be empty".into()));
    }
    if content.len() > 1_048_576 {
        return Err(AppError::BadRequest("content exceeds 1MB limit".into()));
    }
    let content_type = req.content_type;

    // TODO(security): 添加预览端点的速率限制（每用户每分钟最多 30 次）
    // 参考 src/shared/security.rs 中的 LoginRateLimiter 模式实现

    // Markdown → HTML
    let content_html = crate::shared::content::markdown_to_html(&content);

    // 加载 TemplateContext
    let mut ctx = TemplateContext::load(&state).await?;

    // 覆写主题 slug（如果指定）
    if let Some(ref slug) = req.theme_slug {
        validateThemeSlugIsInstalledAndSafeForPreviewRendering(slug, &state.theme_dir)?;
        ctx.active_theme = slug.clone();
    }

    // 覆写主题配置（如果指定）
    // 注意：theme_config 值在 MiniJinja 模板中自动转义。
    // 主题作者不得对 theme_config 值使用 | safe 过滤器。
    if let Some(ref cfg_str) = req.theme_config {
        match serde_json::from_str::<ThemeConfig>(cfg_str) {
            Ok(cfg) => {
                ctx.theme_config = Some(cfg);
            }
            Err(e) => {
                tracing::warn!(
                    module = "theme",
                    event = "preview_theme_config_parse_failed",
                    error = %e,
                    "failed to parse theme_config, ignoring"
                );
            }
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
        content_type,
        allow_comment: false,
        published_at: None,
        created_at: now.clone(),
        updated_at: now,
        author_display_name: "(Preview)".into(),
        category_name: None,
        cover_media_id: None,
    };

    // 构建模板引擎
    let plugin_guard = state.plugin_manager.read().await;
    let current_lang = crate::infra::i18n_middleware::DEFAULT_LANG; // 预览页面使用默认语言
    let env = engine::build_template_engine(
        &ctx,
        &state.theme_dir,
        &*plugin_guard,
        &state.template_env_cache,
        &state.asset_manifest,
        Some(current_lang),
    )
    .await?;

    // 选择模板
    let template_name = if content_type.is_page() && env.get_template("page.html").is_ok() {
        "page.html"
    } else {
        "post.html"
    };

    // 渲染（带超时保护，防止模板死循环）
    // clone Environment 避免生命周期问题（内部 Arc 包装，开销很小）
    let env_for_blocking = env.clone();
    let template_name_owned = template_name.to_string();
    let fake_current_user = crate::shared::auth::AuthUser {
        id: "_preview_".into(),
        username: "(Preview)".into(),
        role: Role::Admin,
    };
    let rendered = match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::task::spawn_blocking(move || {
            let tmpl = env_for_blocking
                .get_template(&template_name_owned)
                .map_err(|e| anyhow::anyhow!("Template error: {}", e))?;
            tmpl.render(minijinja::context! {
                post => fake_post,
                seo_meta => "",
                json_ld => "",
                image => "",
                comments => Vec::<()>::new(),
                current_user => fake_current_user,
                plugins => serde_json::Value::Object(Default::default()),
                post_excerpt => "",
            })
            .map_err(|e| anyhow::anyhow!("Render error: {}", e))
        }),
    )
    .await
    {
        // timeout → JoinHandle → anyhow::Result
        Ok(Ok(Ok(html))) => html,
        Ok(Ok(Err(e))) => {
            return Err(AppError::Anyhow(e));
        }
        Ok(Err(join_err)) => {
            return Err(AppError::Anyhow(anyhow::anyhow!(
                "Render spawn error: {}",
                join_err
            )));
        }
        Err(_elapsed) => {
            return Err(AppError::Anyhow(anyhow::anyhow!("Preview render timeout")));
        }
    };

    // HTTP CSP 头在 iframe srcdoc 中不生效，通过 meta 标签注入 CSP
    let secured_html = injectCspMetaTagIntoHtmlForSrcdocProtection(&rendered);
    let mut response = Html(secured_html).into_response();
    crate::shared::security::mark_response_security_profile(
        &mut response,
        crate::shared::security::SECURITY_PROFILE_PREVIEW,
    );
    Ok(response)
}

/// 验证主题 slug 是否为合法且已安装的主题标识符
#[allow(non_snake_case)]
fn validateThemeSlugIsInstalledAndSafeForPreviewRendering(
    slug: &str,
    theme_dir: &std::path::Path,
) -> Result<(), AppError> {
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

/// 新标签页预览页面（空壳 HTML + 内嵌 JS）
pub async fn preview_page(
    _admin: AdminUser,
    Extension(_csp_nonce): Extension<crate::shared::security::CspNonce>,
) -> Result<Html<String>, AppError> {
    let html = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Colophon 预览</title>
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
            const raw = sessionStorage.getItem('colophon-preview-params');
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
            const iframe = document.createElement('iframe');
            iframe.style.cssText = 'width:100%;height:100vh;border:none';
            iframe.sandbox = 'allow-scripts allow-same-origin';
            iframe.srcdoc = html;
            preview.appendChild(iframe);
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
