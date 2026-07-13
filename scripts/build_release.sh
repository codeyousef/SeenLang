#!/usr/bin/env bash
# Build release packages for the Seen language compiler.
#
# Usage:
#   ./scripts/build_release.sh --version 0.9.3 \
#     --cpu-baseline x86-64 --artifact-suffix linux-x64
#   ./scripts/build_release.sh --version 0.9.3 \
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
BUILD_TRACE_COMMON="$SCRIPT_DIR/build_trace_common.sh"
if [[ -f "$BUILD_TRACE_COMMON" ]]; then
    # shellcheck source=scripts/build_trace_common.sh
    source "$BUILD_TRACE_COMMON"
    seen_build_trace_init "build_release"
    trap 'seen_build_trace_summary' EXIT
fi

VERSION=""
OUTPUT_DIR="$ROOT_DIR/dist"
COMPILER_BIN="$ROOT_DIR/compiler_seen/target/seen"
CPU_BASELINE="${SEEN_RELEASE_CPU_BASELINE:-x86-64}"
ARTIFACT_SUFFIX=""
SKIP_VERIFY="${SEEN_RELEASE_SKIP_VERIFY:-0}"
PACKAGE_JOBS="${SEEN_PACKAGE_JOBS:-}"

prune_packaged_stdlib_artifacts() {
    local stdlib_dir="$1"
    [[ -d "$stdlib_dir" ]] || return 0

    find "$stdlib_dir" -type f -name '*.tmp.*' -exec rm -f {} +
    find "$stdlib_dir" -type d \( -name build -o -name target -o -name .seen \) \
        -prune -exec rm -rf {} +
}

release_payload_hash() {
    if declare -F seen_build_hash_paths >/dev/null 2>&1; then
        seen_build_hash_paths \
            "$ROOT_DIR/seen_std/src" \
            "$ROOT_DIR/seen_runtime" \
            "$ROOT_DIR/languages" \
            "$ROOT_DIR/docs" \
            "$ROOT_DIR/README.md" \
            "$ROOT_DIR/CHANGELOG.md" \
            "$ROOT_DIR/LICENSE" \
            "$ROOT_DIR/scripts/seen_toolchain.sh" \
            "${SEEN_LLVM_BUNDLE_DIR:-}"
    else
        find "$ROOT_DIR/seen_std/src" "$ROOT_DIR/seen_runtime" "$ROOT_DIR/languages" "$ROOT_DIR/docs" \
            -type f -print0 2>/dev/null | sort -z | xargs -0 sha256sum 2>/dev/null | sha256sum | awk '{print $1}'
    fi
}

copy_payload_to_cache() {
    local cache_dir="$1"
    rm -rf "$cache_dir.tmp"
    mkdir -p "$cache_dir.tmp"
    cp -a "$STAGING/lib" "$cache_dir.tmp/" 2>/dev/null || true
    cp -a "$STAGING/share" "$cache_dir.tmp/" 2>/dev/null || true
    mv -f "$cache_dir.tmp" "$cache_dir"
    touch "$cache_dir/.ready"
}

release_package_tool_signature() {
    local tool
    for tool in dpkg-deb rpmbuild appimagetool tar sha256sum; do
        if command -v "$tool" >/dev/null 2>&1; then
            printf '%s=%s\n' "$tool" "$(command -v "$tool")"
        else
            printf '%s=missing\n' "$tool"
        fi
    done | sha256sum | awk '{print $1}'
}

release_artifact_key() {
    local compiler_hash payload_hash script_hash tool_hash
    compiler_hash="$(seen_build_hash_file "$COMPILER_BIN" 2>/dev/null || sha256sum "$COMPILER_BIN" | awk '{print $1}')"
    payload_hash="$1"
    script_hash="$(seen_build_hash_paths "$SCRIPT_DIR/build_release.sh" "$ROOT_DIR/installer/linux" 2>/dev/null || sha256sum "$SCRIPT_DIR/build_release.sh" | awk '{print $1}')"
    tool_hash="$(release_package_tool_signature)"
    {
        printf 'artifact-cache-v1\n'
        printf 'version=%s\n' "$VERSION"
        printf 'package=%s\n' "$PACKAGE_NAME"
        printf 'cpu=%s\n' "$CPU_BASELINE"
        printf 'suffix=%s\n' "$ARTIFACT_SUFFIX"
        printf 'compiler=%s\n' "$compiler_hash"
        printf 'payload=%s\n' "$payload_hash"
        printf 'scripts=%s\n' "$script_hash"
        printf 'tools=%s\n' "$tool_hash"
        printf 'skip_verify=%s\n' "$SKIP_VERIFY"
    } | sha256sum | awk '{print $1}'
}

