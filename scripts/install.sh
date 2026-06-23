#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────
#  Colophon Installer
#  Usage: curl -fsSL https://raw.githubusercontent.com/wenaryHY/colophon/main/scripts/install.sh | bash
# ─────────────────────────────────────────────

GITHUB_REPO="wenaryHY/colophon"
INSTALL_DIR="/opt/colophon"
DATA_DIR="/var/lib/colophon"
CONF_DIR="/etc/colophon"
BACKUP_DIR="/var/backups/colophon"
SERVICE_USER="colophon"
LISTEN_PORT=2000

# ── Colors ──
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}[colophon]${NC} $*"; }
warn()  { echo -e "${YELLOW}[colophon]${NC} $*"; }
error() { echo -e "${RED}[colophon]${NC} $*" >&2; exit 1; }

# ── 1. Pre-flight checks ──
[[ $EUID -eq 0 ]] || error "This script must be run as root (use sudo)."

command -v systemctl >/dev/null 2>&1 || error "systemd is required but systemctl was not found."

ARCH="$(uname -m)"
case "$ARCH" in
    x86_64)  RELEASE_ARCH="x86_64" ;;
    aarch64) RELEASE_ARCH="aarch64" ;;
    *) error "Unsupported architecture: $ARCH. Only x86_64 and aarch64 are supported." ;;
esac

if ss -tlnp 2>/dev/null | grep -q ":${LISTEN_PORT} " || \
   netstat -tlnp 2>/dev/null | grep -q ":${LISTEN_PORT} "; then
    error "Port ${LISTEN_PORT} is already in use. Stop the conflicting service and retry."
fi

# ── 2. Install system dependencies ──
info "Checking dependencies..."
if command -v apt-get >/dev/null 2>&1; then
    PKG_MGR="apt"
    apt-get update -qq >/dev/null 2>&1
    apt-get install -y -qq ca-certificates sqlite3 curl >/dev/null 2>&1
elif command -v dnf >/dev/null 2>&1; then
    PKG_MGR="dnf"
    dnf install -y -q ca-certificates sqlite curl >/dev/null 2>&1
elif command -v yum >/dev/null 2>&1; then
    PKG_MGR="yum"
    yum install -y -q ca-certificates sqlite curl >/dev/null 2>&1
else
    error "No supported package manager found (apt/dnf/yum)."
fi
info "Dependencies OK. (${PKG_MGR})"

