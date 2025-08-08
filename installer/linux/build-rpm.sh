#!/usr/bin/env bash
# Build script for Seen Language RPM package
# Creates .rpm packages for RHEL/CentOS/Fedora/SUSE systems

set -e

# Configuration
VERSION=""
ARCH=""
SOURCE_DIR="../../target-wsl/release"
OUTPUT_DIR="output"
VERBOSE=false
RELEASE="1"

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
Seen Language RPM Package Builder

Usage: $0 <version> <architecture> [options]

Arguments:
  version              Version number (e.g., 1.0.0)
  architecture         Target architecture (x86_64, aarch64, riscv64)

Options:
  --source-dir DIR     Source directory with binaries (default: $SOURCE_DIR)
  --output-dir DIR     Output directory (default: $OUTPUT_DIR)
  --release NUM        Release number (default: $RELEASE)
  --verbose            Enable verbose output
  --help               Show this help message

Examples:
  $0 1.0.0 x86_64
  $0 1.2.3 aarch64 --verbose
  $0 2.0.0 x86_64 --release 2

Requirements:
  - rpmbuild (rpm-build package)
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
        --release)
            RELEASE="$2"
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

header "Building Seen Language $VERSION RPM for $ARCH"

info "Configuration:"
info "  Version: $VERSION"
info "  Release: $RELEASE"
info "  Architecture: $ARCH"
info "  Source: $SOURCE_DIR"
info "  Output: $OUTPUT_DIR"
info "  Project Root: $PROJECT_ROOT"

