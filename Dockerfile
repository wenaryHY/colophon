# ── Stage 1: 前端构建 ──
FROM node:20-slim AS frontend
WORKDIR /app/ui
COPY src/admin/ui/package.json src/admin/ui/package-lock.json* ./
RUN npm ci
COPY src/admin/ui/ .
RUN npx tsc -b && npx vite build

# ── Stage 2: Rust 构建 ──
FROM rust:slim AS backend
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
# 预创建空 src 以缓存依赖编译
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
COPY migrations ./migrations
COPY themes ./themes
COPY config ./config
RUN apt-get update && apt-get install -y pkg-config libsqlite3-dev && \
    touch src/main.rs && cargo build --release && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# ── Stage 3: 最终镜像 ──
FROM debian:bookworm-slim
ARG LITESTREAM_VERSION=0.5.11

RUN groupadd -r colophon && useradd -r -g colophon -d /app -s /sbin/nologin colophon

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libsqlite3-0 curl gettext-base && \
    curl -fsSL "https://github.com/benbjohnson/litestream/releases/download/v${LITESTREAM_VERSION}/litestream-${LITESTREAM_VERSION}-linux-x86_64.deb" -o /tmp/litestream.deb && \
    apt-get install -y /tmp/litestream.deb && rm -f /tmp/litestream.deb && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 二进制
COPY --from=backend /app/target/release/colophon /app/colophon
# 数据
COPY --from=backend /app/migrations /app/migrations
COPY --from=backend /app/themes /app/themes
COPY --from=backend /app/config /app/config
# 前端产物
COPY --from=frontend /app/dist /app/src/admin/dist
# 入口脚本
COPY docker/entrypoint.sh /app/entrypoint.sh

RUN mkdir -p /app/uploads /app/backups /app/data && \
    chmod +x /app/entrypoint.sh && \
    chown -R colophon:colophon /app/uploads /app/backups /app/data

ENV RUST_LOG=info
ENV COLOPHON__SERVER__PORT=3000
ENV COLOPHON__DATABASE__URL=sqlite:///app/data/colophon.db?mode=rwc
ENV COLOPHON__STORAGE__UPLOAD_DIR=/app/uploads
ENV COLOPHON__PATHS__ADMIN_DIST_DIR=/app/src/admin/dist
ENV LITESTREAM_ENABLED=1

EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s CMD curl -f http://localhost:3000/api/v1/health || exit 1
VOLUME ["/app/uploads", "/app/backups", "/app/data"]
USER colophon
ENTRYPOINT ["/app/entrypoint.sh"]
