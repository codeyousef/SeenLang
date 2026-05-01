#!/usr/bin/env bash
# Build release packages for the Seen language compiler.
#
# Usage:
#   ./scripts/build_release.sh --version 0.7.2 \
#     --cpu-baseline x86-64 --artifact-suffix linux-x64
#   ./scripts/build_release.sh --version 0.7.2 \
#     --compiler compiler_seen/target/seen-x86-64-v3 \
#     --cpu-baseline x86-64-v3 --artifact-suffix linux-x64-v3
#
# Outputs:
#   dist/seen-<version>-linux-x64.tar.gz       (portable x86-64 tarball)
#   dist/seen-<version>-linux-x64-v3.tar.gz    (x86-64-v3 tarball)
#   dist/seen-lang_<version>_amd64.deb         (default linux-x64 only)
#   dist/seen-lang-<version>.x86_64.rpm        (default linux-x64 only)
#   dist/SeenLanguage-<version>-x86_64.AppImage (default linux-x64 only)
#   dist/SHA256SUMS

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION=""
OUTPUT_DIR="$ROOT_DIR/dist"
COMPILER_BIN="$ROOT_DIR/compiler_seen/target/seen"
CPU_BASELINE="${SEEN_RELEASE_CPU_BASELINE:-x86-64}"
ARTIFACT_SUFFIX=""
SKIP_VERIFY="${SEEN_RELEASE_SKIP_VERIFY:-0}"

usage() {
    echo "Usage: $0 --version <version> [--output-dir <dir>] [--compiler <path>]"
    echo "          [--cpu-baseline <x86-64|x86-64-v3>] [--artifact-suffix <linux-x64|linux-x64-v3>]"
    echo ""
    echo "Options:"
    echo "  --version          Release version (e.g., 0.7.2) [required]"
    echo "  --output-dir       Output directory (default: dist/)"
    echo "  --compiler         Path to compiler binary (default: compiler_seen/target/seen)"
    echo "  --cpu-baseline     Packaged binary CPU baseline (default: x86-64)"
    echo "  --artifact-suffix  Artifact suffix (default derived from CPU baseline)"
    echo "  --skip-verify      Skip release CPU baseline verifier"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
        --compiler) COMPILER_BIN="$2"; shift 2 ;;
        --cpu-baseline) CPU_BASELINE="$2"; shift 2 ;;
        --artifact-suffix) ARTIFACT_SUFFIX="$2"; shift 2 ;;
        --skip-verify) SKIP_VERIFY=1; shift ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo "Error: --version is required"
    usage
fi

case "$CPU_BASELINE" in
    x86-64)
        : "${ARTIFACT_SUFFIX:=linux-x64}"
        ;;
    x86-64-v3)
        : "${ARTIFACT_SUFFIX:=linux-x64-v3}"
        ;;
    *)
        echo "Error: unsupported CPU baseline: $CPU_BASELINE"
        echo "Supported baselines: x86-64, x86-64-v3"
        exit 1
        ;;
esac

case "$ARTIFACT_SUFFIX" in
    linux-x64|linux-x64-v3) ;;
    *)
        echo "Error: unsupported artifact suffix: $ARTIFACT_SUFFIX"
        echo "Supported suffixes: linux-x64, linux-x64-v3"
        exit 1
        ;;
esac
if [[ "$CPU_BASELINE" == "x86-64" && "$ARTIFACT_SUFFIX" != "linux-x64" ]]; then
    echo "Error: x86-64 baseline must use artifact suffix linux-x64"
    exit 1
fi
if [[ "$CPU_BASELINE" == "x86-64-v3" && "$ARTIFACT_SUFFIX" != "linux-x64-v3" ]]; then
    echo "Error: x86-64-v3 baseline must use artifact suffix linux-x64-v3"
    exit 1
fi

if [[ ! -x "$COMPILER_BIN" ]]; then
    echo "Error: compiler binary not found at $COMPILER_BIN"
    echo "Build it first with a memory-capped rebuild, for example:"
    echo "  SEEN_LOW_MEMORY=1 SEEN_MAIN_VMEM_KB=8388608 SEEN_OPT_VMEM_KB=2097152 SEEN_RELEASE_CPU_BASELINE=$CPU_BASELINE ./scripts/safe_rebuild.sh"
    exit 1
fi

mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

echo "=== Seen Language Release Builder ==="
echo "Version:       $VERSION"
echo "Compiler:      $COMPILER_BIN"
echo "CPU baseline:  $CPU_BASELINE"
echo "Artifact tier: $ARTIFACT_SUFFIX"
echo "Output:        $OUTPUT_DIR"
echo ""

rm -rf "$OUTPUT_DIR/staging"

PACKAGE_NAME="seen-${VERSION}-${ARTIFACT_SUFFIX}"
STAGING="$OUTPUT_DIR/staging/$PACKAGE_NAME"
mkdir -p "$STAGING"/{bin,lib/seen/std,lib/seen/runtime,lib/seen/toolchain,share/seen/languages,share/doc/seen}

