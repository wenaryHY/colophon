#!/bin/bash
set -e

echo "=== InkForge Deploy $(date) ==="

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
cd /mnt/d/codes/inkforge

# 4. 部署到服务器
echo "[4/4] Deploying to server..."
ssh root@162.243.28.76 "systemctl stop inkforge"
scp target/release/inkforge root@162.243.28.76:/opt/inkforge/inkforge
scp -r src/admin/dist/* root@162.243.28.76:/opt/inkforge/src/admin/dist/
scp src/admin/admin.html root@162.243.28.76:/opt/inkforge/src/admin/admin.html
ssh root@162.243.28.76 "chown -R inkforge:inkforge /opt/inkforge && systemctl start inkforge && sleep 2 && curl -s http://127.0.0.1:2000/api/v1/health"

echo "=== Deploy SUCCESS ==="
