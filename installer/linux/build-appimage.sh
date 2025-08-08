#!/usr/bin/env bash
# Build script for Seen Language AppImage
# Creates portable AppImage packages for Linux

set -e

# Configuration
VERSION=""
ARCH=""
SOURCE_DIR="../../target-wsl/release"
OUTPUT_DIR="output"
VERBOSE=false
APPIMAGE_TOOL_URL="https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Logging functions
error() {
    echo -e "${RED}Error: $1${NC}" >&2
    exit 1
}

warning() {
    echo -e "${YELLOW}Warning: $1${NC}"
}

info() {
    echo -e "${BLUE}$1${NC}"
}

success() {
    echo -e "${GREEN}$1${NC}"
}

header() {
    echo ""
    echo -e "${CYAN}===============================================${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}===============================================${NC}"
    echo ""
}

show_help() {
    cat << EOF
Seen Language AppImage Builder

Usage: $0 <version> <architecture> [options]

Arguments:
  version              Version number (e.g., 1.0.0)
  architecture         Target architecture (x86_64, aarch64, riscv64)

Options:
  --source-dir DIR     Source directory with binaries (default: $SOURCE_DIR)
  --output-dir DIR     Output directory (default: $OUTPUT_DIR)
  --verbose            Enable verbose output
  --help               Show this help message

Examples:
  $0 1.0.0 x86_64
  $0 1.2.3 aarch64 --verbose

Requirements:
  - wget or curl (for downloading appimagetool)
  - fuse2 or fuse3 (for running AppImages)
  - Seen binaries built and available in source directory

AppImage provides a portable format that runs on most Linux distributions
without installation. The resulting AppImage contains all dependencies.

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --source-dir)
            SOURCE_DIR="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --help)
            show_help
            exit 0
            ;;
        *)
            if [ -z "$VERSION" ]; then
                VERSION="$1"
            elif [ -z "$ARCH" ]; then
                ARCH="$1"
            else
                error "Unknown argument: $1"
            fi
            shift
            ;;
    esac
done

# Validate required arguments
if [ -z "$VERSION" ]; then
    error "Version is required"
fi

if [ -z "$ARCH" ]; then
    error "Architecture is required"
fi

# Normalize architecture
case "$ARCH" in
    amd64|x64)
        ARCH="x86_64"
        ;;
    arm64)
        ARCH="aarch64"
        ;;
    x86_64|aarch64|riscv64)
        # Already correct
        ;;
    *)
        error "Unsupported architecture: $ARCH. Supported: x86_64, aarch64, riscv64"
        ;;
esac

# Get absolute paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SOURCE_DIR="$(cd "$PROJECT_ROOT/$SOURCE_DIR" && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/$OUTPUT_DIR"

header "Building Seen Language $VERSION AppImage for $ARCH"

info "Configuration:"
info "  Version: $VERSION"
info "  Architecture: $ARCH"
info "  Source: $SOURCE_DIR"
info "  Output: $OUTPUT_DIR"
info "  Project Root: $PROJECT_ROOT"

