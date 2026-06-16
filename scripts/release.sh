#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────
#  Colophon Release Packager
#  Builds release tarballs for GitHub Releases.
#
#  Usage:
#    ./scripts/release.sh [version] [arch] [--allow-dirty]
#
#  Examples:
#    ./scripts/release.sh                     # version from Cargo.toml, arch=$(uname -m)
#    ./scripts/release.sh v1.1.0              # override version (must match Cargo.toml)
#    ./scripts/release.sh v1.1.0 aarch64      # override version + arch
#    ./scripts/release.sh --allow-dirty       # skip git dirty check
# ─────────────────────────────────────────────

# ── 1. Parse arguments ──
ALLOW_DIRTY=false
POSITIONAL=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --allow-dirty)
            ALLOW_DIRTY=true
            shift
            ;;
        --*)
            echo "ERROR: Unknown option: $1"
            exit 1
            ;;
        *)
            POSITIONAL+=("$1")
            shift
            ;;
    esac
done

VERSION="${POSITIONAL[0]:-}"
ARCH="${POSITIONAL[1]:-$(uname -m)}"

# ── 2. Determine version ──
CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
if [ -z "$CARGO_VERSION" ]; then
    echo "ERROR: Could not parse version from Cargo.toml"
    exit 1
fi

if [ -z "$VERSION" ]; then
    VERSION="$CARGO_VERSION"
    echo "Version from Cargo.toml: ${VERSION}"
else
    VERSION="${VERSION#v}"
    # Consistency check: CLI version must match Cargo.toml version
    if [ "$VERSION" != "$CARGO_VERSION" ]; then
        echo "ERROR: CLI version (${VERSION}) does not match Cargo.toml version (${CARGO_VERSION})"
        echo "Update Cargo.toml version before releasing, or pass the correct version."
        exit 1
    fi
    echo "Version from CLI (matches Cargo.toml): ${VERSION}"
fi

TAG="v${VERSION}"

# ── 3. Pre-flight checks ──
if [ "$ALLOW_DIRTY" != "true" ]; then
    if ! git diff --quiet; then
        echo "ERROR: Uncommitted changes in working tree."
        echo "Commit or stash them, or use --allow-dirty to skip this check."
        exit 1
    fi
    if ! git diff --cached --quiet; then
        echo "ERROR: Staged but uncommitted changes."
        echo "Commit or stash them, or use --allow-dirty to skip this check."
        exit 1
    fi
fi

ASSET_NAME="colophon-${TAG}-linux-${ARCH}.tar.gz"
STAGING_DIR="target/release-pkg/colophon-${TAG}-linux-${ARCH}"

echo ""
echo "=== Building Colophon ${TAG} (${ARCH}) ==="

# ── 4. Build backend ──
echo ""
echo "[1/5] cargo build --release ..."
cargo build --release

# ── 5. Build admin frontend ──
echo ""
echo "[2/5] Building admin frontend..."
ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}/src/admin/ui"
npm install --silent 2>/dev/null
npm run build 2>&1 | tail -3
cd "$ROOT_DIR"

# Copy admin.html into dist (self-contained tarball)
cp src/admin/admin.html src/admin/dist/admin.html 2>/dev/null || true

# ── 6. Assemble release directory ──
echo ""
echo "[3/5] Assembling ${STAGING_DIR} ..."
rm -rf "$STAGING_DIR"
mkdir -p "${STAGING_DIR}/themes" \
         "${STAGING_DIR}/admin-dist" \
         "${STAGING_DIR}/migrations" \
         "${STAGING_DIR}/static" \
         "${STAGING_DIR}/config"

# Binary
cp target/release/colophon "${STAGING_DIR}/colophon"
chmod 755 "${STAGING_DIR}/colophon"

# Themes (all themes)
cp -r themes/default "${STAGING_DIR}/themes/"
cp -r themes/scholar-suzhou "${STAGING_DIR}/themes/"

# Admin frontend
cp -r src/admin/dist/* "${STAGING_DIR}/admin-dist/"

# Migrations
cp migrations/*.sql "${STAGING_DIR}/migrations/" 2>/dev/null || true

# Static assets
cp -r static/* "${STAGING_DIR}/static/" 2>/dev/null || true

# Config template (rename to .example)
cp config/default.toml "${STAGING_DIR}/config/default.toml.example"

echo "Release directory assembled:"
find "$STAGING_DIR" -type f | sed "s|${STAGING_DIR}/|  |" | sort

# ── 7. Package ──
echo ""
echo "[4/5] Creating tarball..."
mkdir -p target/release-pkg
tar -czf "target/release-pkg/${ASSET_NAME}" \
    -C "target/release-pkg" \
    "colophon-${TAG}-linux-${ARCH}"

SIZE=$(du -h "target/release-pkg/${ASSET_NAME}" | cut -f1)
echo "  ${ASSET_NAME} (${SIZE})"

# SHA256 checksum
echo ""
echo "[5/5] Generating SHA256 checksum..."
sha256sum "target/release-pkg/${ASSET_NAME}" \
    > "target/release-pkg/${ASSET_NAME}.sha256"
echo "  ${ASSET_NAME}.sha256"

cat "target/release-pkg/${ASSET_NAME}.sha256"

# ── 8. GitHub Release ──
echo ""
if command -v gh >/dev/null 2>&1; then
    echo "Creating draft GitHub Release for ${TAG}..."
    gh release create "${TAG}" \
        "target/release-pkg/${ASSET_NAME}" \
        "target/release-pkg/${ASSET_NAME}.sha256" \
        --generate-notes \
        --draft \
        --title "${TAG}" \
        || echo "WARNING: gh release create failed. Upload assets manually."
    echo ""
    echo "Draft release created. Review and publish at:"
    echo "  https://github.com/wenaryHY/colophon/releases"
else
    echo "gh CLI not found. Upload manually:"
    echo ""
    echo "  1. Create release: https://github.com/wenaryHY/colophon/releases/new?tag=${TAG}"
    echo "  2. Upload files:"
    echo "     target/release-pkg/${ASSET_NAME}"
    echo "     target/release-pkg/${ASSET_NAME}.sha256"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Release ${TAG} ready"
echo "  Package: target/release-pkg/${ASSET_NAME} (${SIZE})"
echo "  Checksum: target/release-pkg/${ASSET_NAME}.sha256"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
