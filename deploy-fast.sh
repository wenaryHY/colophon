#!/bin/bash
set -euo pipefail

# ============================================================
# InkForge 快速部署脚本 (WSL 本地编译 → scp 上传)
# 用法: 在 WSL 中执行 bash deploy-fast.sh
# ============================================================

# ── 颜色定义 ────────────────────────────────────────────────
readonly COLOR_RESET='\033[0m'
readonly COLOR_RED='\033[0;31m'
readonly COLOR_GREEN='\033[0;32m'
readonly COLOR_YELLOW='\033[0;33m'
readonly COLOR_CYAN='\033[0;36m'
readonly COLOR_BOLD='\033[1m'

# ── 配置常量 ────────────────────────────────────────────────
readonly SSH_KEY_PATH="${HOME}/.ssh/id_ed25519"
readonly SERVER_IP="162.243.28.76"
readonly SERVER_USER="root"
readonly REMOTE_BINARY_PATH="/opt/inkforge/inkforge"
readonly REMOTE_DIST_PATH="/opt/inkforge/src/admin/dist"
readonly REMOTE_ADMIN_HTML_PATH="/opt/inkforge/src/admin/admin.html"
readonly REMOTE_THEME_TEMPLATES_PATH="/opt/inkforge/themes"
readonly REMOTE_THEME_STATIC_PATH="/opt/inkforge/themes"
readonly REMOTE_BACKUP_DIR="/root/inkforge-backups"
readonly HEALTH_CHECK_URL="http://127.0.0.1:2000/api/v1/health"
readonly SERVICE_NAME="inkforge"

# ── 脚本路径推导 ────────────────────────────────────────────
# 脚本放在项目根目录，SCRIPT_DIR 即 PROJECT_DIR
readonly PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ── 工具函数 ────────────────────────────────────────────────
log_info()    { echo -e "${COLOR_CYAN}[INFO]${COLOR_RESET}  $*"; }
log_success() { echo -e "${COLOR_GREEN}[OK]${COLOR_RESET}    $*"; }
log_warn()    { echo -e "${COLOR_YELLOW}[WARN]${COLOR_RESET}  $*"; }
log_error()   { echo -e "${COLOR_RED}[ERROR]${COLOR_RESET} $*"; }
log_step()    { echo -e "\n${COLOR_BOLD}${COLOR_CYAN}── 步骤 $1: $2 ──${COLOR_RESET}"; }

ssh_cmd() {
    ssh -i "${SSH_KEY_PATH}" \
        -o StrictHostKeyChecking=accept-new \
        -o ConnectTimeout=10 \
        "${SERVER_USER}@${SERVER_IP}" "$@"
}

scp_upload() {
    scp -i "${SSH_KEY_PATH}" \
        -o StrictHostKeyChecking=accept-new \
        -o ConnectTimeout=10 \
        "$@"
}

# ── 步骤 0: 环境检查 ────────────────────────────────────────
log_step "0" "环境检查"

if ! grep -qiE 'microsoft|wsl' /proc/version 2>/dev/null && ! grep -qi wsl /proc/sys/kernel/osrelease 2>/dev/null; then
    log_warn "未检测到 WSL 环境，继续执行（假设为 Linux）..."
fi

# 加载 Rust 环境（WSL 新终端可能未 source）
if [ -f "${HOME}/.cargo/env" ]; then
    source "${HOME}/.cargo/env"
fi

log_info "项目目录: ${PROJECT_DIR}"
cd "${PROJECT_DIR}"

# 检查必需工具
for tool in rustc cargo node npm ssh scp; do
    if ! command -v "${tool}" &>/dev/null; then
        log_error "未找到 ${tool}，请先安装"
        exit 1
    fi
done

# 检查 SSH 密钥
if [ ! -f "${SSH_KEY_PATH}" ]; then
    log_error "SSH 密钥不存在: ${SSH_KEY_PATH}"
    exit 1
fi

log_success "环境检查通过"
log_info "rustc: $(rustc --version)"
log_info "cargo: $(cargo --version)"
log_info "node:  $(node --version)"
log_info "npm:   $(npm --version)"

# ── 步骤 1: 拉取最新代码 ──────────────────────────────────────
log_step "1" "拉取最新代码"

if [ -d ".git" ]; then
    git pull origin master
    log_success "代码已更新到 $(git rev-parse --short HEAD)"
else
    log_warn "非 git 仓库，跳过拉取"
fi

# ── 步骤 2: 前端构建 ─────────────────────────────────────────
log_step "2" "前端构建"

FRONTEND_DIR="${PROJECT_DIR}/src/admin/ui"
FRONTEND_DIST_DIR="${PROJECT_DIR}/src/admin/dist"
ADMIN_HTML="${PROJECT_DIR}/src/admin/admin.html"

if [ ! -d "${FRONTEND_DIR}" ]; then
    log_error "前端目录不存在: ${FRONTEND_DIR}"
    exit 1
fi

cd "${FRONTEND_DIR}"
log_info "安装依赖..."
npm install --silent
log_info "构建前端..."
npm run build
cd "${PROJECT_DIR}"

# 将 admin.html 一并打包（服务器端需要在 dist 外提供入口页面）
cp "${ADMIN_HTML}" "${FRONTEND_DIST_DIR}/admin.html"
log_success "前端构建完成"

# ── 步骤 3: Rust 构建 ────────────────────────────────────────
log_step "3" "Rust release 构建 (多核并行)"

cargo build --release -p inkforge

readonly BINARY_PATH="${PROJECT_DIR}/target/release/inkforge"
if [ ! -f "${BINARY_PATH}" ]; then
    log_error "构建产物不存在: ${BINARY_PATH}"
    exit 1