# Check dependencies
check_dependencies() {
    local deps=("tar" "wget" "chmod" "find")
    local missing=()
    
    # Check for wget or curl
    if ! command -v wget &> /dev/null && ! command -v curl &> /dev/null; then
        missing+=("wget or curl")
    fi
    
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            missing+=("$dep")
        fi
    done
    
    if [ ${#missing[@]} -gt 0 ]; then
        error "Missing dependencies: ${missing[*]}"
    fi
    
    success "✓ All dependencies found"
}

# Check for FUSE
check_fuse() {
    if [ ! -e /dev/fuse ]; then
        warning "FUSE not available. AppImage may not run on this system."
    fi
    
    # Check if we can run AppImages
    if command -v fusermount &> /dev/null || command -v fusermount3 &> /dev/null; then
        success "✓ FUSE support available"
    else
        warning "FUSE tools not found. Install fuse2 or fuse3 package."
    fi
}

# Validate source files
validate_sources() {
    info "Validating source files..."
    
    local required_files=(
        "$SOURCE_DIR/seen"
        "$PROJECT_ROOT/seen_std"
        "$PROJECT_ROOT/languages"
    )
    
    local missing_files=()
    
    for file in "${required_files[@]}"; do
        if [ ! -e "$file" ]; then
            missing_files+=("$file")
        fi
    done
    
    if [ ${#missing_files[@]} -gt 0 ]; then
        error "Missing required files: ${missing_files[*]}"
    fi
    
    # Check optional files
    local optional_files=(
        "$SOURCE_DIR/seen-lsp"
        "$SOURCE_DIR/seen-riscv"
        "$PROJECT_ROOT/docs"
    )
    
    for file in "${optional_files[@]}"; do
        if [ ! -e "$file" ]; then
            warning "Optional file missing: $file"
        fi
    done
    
    success "✓ Source validation passed"
}

# Download appimagetool
download_appimagetool() {
    local temp_dir="$1"
    local appimagetool="$temp_dir/appimagetool"
    
    info "Downloading appimagetool..."
    
    # Choose download tool
    if command -v wget &> /dev/null; then
        wget -q -O "$appimagetool" "$APPIMAGE_TOOL_URL"
    elif command -v curl &> /dev/null; then
        curl -sL -o "$appimagetool" "$APPIMAGE_TOOL_URL"
    else
        error "Neither wget nor curl available"
    fi
    
    if [ ! -f "$appimagetool" ] || [ ! -s "$appimagetool" ]; then
        error "Failed to download appimagetool"
    fi
    
    chmod +x "$appimagetool"
    success "✓ appimagetool downloaded"
    
    echo "$appimagetool"
}

# Create AppDir structure
create_appdir() {
    local temp_dir="$1"
    local appdir="$temp_dir/SeenLanguage.AppDir"
    
    info "Creating AppDir structure..."
    
    # Create AppDir structure
    mkdir -p "$appdir"/{usr/bin,usr/lib/seen,usr/share/{seen,applications,icons/hicolor/256x256/apps}}
    
    echo "$appdir"
}

# Create AppRun script
create_apprun() {
    local appdir="$1"
    local apprun="$appdir/AppRun"
    
    info "Creating AppRun script..."
    
    cat > "$apprun" << 'EOF'
#!/bin/bash
# AppRun script for Seen Language AppImage

# Get the directory where this AppImage is mounted
HERE="$(dirname "$(readlink -f "${0}")")"

# Set up environment
export PATH="${HERE}/usr/bin:$PATH"
export LD_LIBRARY_PATH="${HERE}/usr/lib:$LD_LIBRARY_PATH"
export SEEN_LIB_PATH="${HERE}/usr/lib/seen"
export SEEN_DATA_PATH="${HERE}/usr/share/seen"

# Handle different invocation methods
BINARY_NAME=$(basename "$0")

case "$BINARY_NAME" in
    seen|SeenLanguage)
        exec "${HERE}/usr/bin/seen" "$@"
        ;;
    seen-lsp)
        if [ -f "${HERE}/usr/bin/seen-lsp" ]; then
            exec "${HERE}/usr/bin/seen-lsp" "$@"
        else
            echo "LSP server not available in this AppImage"
            exit 1
        fi
        ;;
    seen-riscv)
        if [ -f "${HERE}/usr/bin/seen-riscv" ]; then
            exec "${HERE}/usr/bin/seen-riscv" "$@"
        else
            echo "RISC-V tools not available in this AppImage"
            exit 1
        fi
        ;;
    *)
        # Default to main seen binary
        exec "${HERE}/usr/bin/seen" "$@"
        ;;
esac
EOF
    
    chmod +x "$apprun"
    success "✓ AppRun script created"
}

# Create desktop file
create_desktop_file() {
    local appdir="$1"
    local desktop_file="$appdir/SeenLanguage.desktop"
    
    info "Creating desktop file..."
    
    cat > "$desktop_file" << EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=Seen Language
Comment=High-performance systems programming language
Exec=seen
Icon=SeenLanguage
Terminal=true
Categories=Development;IDE;Programming;
Keywords=programming;compiler;systems;development;language;
StartupNotify=false
MimeType=text/x-seen;
X-AppImage-Version=$VERSION
X-AppImage-BuildId=$(date +%Y%m%d%H%M%S)
EOF
    
    # Copy desktop file to applications directory as well
    cp "$desktop_file" "$appdir/usr/share/applications/"
    
    success "✓ Desktop file created"
}

