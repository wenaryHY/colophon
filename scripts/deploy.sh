#!/bin/bash
set -e

# ── 加载部署配置 ──────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "${SCRIPT_DIR}")"

if [ -f "${PROJECT_DIR}/.env.deploy" ]; then
    source "${PROJECT_DIR}/.env.deploy"
    echo "[INFO] 已加载 .env.deploy"
fi

# ── 配置常量（支持环境变量覆盖）────────────────────────────────
SERVER_IP="${DEPLOY_SERVER_IP:?错误: 未设置 DEPLOY_SERVER_IP，请在 .env.deploy 或环境变量中配置}"
SERVER_USER="${DEPLOY_SERVER_USER:-root}"
SERVER_PATH="${DEPLOY_SERVER_PATH:-/opt/colophon}"
SSH_KEY_PATH="${DEPLOY_SSH_KEY_PATH:-${HOME}/.ssh/id_ed25519}"
HEALTH_CHECK_URL="${DEPLOY_HEALTH_CHECK_URL:-http://127.0.0.1:2000/api/v1/health}"

# 加载 Rust 环境（WSL 新终端不会自动 source .bashrc）
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

echo "=== Colophon Deploy $(date) ==="
echo "[INFO] 目标服务器: ${SERVER_USER}@${SERVER_IP}:${SERVER_PATH}"

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
cd "${PROJECT_DIR}"

# 4. 部署到服务器

# 同步主题文件（模板 + 静态资源）
ssh -i "${SSH_KEY_PATH}" "${SERVER_USER}@${SERVER_IP}" "rm -rf ${SERVER_PATH}/themes 2>/dev/null; mkdir -p ${SERVER_PATH}/themes"
scp -i "${SSH_KEY_PATH}" -r themes/* "${SERVER_USER}@${SERVER_IP}:${SERVER_PATH}/themes/"

# 确保活动主题也有共享模板文件
ssh -i "${SSH_KEY_PATH}" "${SERVER_USER}@${SERVER_IP}" "for t in ${SERVER_PATH}/themes/*/; do cp -n ${SERVER_PATH}/themes/default/templates/_header.html \${t}templates/ 2>/dev/null; cp -n ${SERVER_PATH}/themes/default/templates/_footer.html \${t}templates/ 2>/dev/null; done; chown -R colophon:colophon ${SERVER_PATH}/themes"

echo "[4/4] Deploying to server..."
ssh -i "${SSH_KEY_PATH}" "${SERVER_USER}@${SERVER_IP}" "systemctl stop colophon"
# 更新 systemd 服务文件
scp -i "${SSH_KEY_PATH}" deploy/colophon.service "${SERVER_USER}@${SERVER_IP}:/etc/systemd/system/colophon.service"
ssh -i "${SSH_KEY_PATH}" "${SERVER_USER}@${SERVER_IP}" "mkdir -p /var/lib/colophon/static && chown colophon:colophon /var/lib/colophon/static && systemctl daemon-reload"
scp -i "${SSH_KEY_PATH}" target/release/colophon "${SERVER_USER}@${SERVER_IP}:${SERVER_PATH}/colophon"
# 打包前端文件（admin.html 暂存进 dist 目录），单文件 SCP 上传再解压
cp src/admin/admin.html src/admin/dist/admin.html
tar -czf /tmp/colophon-dist.tar.gz -C src/admin/dist .
scp -i "${SSH_KEY_PATH}" /tmp/colophon-dist.tar.gz "${SERVER_USER}@${SERVER_IP}:/tmp/"
ssh -i "${SSH_KEY_PATH}" "${SERVER_USER}@${SERVER_IP}" "tar -xzf /tmp/colophon-dist.tar.gz -C ${SERVER_PATH}/src/admin/dist/ && cp ${SERVER_PATH}/src/admin/dist/admin.html ${SERVER_PATH}/src/admin/admin.html && chown -R colophon:colophon ${SERVER_PATH}/src/admin && rm /tmp/colophon-dist.tar.gz"
rm /tmp/colophon-dist.tar.gz
ssh -i "${SSH_KEY_PATH}" "${SERVER_USER}@${SERVER_IP}" "chown -R colophon:colophon ${SERVER_PATH} && systemctl start colophon && sleep 2 && curl -s ${HEALTH_CHECK_URL}"

echo "=== Deploy SUCCESS ==="
