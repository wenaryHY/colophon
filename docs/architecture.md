# Architecture Overview

This document describes the high-level design of Colophon. It is written for developers who want to understand the system before contributing, writing a plugin, or deploying in production.

## Technology Stack

| Layer | Technology | Role |
|---|---|---|
| HTTP server | **Axum 0.8** (Tokio async runtime) | Request routing, middleware, WebSocket |
| Database | **SQLite 3** via `sqlx 0.7` (WAL mode) | All persistent state: posts, users, comments, settings |
| Templates | **MiniJinja** | Server-side theme rendering |
| Admin frontend | **React 19 + TypeScript + Vite 8** | Embedded SPA, built at compile time |
| Plugin system | **Rust trait + runtime discovery** | PluginLoader scans plugins/ directory at startup |
| Desktop shell | **Tauri 2** (optional) | Shares the same `lib.rs` entry point as the server |
| Deployment | **Single binary** with embedded assets | One file + one config directory |

## Request Lifecycle

```
                    ┌─────────────────────────────┐
                    │     Axum Router (port 2000)     │
                    └──────────────┬──────────────┘
           ┌──────────────────────┼──────────────────────┐
           │                      │                      │
    /api/v1/*              /admin/*               /* (public)
           │                      │                      │
    JWT / API Key           SPA handler            Theme renderer
     auth layer            (embedded)          (MiniJinja + DB)
           │                                         │
    Handler ──► SQLite                     Filter hooks (pre-render)
           │                                         │
    Filter hooks (pre-save)                  Render HTML
           │                                         │
    DB commit                                      Response
           │
    Action hooks (fire-and-forget)
           │
    Response
```

Every request to `/api/v1/*` passes through an auth middleware that validates JWT tokens, session cookies, or API keys. Requests to `/admin/*` serve the pre-built React SPA — if the file exists on disk (or in the embedded assets), it is returned directly; otherwise the SPA handles client-side routing. Public requests go through the theme engine, which fetches data from SQLite and renders it with MiniJinja templates.

## Hook System: Three-Layer Architecture

The hook system is the backbone of extensibility. It operates in three ordered phases:

```
Phase 1: Filter hooks  ──►  synchronous, executed on the request thread
          (can mutate HookContext.data, abort on failure)

          │ DB auto-commits after all filters succeed
          ▼

Phase 2: Database commit  ──►  automatic, happens exactly once per request

          │
          ▼

Phase 3: Action hooks  ──►  fire-and-forget, spawned as tokio tasks
          (cannot affect the response, 5s timeout per handler)
```

### Filter Hooks

Filters run **synchronously** on the request thread, in priority order. They receive a mutable `HookContext` and can modify `ctx.data` — for example, a spam filter hook on `post.before_save` can reject the post by returning an error, which causes the database transaction to roll back and the HTTP response to carry the error.

Available filter events:

- `post.before_save` — mutate post fields before they hit the database
- `post.before_render` — mutate post data before it reaches the template engine
- `comment.before_create` — reject or sanitize comments before insertion

### Action Hooks

Actions are spawned as **fire-and-forget** `tokio` tasks after the database commit succeeds. They cannot affect the HTTP response — the user already sees "Post published" by the time actions start running. Each action has a 5-second timeout; exceeding it logs a timeout and the task is dropped.

Available action events:

- `post.after_save` — post created or updated
- `post.after_publish` — post transitioned to published

The built-in webhook dispatcher registers itself as an action hook on both events. Plugins can register additional action hooks with whatever priority they need.

## PluginManager Lifecycle

```
Startup:     PluginLoader::discover()  ──►  scans plugins/ directory
                                         ──►  reads plugin.toml manifests
                                         ──►  returns Vec<DiscoveredPlugin>

Runtime:     PluginManager::load_with()  ──►  creates PluginManager from discovered list
                │
                ├── init_all(state)  ──►  calls plugin.init() for each
                │                        ──►  registers hooks into HookRegistry
                │
                ├── collect_routes() ──►  merges all plugin api_routes() with auth middleware
                ├── extend_template_env() ──►  adds all plugin template functions
                ├── collect_assets() ──►  gathers CSS/JS for admin panel injection
                │
                └── shutdown_all() ──►  calls plugin.shutdown() for each (on server stop)
```

Plugins are discovered at runtime from the `plugins/` directory. Each plugin must have a `plugin.toml` manifest. The `PluginLoader` scans subdirectories, reads their manifests, and returns a list of `DiscoveredPlugin` structs. `PluginManager::load_with()` takes that list and initializes all plugins.

