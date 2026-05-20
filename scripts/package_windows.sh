#!/bin/bash
# Package the Seen compiler for Windows distribution
# Creates a staging directory with the layout expected by the installer scripts.
#
# Usage: ./scripts/package_windows.sh [version]
#
# Prerequisites: Run scripts/build_windows.sh first to create seen.exe.
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WIN_DIR="${PROJECT_DIR}/target-windows"
VERSION="${1:-1.0.0}"
BUILD_TRACE_COMMON="$SCRIPT_DIR/build_trace_common.sh"
if [ -f "$BUILD_TRACE_COMMON" ]; then
    # shellcheck source=scripts/build_trace_common.sh
    source "$BUILD_TRACE_COMMON"
    seen_build_trace_init "package_windows"
    trap 'seen_build_trace_summary' EXIT
fi

PACKAGE_DIR="${WIN_DIR}/seen-${VERSION}-windows-x64"

prune_packaged_stdlib_artifacts() {
    local stdlib_dir="$1"
    [ -d "$stdlib_dir" ] || return 0

    find "$stdlib_dir" -type f -name '*.tmp.*' -exec rm -f {} +
    find "$stdlib_dir" -type d \( -name build -o -name target -o -name .seen \) \
        -prune -exec rm -rf {} +
}

windows_payload_hash() {
    if declare -F seen_build_hash_paths >/dev/null 2>&1; then
        seen_build_hash_paths \
            "$PROJECT_DIR/seen_std/src" \
            "$PROJECT_DIR/seen_runtime" \
            "$PROJECT_DIR/languages" \
            "$PROJECT_DIR/docs" \
            "$PROJECT_DIR/installer/windows/install-llvm.ps1" \
            "${SEEN_WINDOWS_LLVM_BUNDLE_DIR:-}" \
            "${SEEN_WINDOWS_LLVM_INSTALLER:-}"
    else
        find "$PROJECT_DIR/seen_std/src" "$PROJECT_DIR/seen_runtime" "$PROJECT_DIR/languages" "$PROJECT_DIR/docs" \
            -type f -print0 2>/dev/null | sort -z | xargs -0 sha256sum 2>/dev/null | sha256sum | awk '{print $1}'
    fi
}

windows_toolchain_manifest_hash() {
    if declare -F seen_build_hash_paths >/dev/null 2>&1; then
        seen_build_hash_paths \
            "$PROJECT_DIR/installer/windows/install-llvm.ps1" \
            "${SEEN_WINDOWS_LLVM_BUNDLE_DIR:-}" \
            "${SEEN_WINDOWS_LLVM_INSTALLER:-}"
    else
        printf '%s|%s|%s\n' \
            "${SEEN_WINDOWS_LLVM_BUNDLE_DIR:-}" \
            "${SEEN_WINDOWS_LLVM_INSTALLER:-}" \
            "$(sha256sum "$PROJECT_DIR/installer/windows/install-llvm.ps1" 2>/dev/null | awk '{print $1}')" |
            sha256sum | awk '{print $1}'
    fi
}

manifest_value() {
    local key="$1"
    local file="$2"
    awk -F= -v key="$key" '$1 == key {print $2; exit}' "$file"
}

validate_windows_binary_manifest() {
    local exe="$WIN_DIR/seen.exe"
    local manifest="$WIN_DIR/seen.exe.manifest.env"
    local exe_hash payload_hash toolchain_hash

    exe_hash=$(sha256sum "$exe" | awk '{print $1}')
    payload_hash=$(windows_payload_hash)
    toolchain_hash=$(windows_toolchain_manifest_hash)

    if [ ! -f "$manifest" ]; then
        echo "ERROR: $exe has no reuse manifest: $manifest"
        echo "Rebuild the Windows compiler path so version, binary hash, runtime payload hash, and toolchain manifest can be verified."
        exit 1
    fi
    if [ "$(manifest_value version "$manifest")" != "$VERSION" ]; then
        echo "ERROR: Windows binary manifest version does not match package version $VERSION."
        exit 1
    fi
    if [ "$(manifest_value binary_sha256 "$manifest")" != "$exe_hash" ]; then
        echo "ERROR: Windows binary manifest hash is stale for target-windows/seen.exe."
        exit 1
    fi
    if [ "$(manifest_value payload_sha256 "$manifest")" != "$payload_hash" ]; then
        echo "ERROR: Windows binary manifest payload hash is stale."
        exit 1
    fi
    if [ "$(manifest_value toolchain_sha256 "$manifest")" != "$toolchain_hash" ]; then
        echo "ERROR: Windows binary manifest toolchain hash is stale."
        exit 1
    fi
    if declare -F seen_build_trace_event >/dev/null 2>&1; then
        seen_build_trace_event "windows binary manifest" "ok" "$manifest"
    fi
}