# ── 3. Fetch latest release info from GitHub ──
info "Fetching latest release from GitHub..."
RELEASE_JSON=$(curl -fsSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" 2>/dev/null) \
    || error "Failed to fetch release info. Check your internet connection or create a release at https://github.com/${GITHUB_REPO}/releases"

VERSION=$(echo "$RELEASE_JSON" | grep -m1 '"tag_name"' | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
[[ -n "$VERSION" ]] || error "Could not parse version from release info."

ASSET_NAME="colophon-${VERSION}-linux-${RELEASE_ARCH}.tar.gz"
DOWNLOAD_URL=$(echo "$RELEASE_JSON" | grep -o "\"browser_download_url\"[[:space:]]*:[[:space:]]*\"[^\"]*${ASSET_NAME}\"" | head -1 | sed 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\(.*\)"/\1/')
[[ -n "$DOWNLOAD_URL" ]] || error "Asset '${ASSET_NAME}' not found in release ${VERSION}. Available assets may not match your architecture."

info "Installing Colophon ${VERSION} (${RELEASE_ARCH})..."

# ── 4. Download and extract ──
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

info "Downloading ${ASSET_NAME}..."
curl -fSL --progress-bar -o "${TMPDIR}/${ASSET_NAME}" "$DOWNLOAD_URL" \
    || error "Download failed. Check network and retry."

info "Extracting..."
tar -xzf "${TMPDIR}/${ASSET_NAME}" -C "$TMPDIR" --strip-components=1

# ── 5. Create system user ──
if ! id "$SERVICE_USER" >/dev/null 2>&1; then
    useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"
    info "Created system user: ${SERVICE_USER}"
fi

# ── 6. Create directory structure ──
mkdir -p "$INSTALL_DIR" "$DATA_DIR/uploads" "$DATA_DIR/pages" \
         "$CONF_DIR" "$BACKUP_DIR" "$DATA_DIR/static" \
         "$INSTALL_DIR/themes" "$INSTALL_DIR/src/admin" "$INSTALL_DIR/src/admin/dist" "$INSTALL_DIR/plugins"

# ── 7. Install files ──
# Binary
cp "${TMPDIR}/colophon" "${INSTALL_DIR}/colophon"
chmod 755 "${INSTALL_DIR}/colophon"

# Themes
if [ -d "${TMPDIR}/themes" ]; then
    cp -r "${TMPDIR}/themes/"* "${INSTALL_DIR}/themes/" 2>/dev/null || true
fi

# Admin frontend
if [ -d "${TMPDIR}/admin-dist" ]; then
    cp -r "${TMPDIR}/admin-dist/"* "${INSTALL_DIR}/src/admin/dist/" 2>/dev/null || true
    if [ -f "${TMPDIR}/admin-dist/admin.html" ]; then
        cp "${TMPDIR}/admin-dist/admin.html" "${INSTALL_DIR}/src/admin/admin.html" 2>/dev/null || true
    fi
fi

# Static assets (logo-icon.svg, logo-full.svg)
if [ -d "${TMPDIR}/static" ]; then
    cp -r "${TMPDIR}/static/"* "${DATA_DIR}/static/" 2>/dev/null || true
fi

# Config template (only copy if user doesn't already have one)
if [ -f "${TMPDIR}/config/default.toml.example" ] && [ ! -f "${CONF_DIR}/default.toml" ]; then
    cp "${TMPDIR}/config/default.toml.example" "${CONF_DIR}/default.toml"
    info "Config template installed -> ${CONF_DIR}/default.toml"
fi

# Migrations (needed if using sqlx::migrate! at runtime with file-based path)
if [ -d "${TMPDIR}/migrations" ]; then
    cp -r "${TMPDIR}/migrations" "${INSTALL_DIR}/"
fi

# ── 8. Generate JWT secret ──
ENV_FILE="${CONF_DIR}/colophon.env"
if [ ! -f "$ENV_FILE" ] || ! grep -q "COLOPHON__AUTH__SECRET" "$ENV_FILE" 2>/dev/null; then
    JWT_SECRET=$(head -c 32 /dev/urandom | base64 | tr -d '/+=' | head -c 48)
    cat > "$ENV_FILE" <<EOF
# Colophon secrets — auto-generated by installer
# Regenerate with: head -c 32 /dev/urandom | base64
COLOPHON__AUTH__SECRET=${JWT_SECRET}
EOF
    chmod 600 "$ENV_FILE"
    info "Generated JWT secret → ${ENV_FILE}"
else
    info "Existing JWT secret found, keeping it."
fi

# ── 9. Set ownership ──
chown -R "${SERVICE_USER}:${SERVICE_USER}" "$INSTALL_DIR" "$DATA_DIR" "$BACKUP_DIR"
chown -R "${SERVICE_USER}:${SERVICE_USER}" "$CONF_DIR"
chmod 700 "$CONF_DIR"

# ── 10. Install systemd service ──
cat > /etc/systemd/system/colophon.service <<EOF
[Unit]
Description=Colophon CMS
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_USER}
WorkingDirectory=${INSTALL_DIR}
ExecStart=${INSTALL_DIR}/colophon
Restart=always
RestartSec=5

Environment=RUST_LOG=colophon=info
Environment=WASMTIME_CACHE_DISABLE=1
Environment=COLOPHON__RUNTIME__MODE=production
Environment=COLOPHON__SERVER__HOST=0.0.0.0
Environment=COLOPHON__SERVER__PORT=${LISTEN_PORT}
Environment=COLOPHON__DATABASE__URL=sqlite://${DATA_DIR}/colophon.db?mode=rwc
Environment=COLOPHON__STORAGE__UPLOAD_DIR=${DATA_DIR}/uploads
Environment=COLOPHON__STORAGE__STATIC_DIR=${DATA_DIR}/static
Environment=COLOPHON__PATHS__ADMIN_DIST_DIR=${INSTALL_DIR}/src/admin/dist

EnvironmentFile=-${ENV_FILE}

NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${DATA_DIR} ${BACKUP_DIR} ${INSTALL_DIR}/plugins
ReadOnlyPaths=${INSTALL_DIR} ${CONF_DIR}

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now colophon
info "Service started."

# ── 11. Health check ──
info "Waiting for Colophon to start..."
sleep 3

if curl -sf "http://127.0.0.1:${LISTEN_PORT}/api/v1/health" >/dev/null 2>&1; then
    PUBLIC_IP=$(curl -fsSL --max-time 5 https://api.ipify.org 2>/dev/null || echo "YOUR_SERVER_IP")
    echo ""
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}  ✅ Colophon ${VERSION} installed successfully!${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "  ${YELLOW}→ Setup:${NC}  http://${PUBLIC_IP}:${LISTEN_PORT}/admin"
    echo -e "  ${YELLOW}→ Config:${NC} ${ENV_FILE}"
    echo -e "  ${YELLOW}→ Data:${NC}   ${DATA_DIR}/"
    echo ""
    echo -e "  Manage:"
    echo -e "    systemctl status colophon    # check status"
    echo -e "    systemctl restart colophon   # restart"
    echo -e "    journalctl -u colophon -f    # view logs"
    echo ""
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
else
    warn "Colophon may not be ready yet. Check: systemctl status colophon && journalctl -u colophon -e"
fi
