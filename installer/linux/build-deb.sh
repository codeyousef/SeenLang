#!/usr/bin/env bash
# Build script for Seen Language DEB package
# Creates .deb packages for Debian/Ubuntu systems

set -e

# Configuration
VERSION=""
ARCH=""
SOURCE_DIR="../../target-wsl/release"
OUTPUT_DIR="output"
VERBOSE=false

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
Seen Language DEB Package Builder

Usage: $0 <version> <architecture> [options]

Arguments:
  version              Version number (e.g., 1.0.0)
  architecture         Target architecture (amd64, arm64, riscv64)

Options:
  --source-dir DIR     Source directory with binaries (default: $SOURCE_DIR)
  --output-dir DIR     Output directory (default: $OUTPUT_DIR)
  --verbose            Enable verbose output
  --help               Show this help message

Examples:
  $0 1.0.0 amd64
  $0 1.2.3 arm64 --verbose
  $0 2.0.0 amd64 --source-dir /opt/seen/build

Requirements:
  - dpkg-deb (for package creation)
  - Seen binaries built and available in source directory
  - Standard build tools (tar, gzip, etc.)

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

# Validate architecture
case "$ARCH" in
    amd64|arm64|riscv64)
        ;;
    x64)
        ARCH="amd64"
        ;;
    *)
        error "Unsupported architecture: $ARCH. Supported: amd64, arm64, riscv64"
        ;;
esac

# Get absolute paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SOURCE_DIR="$(cd "$PROJECT_ROOT/$SOURCE_DIR" && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/$OUTPUT_DIR"

header "Building Seen Language $VERSION DEB for $ARCH"

info "Configuration:"
info "  Version: $VERSION"
info "  Architecture: $ARCH"
info "  Source: $SOURCE_DIR"
info "  Output: $OUTPUT_DIR"
info "  Project Root: $PROJECT_ROOT"

# Check dependencies
check_dependencies() {
    local deps=("dpkg-deb" "tar" "gzip" "find" "chmod")
    local missing=()
    
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

# Create package structure
create_package_structure() {
    local temp_dir="$1"
    local package_dir="$temp_dir/seen_${VERSION}_${ARCH}"
    
    info "Creating package structure..."
    
    # Create DEBIAN control directory
    mkdir -p "$package_dir/DEBIAN"
    
    # Create directory structure
    mkdir -p "$package_dir/usr/bin"
    mkdir -p "$package_dir/usr/lib/seen"
    mkdir -p "$package_dir/usr/share/seen"
    mkdir -p "$package_dir/usr/share/doc/seen"
    mkdir -p "$package_dir/usr/share/man/man1"
    mkdir -p "$package_dir/usr/share/applications"
    mkdir -p "$package_dir/usr/share/pixmaps"
    
    echo "$package_dir"
}

# Create control file
create_control_file() {
    local package_dir="$1"
    local control_file="$package_dir/DEBIAN/control"
    
    info "Creating control file..."
    
    # Calculate installed size (in KB)
    local size_kb=$(du -sk "$package_dir" | cut -f1)
    
    cat > "$control_file" << EOF
Package: seen-lang
Version: $VERSION
Section: devel
Priority: optional
Architecture: $ARCH
Essential: no
Installed-Size: $size_kb
Maintainer: Seen Language Team <team@seen-lang.org>
Homepage: https://seen-lang.org
Vcs-Git: https://github.com/seen-lang/seen.git
Vcs-Browser: https://github.com/seen-lang/seen
Depends: libc6 (>= 2.28), libgcc-s1 (>= 3.0), libstdc++6 (>= 5.2)
Suggests: build-essential, gcc, clang
Description: High-performance systems programming language
 Seen is a revolutionary systems programming language designed to be the
 world's most performant language while providing intuitive developer
 experience. Key features include:
 .
  * Dual execution: JIT (<50ms) + AOT (beats C/Rust)
  * Vale-style memory model: Zero overhead safety without borrow checker
  * Universal deployment: Same codebase for backend, web, mobile, desktop
  * Zig-style C interop: Import C headers directly, no bindings needed
  * Multi-target: Native, WASM, mobile from single source
 .
 This package includes the Seen compiler, standard library, language server,
 and documentation.
EOF
    
    success "✓ Control file created"
}

# Create pre/post install scripts
create_install_scripts() {
    local package_dir="$1"
    
    info "Creating install scripts..."
    
    # Post-install script
    cat > "$package_dir/DEBIAN/postinst" << 'EOF'
#!/bin/bash
set -e

case "$1" in
    configure)
        # Update alternatives for seen command
        update-alternatives --install /usr/bin/seen seen /usr/bin/seen 100
        
        # Create symlinks for compatibility
        if [ ! -e /usr/local/bin/seen ]; then
            ln -sf /usr/bin/seen /usr/local/bin/seen 2>/dev/null || true
        fi
        
        # Update man database
        if command -v mandb >/dev/null 2>&1; then
            mandb -q 2>/dev/null || true
        fi
        
        # Print installation success message
        echo "Seen Language installed successfully!"
        echo "Run 'seen --version' to verify installation."
        echo "Documentation: https://docs.seen-lang.org"
        ;;
esac

exit 0
EOF
    
    # Pre-remove script
    cat > "$package_dir/DEBIAN/prerm" << 'EOF'
#!/bin/bash
set -e

case "$1" in
    remove|upgrade|deconfigure)
        # Remove alternatives
        update-alternatives --remove seen /usr/bin/seen 2>/dev/null || true
        
        # Remove symlinks
        rm -f /usr/local/bin/seen 2>/dev/null || true
        ;;