# Create icon
create_icon() {
    local appdir="$1"
    local icon_file="$appdir/SeenLanguage.png"
    local icon_dir="$appdir/usr/share/icons/hicolor/256x256/apps"
    
    info "Creating application icon..."
    
    # Create a simple SVG icon and convert to PNG (if available)
    # For now, create a placeholder PNG using ImageMagick if available
    if command -v convert &> /dev/null; then
        # Create a simple 256x256 icon
        convert -size 256x256 xc:transparent \
                -fill "#2E86C1" -draw "circle 128,128 128,50" \
                -fill white -font DejaVu-Sans-Bold -pointsize 72 \
                -draw "text 96,140 'S'" \
                "$icon_file"
    else
        # Create a minimal PNG header (placeholder)
        # This is a very basic 1x1 transparent PNG
        printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x01\x00\x00\x00\x01\x00\x08\x06\x00\x00\x00\x5c\x72\xa8\x66\x00\x00\x00\x0bIDATx\x9cc\xf8\x0f\x00\x00\x01\x00\x01\x00\x18\xdd\x8d\xb4\x1c\x00\x00\x00\x00IEND\xaeB\x60\x82' > "$icon_file"
        warning "ImageMagick not available. Created placeholder icon."
    fi
    
    # Copy icon to standard location
    cp "$icon_file" "$icon_dir/SeenLanguage.png"
    
    success "✓ Application icon created"
}

