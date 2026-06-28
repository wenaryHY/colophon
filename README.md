# Colophon

[![English](https://img.shields.io/badge/lang-English-blue)](README.md)
[![中文](https://img.shields.io/badge/lang-中文-ff6b35)](README.zh-CN.md)

A CMS for the $6 VPS. Single binary, single-file backup, <20 MB idle memory. No Node runtime, no reverse proxy, no Docker required.

## Quick Start

**One-command install (Linux VPS):**

```bash
curl -fsSL https://raw.githubusercontent.com/wenaryHY/colophon/master/scripts/install.sh | bash
```

Open `http://YOUR_IP:2000/admin` -- the setup wizard runs on first launch. Supports Debian/Ubuntu (apt) and Fedora/CentOS (dnf/yum) on x86_64 and aarch64.

**Build from source (3 commands):**

```bash
git clone https://github.com/wenaryHY/colophon.git && cd colophon
cd src/admin/ui && npm ci && cd - && cargo build --release -p colophon
cargo run --release
```

Requires Rust 1.75+, Node.js 22+, SQLite 3. The admin panel is fully embedded in the binary -- no separate frontend server.

**Development:**

Run the backend server and frontend Vite dev server concurrently:

```bash
npm install
npm run dev
```

**Docker:**

```bash
docker build -t colophon .
docker run -d -p 3000:3000 \
  -e COLOPHON__AUTH__SECRET="$(openssl rand -hex 32)" \
  -v colophon-data:/app/data \
  colophon
```

The Docker image includes Litestream for continuous SQLite replication to S3-compatible storage.

## Why Colophon

Most CMS platforms run on Node.js or PHP, pull in dozens of runtime dependencies, and idle at 150--500 MB of RAM. Colophon compiles to a single static binary that serves your site, admin panel, and API from one process using **under 20 MB of memory**.

The entire request path -- from TLS termination to SQLite query -- lives inside a Rust async runtime with zero garbage-collection pauses. This yields **sub-10ms p95 response times** on commodity VPS hardware, even with SQLite WAL mode out of the box. No Redis, no opcache, no tuning required.

Backup generates a ZIP archive containing the database and media files. (Roadmap: Q3 will migrate media to SQLite BLOB for true single-file backup — `cp colophon.db` is all you need.)

## Comparison

Colophon competes in the headless CMS and blogging-platform space. The table below compares it against the most common alternatives on the dimensions that matter for self-hosted deployments.

| | Colophon | Strapi | Directus | Ghost | WordPress |
|---|---|---|---|---|---|
| **Language** | Rust | Node.js | Node.js | Node.js | PHP |
| **Idle RAM** | ~20 MB | ~300 MB | ~250 MB | ~150 MB | ~256 MB |
| **Response p95** | <10 ms | -- | -- | ~50 ms | ~200 ms |
| **Binary / deps size** | ~14 MB (single file) | ~500 MB (node_modules) | ~400 MB (node_modules) | ~300 MB (node_modules) | N/A |
| **Backup** | One ZIP (DB + media) | DB dump + uploads/ | DB dump + uploads/ | DB + content/ | DB dump + wp-content/ |
| **Deployment** | Copy binary, run | npm install, configure, node server + DB | npm install, configure, node server + DB | Ghost CLI + Node + DB | LAMP/LEMP stack |
| **Min VPS** | 512 MB ($4/mo) | 2 GB ($18/mo) | 2 GB ($18/mo) | 1 GB ($6/mo) | 1 GB ($6/mo) |
| **Database** | SQLite (zero-config) | PostgreSQL / MySQL / SQLite | PostgreSQL / MySQL / SQLite | MySQL | MySQL |
| **Plugin model** | WebAssembly (Wasm) sandbox | JavaScript, runtime | JavaScript, runtime | JavaScript, runtime | PHP, runtime |
| **License** | AGPLv3 | MIT | BSL / MIT | MIT | GPLv2 |

Colophon's plugin system is WebAssembly-native: plugins are pre-compiled `.wasm` modules executed in an isolated Extism sandbox at runtime. This is fundamentally different from Node or PHP plugin models -- safer by construction with zero risk of crashing the core server.

## Architecture

```
                  ┌──────────────────────────────────┐
                  │      Axum Router (port 2000)      │
                  └──────────────┬───────────────────┘
         ┌───────────────────────┼──────────────────────┐
         │                       │                      │
    /api/v1/*               /admin/*               /* (public)
         │                       │                      │
   JWT / API Key           SPA handler            Theme renderer
    auth layer            (React, embedded)       (MiniJinja + DB)
         │                       │                      │
   Handler -- SQLite            --         Filter hooks (pre-render)
         │                                          │
   Filter hooks (pre-save)                   Render HTML
         │                                          │
   DB commit                                     Response
         │
   Action hooks (fire-and-forget)
         │
   Webhooks / Plugins / Email
```

- **Backend:** Rust + Axum 0.8 + SQLite with WAL mode (via `sqlx`)
- **Frontend:** React 19 + TypeScript + Vite 8, compiled and embedded at build time
- **Auth:** Argon2id password hashing, JWT with refresh tokens, session cookies, API keys
- **Templates:** MiniJinja engine; themes are ZIP archives with a `theme.toml` manifest and visual config panel
- **Search:** SQLite FTS5 virtual tables, incrementally rebuilt on content change
- **Desktop:** Tauri 2 (optional), sharing the same `lib.rs` entry point as the server

## Features

### Content

| Feature | Description |
|---|---|
| Dual content types | Posts and Pages with separate URL namespaces |
| Dual-mode editor | Tiptap WYSIWYG and CodeMirror source mode, one-click toggle |
| Hierarchical taxonomy | Nested category trees with multi-tag association |
| Full-text search | SQLite FTS5 with incremental indexing on content change |
| SEO toolkit | Auto-generated sitemap, robots.txt, OpenGraph and JSON-LD metadata |
| Unified trash bin | Posts, categories, tags, and comments share one trash with scheduled purge |

### Media

| Feature | Description |
|---|---|
| Media library | Local storage with category-based organization |
| Supported formats | Images (WebP, PNG, JPEG, GIF, SVG) and audio (MP3, WAV, OGG) |
| Cover images | Per-post cover with automatic thumbnail generation |

### Publishing

| Feature | Description |
|---|---|
| Comment system | Moderation queue with real-time WebSocket push |
| Webhook callbacks | HTTP POST on post lifecycle events with retry and timeout |
| Post lifecycle tracking | Action history for publish, update, trash events |

### Security

| Feature | Description |
|---|---|
| Password hashing | Argon2id with random per-password salt |
| Session management | HTTP-only cookies, 7-day expiry, server-side revocation |
| Brute-force protection | Login rate limiting via `governor` |
| Content sanitization | User-submitted HTML cleaned through `ammonia` |
| Spam prevention | Built-in honeypot fields + optional Cloudflare Turnstile |

### DevOps

| Feature | Description |
|---|---|
| Single binary deploy | One file + one config directory; scp to your server |
| One-command deploy script | Builds, uploads, backs up DB, restarts service, health-checks |
| Docker support | Official image with Litestream for continuous SQLite replication |
| Backup & restore | Local snapshot with one-click restore and cron scheduling |
| API versioning | `/api/v1/` stable routes with legacy fallback |

### Admin UX

| Feature | Description |
|---|---|
| Modern stack | React 19 + TypeScript + Vite 8, embedded at build time |
| Responsive design | Three-breakpoint sidebar, card-ified table layout on mobile |
| Live preview | FAB trigger with inline, modal, and new-tab preview modes |
| Theme config | Visual configuration panel per theme (color, layout, text options) |
| i18n | Admin interface supports multiple languages |

## Extension

Colophon offers two extension paths, designed for different levels of technical investment.

### Webhooks (zero-code)

```
 ┌──────────┐   post.after_publish    ┌──────────────┐
 │ Colophon │ ──────────────────────► │ Your Service │
 └──────────┘   HTTP POST + JSON      └──────────────┘
                                        (rebuild, notify,
                                         index, archive...)
```

Configure webhook URLs in the admin panel. Colophon fires an HTTP POST with a JSON payload on every post lifecycle event. Built-in retry logic, concurrency control, and delivery logging. See [Webhook Guide](docs/webhook-guide.md).

### Plugins (WebAssembly, dynamic sandboxing)

```
 ┌──────────┐
 │ Colophon │
 │          │   Extism SDK
 │ ┌──────┐ │   ┌─────────────┐
 │ │ Core │◄┼───┤ Plugin.wasm │   hooks()           -- lifecycle filters/actions
 │ └──────┘ │   │ (sandboxed) │   settings()        -- custom runtime configuration
 │          │   └─────────────┘
 │ Admin UI │
 └──────────┘
```

Plugins are pre-compiled WebAssembly (`.wasm`) modules discovered from the `plugins/` directory. They run dynamically inside a secure WebAssembly sandbox powered by **Extism**, requiring no recompilation of the main Rust binary.

Each plugin is strictly isolated:
- **Zero Network/FS Access** -- Network and file system calls are blocked by default.
- **Resource Limits** -- Capped memory footprint (10 MB max) and runtime execution timeout (5s max).
- **Webhooks & Hooks** -- Register Filter hooks (synchronous, mutate data) and Action hooks (fire-and-forget, non-blocking) to extend system logic dynamically.

Enable or disable plugins instantly from the admin panel. See [Plugin Guide](docs/plugin-guide.md).

## Performance

Measured on a $6/mo VPS (1 vCPU, 1 GB RAM) serving the default theme with ~100 posts. Benchmarks run with Criterion on Rust 1.75+.

### Database query

| Operation | Colophon | Note |
|---|---|---|
| Single-row lookup (by slug) | ~30.5 us | Indexed |
| Single-row lookup (by id) | ~33 us | Indexed |
| List 20 posts | ~124 us | SELECT with LIMIT 20 |
| Insert post | ~39.6 us | INSERT with indexes |

### Vs. Strapi / Directus (single-row query)

| | Colophon | Strapi | Directus |
|---|---|---|---|
| Single-row lookup | ~33 us | ~500 us | ~400 us |
| Ratio | 1x | 15x slower | 12x slower |

### JSON serialization

| Data size | Serialize | Deserialize |
|---|---|---|
| 1 post | ~350 ns | ~420 ns |
| 10 posts | ~3.3 us | ~4.2 us |
| 100 posts | ~32.0 us | ~43.3 us |

SQLite with WAL mode and proper indexing handles concurrent reads and writes without external coordination. No connection pooling, no cache warming, no read replicas.

Full benchmark methodology and scripts: [benches/BASELINE.md](benches/BASELINE.md).

## Roadmap

### Now (Q2 2026)

- [x] Post lifecycle action tracking
- [x] Webhook reliability improvements with retry logic
- [x] `colophon export` command — exports JSON + media for Astro/Next.js static generation
- [ ] Mobile editor UX polish
- [ ] English documentation site

### Next (Q3 2026)

- [ ] Media assets migration to SQLite BLOB — true single-file backup (`cp colophon.db`)
- [ ] Multi-language content support (per-post locale)
- [ ] Theme marketplace with one-click install
- [ ] Managed hosting early access

### Later

- [ ] Custom content types via `custom_fields` JSON column
- [ ] GraphQL API alongside REST
- [ ] Image lazy-loading with blur-up placeholders

## Static Export (since Q2 2026)

The `colophon export` command extracts all content and media for static site generation:

```bash
# Export all content and media
colophon export --output ./static-data

# Use directly in your frontend build
# Astro: import posts from '../static-data/posts.json'
# Next.js: const posts = require('./static-data/posts.json')
```

## Community

- **Issues**: [github.com/wenaryHY/colophon/issues](https://github.com/wenaryHY/colophon/issues)
- **Contributing**: [CONTRIBUTING.md](CONTRIBUTING.md)
- **Discussions**: [github.com/wenaryHY/colophon/discussions](https://github.com/wenaryHY/colophon/discussions)

Pull requests are welcome. See the [Contributing Guide](CONTRIBUTING.md) for setup instructions and the full workflow.

## License

**AGPLv3** (starting from v1.0.0). See [LICENSE](LICENSE).

You may self-host Colophon for free under the terms of the AGPLv3. If you wish to offer Colophon as a commercial SaaS without releasing your modifications, contact the authors to discuss an alternative license.
