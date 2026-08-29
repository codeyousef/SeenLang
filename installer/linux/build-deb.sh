#!/usr/bin/env bash
# Build script for Seen Language DEB package
# Creates .deb packages for Debian/Ubuntu systems

set -e

# Configuration
VERSION=""
ARCH=""
SOURCE_DIR="${SOURCE_DIR:-../../compiler_seen/target}"
OUTPUT_DIR="output"
VERBOSE=false
INSTALLER_SCOPE=""
WORK_DIR=""

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
    echo -e "${YELLOW}Warning: $1${NC}" >&2
}

info() {
    echo -e "${BLUE}$1${NC}" >&2
}

success() {
    echo -e "${GREEN}$1${NC}" >&2
}

prune_packaged_stdlib_artifacts() {
    local stdlib_dir="$1"
    [ -d "$stdlib_dir" ] || return 0

    find "$stdlib_dir" -type f -name '*.tmp.*' -exec rm -f {} +
    find "$stdlib_dir" -type d \( -name build -o -name target -o -name .seen \) \
        -prune -exec rm -rf {} +
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
  version              Version number (e.g., 0.18.1)
  architecture         Target architecture (amd64, arm64, riscv64)

Options:
  --source-dir DIR     Source directory with binaries (default: $SOURCE_DIR)
  --output-dir DIR     Output directory (default: $OUTPUT_DIR)
  --verbose            Enable verbose output
  --help               Show this help message

Examples:
  $0 0.18.1 amd64
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
ARTIFACT_HELPER="$PROJECT_ROOT/scripts/artifact_root.sh"
if [[ "$SOURCE_DIR" = /* ]]; then
    SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"
else
    SOURCE_DIR="$(cd "$PROJECT_ROOT/$SOURCE_DIR" && pwd)"
fi
if [[ "$OUTPUT_DIR" = /* ]]; then
    : # already absolute
else
    OUTPUT_DIR="$SCRIPT_DIR/$OUTPUT_DIR"
fi

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
        "$SOURCE_DIR/seen-pkg"
        "$SOURCE_DIR/compatibility-manifest.json"
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

    if [ ! -x "$SOURCE_DIR/seen" ]; then
        error "Seen compiler is not executable: $SOURCE_DIR/seen"
    fi
    if [ ! -x "$SOURCE_DIR/seen-pkg" ]; then
        error "Seen package client is not executable: $SOURCE_DIR/seen-pkg"
    fi

    local compiler_version_output
    local compiler_version_line
    if ! compiler_version_output=$("$SOURCE_DIR/seen" --version 2>/dev/null); then
        error "Could not read compiler version from $SOURCE_DIR/seen"
    fi
    IFS= read -r compiler_version_line <<< "$compiler_version_output"
    if [ "$compiler_version_line" != "Seen $VERSION" ]; then
        error "Compiler version '${compiler_version_line:-unknown}' does not match package version $VERSION"
    fi

    local package_client_handshake
    local expected_handshake
    if ! package_client_handshake=$("$SOURCE_DIR/seen-pkg" \
        --expect-version "$VERSION" version --machine 2>/dev/null); then
        error "Package client at $SOURCE_DIR/seen-pkg does not match Seen $VERSION"
    fi
    expected_handshake=$(printf 'protocol=SEENPKG1\nversion=%s' "$VERSION")
    if [ "$package_client_handshake" != "$expected_handshake" ]; then
        error "Package client at $SOURCE_DIR/seen-pkg returned an invalid version handshake"
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

cleanup_work_dir() {
    case "${WORK_DIR:-}" in
        "${INSTALLER_SCOPE:-}"/package.*)
            if [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ]; then
                rm -rf -- "$WORK_DIR"
            fi
            ;;
        "") ;;
        *) warning "Refusing to clean unexpected work directory: $WORK_DIR" ;;
    esac
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
    mkdir -p "$package_dir/usr/lib/seen/toolchain"
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
Vcs-Git: https://github.com/codeyousef/SeenLang.git
Vcs-Browser: https://github.com/codeyousef/SeenLang
Depends: libc6 (>= 2.28), libgcc-s1 (>= 3.0), libstdc++6 (>= 5.2), llvm-20 | llvm-19 | llvm (>= 1:19), clang-20 | clang-19 | clang (>= 1:19), lld-20 | lld-19 | lld (>= 1:19)
Suggests: build-essential, gcc
Description: High-performance systems programming language
 Seen is a systems programming language designed to be a
 high-performance language while providing intuitive developer
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

        if [ -x /usr/lib/seen/toolchain/seen-toolchain.sh ]; then
            /usr/lib/seen/toolchain/seen-toolchain.sh --check --prefix /usr >/dev/null 2>&1 || \
                echo "LLVM 19+ toolchain check failed; run /usr/lib/seen/toolchain/seen-toolchain.sh --install or install clang/opt/llc/llvm-as/lld."
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
    cp "$SOURCE_DIR/seen-pkg" "$package_dir/usr/bin/"
    chmod 755 "$package_dir/usr/bin/seen-pkg"
    cp "$SOURCE_DIR/compatibility-manifest.json" "$package_dir/usr/bin/"
    chmod 644 "$package_dir/usr/bin/compatibility-manifest.json"
    
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
        prune_packaged_stdlib_artifacts "$package_dir/usr/lib/seen/seen_std"
        prune_packaged_stdlib_artifacts "$package_dir/usr/lib/seen/src"
    fi

    # Install toolchain metadata and helper
    cp "$PROJECT_ROOT/scripts/seen_toolchain.sh" "$package_dir/usr/lib/seen/toolchain/seen-toolchain.sh"
    chmod 755 "$package_dir/usr/lib/seen/toolchain/seen-toolchain.sh"
    cat > "$package_dir/usr/lib/seen/toolchain/manifest.env" << EOF
seen_toolchain_manifest_version=1
llvm_min_version=19
llvm_preferred_version=20
required_tools=clang,opt,llc,llvm-as,ld.lld
bundle_mode=system-package
managed_install=/usr/lib/seen/toolchain/seen-toolchain.sh --install
EOF

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
.B seen compile
\fIINPUT.seen\fR [\fIOUTPUT\fR] [\fIOPTIONS\fR]
.br
.B seen check
\fIINPUT.seen\fR
.br
.B seen run
\fIINPUT.seen\fR [\fB--aot\fR]

.SH DESCRIPTION
Seen is a self-hosted programming language with an LLVM native-code backend.

.SH COMMANDS
.TP
\fBcompile\fR
Compile a source file to a native binary or target artifact
.TP
\fBrun\fR
Compile and run a source file (JIT by default; use --aot for an executable)
.TP
\fBcheck\fR
Check a source file for errors without building
.TP
\fBlsp\fR
Start the language server

.SH OPTIONS
.TP
\fB\-v\fR, \fB\-\-verbose\fR
Enable verbose output
.TP
\fB\-\-version\fR
Show version information
.TP
\fB\-h\fR, \fB\-\-help\fR
Show help message

.SH EXAMPLES
.TP
\fBseen check src/main.seen\fR
Check a source file
.TP
\fBseen compile src/main.seen hello\fR
Compile a native executable
.TP
\fBseen run src/main.seen\fR
Compile and run a source file

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
Report bugs at: https://github.com/codeyousef/SeenLang/issues

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
Source: https://github.com/codeyousef/SeenLang

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
    
    # Create an isolated, project-local staging directory.
    [ -f "$ARTIFACT_HELPER" ] || error "Missing artifact-root helper: $ARTIFACT_HELPER"
    # shellcheck source=scripts/artifact_root.sh
    source "$ARTIFACT_HELPER"
    seen_artifact_root_init "$PROJECT_ROOT" || error "Could not validate artifact root"
    INSTALLER_SCOPE=$(seen_artifact_scope_init installer-linux-deb) ||
        error "Could not create DEB installer artifact scope"
    WORK_DIR=$(seen_artifact_mktemp_dir "$INSTALLER_SCOPE" package) ||
        error "Could not create DEB installer work directory"
    trap cleanup_work_dir EXIT
    
    # Create package structure
    local package_dir
    package_dir=$(create_package_structure "$WORK_DIR")
    
    # Build package contents
    create_install_scripts "$package_dir"
    install_package_files "$package_dir"
    create_control_file "$package_dir"
    
    # Build the package
    build_package "$WORK_DIR" "$package_dir"
    
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
