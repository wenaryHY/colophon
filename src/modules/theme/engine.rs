use minijinja::{AutoEscape, Environment, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::context::TemplateContext;
use crate::modules::plugin::manager::PluginManager;
use crate::shared::error::AppResult;
use crate::state::AssetManifest;

/// Build a MiniJinja Environment for the current request.
///
/// `theme_dir` is the base themes directory (e.g., `state.theme_dir`).
/// The active theme slug comes from `ctx.active_theme`.
/// All other data has been pre-fetched into `ctx`.
/// No async DB queries happen inside synchronous MiniJinja closures,
/// eliminating the `block_in_place` deadlock risk.
///
/// Caching: The base Environment (loader + static filters + theme_assets_url)
/// is cached per active_theme slug in `env_cache`. On each request, the cached
/// base is cloned and per-request data (globals, data functions, plugin hooks)
/// is added. This avoids rebuilding the template loader on every request.
///
/// `current_lang`: Current language preference for i18n (e.g., "zh" or "en").
pub async fn build_template_engine(
    ctx: &TemplateContext,
    theme_dir: &Path,
    plugin_manager: &PluginManager,
    env_cache: &Arc<RwLock<HashMap<String, Environment<'static>>>>,
    asset_manifest: &Arc<AssetManifest>,
    current_lang: Option<&str>,
) -> AppResult<Environment<'static>> {
    // 尝试从缓存获取基础 Environment
    let base_env = {
        let cache = env_cache.read().await;
        cache.get(&ctx.active_theme).cloned()
    };

    let mut env = if let Some(cached) = base_env {
        cached
    } else {
        // 构建新的基础 Environment：loader + autoescape + static filters + theme_assets_url
        let template_dir = theme_dir.join(&ctx.active_theme).join("templates");
        let mut new_env = Environment::new();
        new_env.set_auto_escape_callback(|name| {
            if name.ends_with(".html") || name.ends_with(".htm") || name.ends_with(".xml") {
                AutoEscape::Html
            } else {
                AutoEscape::None
            }
        });

        // 每次渲染最多执行 50,000 条指令（正常页面 2000-5000 步）。
        // 超过配额时引擎抛出错误，防止用户编写的恶意模板死循环耗尽 CPU。
        new_env.set_fuel(Some(50_000));

        // Dynamic template loader with path traversal protection
        let loader_path =
            std::fs::canonicalize(&template_dir).unwrap_or_else(|_| template_dir.clone());
        new_env.set_loader(move |name| {
            let raw_path = loader_path.join(name);
            match std::fs::canonicalize(&raw_path) {
                Ok(canonical) => {
                    if !canonical.starts_with(&loader_path) {
                        return Err(minijinja::Error::new(
                            minijinja::ErrorKind::InvalidOperation,
                            "path traversal detected".to_string(),
                        ));
                    }
                    match std::fs::read_to_string(&canonical) {
                        Ok(content) => Ok(Some(content)),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                        Err(e) => Err(minijinja::Error::new(
                            minijinja::ErrorKind::InvalidOperation,
                            format!("IO error reading template: {}", e),
                        )),
                    }
                }
                Err(_) => Ok(None),
            }
        });

        // theme_assets_url helper (per-theme, cached)
        // 通过 AssetManifest 注入构建期生成的 hash 版本号，
        // 让浏览器在主题资源变更时自动失效缓存。
        let slug = ctx.active_theme.clone();
        let manifest = asset_manifest.clone();
        new_env.add_function(
            "theme_assets_url",
            move |path: String| -> Result<Value, minijinja::Error> {
                let resolved = manifest.resolve(&slug, &path);
                Ok(Value::from(format!("/static/themes/{}/{}", slug, resolved)))
            },
        );

        // Static filters (content-independent, cached)
        new_env.add_filter(
            "tojson",
            |value: Value| -> Result<String, minijinja::Error> {
                serde_json::to_string(&value).map_err(|err| {
                    minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, err.to_string())
                })
            },
        );
        new_env.add_filter(
            "tojson_script",
            |value: Value| -> Result<String, minijinja::Error> {
                serde_json::to_string(&value)
                    .map(|json| {
                        json.replace('<', "\\u003c")
                            .replace('>', "\\u003e")
                            .replace('&', "\\u0026")
                            .replace('\u{2028}', "\\u2028")
                            .replace('\u{2029}', "\\u2029")
                    })
                    .map_err(|err| {
                        minijinja::Error::new(
                            minijinja::ErrorKind::InvalidOperation,
                            err.to_string(),
                        )
                    })
            },
        );

        // 缓存基础 Environment
        {
            let mut cache = env_cache.write().await;
            let cached_env = new_env.clone();
            cache.insert(ctx.active_theme.clone(), cached_env);
        }

        new_env
    };

    // ── 每请求变化的数据 ──────────────────────────────────────────────
    // Globals (from context, may change between requests)
    env.add_global("site_title", Value::from(&ctx.site_title));
    env.add_global("site_description", Value::from(&ctx.site_description));
    env.add_global("site_url", Value::from(&ctx.site_url));
    env.add_global("admin_url", Value::from(&ctx.admin_url));
    env.add_global("current_lang", Value::from(current_lang.unwrap_or("zh")));

    if let Some(ref cfg) = ctx.theme_config {
        env.add_global("theme_config", Value::from_serialize(cfg));
    }

    // Data functions (from context, per-request data)
    let posts = ctx.recent_posts.clone();
    env.add_function(
        "get_recent_posts",
        move |limit: Option<i64>| -> Result<Value, minijinja::Error> {
            let posts = match limit {
                Some(n) => posts.iter().take(n as usize).cloned().collect::<Vec<_>>(),
                None => posts.clone(),
            };
            Ok(Value::from_serialize(&posts))
        },
    );

    let tags = ctx.tags.clone();
    env.add_function("get_tags", move || -> Result<Value, minijinja::Error> {
        Ok(Value::from_serialize(&tags))
    });

    let cats = ctx.categories.clone();
    env.add_function(
        "get_categories",
        move || -> Result<Value, minijinja::Error> { Ok(Value::from_serialize(&cats)) },
    );

    // Plugin hooks (per-request)
    plugin_manager.extend_template_env(&mut env)?;

    let head_html = plugin_manager.render_asset_html("head");
    env.add_global("plugin_head", Value::from_safe_string(head_html));

    let body_html = plugin_manager.render_asset_html("body");
    env.add_global("plugin_body", Value::from_safe_string(body_html));

    Ok(env)
}