fi

log_success "Rust 构建完成"
log_info "二进制大小: $(du -h "${BINARY_PATH}" | cut -f1)"

# ── 步骤 4: 上传到服务器 ──────────────────────────────────────
log_step "4" "上传到服务器"

# 4a. 上传二进制（先以 .new 落地）
log_info "上传二进制到 ${REMOTE_BINARY_PATH}.new ..."
scp_upload "${BINARY_PATH}" "${SERVER_USER}@${SERVER_IP}:${REMOTE_BINARY_PATH}.new"

# 4b. 打包并上传前端文件
log_info "打包前端文件..."
TAR_PATH="/tmp/inkforge-dist-$(date +%s).tar.gz"
tar -czf "${TAR_PATH}" -C "${FRONTEND_DIST_DIR}" .
log_info "上传前端文件..."
scp_upload "${TAR_PATH}" "${SERVER_USER}@${SERVER_IP}:/tmp/inkforge-dist.tar.gz"
rm -f "${TAR_PATH}"

# 4c. 上传主题模板 — 打包为 tar.gz
log_info "上传主题模板..."
THEME_SOURCE_DIR="${PROJECT_DIR}/themes"

if [ -d "${THEME_SOURCE_DIR}" ] && [ -d "${THEME_SOURCE_DIR}/templates" ]; then
    THEME_TAR="/tmp/inkforge-theme-$$.tar.gz"
    tar -czf "$THEME_TAR" -C "$THEME_SOURCE_DIR" templates static
    scp_upload "$THEME_TAR" "${SERVER_USER}@${SERVER_IP}:/tmp/"
    rm -f "$THEME_TAR"

    ssh_cmd "
        mkdir -p ${REMOTE_THEME_TEMPLATES_PATH} ${REMOTE_THEME_STATIC_PATH}
        tar -xzf /tmp/inkforge-theme-*.tar.gz -C ${REMOTE_THEME_TEMPLATES_PATH}/..
        rm -f /tmp/inkforge-theme-*.tar.gz
        chown -R inkforge:inkforge ${REMOTE_THEME_TEMPLATES_PATH} ${REMOTE_THEME_STATIC_PATH}
    "
    log_success "主题模板上传完成"
else
    log_warn "主题模板目录不存在，跳过"
fi

# 4d. 服务器端解压前端文件
log_info "服务器端解压前端文件..."
ssh_cmd "
    mkdir -p ${REMOTE_DIST_PATH}
    tar -xzf /tmp/inkforge-dist.tar.gz -C ${REMOTE_DIST_PATH}/
    cp ${REMOTE_DIST_PATH}/admin.html ${REMOTE_ADMIN_HTML_PATH}
    chown -R inkforge:inkforge /opt/inkforge/src/admin
    rm /tmp/inkforge-dist.tar.gz
"

log_success "全部文件上传完成"

# ── 步骤 5: 服务器切换部署 ────────────────────────────────────
log_step "5" "服务器切换部署"

log_info "开始服务器端切换 (备份 → 停止 → 替换 → 启动 → 健康检查)..."

ssh_cmd "bash -s" << 'REMOTE_SCRIPT'
set -e

COLOR_GREEN='\033[0;32m'
COLOR_RED='\033[0;31m'
COLOR_RESET='\033[0m'

# 5a. 确保备份目录存在
mkdir -p /root/inkforge-backups

# 5b. 数据库备份
echo "[server] 备份数据库..."
BACKUP_FILE="/root/inkforge-backups/predeploy-$(date -u +%Y%m%d-%H%M%S).db.sql.gz"
sqlite3 /var/lib/inkforge/inkforge.db '.dump' | gzip > "${BACKUP_FILE}"
echo "[server] 备份完成: ${BACKUP_FILE}"

# 5c. 保留旧版本
echo "[server] 保留旧版本二进制..."
cp /opt/inkforge/inkforge /opt/inkforge/inkforge.old 2>/dev/null || true

# 5d. 停止服务
echo "[server] 停止 inkforge 服务..."
systemctl stop inkforge
sleep 1

# 5e. 替换二进制
echo "[server] 替换二进制..."
mv /opt/inkforge/inkforge.new /opt/inkforge/inkforge
chown inkforge:inkforge /opt/inkforge/inkforge
chmod +x /opt/inkforge/inkforge

# 5f. 启动服务
echo "[server] 启动 inkforge 服务..."
systemctl start inkforge
sleep 3

# 5g. 健康检查
echo "[server] 健康检查..."
if curl -fsS http://127.0.0.1:2000/api/v1/health >/dev/null 2>&1; then
    echo -e "${COLOR_GREEN}[server] DEPLOY SUCCESS ${COLOR_RESET}"
else
    echo -e "${COLOR_RED}[server] DEPLOY FAILED - 回滚请执行:"
    echo "  cp /opt/inkforge/inkforge.old /opt/inkforge/inkforge"
    echo "  systemctl restart inkforge${COLOR_RESET}"
    exit 1
fi
REMOTE_SCRIPT

DEPLOY_EXIT_CODE=$?

# ── 清理本地临时文件 ──────────────────────────────────────────
rm -f "${FRONTEND_DIST_DIR}/admin.html"

# ── 结果 ──────────────────────────────────────────────────────
if [ ${DEPLOY_EXIT_CODE} -eq 0 ]; then
    echo ""
    log_success " 部署完成! https://wenary.me"
else
    echo ""
    log_error "部署失败，请检查服务器日志"
    exit ${DEPLOY_EXIT_CODE}
fi
