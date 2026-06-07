#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────
#  InkForge Release Packager
#  Builds release tarballs for GitHub Releases.
#  Usage: ./scripts/release.sh [version]
#  Example: ./scripts/release.sh v1.0.1
# ─────────────────────────────────────────────

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 v1.0.1"
    exit 1
fi

# Strip leading 'v' for consistency
VERSION="${VERSION#v}"
TAG="v${VERSION}"

ARCH="$(uname -m)"
ASSET_NAME="inkforge-${TAG}-linux-${ARCH}.tar.gz"
STAGING_DIR="target/release-pkg/inkforge-${TAG}-linux-${ARCH}"

echo "=== Building InkForge ${TAG} (${ARCH}) ==="

# ── 1. Build backend ──
echo "[1/3] cargo build --release ..."
cargo build --release

# ── 2. Build frontend ──
echo "[2/3] Building admin frontend..."
cd src/admin/ui
npm install --silent 2>/dev/null
npm run build 2>&1 | tail -3
cd "$(git rev-parse --show-toplevel)"

# Copy admin.html into dist so the tarball is self-contained
cp src/admin/admin.html src/admin/dist/admin.html 2>/dev/null || true

# ── 3. Package ──
echo "[3/3] Packaging ${ASSET_NAME}..."
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"/{themes,admin-dist,migrations}

# Binary
cp target/release/inkforge "$STAGING_DIR/inkforge"

# Themes
cp -r themes/default "$STAGING_DIR/themes/"

# Admin frontend
cp -r src/admin/dist/* "$STAGING_DIR/admin-dist/"

# Migrations (for reference / manual use)
cp -r migrations/*.sql "$STAGING_DIR/migrations/"

# Create tarball
mkdir -p target/release-pkg
tar -czf "target/release-pkg/${ASSET_NAME}" -C "target/release-pkg" \
    "inkforge-${TAG}-linux-${ARCH}"

SIZE=$(du -h "target/release-pkg/${ASSET_NAME}" | cut -f1)
echo ""
echo "✅ Release package ready:"
echo "   target/release-pkg/${ASSET_NAME} (${SIZE})"
echo ""
echo "Upload to GitHub:"
echo "   gh release create ${TAG} target/release-pkg/${ASSET_NAME} --title '${TAG}' --notes 'Release ${TAG}'"
echo ""
echo "Or manually at: https://github.com/wenaryHY/inkforge/releases/new?tag=${TAG}"