windows_package_artifact_key() {
    local exe_hash payload_hash toolchain_hash script_hash
    exe_hash=$(sha256sum "$WIN_DIR/seen.exe" | awk '{print $1}')
    payload_hash=$(windows_payload_hash)
    toolchain_hash=$(windows_toolchain_manifest_hash)
    if declare -F seen_build_hash_paths >/dev/null 2>&1; then
        script_hash=$(seen_build_hash_paths "$SCRIPT_DIR/package_windows.sh" "$PROJECT_DIR/installer/windows" 2>/dev/null)
    else
        script_hash=$(sha256sum "$SCRIPT_DIR/package_windows.sh" | awk '{print $1}')
    fi
    {
        printf 'windows-package-v1\n'
        printf 'version=%s\n' "$VERSION"
        printf 'exe=%s\n' "$exe_hash"
        printf 'payload=%s\n' "$payload_hash"
        printf 'toolchain=%s\n' "$toolchain_hash"
        printf 'scripts=%s\n' "$script_hash"
    } | sha256sum | awk '{print $1}'
}

restore_windows_package_artifact() {
    local cache_dir="$1"
    local zip_name="seen-${VERSION}-windows-x64.zip"
    local tgz_name="seen-${VERSION}-windows-x64.tar.gz"
    if [ -f "$cache_dir/$zip_name" ]; then
        cp "$cache_dir/$zip_name" "$WIN_DIR/$zip_name"
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "windows package artifact cache" "hit" "$(basename "$cache_dir")"
        fi
        echo "Reused Windows package artifact cache: $cache_dir"
        echo "  Archive: $WIN_DIR/$zip_name"
        exit 0
    fi
    if [ -f "$cache_dir/$tgz_name" ]; then
        cp "$cache_dir/$tgz_name" "$WIN_DIR/$tgz_name"
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "windows package artifact cache" "hit" "$(basename "$cache_dir")"
        fi
        echo "Reused Windows package artifact cache: $cache_dir"
        echo "  Archive: $WIN_DIR/$tgz_name"
        exit 0
    fi
    if declare -F seen_build_trace_event >/dev/null 2>&1; then
        seen_build_trace_event "windows package artifact cache" "miss" "$(basename "$cache_dir")"
    fi
}

echo "=== Packaging Seen $VERSION for Windows ==="
echo "  Output: $PACKAGE_DIR"

# --- Compiler binary ---
if [ ! -f "$WIN_DIR/seen.exe" ]; then
    echo "ERROR: seen.exe not found in $WIN_DIR"
    echo "Run scripts/build_windows.sh first."
    exit 1
fi
validate_windows_binary_manifest
WINDOWS_ARTIFACT_CACHE_ROOT="$WIN_DIR/package-artifacts"
WINDOWS_ARTIFACT_KEY="$(windows_package_artifact_key)"
WINDOWS_ARTIFACT_CACHE_DIR="$WINDOWS_ARTIFACT_CACHE_ROOT/$WINDOWS_ARTIFACT_KEY"
mkdir -p "$WINDOWS_ARTIFACT_CACHE_ROOT"
restore_windows_package_artifact "$WINDOWS_ARTIFACT_CACHE_DIR"

# Clean and create directory structure
rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR/bin"
mkdir -p "$PACKAGE_DIR/lib/seen/std"
mkdir -p "$PACKAGE_DIR/lib/seen/runtime"
mkdir -p "$PACKAGE_DIR/lib/seen/toolchain"
mkdir -p "$PACKAGE_DIR/share/seen/languages"
mkdir -p "$PACKAGE_DIR/share/seen/docs"

cp "$WIN_DIR/seen.exe" "$PACKAGE_DIR/bin/"
echo "  bin/seen.exe"

cat > "$PACKAGE_DIR/bin/seen-env.cmd" << 'CMD_EOF'
@echo off
set "SEEN_HOME=%~dp0.."
if exist "%SEEN_HOME%\lib\seen\toolchain\llvm\bin\clang.exe" set "PATH=%SEEN_HOME%\lib\seen\toolchain\llvm\bin;%PATH%"
set "PATH=%SEEN_HOME%\bin;%PATH%"
cmd /k
CMD_EOF
echo "  bin/seen-env.cmd"

