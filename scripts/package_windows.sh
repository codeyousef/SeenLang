#!/bin/bash
# Package the Seen compiler for Windows distribution
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WIN_DIR="${PROJECT_DIR}/target-windows"
VERSION="${1:-1.0.0}"

PACKAGE_DIR="${WIN_DIR}/seen-${VERSION}-windows-x64"
rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR/bin"
mkdir -p "$PACKAGE_DIR/lib/seen/std"
mkdir -p "$PACKAGE_DIR/lib/seen/runtime"
mkdir -p "$PACKAGE_DIR/share/seen/languages"

# Copy compiler binary
if [ ! -f "$WIN_DIR/seen.exe" ]; then
    echo "ERROR: seen.exe not found. Run scripts/build_windows.sh first."
    exit 1
fi
cp "$WIN_DIR/seen.exe" "$PACKAGE_DIR/bin/"

# Copy standard library
if [ -d "$PROJECT_DIR/seen_std/src" ]; then
    cp -r "$PROJECT_DIR/seen_std/src/"* "$PACKAGE_DIR/lib/seen/std/"
fi
if [ -f "$PROJECT_DIR/seen_std/Seen.toml" ]; then
    cp "$PROJECT_DIR/seen_std/Seen.toml" "$PACKAGE_DIR/lib/seen/std/"
fi

# Copy runtime headers (needed for compilation)
cp "$PROJECT_DIR/seen_runtime/seen_runtime.h" "$PACKAGE_DIR/lib/seen/runtime/"
cp "$PROJECT_DIR/seen_runtime/seen_region.h" "$PACKAGE_DIR/lib/seen/runtime/"

# Copy language files
for lang_dir in "$PROJECT_DIR/languages"/*/; do
    lang=$(basename "$lang_dir")
    mkdir -p "$PACKAGE_DIR/share/seen/languages/$lang"
    cp "$lang_dir"*.toml "$PACKAGE_DIR/share/seen/languages/$lang/" 2>/dev/null || true
done

# Create a simple setup batch file
cat > "$PACKAGE_DIR/setup.bat" << 'SETUP_EOF'
@echo off
echo ============================================
echo     Seen Language Setup for Windows
echo ============================================
echo.

:: Get the directory where this script is located
set "SEEN_HOME=%~dp0"
set "SEEN_BIN=%SEEN_HOME%bin"

:: Check if already in PATH
echo %PATH% | findstr /I /C:"%SEEN_BIN%" >nul 2>&1
if %errorlevel% equ 0 (
    echo Seen is already in your PATH.
) else (
    echo Adding Seen to your user PATH...
    setx PATH "%PATH%;%SEEN_BIN%" >nul 2>&1
    if %errorlevel% equ 0 (
        echo Successfully added to PATH. Restart your terminal to use 'seen'.
    ) else (
        echo Failed to update PATH automatically.
        echo Please add this directory to your PATH manually:
        echo   %SEEN_BIN%
    )
)

:: Set SEEN_HOME environment variable
setx SEEN_HOME "%SEEN_HOME%" >nul 2>&1

echo.
echo Installation complete!
echo Open a new terminal and run: seen build hello.seen -o hello
echo.
pause
SETUP_EOF

# Create README
cat > "$PACKAGE_DIR/README.txt" << 'README_EOF'
Seen Language for Windows
=========================

Quick Start:
  1. Run setup.bat to add Seen to your PATH
  2. Create hello.seen:
     fun main() { println("Hello from Seen!") }
  3. Compile: seen build hello.seen -o hello
  4. Run: hello.exe

Requirements:
  - LLVM 18+ (clang, opt) must be installed and in PATH
  - Download from: https://github.com/llvm/llvm-project/releases

For more info: https://github.com/codeyousef/SeenLang
README_EOF

# Create zip
cd "$WIN_DIR"
ZIPFILE="seen-${VERSION}-windows-x64.zip"
rm -f "$ZIPFILE"

if command -v zip &>/dev/null; then
    zip -r "$ZIPFILE" "seen-${VERSION}-windows-x64/"
elif command -v 7z &>/dev/null; then
    7z a "$ZIPFILE" "seen-${VERSION}-windows-x64/"
else
    # Fallback: tar.gz
    ZIPFILE="seen-${VERSION}-windows-x64.tar.gz"
    tar czf "$ZIPFILE" "seen-${VERSION}-windows-x64/"
fi

echo ""
echo "=== Package created ==="
echo "  $WIN_DIR/$ZIPFILE"
echo "  Size: $(du -h "$WIN_DIR/$ZIPFILE" | cut -f1)"
