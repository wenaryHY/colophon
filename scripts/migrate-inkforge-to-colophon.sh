#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# Colophon 服务器迁移脚本
# 功能：将 inkforge 重命名为 colophon（零数据丢失）
# 用法：在服务器上以 root 执行
# ============================================================

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}[migrate]${NC} $*"; }
success() { echo -e "${GREEN}[migrate]${NC} $*"; }
warn()  { echo -e "${YELLOW}[migrate]${NC} $*"; }
error() { echo -e "${RED}[migrate]${NC} $*" >&2; exit 1; }

[[ $EUID -eq 0 ]] || error "此脚本必须以 root 执行"

echo ""
echo "════════════════════════════════════════════════════════"
echo "  Colophon 服务器迁移：inkforge → colophon"
echo "════════════════════════════════════════════════════════"
echo ""

# 0. 预检查
info "预检查..."
if [ ! -d "/opt/inkforge" ]; then
    error "/opt/inkforge 不存在，无需迁移"
fi
if [ -d "/opt/colophon" ]; then
    error "/opt/colophon 已存在，请先手动处理"
fi
if ! systemctl is-active --quiet inkforge; then
    warn "inkforge 服务未运行"
fi
if ! id inkforge &>/dev/null; then
    error "用户 inkforge 不存在"
fi
success "预检查通过"

# 1. 备份数据库
info "步骤 1/8: 备份数据库..."
BACKUP_FILE="/root/pre-rename-$(date -u +%Y%m%d-%H%M%S).db.sql.gz"
if [ -f "/var/lib/inkforge/inkforge.db" ]; then
    sqlite3 /var/lib/inkforge/inkforge.db '.dump' | gzip > "${BACKUP_FILE}"
    success "备份完成: ${BACKUP_FILE}"
else
    warn "数据库文件不存在，跳过备份"
fi

# 2. 停止服务
info "步骤 2/8: 停止 inkforge 服务..."
systemctl stop inkforge
success "服务已停止"

# 3. 重命名目录
info "步骤 3/8: 重命名目录..."
[ -d "/opt/inkforge" ] && mv /opt/inkforge /opt/colophon && success "  /opt/inkforge → /opt/colophon"
[ -d "/var/lib/inkforge" ] && mv /var/lib/inkforge /var/lib/colophon && success "  /var/lib/inkforge → /var/lib/colophon"
[ -d "/etc/inkforge" ] && mv /etc/inkforge /etc/colophon && success "  /etc/inkforge → /etc/colophon"
[ -d "/var/backups/inkforge" ] && mv /var/backups/inkforge /var/backups/colophon && success "  /var/backups/inkforge → /var/backups/colophon"

# 4. 重命名用户和组
info "步骤 4/8: 重命名用户和组..."
groupmod -n colophon inkforge
usermod -l colophon inkforge
success "  inkforge:inkforge → colophon:colophon"

# 5. 修复文件所有权
info "步骤 5/8: 修复文件所有权..."
[ -d "/opt/colophon" ] && chown -R colophon:colophon /opt/colophon
[ -d "/var/lib/colophon" ] && chown -R colophon:colophon /var/lib/colophon
[ -d "/etc/colophon" ] && chown -R colophon:colophon /etc/colophon
[ -d "/var/backups/colophon" ] && chown -R colophon:colophon /var/backups/colophon
success "所有权已更新"

# 6. 创建新的 systemd 服务文件
info "步骤 6/8: 创建新的 systemd 服务文件..."
cat > /etc/systemd/system/colophon.service <<'EOF'
[Unit]
Description=Colophon CMS
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=colophon
Group=colophon
WorkingDirectory=/opt/colophon
ExecStart=/opt/colophon/colophon
Restart=always
RestartSec=5
Environment=RUST_LOG=colophon=info
Environment=COLOPHON__RUNTIME__MODE=production
Environment=COLOPHON__SERVER__HOST=127.0.0.1
Environment=COLOPHON__SERVER__PORT=2000
Environment=COLOPHON__DATABASE__URL=sqlite:///var/lib/colophon/colophon.db?mode=rwc
Environment=COLOPHON__STORAGE__UPLOAD_DIR=/var/lib/colophon/uploads
Environment=COLOPHON__STORAGE__STATIC_DIR=/var/lib/colophon/static
Environment=COLOPHON__THEME__THEME_DIR=/var/lib/colophon/themes
Environment=COLOPHON__PATHS__ADMIN_DIST_DIR=/opt/colophon/src/admin/dist
EnvironmentFile=/etc/colophon/colophon.env
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/colophon /var/backups/colophon
ReadOnlyPaths=/opt/colophon /etc/colophon

[Install]
WantedBy=multi-user.target
EOF
success "服务文件已创建"

# 7. 重载并启用新服务
info "步骤 7/8: 重载并启用新服务..."
systemctl daemon-reload
systemctl disable inkforge --now 2>/dev/null || true
systemctl enable colophon
systemctl start colophon
sleep 3
success "服务已启动"

# 8. 健康检查
info "步骤 8/8: 健康检查..."
if curl -fsS http://127.0.0.1:2000/api/v1/health >/dev/null 2>&1; then
    success "健康检查通过 ✓"
else
    error "健康检查失败，请检查日志: journalctl -u colophon -n 50"
fi

echo ""
echo "════════════════════════════════════════════════════════"
echo -e "${GREEN}  ✅ 迁移完成！${NC}"
echo "════════════════════════════════════════════════════════"
echo ""
echo "  数据库备份: ${BACKUP_FILE}"
echo "  新服务名称: colophon"
echo ""
echo "  管理命令:"
echo "    systemctl status colophon"
echo "    systemctl restart colophon"
echo "    journalctl -u colophon -f"
echo ""
echo "  旧服务文件可删除:"
echo "    rm /etc/systemd/system/inkforge.service"
echo "    systemctl daemon-reload"
echo ""
