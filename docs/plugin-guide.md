# Plugin Development Guide

Colophon plugins are Rust crates compiled and statically linked into the server binary. There is no runtime dynamic dispatch overhead — the Rust compiler verifies type safety and API contracts at build time. Enabling or disabling a plugin is a boolean toggle in the admin panel.

## How Plugins Are Discovered

Plugins are discovered at runtime from the `plugins/` directory. Each plugin must have a `plugin.toml` manifest. The `PluginLoader` scans subdirectories containing both a `plugin.toml` manifest and source files, auto-discovers them, and registers them into the `PluginManager`.

The manifest's `[plugin].id` field must match the directory name exactly (e.g. `plugins/hello-world-a3f9b2c1/` → `id = "hello-world-a3f9b2c1"`). The loader enforces this invariant and skips mismatches with a warning.

## The Plugin Trait

Every plugin implements the `Plugin` trait, which offers seven methods. All but `name` and `version` have sensible defaults — you only override what you need.

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    async fn init(&self, _state: &Arc<AppState>) -> AppResult<()> { Ok(()) }
    async fn shutdown(&self) -> AppResult<()> { Ok(()) }

    fn api_routes(&self) -> Router<Arc<AppState>> { Router::new() }
    fn extend_template_env(&self, _env: &mut Environment<'_>) -> AppResult<()> { Ok(()) }
    fn frontend_assets(&self) -> Vec<PluginAsset> { vec![] }
    fn hooks(&self) -> Vec<Hook> { vec![] }
}
```

| Method | Purpose | When to override |
|---|---|---|
| `name()` | Unique plugin identifier (must match directory name) | Always |
| `version()` | Semver string for logging and display | Always |
| `init()` | One-time setup: open files, warm caches, validate config | When you have startup work |
| `shutdown()` | Graceful teardown: flush buffers, close connections | When you hold external resources |
| `api_routes()` | Register custom `axum::Router` handlers under `/api/v1/plugins/` | When you need API endpoints |
| `extend_template_env()` | Add MiniJinja functions/filters usable in theme templates | When you want template helpers |
| `frontend_assets()` | Inject CSS or JS into the admin panel `<head>` or `<body>` | When you need custom styles or scripts |
| `hooks()` | Register Filter or Action hooks for lifecycle events | When you want to react to content changes |

## Full Example: Hello World

The repository ships a demo plugin at `plugins/hello-world-a3f9b2c1/`. It demonstrates all seven trait methods in a single file. Below is the annotated version.

### Directory structure

```
plugins/hello-world-a3f9b2c1/
├── plugin.toml
├── lib.rs
└── static/
    └── hello.css
```

### plugin.toml

```toml
[plugin]
id = "hello-world-a3f9b2c1"
title = "Hello World"
version = "0.1.0"
description = "A demo plugin"
author = "Colophon Team"

[engine]
colophon = ">=0.3.0"

[hooks]
template = true
routes = true
assets = ["css"]

[[settings]]
key = "greeting_target"
label = "Greeting target"
type = "text"
default = "World"
description = "Default argument for the hello_world template function"

[admin]
enabled = true
entry = "settings.html"
```

Fields explained:

- **`[plugin]`** — identity and metadata. `id` must match the directory name.
- **`[engine]`** — minimum Colophon version required. The manager checks this at load time.
- **`[hooks]`** — capability flags declaring what the plugin uses. Set `template = true` if you call `extend_template_env`, `routes = true` for `api_routes`, and `assets = ["css"]` (or `["js"]`) for `frontend_assets`.
- **`[[settings]]`** — user-configurable settings surfaced in the admin panel. Each entry has a `key`, `label`, `type`, `default`, and `description`.
- **`[admin]`** — enables the plugin settings page and points to an optional custom settings template.

### lib.rs

```rust
use async_trait::async_trait;
use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use minijinja::Environment;
use std::sync::Arc;

use crate::modules::plugin::asset::{AssetPlacement, PluginAsset};
use crate::modules::plugin::Plugin;
use crate::shared::error::AppResult;
use crate::state::AppState;

#[derive(Default)]
pub struct HelloWorldPlugin;