Enabling or disabling a plugin at runtime toggles a row in the `plugin_status` table. The plugin's hooks remain registered but are checked against this table before execution. Plugin routes return 404 if disabled.

## Action Registry

The `ActionRegistry` is a global singleton that tracks every spawned action hook. It records:

- **`Spawned`** — action ID created, `tokio::spawn` called
- **`Running`** — handler started executing
- **`Done`** — handler completed successfully
- **`Failed(error)`** — handler returned an error
- **`Timeout`** — handler exceeded the 5-second limit

All state transitions are logged at the appropriate level (`info` for spawn/done, `error` for failures, `warn` for timeouts), making it straightforward to debug why an action isn't behaving as expected. Search logs for `module = "action_registry"`.

Expired records (completed > 1 hour ago) are cleaned up periodically to bound memory usage.

## Key Modules

```
src/
├── main.rs                    Entry point — builds router, starts server
├── lib.rs                     Shared library entry (used by Tauri too)
├── state.rs                   AppState: pool + plugin_manager + config
├── bootstrap/
│   └── config.rs              Configuration loading (TOML, env overrides)
├── modules/
│   ├── auth/                  JWT, sessions, API keys, Argon2 hashing
│   ├── post/                  CRUD, FTS5 search, slug generation
│   ├── comment/               Moderation queue, WebSocket push
│   ├── media/                 File upload, storage, category organization
│   ├── theme/                 MiniJinja rendering, ZIP upload, settings
│   ├── webhook/               Dispatcher, retry logic, delivery logs, HMAC
│   └── plugin/                Plugin trait, manager, hooks, action registry
│       ├── hook.rs            Hook, HookContext, HookData types
│       ├── hook_registry.rs   Dispatch logic (filter sync, action spawn)
│       ├── action_registry.rs Action lifecycle tracking
│       ├── manager.rs         PluginManager: init, routes, assets
│       ├── registry.rs        Global plugin registry (Lazy<Mutex>)
│       ├── asset.rs           PluginAsset: CSS/JS injection
│       └── settings.rs        Plugin settings persistence
├── shared/
│   ├── error.rs               AppError / AppResult types
│   └── auth.rs                Auth middleware and extractors
└── infra/                     Infrastructure abstractions
    └── plugin/                Plugin trait interface for external crates
```

## Performance Characteristics

Measurements from a $6/month VPS (1 vCPU, 1 GB RAM, DigitalOcean) serving the default theme with 100 cached posts:

| Metric | Value |
|---|---|
| **p95 response time** | < 10 ms (server-side) |
| **RAM idle** | ~ 20 MB |
| **Binary size** | ~ 15 MB (stripped, with embedded frontend) |
| **Database** | SQLite WAL mode, no separate process |
| **Startup time** | < 500 ms (cold), < 100 ms (warm) |

Colophon achieves these numbers through several design decisions:

- **Single binary, single process** — no IPC overhead between web server and database.
- **SQLite WAL mode** — concurrent reads scale well; writes are serialized by SQLite's internal locking.
- **Fire-and-forget actions** — webhooks, notifications, and search indexing never delay the HTTP response. The user always gets a response after the database commit, regardless of how many downstream integrations are configured.
- **Static plugin linking** — zero dynamic dispatch overhead. Disabled plugins cost nothing beyond the `plugin_status` table lookup.
- **Embedded frontend** — the React SPA is built at compile time and served from memory via `rust-embed`. No separate static file server, no CDN required for basic operation.

## Database Schema (Simplified)

The SQLite database uses WAL journal mode and foreign keys. Core tables:

- `posts` — id, title, slug, content_html, content_markdown, status, category_id, created_at, updated_at
- `post_tags` — junction table linking posts to tags
- `categories` — id, name, slug, parent_id (hierarchical)
- `tags` — id, name, slug
- `comments` — id, post_id, author_name, content, status (pending/approved/spam)
- `users` — id, username, password_hash (Argon2id), role
- `sessions` — id, user_id, token, expires_at
- `api_keys` — id, user_id, key_hash, scopes, expires_at
- `webhooks` — id, name, url, events, secret, enabled, max_retries
- `webhook_deliveries` — id, webhook_id, event, request_url, request_body, response_status, response_body, duration_ms, success
- `plugin_status` — plugin_id, enabled
- `plugin_settings` — plugin_id, key, value
- `options` — key-value store for site settings (title, description, theme, etc.)

## Further Reading

- [Quick Start](./quickstart.md) — 15-minute setup guide
- [Plugin Development](./plugin-guide.md) — build your own plugin
- [Webhook Guide](./webhook-guide.md) — configure event-driven integrations