# Install application files
install_app_files() {
    local appdir="$1"
    
    info "Installing application files..."
    
    # Install binaries
    cp "$SOURCE_DIR/seen" "$appdir/usr/bin/"
    chmod +x "$appdir/usr/bin/seen"
    
    if [ -f "$SOURCE_DIR/seen-lsp" ]; then
        cp "$SOURCE_DIR/seen-lsp" "$appdir/usr/bin/"
        chmod +x "$appdir/usr/bin/seen-lsp"
        info "  ✓ LSP server included"
    fi
    
    if [ -f "$SOURCE_DIR/seen-riscv" ]; then
        cp "$SOURCE_DIR/seen-riscv" "$appdir/usr/bin/"  
        chmod +x "$appdir/usr/bin/seen-riscv"
        info "  ✓ RISC-V tools included"
    fi
    
    # Install standard library
    if [ -d "$PROJECT_ROOT/seen_std" ]; then
        cp -r "$PROJECT_ROOT/seen_std"/* "$appdir/usr/lib/seen/"
        info "  ✓ Standard library installed"
    fi
    
    # Install language configurations
    if [ -d "$PROJECT_ROOT/languages" ]; then
        cp -r "$PROJECT_ROOT/languages" "$appdir/usr/share/seen/"
        info "  ✓ Language configurations installed"
    fi
    
    # Install documentation
    if [ -d "$PROJECT_ROOT/docs" ]; then
        mkdir -p "$appdir/usr/share/doc/seen"
        cp -r "$PROJECT_ROOT/docs"/* "$appdir/usr/share/doc/seen/" 2>/dev/null || true
        info "  ✓ Documentation installed"
    fi
    
    success "✓ Application files installed"
}

# Bundle dependencies
bundle_dependencies() {
    local appdir="$1"
    
    info "Bundling dependencies..."
    
    # Find shared library dependencies
    local binaries=($(find "$appdir/usr/bin" -type f -executable 2>/dev/null))
    local needed_libs=()
    
    for binary in "${binaries[@]}"; do
        if command -v ldd &> /dev/null; then
            # Get library dependencies
            local deps=$(ldd "$binary" 2>/dev/null | grep -E '^\s*[^/]*\.so' | awk '{print $3}' | grep -v '^$' || true)
            
            for dep in $deps; do
                if [ -f "$dep" ] && [[ ! "$dep" =~ ^/(lib|usr/lib)/(x86_64-linux-gnu/)?(ld-|libc\.|libdl\.|libm\.|libpthread\.|libresolv\.|librt\.) ]]; then
                    needed_libs+=("$dep")
                fi
            done
        fi
    done
    
    # Copy needed libraries (excluding system libraries)
    if [ ${#needed_libs[@]} -gt 0 ]; then
        mkdir -p "$appdir/usr/lib"
        
        for lib in "${needed_libs[@]}"; do
            if [ ! -f "$appdir/usr/lib/$(basename "$lib")" ]; then
                cp "$lib" "$appdir/usr/lib/" 2>/dev/null || true
            fi
        done
        
        info "  ✓ Bundled ${#needed_libs[@]} dependencies"
    else
        info "  ✓ No additional dependencies needed"
    fi
}

# Build AppImage
build_appimage() {
    local temp_dir="$1"
    local appdir="$2"
    local appimagetool="$3"
    
    info "Building AppImage..."
    
    # Create output directory
    mkdir -p "$OUTPUT_DIR"
    
    # Set output filename
    local appimage_file="$OUTPUT_DIR/SeenLanguage-$VERSION-$ARCH.AppImage"
    
    # Build AppImage
    local build_opts=()
    if $VERBOSE; then
        build_opts+=("-v")
    else
        build_opts+=("--no-appstream")
    fi
    
    # Set environment for appimagetool
    export ARCH="$ARCH"
    export VERSION="$VERSION"
    
    # Run appimagetool
    "$appimagetool" "${build_opts[@]}" "$appdir" "$appimage_file"
    
    if [ $? -eq 0 ] && [ -f "$appimage_file" ]; then
        success "✓ AppImage created: $appimage_file"
        
        # Make it executable
        chmod +x "$appimage_file"
        
        # Show package info
        local size_mb=$(du -m "$appimage_file" | cut -f1)
        info "  Size: ${size_mb}MB"
        
        # Generate checksum
        local checksum=$(sha256sum "$appimage_file" | cut -d' ' -f1)
        echo "$checksum  $(basename "$appimage_file")" > "$appimage_file.sha256"
        info "  SHA256: $appimage_file.sha256"
        
        return 0
    else
        error "Failed to create AppImage"
    fi
}

# Test AppImage
test_appimage() {
    local appimage_file="$1"
    
    info "Testing AppImage..."
    
    # Basic test - try to get version
    if [ -x "$appimage_file" ]; then
        local version_output
        if version_output=$("$appimage_file" --version 2>&1); then
            success "  ✓ AppImage runs successfully: $version_output"
            return 0
        else
            warning "  AppImage test failed: $version_output"
            return 1
        fi
    else
        warning "  AppImage is not executable"
        return 1
    fi
}

# Main build process
main() {
    header "AppImage Build"
    
    # Validate environment
    check_dependencies
    check_fuse
    validate_sources
    
    # Create temporary directory
    local temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT
    
    # Download tools
    local appimagetool=$(download_appimagetool "$temp_dir")
    
    # Create AppDir
    local appdir=$(create_appdir "$temp_dir")
    
    # Build AppDir contents
    create_apprun "$appdir"
    create_desktop_file "$appdir"
    create_icon "$appdir"
    install_app_files "$appdir"
    bundle_dependencies "$appdir"
    
    # Build AppImage
    build_appimage "$temp_dir" "$appdir" "$appimagetool"
    
    # Test the result
    local appimage_file="$OUTPUT_DIR/SeenLanguage-$VERSION-$ARCH.AppImage"
    test_appimage "$appimage_file"
    
    success ""
    success "==============================================="
    success "     AppImage build completed!                "
    success "==============================================="
    success ""
    success "AppImage: $appimage_file"
    success ""
    success "To run:"
    success "  chmod +x $appimage_file"
    success "  $appimage_file --version"
    success "  # OR double-click in file manager"
    success ""
    success "The AppImage is portable and runs on most Linux distributions"
    success "without installation. It contains all necessary dependencies."
    success ""
}

# Run main function
main "$@"