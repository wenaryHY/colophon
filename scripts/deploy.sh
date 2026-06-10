#!/bin/bash
set -e

# 加载 Rust 环境（WSL 新终端不会自动 source .bashrc）
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

echo "=== Colophon Deploy $(date) ==="

# 1. 换行符统一（WSL 和服务器之间 CRLF/LF 差异会导致 sqlx migration hash 不同）
echo "[1/4] Normalizing line endings..."
find migrations -name "*.sql" -exec dos2unix -q {} \; 2>/dev/null || echo "  (dos2unix not found, skipping)"

# 2. 编译 Rust release
echo "[2/4] Building Rust release..."
cargo build --release

# 3. 构建前端
echo "[3/4] Building frontend..."
cd src/admin/ui
npm install --silent 2>/dev/null
npm run build 2>&1 | tail -1
cd /mnt/d/codes/colophon

# 4. 部署到服务器

# 同步主题文件（模板 + 静态资源）
ssh root@162.243.28.76 "rm -rf /opt/colophon/themes 2>/dev/null; mkdir -p /opt/colophon/themes"
scp -r themes/* root@162.243.28.76:/opt/colophon/themes/

echo "[4/4] Deploying to server..."
ssh root@162.243.28.76 "systemctl stop colophon"
scp target/release/colophon root@162.243.28.76:/opt/colophon/colophon
# 打包前端文件（admin.html 暂存进 dist 目录），单文件 SCP 上传再解压
cp src/admin/admin.html src/admin/dist/admin.html
tar -czf /tmp/colophon-dist.tar.gz -C src/admin/dist .
scp /tmp/colophon-dist.tar.gz root@162.243.28.76:/tmp/
ssh root@162.243.28.76 "tar -xzf /tmp/colophon-dist.tar.gz -C /opt/colophon/src/admin/dist/ && cp /opt/colophon/src/admin/dist/admin.html /opt/colophon/src/admin/admin.html && chown -R colophon:colophon /opt/colophon/src/admin && rm /tmp/colophon-dist.tar.gz"
rm /tmp/colophon-dist.tar.gz
ssh root@162.243.28.76 "chown -R colophon:colophon /opt/colophon && systemctl start colophon && sleep 2 && curl -s http://127.0.0.1:2000/api/v1/health"

echo "=== Deploy SUCCESS ==="