# Check dependencies
check_dependencies() {
    local deps=("rpmbuild" "tar" "gzip" "find")
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

# Create RPM build environment
setup_rpm_build_env() {
    local temp_dir="$1"
    local rpmbuild_dir="$temp_dir/rpmbuild"
    
    info "Setting up RPM build environment..."
    
    # Create rpmbuild directory structure
    mkdir -p "$rpmbuild_dir"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
    
    echo "$rpmbuild_dir"
}

# Create spec file
create_spec_file() {
    local rpmbuild_dir="$1"
    local spec_file="$rpmbuild_dir/SPECS/seen-lang.spec"
    
    info "Creating RPM spec file..."
    
    cat > "$spec_file" << EOF
%global debug_package %{nil}

Name:           seen-lang
Version:        $VERSION
Release:        $RELEASE%{?dist}
Summary:        High-performance systems programming language

License:        MIT
URL:            https://seen-lang.org
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  gcc
BuildRequires:  make

Requires:       glibc >= 2.17
Requires:       libgcc
Requires:       libstdc++

%description
Seen is a revolutionary systems programming language designed to be the
world's most performant language while providing intuitive developer
experience. Key features include:

* Dual execution: JIT (<50ms) + AOT (beats C/Rust)
* Vale-style memory model: Zero overhead safety without borrow checker
* Universal deployment: Same codebase for backend, web, mobile, desktop
* Zig-style C interop: Import C headers directly, no bindings needed
* Multi-target: Native, WASM, mobile from single source

This package includes the Seen compiler, standard library, language server,
and documentation.

%package devel
Summary:        Development files for Seen Language
Requires:       %{name} = %{version}-%{release}

%description devel
Development files and headers for the Seen programming language.
This package is required for developing applications with Seen.

%package docs
Summary:        Documentation for Seen Language  
Requires:       %{name} = %{version}-%{release}
BuildArch:      noarch

%description docs
Complete documentation for the Seen programming language, including
API reference, tutorials, and language specification.

%prep
%setup -q

%build
# No build needed - using pre-built binaries

%install
rm -rf %{buildroot}

# Create directory structure
install -d %{buildroot}%{_bindir}
install -d %{buildroot}%{_libdir}/seen
install -d %{buildroot}%{_datadir}/seen
install -d %{buildroot}%{_docdir}/%{name}
install -d %{buildroot}%{_mandir}/man1
install -d %{buildroot}%{_datadir}/applications
install -d %{buildroot}%{_datadir}/pixmaps

# Install binaries
install -m 755 seen %{buildroot}%{_bindir}/seen
%if 0%{?with_lsp:1}
install -m 755 seen-lsp %{buildroot}%{_bindir}/seen-lsp
%endif
%if 0%{?with_riscv:1}
install -m 755 seen-riscv %{buildroot}%{_bindir}/seen-riscv
%endif

# Install standard library
cp -r stdlib/* %{buildroot}%{_libdir}/seen/

# Install language configurations  
cp -r languages %{buildroot}%{_datadir}/seen/

# Install documentation
%if 0%{?with_docs:1}
cp -r docs/* %{buildroot}%{_docdir}/%{name}/
%endif

# Install man page
cat > %{buildroot}%{_mandir}/man1/seen.1 << 'MANEOF'
.TH SEEN 1 "$(date +'%B %Y')" "seen $VERSION" "User Commands"
.SH NAME
seen \- Seen programming language compiler and toolchain
.SH SYNOPSIS
.B seen
[\fIOPTION\fR]... [\fICOMMAND\fR] [\fIARGS\fR]...
.SH DESCRIPTION
Seen is a high-performance systems programming language compiler and toolchain.
.SH OPTIONS
.TP
\fB\-\-version\fR
Show version information
.TP
\fB\-h\fR, \fB\-\-help\fR
Show help message
.SH COMMANDS
.TP
\fBbuild\fR
Build the current project
.TP
\fBrun\fR
Run the current project
.TP
\fBtest\fR
Run project tests
.SH EXAMPLES
.TP
\fBseen init hello\fR
Create a new project
.TP
\fBseen build\fR
Build the current project
.SH SEE ALSO
Documentation: https://docs.seen-lang.org
MANEOF

# Create desktop entry
cat > %{buildroot}%{_datadir}/applications/seen.desktop << 'DESKTOPEOF'
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
DESKTOPEOF

%files
%license LICENSE
%doc README.md
%{_bindir}/seen
%{_libdir}/seen/
%{_datadir}/seen/
%{_mandir}/man1/seen.1*
%{_datadir}/applications/seen.desktop

%files devel
%{_libdir}/seen/

%files docs
%{_docdir}/%{name}/

%post
# Update alternatives
update-alternatives --install %{_bindir}/seen seen %{_bindir}/seen 100 || :

# Update desktop database
update-desktop-database &> /dev/null || :

# Update man database
mandb -q &> /dev/null || :

echo "Seen Language installed successfully!"
echo "Run 'seen --version' to verify installation."

%preun
# Remove alternatives
if [ "\$1" = 0 ]; then
    update-alternatives --remove seen %{_bindir}/seen || :
fi

%postun
# Clean up
if [ "\$1" = 0 ]; then
    # Update desktop database
    update-desktop-database &> /dev/null || :
    
    # Update man database
    mandb -q &> /dev/null || :
    
    echo "Seen Language removed."
fi

%changelog
* $(date +'%a %b %d %Y') Seen Language Team <team@seen-lang.org> - $VERSION-$RELEASE
- Initial RPM release of Seen Language $VERSION
- High-performance systems programming language
- JIT and AOT compilation support
- Multi-platform targeting
- Comprehensive standard library
- Language server integration
EOF

    echo "$spec_file"
}

# Create source tarball
create_source_tarball() {
    local rpmbuild_dir="$1" 
    local temp_dir="$2"
    
    info "Creating source tarball..."
    
    local source_name="seen-lang-$VERSION"
    local source_dir="$temp_dir/$source_name"
    local tarball="$rpmbuild_dir/SOURCES/$source_name.tar.gz"
    
    # Create source directory structure
    mkdir -p "$source_dir"
    
    # Copy binaries
    cp "$SOURCE_DIR/seen" "$source_dir/"
    
    if [ -f "$SOURCE_DIR/seen-lsp" ]; then
        cp "$SOURCE_DIR/seen-lsp" "$source_dir/"
    fi
    
    if [ -f "$SOURCE_DIR/seen-riscv" ]; then
        cp "$SOURCE_DIR/seen-riscv" "$source_dir/"
    fi
    
    # Copy standard library
    if [ -d "$PROJECT_ROOT/seen_std" ]; then
        mkdir -p "$source_dir/stdlib"
        cp -r "$PROJECT_ROOT/seen_std"/* "$source_dir/stdlib/"
    fi
    
    # Copy language configurations
    if [ -d "$PROJECT_ROOT/languages" ]; then
        cp -r "$PROJECT_ROOT/languages" "$source_dir/"
    fi
    
    # Copy documentation
    if [ -d "$PROJECT_ROOT/docs" ]; then
        cp -r "$PROJECT_ROOT/docs" "$source_dir/"
    fi
    
    # Create basic files
    echo "MIT" > "$source_dir/LICENSE"
    cat > "$source_dir/README.md" << 'EOF'
# Seen Language

High-performance systems programming language.

## Installation

This RPM package provides the complete Seen toolchain.

## Usage

```bash
seen init my-project
cd my-project
seen build
seen run
```

## Documentation

Visit https://docs.seen-lang.org for complete documentation.
EOF
    
    # Create tarball
    (cd "$temp_dir" && tar -czf "$tarball" "$source_name")
    
    if [ -f "$tarball" ]; then
        success "✓ Source tarball created: $(basename "$tarball")"
    else
        error "Failed to create source tarball"
    fi
}

# Build RPM package
build_rpm() {
    local rpmbuild_dir="$1"
    local spec_file="$2"
    
    info "Building RPM package..."
    
    # Set rpmbuild options
    local rpmbuild_opts=(
        "--define" "_topdir $rpmbuild_dir"
        "--define" "_target_cpu $ARCH"
    )
    
    # Add conditional builds based on available files
    if [ -f "$SOURCE_DIR/seen-lsp" ]; then
        rpmbuild_opts+=("--define" "with_lsp 1")
    fi
    
    if [ -f "$SOURCE_DIR/seen-riscv" ]; then
        rpmbuild_opts+=("--define" "with_riscv 1") 
    fi
    
    if [ -d "$PROJECT_ROOT/docs" ]; then
        rpmbuild_opts+=("--define" "with_docs 1")
    fi
    
    if $VERBOSE; then
        rpmbuild_opts+=("-v")
    fi
    
    # Build RPM
    rpmbuild "${rpmbuild_opts[@]}" -bb "$spec_file"
    
    if [ $? -eq 0 ]; then
        success "✓ RPM build completed"
    else
        error "RPM build failed"
    fi
}

# Copy and validate RPM files
finalize_packages() {
    local rpmbuild_dir="$1"
    
    info "Finalizing packages..."
    
    # Create output directory
    mkdir -p "$OUTPUT_DIR"
    
    # Find and copy RPM files
    local rpm_files=($(find "$rpmbuild_dir/RPMS" -name "*.rpm" 2>/dev/null))
    
    if [ ${#rpm_files[@]} -eq 0 ]; then
        error "No RPM files found"
    fi
    
    for rpm_file in "${rpm_files[@]}"; do
        local basename_rpm=$(basename "$rpm_file")
        local output_rpm="$OUTPUT_DIR/$basename_rpm"
        
        cp "$rpm_file" "$output_rpm"
        
        if [ -f "$output_rpm" ]; then
            success "✓ RPM package: $output_rpm"
            
            # Show package info
            local size_mb=$(du -m "$output_rpm" | cut -f1)
            info "  Size: ${size_mb}MB"
            
            # Generate checksum
            local checksum=$(sha256sum "$output_rpm" | cut -d' ' -f1)
            echo "$checksum  $basename_rpm" > "$output_rpm.sha256"
            info "  SHA256: $output_rpm.sha256"
            
            # Validate RPM (if rpm command available)
            if command -v rpm &> /dev/null; then
                if rpm -qpl "$output_rpm" >/dev/null 2>&1; then
                    success "  ✓ RPM validation passed"
                else
                    warning "  RPM validation failed"
                fi
            fi
        fi
    done
}

# Main build process
main() {
    header "RPM Package Build"
    
    # Validate environment
    check_dependencies
    validate_sources
    
    # Create temporary directory
    local temp_dir=$(mktemp -d)
    trap "rm -rf $temp_dir" EXIT
    
    # Set up RPM build environment
    local rpmbuild_dir=$(setup_rpm_build_env "$temp_dir")
    
    # Create build files
    local spec_file=$(create_spec_file "$rpmbuild_dir")
    create_source_tarball "$rpmbuild_dir" "$temp_dir"
    
    # Build RPM
    build_rpm "$rpmbuild_dir" "$spec_file"
    
    # Copy results
    finalize_packages "$rpmbuild_dir"
    
    success ""
    success "==============================================="
    success "     RPM package build completed!             "
    success "==============================================="
    success ""
    success "Packages in: $OUTPUT_DIR"
    success ""
    success "To install:"
    success "  sudo rpm -ivh $OUTPUT_DIR/seen-lang-${VERSION}-${RELEASE}.*.rpm"
    success "  # OR"
    success "  sudo dnf install $OUTPUT_DIR/seen-lang-${VERSION}-${RELEASE}.*.rpm"
    success "  sudo yum install $OUTPUT_DIR/seen-lang-${VERSION}-${RELEASE}.*.rpm"
    success ""
}

# Run main function
main "$@"