restore_release_artifacts_from_cache() {
    local cache_dir="$1"
    local manifest="$cache_dir/manifest.env"
    [[ -f "$manifest" ]] || return 1
    if [[ "$SKIP_VERIFY" != "1" ]] && ! grep -q '^verified=1$' "$manifest"; then
        return 1
    fi
    shopt -s nullglob
    local artifacts=("$cache_dir"/*)
    shopt -u nullglob
    [[ "${#artifacts[@]}" -gt 1 ]] || return 1
    cp -a "$cache_dir"/. "$OUTPUT_DIR/"
    rm -f "$OUTPUT_DIR/manifest.env"
    if declare -F seen_build_trace_event >/dev/null 2>&1; then
        seen_build_trace_event "package artifact cache" "hit" "$(basename "$cache_dir")"
    fi
    echo "Reused release package artifact cache: $cache_dir"
    return 0
}

store_release_artifacts_to_cache() {
    local cache_dir="$1"
    rm -rf "$cache_dir.tmp"
    mkdir -p "$cache_dir.tmp"
    shopt -s nullglob
    cp -a "$OUTPUT_DIR"/*.tar.gz "$OUTPUT_DIR"/*.deb "$OUTPUT_DIR"/*.rpm \
        "$OUTPUT_DIR"/*.AppImage "$OUTPUT_DIR"/SHA256SUMS "$cache_dir.tmp/" 2>/dev/null || true
    shopt -u nullglob
    {
        printf 'artifact_manifest_version=1\n'
        printf 'version=%s\n' "$VERSION"
        printf 'package=%s\n' "$PACKAGE_NAME"
        printf 'cpu_baseline=%s\n' "$CPU_BASELINE"
        printf 'artifact_suffix=%s\n' "$ARTIFACT_SUFFIX"
        if [[ "$SKIP_VERIFY" == "1" ]]; then
            printf 'verified=0\n'
        else
            printf 'verified=1\n'
        fi
        printf 'created_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    } > "$cache_dir.tmp/manifest.env"
    rm -rf "$cache_dir"
    mv "$cache_dir.tmp" "$cache_dir"
    if declare -F seen_build_trace_event >/dev/null 2>&1; then
        seen_build_trace_event "package artifact cache" "store" "$(basename "$cache_dir")"
    fi
}

usage() {
    echo "Usage: $0 --version <version> [--output-dir <dir>] [--compiler <path>]"
    echo "          [--cpu-baseline <x86-64|x86-64-v3>] [--artifact-suffix <linux-x64|linux-x64-v3>]"
    echo ""
    echo "Options:"
    echo "  --version          Release version (e.g., 0.9.3) [required]"
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
PAYLOAD_CACHE_ROOT="$ROOT_DIR/target/seen-build/package-payloads/linux"
PAYLOAD_KEY="$(release_payload_hash)"
PAYLOAD_CACHE_DIR="$PAYLOAD_CACHE_ROOT/$PAYLOAD_KEY"
PAYLOAD_CACHE_HIT=0
ARTIFACT_CACHE_ROOT="$ROOT_DIR/target/seen-build/package-artifacts/linux"
ARTIFACT_KEY="$(release_artifact_key "$PAYLOAD_KEY")"
ARTIFACT_CACHE_DIR="$ARTIFACT_CACHE_ROOT/$ARTIFACT_KEY"
mkdir -p "$ARTIFACT_CACHE_ROOT"

if restore_release_artifacts_from_cache "$ARTIFACT_CACHE_DIR"; then
    echo ""
    echo "=== Release build complete ==="
    echo "Artifacts in $OUTPUT_DIR:"
    ls -lh "$OUTPUT_DIR"/ 2>/dev/null | grep -v '^total'
    exit 0
fi
if declare -F seen_build_trace_event >/dev/null 2>&1; then
    seen_build_trace_event "package artifact cache" "miss" "$ARTIFACT_KEY"
fi

STAGING="$OUTPUT_DIR/staging/$PACKAGE_NAME"
mkdir -p "$STAGING"/{bin,lib/seen/std,lib/seen/runtime,lib/seen/toolchain,share/seen/languages,share/doc/seen}

echo "[1/6] Copying compiler binary..."
cp "$COMPILER_BIN" "$STAGING/bin/seen"
chmod +x "$STAGING/bin/seen"
strip "$STAGING/bin/seen" 2>/dev/null || true

if [[ -f "$PAYLOAD_CACHE_DIR/.ready" ]]; then
    echo "[2-5/6] Reusing package payload cache..."
    cp -a "$PAYLOAD_CACHE_DIR"/. "$STAGING/"
    PAYLOAD_CACHE_HIT=1
    if declare -F seen_build_trace_event >/dev/null 2>&1; then
        seen_build_trace_event "package payload cache" "hit" "$PAYLOAD_KEY"
    fi
else
    mkdir -p "$PAYLOAD_CACHE_ROOT"
    if declare -F seen_build_trace_event >/dev/null 2>&1; then
        seen_build_trace_event "package payload cache" "miss" "$PAYLOAD_KEY"
    fi
fi

if [[ "$PAYLOAD_CACHE_HIT" != "1" ]]; then
    echo "[2/6] Copying standard library..."
    if [[ -d "$ROOT_DIR/seen_std/src" ]]; then
        cp -r "$ROOT_DIR/seen_std/src/"* "$STAGING/lib/seen/std/"
        prune_packaged_stdlib_artifacts "$STAGING/lib/seen/std"
    fi

    echo "[3/6] Copying runtime..."
    if [[ -d "$ROOT_DIR/seen_runtime" ]]; then
        for f in "$ROOT_DIR/seen_runtime"/*.{c,h}; do
            [[ -f "$f" ]] && cp "$f" "$STAGING/lib/seen/runtime/"
        done
        for f in "$ROOT_DIR/seen_runtime"/*.a "$ROOT_DIR/seen_runtime"/*.o "$ROOT_DIR/seen_runtime"/*.sig; do
            [[ -f "$f" ]] && cp "$f" "$STAGING/lib/seen/runtime/"
        done
    fi

    echo "[4/6] Copying language files..."
    if [[ -d "$ROOT_DIR/languages" ]]; then
        cp -r "$ROOT_DIR/languages/"* "$STAGING/share/seen/languages/"
    fi

    echo "[5/6] Copying documentation and shared toolchain payload..."
    cp "$ROOT_DIR/README.md" "$STAGING/share/doc/seen/"
    cp "$ROOT_DIR/CHANGELOG.md" "$STAGING/share/doc/seen/"
    [[ -f "$ROOT_DIR/LICENSE" ]] && cp "$ROOT_DIR/LICENSE" "$STAGING/share/doc/seen/"
fi

cat > "$STAGING/share/doc/seen/release-cpu-baseline.txt" << METADATA_EOF
version=$VERSION
artifact=$PACKAGE_NAME
cpu-baseline=$CPU_BASELINE
METADATA_EOF

TOOLCHAIN_DIR="$STAGING/lib/seen/toolchain"
TOOLCHAIN_BUNDLE_MODE="external"
if [[ "$PAYLOAD_CACHE_HIT" != "1" ]]; then
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
    copy_payload_to_cache "$PAYLOAD_CACHE_DIR"
fi

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
tar_start=""
if declare -F seen_build_trace_step_start >/dev/null 2>&1; then
    tar_start=$(seen_build_trace_step_start "linux tarball")
fi
(cd "$OUTPUT_DIR/staging" && tar czf "$OUTPUT_DIR/$TARBALL" "$PACKAGE_NAME")
if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
    seen_build_trace_step_end "linux tarball" "$tar_start" "ok" "$TARBALL"
fi
echo "  -> $OUTPUT_DIR/$TARBALL"

if [[ "$SKIP_VERIFY" != "1" ]]; then
    echo ""
    echo "Verifying release CPU baseline..."
    "$SCRIPT_DIR/verify_release_cpu_baseline.sh" --cpu-baseline "$CPU_BASELINE" "$OUTPUT_DIR/$TARBALL"
fi

if [[ "$ARTIFACT_SUFFIX" == "linux-x64" ]]; then
    if [[ -z "$PACKAGE_JOBS" ]]; then
        PACKAGE_JOBS=2
        if declare -F seen_build_cpu_count >/dev/null 2>&1; then
            CPU_COUNT="$(seen_build_cpu_count)"
            if [[ "$CPU_COUNT" -gt 2 ]]; then
                PACKAGE_JOBS=3
            fi
        fi
    fi
    echo ""
    echo "Building optional Linux package formats (jobs=$PACKAGE_JOBS)..."
    PKG_LOG_DIR="$ROOT_DIR/target/seen-build/package-logs"
    mkdir -p "$PKG_LOG_DIR"
    PKG_PIDS=()
    PKG_LABELS=()
    PKG_LOGS=()

    wait_for_package_slot() {
        while [[ "${#PKG_PIDS[@]}" -ge "$PACKAGE_JOBS" ]]; do
            wait "${PKG_PIDS[0]}" || true
            PKG_PIDS=("${PKG_PIDS[@]:1}")
            PKG_LABELS=("${PKG_LABELS[@]:1}")
            PKG_LOGS=("${PKG_LOGS[@]:1}")
        done
    }

    start_package_job() {
        local label="$1"
        local log_file="$2"
        shift 2
        wait_for_package_slot
        (
            local pkg_start=""
            if declare -F seen_build_trace_step_start >/dev/null 2>&1; then
                pkg_start=$(seen_build_trace_step_start "$label")
            fi
            if "$@" > "$log_file" 2>&1; then
                tail -5 "$log_file"
                if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
                    seen_build_trace_step_end "$label" "$pkg_start" "ok" "log=$log_file"
                fi
            else
                echo "  $label build skipped (command failed)"
                tail -5 "$log_file" 2>/dev/null || true
                if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
                    seen_build_trace_step_end "$label" "$pkg_start" "failed" "log=$log_file"
                fi
            fi
        ) &
        PKG_PIDS+=("$!")
        PKG_LABELS+=("$label")
        PKG_LOGS+=("$log_file")
    }

    if command -v dpkg-deb &>/dev/null; then
        start_package_job "deb package" "$PKG_LOG_DIR/build-deb.log" \
            bash -c 'cd "$1" && SOURCE_DIR="$2" bash build-deb.sh "$3" amd64 --output-dir "$4"' \
            bash "$ROOT_DIR/installer/linux" "$STAGING/bin" "$VERSION" "$OUTPUT_DIR"
    fi

    if command -v rpmbuild &>/dev/null; then
        start_package_job "rpm package" "$PKG_LOG_DIR/build-rpm.log" \
            bash -c 'cd "$1" && SOURCE_DIR="$2" bash build-rpm.sh "$3" x86_64 --output-dir "$4"' \
            bash "$ROOT_DIR/installer/linux" "$STAGING/bin" "$VERSION" "$OUTPUT_DIR"
    fi

    if command -v appimagetool &>/dev/null; then
        start_package_job "appimage package" "$PKG_LOG_DIR/build-appimage.log" \
            bash -c 'cd "$1" && SOURCE_DIR="$2" bash build-appimage.sh "$3" x86_64 --output-dir "$4"' \
            bash "$ROOT_DIR/installer/linux" "$STAGING/bin" "$VERSION" "$OUTPUT_DIR"
    fi
    for pkg_pid in "${PKG_PIDS[@]}"; do
        wait "$pkg_pid" || true
    done
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

store_release_artifacts_to_cache "$ARTIFACT_CACHE_DIR"
rm -rf "$OUTPUT_DIR/staging"

echo ""
echo "=== Release build complete ==="
echo "Artifacts in $OUTPUT_DIR:"
ls -lh "$OUTPUT_DIR"/ 2>/dev/null | grep -v '^total'