# --- Standard library ---
if [ -d "$PROJECT_DIR/seen_std/src" ]; then
    cp -r "$PROJECT_DIR/seen_std/src/"* "$PACKAGE_DIR/lib/seen/std/"
    prune_packaged_stdlib_artifacts "$PACKAGE_DIR/lib/seen/std"
    STD_COUNT=$(find "$PACKAGE_DIR/lib/seen/std" -type f | wc -l)
    echo "  lib/seen/std/ ($STD_COUNT files)"
fi
if [ -f "$PROJECT_DIR/seen_std/Seen.toml" ]; then
    cp "$PROJECT_DIR/seen_std/Seen.toml" "$PACKAGE_DIR/lib/seen/std/"
fi

# --- Runtime headers ---
for header in seen_runtime.h seen_region.h seen_gpu.h seen_compat_win32.h; do
    if [ -f "$PROJECT_DIR/seen_runtime/$header" ]; then
        cp "$PROJECT_DIR/seen_runtime/$header" "$PACKAGE_DIR/lib/seen/runtime/"
    fi
done
HEADER_COUNT=$(find "$PACKAGE_DIR/lib/seen/runtime" -type f | wc -l)
echo "  lib/seen/runtime/ ($HEADER_COUNT headers)"

# --- LLVM toolchain / prerequisite helper ---
TOOLCHAIN_MODE="managed-installer"
if [ -f "$PROJECT_DIR/installer/windows/install-llvm.ps1" ]; then
    cp "$PROJECT_DIR/installer/windows/install-llvm.ps1" "$PACKAGE_DIR/lib/seen/toolchain/"
fi

if [ -n "${SEEN_WINDOWS_LLVM_BUNDLE_DIR:-}" ]; then
    LLVM_BIN="$SEEN_WINDOWS_LLVM_BUNDLE_DIR/bin"
    MISSING_LLVM=""
    for tool in clang.exe opt.exe llc.exe llvm-as.exe; do
        if [ ! -f "$LLVM_BIN/$tool" ]; then
            MISSING_LLVM="$MISSING_LLVM $tool"
        fi
    done
    if [ ! -f "$LLVM_BIN/lld-link.exe" ] && [ ! -f "$LLVM_BIN/ld.lld.exe" ]; then
        MISSING_LLVM="$MISSING_LLVM lld-link.exe"
    fi
    if [ -n "$MISSING_LLVM" ]; then
        echo "ERROR: SEEN_WINDOWS_LLVM_BUNDLE_DIR is missing:$MISSING_LLVM"
        exit 1
    fi
    mkdir -p "$PACKAGE_DIR/lib/seen/toolchain/llvm"
    cp -r "$SEEN_WINDOWS_LLVM_BUNDLE_DIR"/. "$PACKAGE_DIR/lib/seen/toolchain/llvm/"
    TOOLCHAIN_MODE="bundled"
    TOOL_COUNT=$(find "$PACKAGE_DIR/lib/seen/toolchain/llvm/bin" -maxdepth 1 -type f | wc -l)
    echo "  lib/seen/toolchain/llvm/ ($TOOL_COUNT bin files)"
elif [ -n "${SEEN_WINDOWS_LLVM_INSTALLER:-}" ]; then
    if [ ! -f "$SEEN_WINDOWS_LLVM_INSTALLER" ]; then
        echo "ERROR: SEEN_WINDOWS_LLVM_INSTALLER not found: $SEEN_WINDOWS_LLVM_INSTALLER"
        exit 1
    fi
    cp "$SEEN_WINDOWS_LLVM_INSTALLER" "$PACKAGE_DIR/lib/seen/toolchain/llvm-installer.exe"
    TOOLCHAIN_MODE="bundled-installer"
    echo "  lib/seen/toolchain/llvm-installer.exe"
else
    echo "  lib/seen/toolchain/install-llvm.ps1 (managed LLVM installer helper)"
fi

cat > "$PACKAGE_DIR/lib/seen/toolchain/manifest.env" << EOF
seen_toolchain_manifest_version=1
llvm_min_version=18
llvm_preferred_version=20
required_tools=clang,opt,llc,llvm-as,lld-link
bundle_mode=$TOOLCHAIN_MODE
EOF

