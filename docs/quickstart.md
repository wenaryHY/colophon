# Quick Start

InkForge compiles to a single static binary — no reverse proxy, no separate Node process, no runtime dependency beyond `libsqlite3`. You will be writing in under two minutes.

## Prerequisites

- **Rust 1.85+** — `rustup default stable`
- **Node.js 22+** — for building the React admin panel
- **SQLite 3** — shipped with most Linux distributions; macOS has it built in

Verify your toolchain:

```bash
rustc --version   # rustc 1.85.0 or later
node --version    # v22.0.0 or later
sqlite3 --version # 3.x
```

## Install & Run

```bash
git clone https://github.com/wenaryHY/inkforge.git
cd inkforge
cd src/admin/ui && npm ci && cd -
cargo build --release -p inkforge
cargo run --release
```

Open **http://localhost:2000/admin** — the setup wizard guides you through creating your admin account and picking a theme. No config file editing required.

On first launch, InkForge creates a SQLite database at `inkforge.db` in the project root, runs migrations automatically, and starts serving on port 2000. Both the admin panel and the public site share the same port.

## First Post

1. Click **"New Post"** in the sidebar.
2. Write in Markdown (source mode) or switch to WYSIWYG with the toggle in the toolbar.
3. Add tags, a category, and an excerpt in the right sidebar.
4. Click **"Publish"**.

Your post is now live at `http://localhost:2000/posts/{slug}` — for example, if your slug is `hello-world`, visit `http://localhost:2000/posts/hello-world`.

## Deploy to a VPS

For Linux servers, the fastest path is the one-command deploy script:

```bash
bash deploy-fast.sh
```

It builds the frontend and Rust binary inside your local environment, uploads everything to your server via `scp`, backs up the database, swaps the binary, restarts the `systemd` service, and runs a health check — all in one shot. See the [Deployment Guide](./deploy.md) for server setup prerequisites (user, data directories, systemd unit).

If you prefer Docker:

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

The Docker image includes Litestream for continuous SQLite replication to S3-compatible storage.

## Next Steps

- [Plugin Development](./plugin-guide.md) — extend InkForge with Rust-native plugins
- [Webhook Configuration](./webhook-guide.md) — trigger external services on post events
- [Architecture Overview](./architecture.md) — understand the design and performance characteristics
