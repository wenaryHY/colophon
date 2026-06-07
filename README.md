# InkForge

[![English](https://img.shields.io/badge/lang-English-blue)](README.md)
[![中文](https://img.shields.io/badge/lang-中文-ff6b35)](README.zh-CN.md)

> A CMS that runs as a single file.
> No Node.js. No runtime. No Docker required.
> `scp` it to your server and you're done.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/wenaryHY/inkforge/main/scripts/install.sh | sudo bash
```

That's it. Open `http://YOUR_SERVER_IP:2000/admin` and follow the setup wizard.

<details>
<summary>What does the install script do?</summary>

1. Detects your OS (Ubuntu/Debian/CentOS) and architecture (x86_64/aarch64)
2. Installs system dependencies (sqlite3, ca-certificates)
3. Downloads the latest release binary from GitHub Releases
4. Creates a dedicated `inkforge` system user
5. Sets up directories: `/opt/inkforge` (app), `/var/lib/inkforge` (data), `/etc/inkforge` (secrets)
6. Generates a random JWT secret
7. Installs and starts the systemd service on port 2000

</details>

**Manage your installation:**

```bash
systemctl status inkforge     # check status
systemctl restart inkforge    # restart
journalctl -u inkforge -f     # view logs
```

**Update to a new version:**

```bash
curl -fsSL https://raw.githubusercontent.com/wenaryHY/inkforge/main/scripts/install.sh | sudo bash
```

Re-running the installer downloads the latest version and replaces the binary. Your data and config are preserved.

### Build from Source

```bash
# Prerequisites: Rust 1.75+, Node.js 22+, SQLite 3
git clone https://github.com/wenaryHY/inkforge.git
cd inkforge
cd src/admin/ui && npm ci && cd -
cargo build --release -p inkforge
cargo run --release
# → http://localhost:2000/admin — create your admin account
```

> 📖 **Full documentation**: [docs/quickstart.md](docs/quickstart.md) — 15-minute setup guide

On first launch, InkForge opens the setup wizard in your browser. Choose an admin username and password, pick a theme, and you are writing within 60 seconds. The frontend assets are prebuilt and embedded into the single binary — no reverse proxy, no separate Node process.

## Why InkForge?

InkForge is a blogging platform built around one conviction: your content stack should not require a DevOps team. Most CMS platforms run on Node.js or PHP, pull in dozens of dependencies at runtime, and idle at 150–300 MB of RAM. InkForge compiles to a single static binary that serves your blog, admin panel, and API from a single process using under 20 MB of memory.

Performance is not an afterthought — it is the foundation. The entire request path, from TLS termination to SQLite query, lives inside a Rust async runtime with zero garbage-collection pauses. This means sub-10ms p95 response times on commodity VPS hardware, even under the default SQLite WAL-mode configuration. No Redis, no opcache, no tuning required.

The plugin system is compile-time safe by design. Plugins are Rust crates that implement a trait — the compiler verifies type safety and API contracts before your site ever starts. When you want to disable a plugin, flip a boolean in the admin panel and it is gone from the request path. No runtime dynamic dispatch overhead, noeval, no monkey-patching.

## Features

- **Post and page content types** — dual-type system; pages can carry both Markdown body and custom HTML
- **Dual-mode editor** — Tiptap WYSIWYG and CodeMirror source mode, switchable with one click
- **Web-based setup wizard** — first-run installation flow with status backfill and admin path configuration
- **Hierarchical categories and tags** — nested category trees with multi-tag association
- **Comment system with moderation** — approval queue with real-time WebSocket push
- **Media library** — local storage, category-based organization, support for images and audio
- **Unified authentication** — Argon2 password hashing, JWT + session cookies + API keys, 7-day persistent login
- **Theme engine** — MiniJinja templating with visual configuration panel and ZIP upload
- **Live preview** — FAB trigger with inline, modal, and new-tab preview modes; theme-switch preview
- **Full-text search** — SQLite FTS5 with incremental indexing
- **Unified trash bin** — posts, categories, tags, and comments share one trash with scheduled purge
- **SEO toolkit** — auto-generated sitemap, robots.txt, OpenGraph and JSON-LD metadata
- **Webhook callbacks** — HTTP notifications on post publish and update events, configurable per URL
- **Backup and restore** — local backup with one-click restore and cron-scheduled snapshots
- **API versioning** — `/api/v1/` stable routes with legacy route fallback
- **Responsive admin panel** — three-breakpoint sidebar, card-ified table layout on mobile, collapsible editor panels
- **i18n** — admin interface supports multiple languages
- **Plugin system** — Rust trait-based: custom API routes, template functions, frontend assets, lifecycle hooks, settings panels, and UI slots
- **Single-binary deployment** — WSL cross-compilation pipeline produces one tarball containing binary and assets
- **Database abstraction** — `SqlitePool` behind an `Executor` trait for testability and backend portability

## Performance

| | InkForge | Ghost | WordPress |
|---|---|---|---|
| **Language** | Rust | Node.js | PHP |
| **Response (p95)** | <10ms | ~50ms | ~200ms |
| **RAM idle** | ~20MB | ~150MB | ~256MB |
| **Monthly VPS** | $6 | $15 | $20 |

Measured on a $6/mo VPS (1 vCPU, 1 GB RAM) serving the default theme with 100 cached posts. Response times are server-side p95; end-to-end latency depends on CDN and network. InkForge runs comfortably on the smallest DigitalOcean droplet; Ghost and WordPress typically need the next tier up for comparable reliability.

## Architecture

- **Backend:** Rust + Axum 0.8 + SQLite with WAL mode (via `sqlx`)
- **Frontend:** React 19 + TypeScript + Vite 8, embedded at build time
- **Auth:** JWT with refresh tokens + Argon2 hashing + API key for headless CMS access
- **Plugins:** compile-time registration via `build.rs` auto-discovery + runtime enable/disable toggle
- **Webhooks:** HTTP POST callbacks triggered by post lifecycle events with retry and timeout
- **Themes:** MiniJinja template engine; themes are ZIP archives with `theme.toml` manifest
- **Search:** SQLite FTS5 virtual tables, incrementally rebuilt on content change
- **Desktop shell:** Tauri 2 in-process mode, sharing the same `lib.rs` entry point as the web server

## Plugin Example

A minimal plugin that logs every published post. Create two files under `plugins/hello-world/`:

**`plugin.toml`**

```toml
[plugin]
id = "hello-world"
title = "Hello World"
version = "0.1.0"
description = "Logs a message when a post is published"
author = "You"

[engine]
inkforge = ">=1.0.0"

[hooks]
template = false
routes = false
assets = []
```

**`src/lib.rs`**

```rust
use async_trait::async_trait;
use std::sync::Arc;

use crate::modules::plugin::hook::{Hook, HookContext, HookData, HookHandler};
use crate::modules::plugin::Plugin;
use crate::shared::error::AppResult;

pub struct HelloPlugin;

impl HelloPlugin {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Plugin for HelloPlugin {
    fn name(&self) -> &str { "hello-world" }
    fn version(&self) -> &str { "0.1.0" }

    fn hooks(&self) -> Vec<Hook> {
        struct PublishLogger;

        #[async_trait]
        impl HookHandler for PublishLogger {
            async fn run(&self, ctx: &mut HookContext) -> AppResult<()> {
                if let HookData::PostAfterPublish(ref data) = ctx.data {
                    tracing::info!(
                        "Post published: {} (slug: {})",
                        data.title,
                        data.slug,
                    );
                }
                Ok(())
            }
        }

        vec![Hook::new_action(
            "post.after_publish",
            10,
            self.name(),
            Arc::new(PublishLogger),
        )]
    }
}
```

Rebuild the project — `build.rs` discovers the plugin directory automatically and links it into the binary. Enable or disable it from the admin panel at runtime.

## Deploy

### One-command (Linux VPS via WSL)

```bash
bash deploy-fast.sh
```

Builds the frontend and Rust binary locally inside WSL, uploads both to your server via `scp`, backs up the database, swaps the binary, and restarts the systemd service. A health check confirms the deploy succeeded before the script exits. See `docs/DEPLOY.md` for server setup prerequisites (user, data directories, systemd unit).

### Docker

```bash
docker build -t inkforge .
docker run -d \
  -p 3000:3000 \
  -e INKFORGE__AUTH__SECRET="$(openssl rand -hex 32)" \
  -v inkforge-uploads:/app/uploads \
  -v inkforge-backups:/app/backups \
  -v inkforge-data:/app/data \
  inkforge
```

The image includes Litestream for continuous SQLite replication to S3-compatible storage. Configure replication in `config/litestream.yml`.

### Binary

Download a prebuilt binary from the [Releases](https://github.com/wenaryHY/inkforge/releases) page, or build from source:

```bash
cd src/admin/ui && npm ci && npm run build && cd -
cargo build --release -p inkforge
```

Copy `target/release/inkforge`, your `config/` directory, `migrations/`, and `themes/` to your server. Run the binary directly — no runtime dependencies beyond `libsqlite3`.

## Security

- **Brute-force protection:** login rate limiting via `governor` with configurable burst and per-second quotas
- **Password storage:** Argon2id hashing with random per-password salt
- **Session management:** HTTP-only secure cookies with 7-day expiry and server-side revocation
- **API keys:** scoped keys for headless CMS access, revocable from the admin panel
- **Spam prevention:** built-in honeypot fields and optional Cloudflare Turnstile integration
- **Content sanitization:** user-submitted HTML is cleaned through `ammonia` before rendering
- **Dependency audit:** every `cargo audit` run against the full dependency tree (see Security Audit section below for latest results)

## Comparison

InkForge is a good fit for personal blogs, developer portfolios, documentation sites, and small-to-medium publications where speed and low operating cost matter more than an ecosystem of third-party integrations.

Ghost offers a more mature admin experience, a built-in membership and newsletter system, and a larger theme marketplace. If you need subscription billing or a multi-author newsroom workflow today, Ghost is the safer choice. However, Ghost runs on Node.js and idles at roughly 7–8× the memory footprint of InkForge.

WordPress has the largest plugin ecosystem of any CMS by an order of magnitude. If your site depends on a specific WooCommerce extension, a page builder, or a deep SEO plugin chain, WordPress is the pragmatic option. The tradeoff is runtime cost and attack surface — WordPress sites require regular patching, a PHP opcache layer, and typically a separate caching reverse proxy to achieve response times comparable to InkForge out of the box.

InkForge's plugin system is Rust-native: plugins are compiled, statically linked, and verified by the type system before deployment. This is fundamentally different from PHP or JavaScript plugin models — safer by construction, but with a higher bar for plugin authorship.

## License

**AGPLv3** (starting from v1.0.0). See [LICENSE](LICENSE).

You may self-host InkForge for free under the terms of the AGPLv3. If you wish to offer InkForge as a commercial SaaS without releasing your modifications, please contact the authors to discuss an alternative license.

## Roadmap

### Now (Q2 2026)

- [x] Post lifecycle action tracking
- [x] Webhook reliability improvements with retry logic
- [ ] Mobile editor UX polish
- [ ] English documentation site

### Next (Q3 2026)

- [ ] Multi-language content support (per-post locale)
- [ ] Theme marketplace with one-click install
- [ ] Managed hosting early access

### Later

- [ ] Custom content types via `custom_fields` JSON column
- [ ] GraphQL API alongside REST
- [ ] Image lazy-loading with blur-up placeholders

## Security Audit

`cargo audit` scan of 760 crates (2026-06-02). **No critical vulnerabilities in the server binary.**

| Vulnerability | Severity | Path | Status |
|---|---|---|---|
| `sqlx` binary protocol overflow | Medium | sqlx-mysql / sqlx-postgres (SQLite unaffected) | Benign |
| `rsa` timing side-channel | Medium | rsa → sqlx-mysql (MySQL only) | Benign |
| `rustls-webpki` CRL panic | High | aws-sdk-s3 (not currently enabled) | Benign |
| `rustls-webpki` wildcard cert | High | aws-sdk-s3 (not currently enabled) | Benign |
| `glib` unsafe iterator | Undefined | tauri → webkit2gtk (desktop only) | Benign |
| `lru` dangling pointer | Undefined | aws-sdk-s3 (not currently enabled) | Benign |
| `rand` custom logger | Undefined | tauri-utils (desktop only) | Benign |
| 12 unmaintained warnings | — | gtk-rs crates (desktop only) | Benign |

**Before enabling these features, upgrade the listed dependencies:**

- **PostgreSQL / MySQL backend** → upgrade `sqlx` to ≥0.8.1
- **S3 object storage** → upgrade `rustls-webpki` to ≥0.103.13
- **Tauri desktop shell** → upgrade the full Tauri toolchain

## Contributing

Pull requests are welcome. Write commit messages and documentation in English where possible. See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guidelines.
