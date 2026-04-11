#!/bin/bash
# Build Windows installer for Seen Language from Linux
#
# Usage:
#   ./scripts/build_windows_installer.sh [version]
#   ./scripts/build_windows_installer.sh 1.0.0 --skip-compile   # Skip cross-compilation (use existing seen.exe)
#   ./scripts/build_windows_installer.sh 1.0.0 --nsis            # Build NSIS installer only
#   ./scripts/build_windows_installer.sh 1.0.0 --zip-only        # Build ZIP package only
#
# Prerequisites:
#   Cross-compilation: llvm-20, clang-20, gcc-mingw-w64-x86-64, python3
#   NSIS installer:    nsis (sudo apt-get install nsis)
#   Inno Setup:        wine + Inno Setup 6 (optional)
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
VERSION="${1:-1.0.0}"
shift 2>/dev/null || true

# Parse options
SKIP_COMPILE=false
NSIS_ONLY=false
ZIP_ONLY=false
while [ $# -gt 0 ]; do
    case "$1" in
        --skip-compile) SKIP_COMPILE=true ;;
        --nsis) NSIS_ONLY=true ;;
        --zip-only) ZIP_ONLY=true ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
    shift
done

WIN_DIR="${PROJECT_DIR}/target-windows"
PACKAGE_DIR="${WIN_DIR}/seen-${VERSION}-windows-x64"
INSTALLER_DIR="${PROJECT_DIR}/installer/windows"
NUMERIC_VERSION="${VERSION%%-*}"
IFS='.' read -r VI_MAJOR VI_MINOR VI_PATCH _ <<EOF
$NUMERIC_VERSION
EOF
PRODUCT_VERSION="${VI_MAJOR:-0}.${VI_MINOR:-0}.${VI_PATCH:-0}.0"

echo "============================================"
echo "  Seen Language Windows Build (from Linux)"
echo "  Version: $VERSION"
echo "============================================"
echo ""

# Step 1: Cross-compile seen.exe
if [ "$SKIP_COMPILE" = false ]; then
    echo "[1/3] Cross-compiling seen.exe for Windows..."
    echo ""

    # Check prerequisites
    MISSING=""
    command -v x86_64-w64-mingw32-gcc &>/dev/null || MISSING="$MISSING gcc-mingw-w64-x86-64"
    command -v python3 &>/dev/null || MISSING="$MISSING python3"
    (command -v opt-20 &>/dev/null || command -v opt &>/dev/null) || MISSING="$MISSING opt(llvm-20)"
    (command -v llc-20 &>/dev/null || command -v llc &>/dev/null) || MISSING="$MISSING llc(llvm-20)"

    if [ -n "$MISSING" ]; then
        echo "ERROR: Missing tools:$MISSING"
        echo ""
        echo "Install: sudo apt-get install llvm-20 clang-20 lld-20 gcc-mingw-w64-x86-64 python3"
        exit 1
    fi

    mkdir -p "$WIN_DIR"

    # The full 50-module compiler cannot be cold-compiled as a single file.
    # Check for pre-built artifacts in target-windows/ instead.
    if [ -f "$WIN_DIR/seen.exe" ]; then
        echo "  Found existing seen.exe in target-windows/, reusing it."
    elif [ -d "$WIN_DIR" ] && ls "$WIN_DIR"/seen_module_*_win.s &>/dev/null; then
        echo "  Found pre-built Windows assembly files, linking..."
        bash "$SCRIPT_DIR/build_windows.sh" \
            "$PROJECT_DIR/compiler_seen/src/main.seen" \
            "$WIN_DIR/seen.exe"
    else
        echo "ERROR: Cannot cross-compile the full compiler as a single file."
        echo ""
        echo "The Seen compiler (50 modules) requires IR cache to build."
        echo "Options:"
        echo "  1. Cross-compile individual .seen programs:"
        echo "     bash scripts/build_windows.sh <source.seen> <output.exe>"
        echo "  2. Place a pre-built seen.exe in target-windows/seen.exe"
        echo "  3. Use --skip-compile with an existing seen.exe"
        exit 1
    fi

    if [ ! -f "$WIN_DIR/seen.exe" ]; then
        echo "ERROR: seen.exe not created in $WIN_DIR"
        exit 1
    fi
    echo ""