echo "[1/6] Copying compiler binary..."
cp "$COMPILER_BIN" "$STAGING/bin/seen"
chmod +x "$STAGING/bin/seen"
strip "$STAGING/bin/seen" 2>/dev/null || true

echo "[2/6] Copying standard library..."
if [[ -d "$ROOT_DIR/seen_std/src" ]]; then
    cp -r "$ROOT_DIR/seen_std/src/"* "$STAGING/lib/seen/std/"
fi

echo "[3/6] Copying runtime..."
if [[ -d "$ROOT_DIR/seen_runtime" ]]; then
    for f in "$ROOT_DIR/seen_runtime"/*.{c,h}; do
        [[ -f "$f" ]] && cp "$f" "$STAGING/lib/seen/runtime/"
    done
    for f in "$ROOT_DIR/seen_runtime"/*.a "$ROOT_DIR/seen_runtime"/*.o; do
        [[ -f "$f" ]] && cp "$f" "$STAGING/lib/seen/runtime/"
    done
fi

echo "[4/6] Copying language files..."
if [[ -d "$ROOT_DIR/languages" ]]; then
    cp -r "$ROOT_DIR/languages/"* "$STAGING/share/seen/languages/"
fi

echo "[5/6] Copying documentation and metadata..."
cp "$ROOT_DIR/README.md" "$STAGING/share/doc/seen/"
[[ -f "$ROOT_DIR/LICENSE" ]] && cp "$ROOT_DIR/LICENSE" "$STAGING/share/doc/seen/"
cat > "$STAGING/share/doc/seen/release-cpu-baseline.txt" << METADATA_EOF
version=$VERSION
artifact=$PACKAGE_NAME
cpu-baseline=$CPU_BASELINE
METADATA_EOF

TOOLCHAIN_DIR="$STAGING/lib/seen/toolchain"
TOOLCHAIN_BUNDLE_MODE="external"
cp "$ROOT_DIR/scripts/seen_toolchain.sh" "$TOOLCHAIN_DIR/seen-toolchain.sh"
chmod +x "$TOOLCHAIN_DIR/seen-toolchain.sh"

if [[ -n "${SEEN_LLVM_BUNDLE_DIR:-}" ]]; then
    if [[ ! -d "$SEEN_LLVM_BUNDLE_DIR" ]]; then
        echo "Error: SEEN_LLVM_BUNDLE_DIR does not exist: $SEEN_LLVM_BUNDLE_DIR"
        exit 1
    fi
    echo "Bundling LLVM toolchain from $SEEN_LLVM_BUNDLE_DIR"
    rm -rf "$TOOLCHAIN_DIR/llvm"
    if [[ -d "$SEEN_LLVM_BUNDLE_DIR/bin" ]]; then
        cp -a "$SEEN_LLVM_BUNDLE_DIR" "$TOOLCHAIN_DIR/llvm"
    else
        mkdir -p "$TOOLCHAIN_DIR/llvm/bin"
        cp -a "$SEEN_LLVM_BUNDLE_DIR"/. "$TOOLCHAIN_DIR/llvm/bin/"
    fi
    TOOLCHAIN_BUNDLE_MODE="bundled"
fi

cat > "$TOOLCHAIN_DIR/manifest.env" << TOOLCHAIN_EOF
seen_toolchain_manifest_version=1
llvm_min_version=18
llvm_preferred_version=18
required_tools=clang,opt,llc,llvm-as,ld.lld
bundle_mode=$TOOLCHAIN_BUNDLE_MODE
managed_install=use SEEN_MANAGED_TOOLCHAIN=1 with install.sh, or run lib/seen/toolchain/seen-toolchain.sh --install
TOOLCHAIN_EOF

cat > "$STAGING/share/doc/seen/toolchain-dependencies.txt" << TOOLCHAIN_DOC_EOF
Seen native builds require LLVM 18 or newer with these tools:
  clang, opt, llc, llvm-as, ld.lld or lld

Package toolchain mode: $TOOLCHAIN_BUNDLE_MODE

If this package was built with SEEN_LLVM_BUNDLE_DIR, bundled tools are under:
  lib/seen/toolchain/llvm/bin

Otherwise install LLVM with your platform package manager, or run:
  SEEN_MANAGED_TOOLCHAIN=1 ./install.sh <prefix>

The helper script can be run directly:
  <prefix>/lib/seen/toolchain/seen-toolchain.sh --check
  <prefix>/lib/seen/toolchain/seen-toolchain.sh --install
TOOLCHAIN_DOC_EOF

cat > "$STAGING/install.sh" << 'INSTALL_EOF'
#!/usr/bin/env bash
set -euo pipefail

PREFIX="${1:-/usr/local}"
SUDO=()
if [[ "$EUID" -ne 0 && ( "$PREFIX" == "/usr"* || "$PREFIX" == "/opt"* ) ]]; then
    SUDO=(sudo)
fi

run_install() {
    "${SUDO[@]}" "$@"
}

install_file_no_follow() {
    local src="$1"
    local dest="$2"
    local mode="${3:-755}"
    local dest_dir
    local tmp
    dest_dir="$(dirname "$dest")"
    tmp="$dest_dir/.${dest##*/}.tmp.$$"

    run_install mkdir -p "$dest_dir"
    run_install rm -f "$tmp"
    run_install cp "$src" "$tmp"
    run_install chmod "$mode" "$tmp"
    run_install mv -f "$tmp" "$dest"
}

