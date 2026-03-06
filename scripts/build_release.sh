#!/usr/bin/env bash
# Build release packages for the Seen language compiler.
#
# Usage: ./scripts/build_release.sh --version 1.0.0-alpha [--output-dir dist/]
#
# Outputs:
#   dist/seen-<version>-linux-x64.tar.gz       (universal tarball)
#   dist/seen-lang_<version>_amd64.deb          (Debian/Ubuntu)
#   dist/seen-lang-<version>.x86_64.rpm         (Fedora/RHEL)
#   dist/SeenLanguage-<version>-x86_64.AppImage (portable)
#   dist/SHA256SUMS

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION=""
OUTPUT_DIR="$ROOT_DIR/dist"
COMPILER_BIN="$ROOT_DIR/compiler_seen/target/seen"

usage() {
    echo "Usage: $0 --version <version> [--output-dir <dir>] [--compiler <path>]"
    echo ""
    echo "Options:"
    echo "  --version     Release version (e.g., 1.0.0-alpha) [required]"
    echo "  --output-dir  Output directory (default: dist/)"
    echo "  --compiler    Path to compiler binary (default: compiler_seen/target/seen)"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)   VERSION="$2"; shift 2 ;;
        --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
        --compiler)  COMPILER_BIN="$2"; shift 2 ;;
        -h|--help)   usage ;;
        *)           echo "Unknown option: $1"; usage ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo "Error: --version is required"
    usage
fi

if [[ ! -x "$COMPILER_BIN" ]]; then
    echo "Error: compiler binary not found at $COMPILER_BIN"
    echo "Build it first with: ./scripts/safe_rebuild.sh"
    exit 1
fi

echo "=== Seen Language Release Builder ==="
echo "Version:  $VERSION"
echo "Compiler: $COMPILER_BIN"
echo "Output:   $OUTPUT_DIR"
echo ""

# Create output and staging directories
mkdir -p "$OUTPUT_DIR"
STAGING="$OUTPUT_DIR/staging/seen-${VERSION}-linux-x64"
rm -rf "$OUTPUT_DIR/staging"
mkdir -p "$STAGING"/{bin,lib/seen/std,lib/seen/runtime,share/seen/languages,share/doc/seen}

# 1. Copy compiler binary
echo "[1/6] Copying compiler binary..."
cp "$COMPILER_BIN" "$STAGING/bin/seen"
chmod +x "$STAGING/bin/seen"
strip "$STAGING/bin/seen" 2>/dev/null || true

# 2. Copy standard library
echo "[2/6] Copying standard library..."
if [[ -d "$ROOT_DIR/seen_std/src" ]]; then
    cp -r "$ROOT_DIR/seen_std/src/"* "$STAGING/lib/seen/std/"
fi

# 3. Copy runtime
echo "[3/6] Copying runtime..."
if [[ -d "$ROOT_DIR/seen_runtime" ]]; then
    for f in "$ROOT_DIR/seen_runtime"/*.{c,h}; do
        [[ -f "$f" ]] && cp "$f" "$STAGING/lib/seen/runtime/"
    done
    # Copy pre-compiled runtime object if available
    for f in "$ROOT_DIR/seen_runtime"/*.a "$ROOT_DIR/seen_runtime"/*.o; do
        [[ -f "$f" ]] && cp "$f" "$STAGING/lib/seen/runtime/"
    done
fi

# 4. Copy language files
echo "[4/6] Copying language files..."
if [[ -d "$ROOT_DIR/languages" ]]; then
    cp -r "$ROOT_DIR/languages/"* "$STAGING/share/seen/languages/"
fi

# 5. Copy documentation
echo "[5/6] Copying documentation..."
cp "$ROOT_DIR/README.md" "$STAGING/share/doc/seen/"
[[ -f "$ROOT_DIR/LICENSE" ]] && cp "$ROOT_DIR/LICENSE" "$STAGING/share/doc/seen/"

# Create install.sh helper
cat > "$STAGING/install.sh" << 'INSTALL_EOF'
#!/usr/bin/env bash
set -e
PREFIX="${1:-/usr/local}"
echo "Installing Seen Language to $PREFIX ..."
sudo mkdir -p "$PREFIX/bin" "$PREFIX/lib/seen" "$PREFIX/share/seen" "$PREFIX/share/doc/seen"
sudo cp bin/seen "$PREFIX/bin/"
sudo cp -r lib/seen/* "$PREFIX/lib/seen/"
sudo cp -r share/seen/* "$PREFIX/share/seen/"
sudo cp -r share/doc/seen/* "$PREFIX/share/doc/seen/"
echo "Seen Language installed to $PREFIX"
echo "Run 'seen --version' to verify."
INSTALL_EOF
chmod +x "$STAGING/install.sh"

# 6. Build tarball
echo "[6/6] Building tarball..."
TARBALL="seen-${VERSION}-linux-x64.tar.gz"
(cd "$OUTPUT_DIR/staging" && tar czf "$OUTPUT_DIR/$TARBALL" "seen-${VERSION}-linux-x64")
echo "  -> $OUTPUT_DIR/$TARBALL"

# Build DEB package (if dpkg-deb is available)
if command -v dpkg-deb &>/dev/null; then
    echo ""
    echo "Building DEB package..."
    (cd "$ROOT_DIR/installer/linux" && \
        SOURCE_DIR="$STAGING/bin" bash build-deb.sh \
            "$VERSION" amd64 --output-dir "$OUTPUT_DIR" 2>&1 | tail -5) || \
        echo "  DEB build skipped (build-deb.sh failed)"
fi

# Build RPM package (if rpmbuild is available)
if command -v rpmbuild &>/dev/null; then
    echo ""
    echo "Building RPM package..."
    (cd "$ROOT_DIR/installer/linux" && \
        SOURCE_DIR="$STAGING/bin" bash build-rpm.sh \
            "$VERSION" x86_64 --output-dir "$OUTPUT_DIR" 2>&1 | tail -5) || \
        echo "  RPM build skipped (build-rpm.sh failed)"
fi

# Build AppImage (if appimagetool is available)
if command -v appimagetool &>/dev/null; then
    echo ""
    echo "Building AppImage..."
    (cd "$ROOT_DIR/installer/linux" && \
        SOURCE_DIR="$STAGING/bin" bash build-appimage.sh \
            "$VERSION" x86_64 --output-dir "$OUTPUT_DIR" 2>&1 | tail -5) || \
        echo "  AppImage build skipped (build-appimage.sh failed)"
fi

# Generate checksums
echo ""
echo "Generating checksums..."
(cd "$OUTPUT_DIR" && sha256sum *.tar.gz *.deb *.rpm *.AppImage 2>/dev/null > SHA256SUMS || \
    sha256sum *.tar.gz > SHA256SUMS)
echo "  -> $OUTPUT_DIR/SHA256SUMS"

# Cleanup staging
rm -rf "$OUTPUT_DIR/staging"

echo ""
echo "=== Release build complete ==="
echo "Artifacts in $OUTPUT_DIR:"
ls -lh "$OUTPUT_DIR"/ 2>/dev/null | grep -v '^total'