else
    echo "[1/3] Skipping cross-compilation (--skip-compile)"
    if [ ! -f "$WIN_DIR/seen.exe" ]; then
        echo "ERROR: seen.exe not found. Run without --skip-compile first."
        exit 1
    fi
    echo ""
fi

# Step 2: Package
echo "[2/3] Packaging..."
bash "$SCRIPT_DIR/package_windows.sh" "$VERSION"
echo ""

if [ "$ZIP_ONLY" = true ]; then
    echo "Done (--zip-only). Output: $WIN_DIR/seen-${VERSION}-windows-x64.zip"
    exit 0
fi

# Step 3: Build installer
echo "[3/3] Building installer..."

# Try NSIS (native on Linux)
if command -v makensis &>/dev/null; then
    echo "  Using NSIS (makensis)..."
    mkdir -p "$INSTALLER_DIR/output"

    # NSIS on Linux uses forward slashes
    makensis \
        -DVERSION="$VERSION" \
        -DPRODUCT_VERSION="$PRODUCT_VERSION" \
        -DSOURCE_DIR="$PACKAGE_DIR" \
        "$INSTALLER_DIR/seen.nsi"

    NSIS_EXE="$INSTALLER_DIR/output/Seen-${VERSION}-windows-x64-setup.exe"
    if [ -f "$NSIS_EXE" ]; then
        SIZE=$(du -h "$NSIS_EXE" | cut -f1)
        SHA=$(sha256sum "$NSIS_EXE" | cut -d' ' -f1)
        echo "$SHA  $(basename "$NSIS_EXE")" > "$NSIS_EXE.sha256"

        echo ""
        echo "=== NSIS Installer Created ==="
        echo "  File: $NSIS_EXE"
        echo "  Size: $SIZE"
        echo "  SHA256: $SHA"
    else
        echo "WARNING: NSIS build did not produce output"
    fi

elif [ "$NSIS_ONLY" = true ]; then
    echo "ERROR: makensis not found. Install: sudo apt-get install nsis"
    exit 1
else
    echo "  makensis not found, skipping NSIS installer"
    echo "  Install: sudo apt-get install nsis"
fi

# Try Inno Setup via Wine (optional)
if [ "$NSIS_ONLY" = false ]; then
    ISCC=""
    for candidate in \
        "$HOME/.wine/drive_c/Program Files (x86)/Inno Setup 6/ISCC.exe" \
        "$HOME/.wine/drive_c/Program Files/Inno Setup 6/ISCC.exe"; do
        if [ -f "$candidate" ]; then
            ISCC="$candidate"
            break
        fi
    done

    if [ -n "$ISCC" ] && command -v wine &>/dev/null; then
        echo ""
        echo "  Using Inno Setup via Wine..."
        mkdir -p "$INSTALLER_DIR/output"

        # Convert paths to Wine format
        WINE_SOURCE=$(winepath -w "$PACKAGE_DIR" 2>/dev/null || echo "")
        if [ -n "$WINE_SOURCE" ]; then
            wine "$ISCC" \
                "/DVersion=$VERSION" \
                "/DSOURCE_DIR=$WINE_SOURCE" \
                "$INSTALLER_DIR/seen.iss" 2>&1 || echo "  Inno Setup build failed (non-fatal)"
        fi
    fi
fi

echo ""
echo "=== Build complete ==="
echo ""
echo "Artifacts in:"
echo "  ZIP package:  $WIN_DIR/seen-${VERSION}-windows-x64.zip"
if [ -f "$INSTALLER_DIR/output/Seen-${VERSION}-windows-x64-setup.exe" ]; then
    echo "  NSIS installer: $INSTALLER_DIR/output/Seen-${VERSION}-windows-x64-setup.exe"
fi
echo ""
echo "To test on Windows:"
echo "  1. Copy the installer to a Windows machine"
echo "  2. Run the .exe installer, or extract the .zip to any directory"
echo "  3. Ensure LLVM is installed: winget install LLVM.LLVM"
echo "  4. Open a new terminal and run: seen compile hello.seen hello --fast"