echo "Installing Seen Language to $PREFIX ..."
run_install mkdir -p "$PREFIX/bin" "$PREFIX/lib/seen" "$PREFIX/share/seen" "$PREFIX/share/doc/seen"
install_file_no_follow "bin/seen" "$PREFIX/bin/seen" 755
run_install cp -r lib/seen/* "$PREFIX/lib/seen/"
run_install cp -r share/seen/* "$PREFIX/share/seen/"
run_install cp -r share/doc/seen/* "$PREFIX/share/doc/seen/"
TOOLCHAIN_HELPER="$PREFIX/lib/seen/toolchain/seen-toolchain.sh"
if [[ -x "$TOOLCHAIN_HELPER" ]]; then
    if [[ "${SEEN_SKIP_TOOLCHAIN:-0}" == "1" ]]; then
        echo "LLVM toolchain check skipped."
    elif [[ "${SEEN_MANAGED_TOOLCHAIN:-0}" == "1" ]]; then
        "$TOOLCHAIN_HELPER" --install --prefix "$PREFIX"
    elif ! "$TOOLCHAIN_HELPER" --check --prefix "$PREFIX"; then
        echo "Seen installed, but LLVM 18+ tools are not ready." >&2
        echo "Install clang, opt, llc, llvm-as, and lld, or rerun with SEEN_MANAGED_TOOLCHAIN=1." >&2
    fi
fi
echo "Seen Language installed to $PREFIX"
echo "Run 'seen --version' to verify."
INSTALL_EOF
chmod +x "$STAGING/install.sh"

echo "[6/6] Building tarball..."
TARBALL="$PACKAGE_NAME.tar.gz"
(cd "$OUTPUT_DIR/staging" && tar czf "$OUTPUT_DIR/$TARBALL" "$PACKAGE_NAME")
echo "  -> $OUTPUT_DIR/$TARBALL"

if [[ "$SKIP_VERIFY" != "1" ]]; then
    echo ""
    echo "Verifying release CPU baseline..."
    "$SCRIPT_DIR/verify_release_cpu_baseline.sh" --cpu-baseline "$CPU_BASELINE" "$OUTPUT_DIR/$TARBALL"
fi

if [[ "$ARTIFACT_SUFFIX" == "linux-x64" ]]; then
    if command -v dpkg-deb &>/dev/null; then
        echo ""
        echo "Building DEB package..."
        (cd "$ROOT_DIR/installer/linux" && \
            SOURCE_DIR="$STAGING/bin" bash build-deb.sh \
                "$VERSION" amd64 --output-dir "$OUTPUT_DIR" 2>&1 | tail -5) || \
            echo "  DEB build skipped (build-deb.sh failed)"
    fi

    if command -v rpmbuild &>/dev/null; then
        echo ""
        echo "Building RPM package..."
        (cd "$ROOT_DIR/installer/linux" && \
            SOURCE_DIR="$STAGING/bin" bash build-rpm.sh \
                "$VERSION" x86_64 --output-dir "$OUTPUT_DIR" 2>&1 | tail -5) || \
            echo "  RPM build skipped (build-rpm.sh failed)"
    fi

    if command -v appimagetool &>/dev/null; then
        echo ""
        echo "Building AppImage..."
        (cd "$ROOT_DIR/installer/linux" && \
            SOURCE_DIR="$STAGING/bin" bash build-appimage.sh \
                "$VERSION" x86_64 --output-dir "$OUTPUT_DIR" 2>&1 | tail -5) || \
            echo "  AppImage build skipped (build-appimage.sh failed)"
    fi
else
    echo ""
    echo "Skipping DEB/RPM/AppImage for non-default CPU tier $ARTIFACT_SUFFIX."
fi

echo ""
echo "Generating checksums..."
(cd "$OUTPUT_DIR" && find . -maxdepth 1 -type f \
    \( -name '*.tar.gz' -o -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
    -printf '%f\n' | sort | xargs -r sha256sum > SHA256SUMS)
echo "  -> $OUTPUT_DIR/SHA256SUMS"

rm -rf "$OUTPUT_DIR/staging"

echo ""
echo "=== Release build complete ==="
echo "Artifacts in $OUTPUT_DIR:"
ls -lh "$OUTPUT_DIR"/ 2>/dev/null | grep -v '^total'
