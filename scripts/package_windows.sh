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

PACKAGE_DIR="${WIN_DIR}/seen-${VERSION}-windows-x64"
echo "=== Packaging Seen $VERSION for Windows ==="
echo "  Output: $PACKAGE_DIR"

# Clean and create directory structure
rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR/bin"
mkdir -p "$PACKAGE_DIR/lib/seen/std"
mkdir -p "$PACKAGE_DIR/lib/seen/runtime"
mkdir -p "$PACKAGE_DIR/lib/seen/toolchain"
mkdir -p "$PACKAGE_DIR/share/seen/languages"
mkdir -p "$PACKAGE_DIR/share/seen/docs"

# --- Compiler binary ---
if [ ! -f "$WIN_DIR/seen.exe" ]; then
    echo "ERROR: seen.exe not found in $WIN_DIR"
    echo "Run scripts/build_windows.sh first."
    exit 1
fi
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
    find "$PACKAGE_DIR/lib/seen/std" -type f -name '*.tmp.*' -delete
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