impl HelloWorldPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for HelloWorldPlugin {
    fn name(&self) -> &str {
        "hello-world-a3f9b2c1"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    async fn init(&self, _state: &Arc<AppState>) -> AppResult<()> {
        tracing::info!(
            module = "plugin",
            plugin = "hello-world-a3f9b2c1",
            "HelloWorld plugin initialized"
        );
        Ok(())
    }

    fn api_routes(&self) -> Router<Arc<AppState>> {
        async fn hello_handler(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
            Json(serde_json::json!({
                "plugin": "hello-world-a3f9b2c1",
                "status": "ok"
            }))
        }

        Router::new().route("/api/v1/plugins/hello", get(hello_handler))
    }

    fn extend_template_env(&self, env: &mut Environment<'_>) -> AppResult<()> {
        env.add_function(
            "hello_world",
            |name: Option<String>| -> Result<String, minijinja::Error> {
                let who = name.unwrap_or_else(|| "World".to_string());
                Ok(format!("Hello, {}!", who))
            },
        );
        Ok(())
    }

    fn frontend_assets(&self) -> Vec<PluginAsset> {
        vec![PluginAsset::css(self.name(), "hello.css", AssetPlacement::Head)]
    }

    fn hooks(&self) -> Vec<crate::modules::plugin::hook::Hook> {
        use crate::modules::plugin::hook::{Hook, HookContext, HookHandler};
        use crate::shared::error::AppResult;
        use async_trait::async_trait;
        use std::sync::Arc;

        struct LogPublishHook;

        #[async_trait]
        impl HookHandler for LogPublishHook {
            async fn run(&self, ctx: &mut HookContext) -> AppResult<()> {
                if let crate::modules::plugin::hook::HookData::PostAfterPublish(ref data) = ctx.data {
                    tracing::info!(
                        module = "plugin",
                        plugin = "hello-world",
                        post_id = data.post_id,
                        title = data.title,
                        slug = data.slug,
                        "post published: {title}",
                        title = data.title,
                    );
                }
                Ok(())
            }
        }

        vec![
            Hook::new_action("post.after_publish", 10, self.name(), Arc::new(LogPublishHook)),
        ]
    }
}
```

### What the plugin does

1. **`init`** — logs a message at startup so you can confirm the plugin loaded.
2. **`api_routes`** — serves `GET /api/v1/plugins/hello` returning `{"plugin":"hello-world-a3f9b2c1","status":"ok"}`. All plugin routes are automatically prefixed and protected by admin authentication. If the plugin is disabled in the admin panel, the route returns 404.
3. **`extend_template_env`** — registers a `hello_world(name?)` function callable from any MiniJinja theme template:
   ```jinja
   {{ hello_world() }}        {# Hello, World! #}
   {{ hello_world("Reader") }} {# Hello, Reader! #}
   ```
4. **`frontend_assets`** — injects `hello.css` into the `<head>` of every admin page. The file is served from `/static/plugins/hello-world-a3f9b2c1/hello.css`.
5. **`hooks`** — registers an Action hook on `post.after_publish` with priority 10. Every time a post is published, the handler logs the post ID, title, and slug.

## Hook System: Filter vs Action

Colophon has two hook types, each with different semantics:

| | Filter | Action |
|---|---|---|
| **Execution** | Synchronous, in order of priority | Spawned as a `tokio` task (fire-and-forget) |
| **Can mutate?** | Yes — modify `HookContext.data` to change what gets saved | No — consume data, but cannot affect the request |
| **Failure behavior** | Aborts the operation (transaction rolls back) | Logged and ignored; the post still publishes |
| **Timeout** | None — runs on the request thread | 5 seconds per handler (killed if exceeded) |
| **Tracking** | Not tracked | Action Registry records every spawn, run, completion, and failure |

The three-layer execution model is:

```
Filter hooks (sync, in priority order)
       │
       ▼
Database commit (auto)
       │
       ▼
Action hooks (fire-and-forget, spawned per handler)
```

A Filter hook on `post.before_save` can sanitize content or reject spam. An Action hook on `post.after_publish` can trigger webhooks, send emails, or update search indexes — without blocking the HTTP response.

## Registering Routes

Plugins return an `axum::Router<Arc<AppState>>` from `api_routes()`. The `PluginManager` merges all plugin routers and wraps them with a middleware that:

1. Validates the admin session (`AdminUser` extractor).
2. Checks the plugin is enabled in the `plugin_status` database table.
3. Returns `404 Plugin disabled` if the plugin is toggled off.

Route paths are under the plugin's control — the convention is to prefix with `/api/v1/plugins/{plugin-name}/`.

## Injecting Frontend Assets

Use `PluginAsset::css(slug, filename, placement)` or `PluginAsset::js(slug, filename, placement)`:

```rust
fn frontend_assets(&self) -> Vec<PluginAsset> {
    vec![
        PluginAsset::css(self.name(), "editor-tweaks.css", AssetPlacement::Head),
        PluginAsset::js(self.name(), "editor-tweaks.js", AssetPlacement::Body),
    ]
}
```

Place asset files in `plugins/{your-plugin}/static/`. They are served at `/static/plugins/{your-plugin}/{filename}`. The `PluginManager` renders them as `<link>` or `<script>` tags on every admin page. Use `AssetPlacement::Head` for styles and critical scripts, `AssetPlacement::Body` for deferred scripts.

## Template Functions

`extend_template_env` receives a mutable reference to the MiniJinja `Environment`. You can add functions, filters, and global variables:

```rust
fn extend_template_env(&self, env: &mut Environment<'_>) -> AppResult<()> {
    env.add_function("reading_time", |content: String| -> f64 {
        (content.split_whitespace().count() as f64 / 200.0).ceil()
    });
    Ok(())
}
```

These are available in all theme templates across every page render.

## Debugging Tips

- **Startup logs**: every plugin logs its name and version at `info` level during `PluginManager::init_all`. Check for `module = "plugin"` in your logs.
- **Hook registration**: `module = "hook"` logs show which plugin registered which hooks.
- **Action Registry**: `module = "action_registry"` logs every action's lifecycle — `spawned`, `running`, `done`, `failed`, or `timeout`. This is invaluable for debugging fire-and-forget hooks that seem to do nothing.
- **Disabled plugins**: if a plugin route returns 404 unexpectedly, check the `plugin_status` table or the admin panel's plugin toggle.
- **Plugin loader warnings**: the PluginLoader logs a warning when a manifest `id` doesn't match the directory name — check startup logs for `module = "plugin"`.

## Next Steps

- Study the `hello-world-a3f9b2c1` plugin as a template for your own.
- Read the [Architecture Overview](./architecture.md) to understand how the PluginManager fits into the request lifecycle.
- See the [Webhook Guide](./webhook-guide.md) for an example of a built-in Action hook listener.