esac

exit 0
EOF
    
    # Post-remove script  
    cat > "$package_dir/DEBIAN/postrm" << 'EOF'
#!/bin/bash
set -e

case "$1" in
    purge)
        # Clean up any remaining configuration files
        rm -rf /usr/share/seen 2>/dev/null || true
        
        # Update man database
        if command -v mandb >/dev/null 2>&1; then
            mandb -q 2>/dev/null || true
        fi
        
        echo "Seen Language completely removed."
        ;;
esac

exit 0
EOF
    
    # Make scripts executable
    chmod 755 "$package_dir/DEBIAN/postinst"
    chmod 755 "$package_dir/DEBIAN/prerm"
    chmod 755 "$package_dir/DEBIAN/postrm"
    
    success "✓ Install scripts created"
}

# Install files into package
install_package_files() {
    local package_dir="$1"
    
    info "Installing package files..."
    
    # Install binaries
    cp "$SOURCE_DIR/seen" "$package_dir/usr/bin/"
    chmod 755 "$package_dir/usr/bin/seen"
    
    if [ -f "$SOURCE_DIR/seen-lsp" ]; then
        cp "$SOURCE_DIR/seen-lsp" "$package_dir/usr/bin/"
        chmod 755 "$package_dir/usr/bin/seen-lsp"
    fi
    
    if [ -f "$SOURCE_DIR/seen-riscv" ]; then
        cp "$SOURCE_DIR/seen-riscv" "$package_dir/usr/bin/"
        chmod 755 "$package_dir/usr/bin/seen-riscv"
    fi
    
    # Install standard library
    if [ -d "$PROJECT_ROOT/seen_std" ]; then
        cp -r "$PROJECT_ROOT/seen_std"/* "$package_dir/usr/lib/seen/"
    fi
    
    # Install language configurations
    if [ -d "$PROJECT_ROOT/languages" ]; then
        cp -r "$PROJECT_ROOT/languages" "$package_dir/usr/share/seen/"
    fi
    
    # Install documentation
    if [ -d "$PROJECT_ROOT/docs" ]; then
        cp -r "$PROJECT_ROOT/docs"/* "$package_dir/usr/share/doc/seen/" 2>/dev/null || true
    fi
    
    # Create man page
    create_man_page "$package_dir/usr/share/man/man1/seen.1"
    gzip "$package_dir/usr/share/man/man1/seen.1"
    
    # Create desktop entry
    create_desktop_entry "$package_dir/usr/share/applications/seen.desktop"
    
    # Create copyright file
    create_copyright_file "$package_dir/usr/share/doc/seen/copyright"
    
    # Create changelog
    create_changelog "$package_dir/usr/share/doc/seen/changelog.Debian"
    gzip "$package_dir/usr/share/doc/seen/changelog.Debian"
    
    success "✓ Package files installed"
}

# Create man page
create_man_page() {
    local man_file="$1"
    
    cat > "$man_file" << EOF
.TH SEEN 1 "$(date +'%B %Y')" "seen $VERSION" "User Commands"
.SH NAME
seen \- Seen programming language compiler and toolchain

.SH SYNOPSIS
.B seen
[\fIOPTION\fR]...
[\fICOMMAND\fR]
[\fIARGS\fR]...

.SH DESCRIPTION
Seen is a high-performance systems programming language that combines the safety of modern languages with the performance of C and C++.

The \fBseen\fR command provides a complete toolchain for building, running, testing, and managing Seen projects.

.SH COMMANDS
.TP
\fBbuild\fR
Build the current project
.TP
\fBrun\fR
Run the current project (JIT mode)
.TP
\fBcheck\fR
Check the current project for errors without building
.TP
\fBtest\fR
Run project tests
.TP
\fBclean\fR
Clean build artifacts
.TP
\fBformat\fR
Format source code and documents
.TP
\fBinit\fR \fINAME\fR
Create a new Seen project
.TP
\fBlsp\fR
Start the language server
.TP
\fBdoc\fR
Generate documentation

.SH OPTIONS
.TP
\fB\-v\fR, \fB\-\-verbose\fR
Enable verbose output
.TP
\fB\-q\fR, \fB\-\-quiet\fR
Suppress output
.TP
\fB\-\-version\fR
Show version information
.TP
\fB\-h\fR, \fB\-\-help\fR
Show help message

.SH EXAMPLES
.TP
\fBseen init hello\fR
Create a new project named "hello"
.TP
\fBseen build\fR
Build the current project
.TP
\fBseen run\fR
Run the current project
.TP
\fBseen test\fR
Run project tests

.SH FILES
.TP
\fBSeen.toml\fR
Project configuration file
.TP
\fB/usr/lib/seen/\fR
Standard library location
.TP
\fB/usr/share/seen/\fR
Shared data files

.SH SEE ALSO
Full documentation at: https://docs.seen-lang.org

.SH BUGS
Report bugs at: https://github.com/seen-lang/seen/issues

.SH AUTHORS
Seen Language Team <team@seen-lang.org>
EOF
}

# Create desktop entry
create_desktop_entry() {
    local desktop_file="$1"
    
    cat > "$desktop_file" << EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=Seen Language
Comment=High-performance systems programming language
Exec=seen
Icon=seen
Terminal=true
Categories=Development;IDE;
Keywords=programming;compiler;systems;development;
StartupNotify=false
EOF
}

# Create copyright file
create_copyright_file() {
    local copyright_file="$1"
    
    cat > "$copyright_file" << EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: Seen Language
Upstream-Contact: Seen Language Team <team@seen-lang.org>
Source: https://github.com/seen-lang/seen

Files: *
Copyright: $(date +%Y) Seen Language Team
License: MIT

License: MIT
 Permission is hereby granted, free of charge, to any person obtaining a copy
 of this software and associated documentation files (the "Software"), to deal
 in the Software without restriction, including without limitation the rights
 to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 copies of the Software, and to permit persons to whom the Software is
 furnished to do so, subject to the following conditions:
 .
 The above copyright notice and this permission notice shall be included in all
 copies or substantial portions of the Software.
 .
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 SOFTWARE.
EOF
}

# Create changelog
create_changelog() {
    local changelog_file="$1"
    
    cat > "$changelog_file" << EOF
seen-lang ($VERSION) unstable; urgency=medium

  * Initial release of Seen Language $VERSION
  * High-performance systems programming language
  * JIT and AOT compilation support
  * Multi-platform targeting (native, WASM, mobile)
  * Comprehensive standard library
  * Language server integration

 -- Seen Language Team <team@seen-lang.org>  $(date -R)
EOF
}

# Build the DEB package
build_package() {
    local temp_dir="$1"
    local package_dir="$2"
    
    info "Building DEB package..."
    
    # Create output directory
    mkdir -p "$OUTPUT_DIR"
    
    # Build package
    local deb_file="$OUTPUT_DIR/seen-lang_${VERSION}_${ARCH}.deb"
    
    if $VERBOSE; then
        dpkg-deb --build --root-owner-group "$package_dir" "$deb_file"
    else
        dpkg-deb --build --root-owner-group "$package_dir" "$deb_file" >/dev/null 2>&1
    fi
    
    if [ $? -eq 0 ] && [ -f "$deb_file" ]; then
        success "✓ DEB package created: $deb_file"
        
        # Show package info
        local size_mb=$(du -m "$deb_file" | cut -f1)
        info "  Size: ${size_mb}MB"
        
        # Generate checksum
        local checksum=$(sha256sum "$deb_file" | cut -d' ' -f1)
        echo "$checksum  $(basename "$deb_file")" > "$deb_file.sha256"
        info "  SHA256: $deb_file.sha256"
        
        return 0
    else
        error "Failed to create DEB package"
    fi
}

# Main build process
main() {
    header "DEB Package Build"
    
    # Validate environment
    check_dependencies
    validate_sources
    
    # Create temporary directory
    local temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT
    
    # Create package structure
    local package_dir=$(create_package_structure "$temp_dir")
    
    # Build package contents
    create_control_file "$package_dir"
    create_install_scripts "$package_dir"
    install_package_files "$package_dir"
    
    # Build the package
    build_package "$temp_dir" "$package_dir"
    
    success ""
    success "==============================================="
    success "     DEB package build completed!             "
    success "==============================================="
    success ""
    success "Package: $OUTPUT_DIR/seen-lang_${VERSION}_${ARCH}.deb"
    success ""
    success "To install:"
    success "  sudo dpkg -i $OUTPUT_DIR/seen-lang_${VERSION}_${ARCH}.deb"
    success "  sudo apt-get install -f  # Fix dependencies if needed"
    success ""
}

# Run main function
main "$@"