# --- Language configurations ---
for lang_dir in "$PROJECT_DIR/languages"/*/; do
    lang=$(basename "$lang_dir")
    mkdir -p "$PACKAGE_DIR/share/seen/languages/$lang"
    cp "$lang_dir"*.toml "$PACKAGE_DIR/share/seen/languages/$lang/" 2>/dev/null || true
done
LANG_COUNT=$(find "$PACKAGE_DIR/share/seen/languages" -type f | wc -l)
echo "  share/seen/languages/ ($LANG_COUNT files across 6 languages)"

# --- Documentation ---
if [ -d "$PROJECT_DIR/docs" ]; then
    # Copy markdown docs (skip private/ and images/)
    find "$PROJECT_DIR/docs" -maxdepth 1 -name "*.md" -exec cp {} "$PACKAGE_DIR/share/seen/docs/" \;
    if [ -d "$PROJECT_DIR/docs/api-reference" ]; then
        cp -r "$PROJECT_DIR/docs/api-reference" "$PACKAGE_DIR/share/seen/docs/"
    fi
    DOC_COUNT=$(find "$PACKAGE_DIR/share/seen/docs" -type f | wc -l)
    echo "  share/seen/docs/ ($DOC_COUNT files)"
fi

# --- README ---
cat > "$PACKAGE_DIR/README.txt" << 'README_EOF'
Seen Language for Windows
=========================

Quick Start:
  1. Add the bin\ directory to your PATH (the installer does this automatically)
  2. Create hello.seen:
       fun main() { println("Hello from Seen!") }
  3. Compile:
       seen compile hello.seen hello --fast
  4. Run:
       hello.exe

Requirements:
  The package includes the Seen compiler, standard library, runtime headers,
  language files, and LLVM toolchain support.

  Installer users: the setup program adds Seen to PATH and uses the packaged
  LLVM payload when present. If no embedded LLVM payload was supplied at release
  build time, the installer runs the included managed LLVM installer helper.

  ZIP users: run bin\seen-env.cmd for a shell configured for Seen. If this ZIP
  was built without an embedded LLVM payload, run:
      powershell -ExecutionPolicy Bypass -File lib\seen\toolchain\install-llvm.ps1

Multi-language support:
  The Seen compiler supports keywords in 6 languages:
  English, Arabic, Spanish, Russian, Chinese, Japanese.
  Language files are in share\seen\languages\.

For more info: https://github.com/codeyousef/SeenLang
README_EOF

# --- LICENSE ---
if [ -f "$PROJECT_DIR/LICENSE" ]; then
    cp "$PROJECT_DIR/LICENSE" "$PACKAGE_DIR/LICENSE.txt"
fi

# --- Create ZIP ---
cd "$WIN_DIR"
ZIPFILE="seen-${VERSION}-windows-x64.zip"
rm -f "$ZIPFILE"

if command -v zip &>/dev/null; then
    zip -r -q "$ZIPFILE" "seen-${VERSION}-windows-x64/"
elif command -v 7z &>/dev/null; then
    7z a -mx=9 "$ZIPFILE" "seen-${VERSION}-windows-x64/"
else
    # Fallback: tar.gz
    ZIPFILE="seen-${VERSION}-windows-x64.tar.gz"
    tar czf "$ZIPFILE" "seen-${VERSION}-windows-x64/"
fi

# --- Summary ---
TOTAL_FILES=$(find "$PACKAGE_DIR" -type f | wc -l)
TOTAL_SIZE=$(du -sh "$PACKAGE_DIR" | cut -f1)
ZIP_SIZE=$(du -h "$WIN_DIR/$ZIPFILE" | cut -f1)
rm -rf "$WINDOWS_ARTIFACT_CACHE_DIR.tmp"
mkdir -p "$WINDOWS_ARTIFACT_CACHE_DIR.tmp"
cp "$WIN_DIR/$ZIPFILE" "$WINDOWS_ARTIFACT_CACHE_DIR.tmp/"
cat > "$WINDOWS_ARTIFACT_CACHE_DIR.tmp/manifest.env" << EOF
artifact_manifest_version=1
version=$VERSION
artifact=$ZIPFILE
created_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF
rm -rf "$WINDOWS_ARTIFACT_CACHE_DIR"
mv "$WINDOWS_ARTIFACT_CACHE_DIR.tmp" "$WINDOWS_ARTIFACT_CACHE_DIR"
if declare -F seen_build_trace_event >/dev/null 2>&1; then
    seen_build_trace_event "windows package artifact cache" "store" "$WINDOWS_ARTIFACT_KEY"
fi

echo ""
echo "=== Package created ==="
echo "  Staging: $PACKAGE_DIR"
echo "  Archive: $WIN_DIR/$ZIPFILE ($ZIP_SIZE)"
echo "  Total: $TOTAL_FILES files, $TOTAL_SIZE uncompressed"
echo ""
echo "Next steps:"
echo "  Build NSIS installer:  ./scripts/build_windows_installer.sh $VERSION"
echo "  Build Inno Setup:      wine iscc installer/windows/seen.iss /DVersion=$VERSION"
echo "  Build WiX MSI:         (on Windows) cd installer/windows && .\\build.bat $VERSION x64"
