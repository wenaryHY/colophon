use minijinja::{AutoEscape, Environment, Value};
use std::path::Path;

use super::context::TemplateContext;
use crate::shared::error::AppResult;

/// Build a MiniJinja Environment for the current request.
///
/// `theme_dir` is the base themes directory (e.g., `state.theme_dir`).
/// The active theme slug comes from `ctx.active_theme`.
/// All other data has been pre-fetched into `ctx`.
/// No async DB queries happen inside synchronous MiniJinja closures,
/// eliminating the `block_in_place` deadlock risk.
pub fn build_template_engine(
    ctx: &TemplateContext,
    theme_dir: &Path,
) -> AppResult<Environment<'static>> {
    let template_dir = theme_dir.join(&ctx.active_theme).join("templates");

    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") || name.ends_with(".htm") || name.ends_with(".xml") {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });

    // ── 1A: Dynamic template loader ──────────────────────────────────
    let loader_path = template_dir.clone();
    env.set_loader(move |name| {
        let path = loader_path.join(name);
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(minijinja::Error::new(
                minijinja::ErrorKind::TemplateNotFound,
                format!("IO error: {}", e),
            )),
        }
    });

    // ── Globals ──────────────────────────────────────────────────────
    env.add_global("site_title", Value::from(&ctx.site_title));
    env.add_global("site_description", Value::from(&ctx.site_description));
    env.add_global("site_url", Value::from(&ctx.site_url));
    env.add_global("admin_url", Value::from(&ctx.admin_url));

    if let Some(ref cfg) = ctx.theme_config {
        env.add_global("theme_config", Value::from_serialize(cfg));
    }

    // ── 3B: theme_assets_url helper ──────────────────────────────────
    let slug = ctx.active_theme.clone();
    env.add_function(
        "theme_assets_url",
        move |path: String| -> Result<Value, minijinja::Error> {
            Ok(Value::from(format!("/static/themes/{}/{}", slug, path)))
        },
    );

    env.add_filter(
        "tojson",
        |value: Value| -> Result<String, minijinja::Error> {
            serde_json::to_string(&value)
                .map_err(|err| {
                    minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, err.to_string())
                })
        },
    );
    env.add_filter(
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
                    minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, err.to_string())
                })
        },
    );

    // ── 2B: Context Functions (pre-fetched data, no block_in_place) ──
    let posts = ctx.recent_posts.clone();
    env.add_function(
        "get_recent_posts",
        move |_limit: Option<i64>| -> Result<Value, minijinja::Error> {
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
        move || -> Result<Value, minijinja::Error> {
            Ok(Value::from_serialize(&cats))
        },
    );

    Ok(env)
}
