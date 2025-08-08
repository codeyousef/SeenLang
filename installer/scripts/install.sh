#!/usr/bin/env bash
# Universal Seen Language Installer for Linux/macOS
# Usage: curl -sSL https://install.seen-lang.org | bash
#    or: bash install.sh

set -e

# Configuration
VERSION="${VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local}"
ARCH="${ARCH:-$(uname -m)}"
ADD_TO_PATH="${ADD_TO_PATH:-true}"
INSTALL_STDLIB="${INSTALL_STDLIB:-true}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Logging functions
print_header() {
    echo ""
    echo -e "${CYAN}===============================================${NC}"
    echo -e "${CYAN}     Seen Language Installer for Unix        ${NC}"
    echo -e "${CYAN}===============================================${NC}"
    echo ""
}

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

# Platform detection
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    elif [[ "$OSTYPE" == "cygwin" || "$OSTYPE" == "msys" ]]; then
        error "This script is for Unix systems. Use install.ps1 for Windows."
    else
        error "Unsupported OS: $OSTYPE"
    fi
}

detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        echo "$ID"
    elif type lsb_release >/dev/null 2>&1; then
        lsb_release -si | tr '[:upper:]' '[:lower:]'
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    else
        echo "unknown"
    fi
}

detect_architecture() {
    case "$ARCH" in
        x86_64|amd64)
            echo "x64"
            ;;
        aarch64|arm64)
            echo "arm64"
            ;;
        riscv64)
            echo "riscv64"
            ;;
        *)
            error "Unsupported architecture: $ARCH. Supported: x64, arm64, riscv64"
            ;;
    esac
}

check_dependencies() {
    local deps=("curl" "tar")
    local missing=()
    
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            missing+=("$dep")
        fi
    done
    
    if [ ${#missing[@]} -gt 0 ]; then
        error "Missing dependencies: ${missing[*]}. Please install them first."
    fi
    
    success "All dependencies found"
}

check_permissions() {
    # Check if we need sudo for the target directory
    if [ "$INSTALL_DIR" = "/usr/local" ] || [ "$INSTALL_DIR" = "/usr" ]; then
        if [ "$EUID" -ne 0 ] && ! sudo -n true 2>/dev/null; then
            error "This script requires sudo privileges for system-wide installation to $INSTALL_DIR"
        fi
    fi
}

download_release() {
    local version="$1"
    local os="$2"
    local arch="$3"
    local temp_dir="$4"
    
    local base_url="https://github.com/seen-lang/seen/releases"
    local filename="seen-${version}-${os}-${arch}.tar.gz"
    
    if [ "$version" = "latest" ]; then
        local url="$base_url/latest/download/$filename"
    else
        local url="$base_url/download/v${version}/$filename"
    fi
    
    info "Downloading Seen $version for $os ($arch)..."
    info "URL: $url"
    
    if ! curl -fL -o "$temp_dir/seen.tar.gz" "$url"; then
        error "Failed to download Seen release. Check your internet connection and try again."
    fi
    
    success "Download completed"
}

verify_download() {
    local temp_dir="$1"
    
    info "Verifying download..."
    
    # Check if the downloaded file is a valid tar.gz
    if ! tar -tzf "$temp_dir/seen.tar.gz" >/dev/null 2>&1; then
        error "Downloaded file is not a valid tar.gz archive"
    fi
    
    success "Download verification passed"
}

install_seen() {
    local temp_dir="$1"
    local install_dir="$2"
    
    info "Extracting files..."
    tar -xzf "$temp_dir/seen.tar.gz" -C "$temp_dir"
    
    info "Installing Seen to $install_dir..."
    
    # Create directories with sudo if needed
    if [[ "$install_dir" == "/usr/local" || "$install_dir" == "/usr" ]]; then
        sudo mkdir -p "$install_dir/bin"
        sudo mkdir -p "$install_dir/lib/seen"
        sudo mkdir -p "$install_dir/share/seen"
        
        # Install binaries
        sudo cp "$temp_dir/seen" "$install_dir/bin/"
        sudo chmod +x "$install_dir/bin/seen"
        
        # Install LSP server if present
        if [ -f "$temp_dir/seen-lsp" ]; then
            sudo cp "$temp_dir/seen-lsp" "$install_dir/bin/"
            sudo chmod +x "$install_dir/bin/seen-lsp"
        fi
        
        # Install RISC-V cross-compilation tools if present
        if [ -f "$temp_dir/seen-riscv" ]; then
            sudo cp "$temp_dir/seen-riscv" "$install_dir/bin/"
            sudo chmod +x "$install_dir/bin/seen-riscv"
        fi
        
        # Install standard library
        if [ "$INSTALL_STDLIB" = "true" ] && [ -d "$temp_dir/stdlib" ]; then
            sudo cp -r "$temp_dir/stdlib" "$install_dir/lib/seen/"
        fi
        
        # Install language configurations
        if [ -d "$temp_dir/languages" ]; then
            sudo cp -r "$temp_dir/languages" "$install_dir/share/seen/"
        fi
        
        # Install documentation
        if [ -d "$temp_dir/docs" ]; then
            sudo cp -r "$temp_dir/docs" "$install_dir/share/seen/"
        fi
        
        # Install man pages
        if [ -f "$temp_dir/seen.1" ]; then
            sudo mkdir -p "$install_dir/share/man/man1"
            sudo cp "$temp_dir/seen.1" "$install_dir/share/man/man1/"
            sudo gzip -f "$install_dir/share/man/man1/seen.1"
        fi
    else
        # User-space installation
        mkdir -p "$install_dir/bin"
        mkdir -p "$install_dir/lib/seen"
        mkdir -p "$install_dir/share/seen"
        
        cp "$temp_dir/seen" "$install_dir/bin/"
        chmod +x "$install_dir/bin/seen"
        
        if [ -f "$temp_dir/seen-lsp" ]; then
            cp "$temp_dir/seen-lsp" "$install_dir/bin/"
            chmod +x "$install_dir/bin/seen-lsp"
        fi
        
        if [ -f "$temp_dir/seen-riscv" ]; then
            cp "$temp_dir/seen-riscv" "$install_dir/bin/"
            chmod +x "$install_dir/bin/seen-riscv"
        fi
        
        if [ "$INSTALL_STDLIB" = "true" ] && [ -d "$temp_dir/stdlib" ]; then
            cp -r "$temp_dir/stdlib" "$install_dir/lib/seen/"
        fi
        
        if [ -d "$temp_dir/languages" ]; then
            cp -r "$temp_dir/languages" "$install_dir/share/seen/"
        fi
        
        if [ -d "$temp_dir/docs" ]; then
            cp -r "$temp_dir/docs" "$install_dir/share/seen/"
        fi
    fi
    
    success "Seen installed successfully"
}

setup_path() {
    local install_dir="$1"
    
    if [ "$ADD_TO_PATH" != "true" ]; then
        return
    fi
    
    info "Setting up PATH..."
    
    local shell_rc=""
    local bin_path="$install_dir/bin"
    
    # Detect shell configuration file
    if [ -n "$BASH_VERSION" ]; then
        if [ -f "$HOME/.bashrc" ]; then
            shell_rc="$HOME/.bashrc"
        elif [ -f "$HOME/.bash_profile" ]; then
            shell_rc="$HOME/.bash_profile"
        fi
    elif [ -n "$ZSH_VERSION" ]; then
        shell_rc="$HOME/.zshrc"
    elif [ -f "$HOME/.profile" ]; then
        shell_rc="$HOME/.profile"
    fi
    
    if [ -n "$shell_rc" ]; then
        # Check if PATH is already set
        if ! grep -q "$bin_path" "$shell_rc" 2>/dev/null; then
            echo "" >> "$shell_rc"
            echo "# Added by Seen installer" >> "$shell_rc"
            echo "export PATH=\"$bin_path:\$PATH\"" >> "$shell_rc"
            info "Added $bin_path to PATH in $shell_rc"
        else
            info "PATH already configured in $shell_rc"
        fi
    else
        warning "Could not detect shell configuration file. Please add $bin_path to your PATH manually."
    fi
    
    # Update current session PATH
    export PATH="$bin_path:$PATH"
    
    success "PATH configuration completed"
}

create_symlinks() {
    local install_dir="$1"
    
    # Create symlinks in /usr/local/bin if installing elsewhere
    if [ "$install_dir" != "/usr/local" ] && [ -d "/usr/local/bin" ]; then
        info "Creating symlinks in /usr/local/bin..."
        
        if command -v sudo >/dev/null && [ -w /usr/local/bin ] || sudo -n true 2>/dev/null; then
            sudo ln -sf "$install_dir/bin/seen" "/usr/local/bin/seen" 2>/dev/null || true
            sudo ln -sf "$install_dir/bin/seen-lsp" "/usr/local/bin/seen-lsp" 2>/dev/null || true
        fi
    fi
}

verify_installation() {
    info "Verifying installation..."
    
    if command -v seen &> /dev/null; then
        local version_output=$(seen --version 2>&1)
        success "✓ Seen installed: $version_output"
        return 0
    else
        error "Installation verification failed. 'seen' command not found in PATH."
    fi
}

cleanup() {
    if [ -n "$1" ] && [ -d "$1" ]; then
        rm -rf "$1"
    fi
}

show_getting_started() {
    echo ""
    success "==============================================="
    success "     Installation completed successfully!      "
    success "==============================================="
    echo ""
    echo "To get started with Seen:"
    echo ""
    echo -e "${BLUE}  # Create a new project${NC}"
    echo -e "${BLUE}  seen init my-project${NC}"
    echo -e "${BLUE}  cd my-project${NC}"
    echo ""
    echo -e "${BLUE}  # Build your project${NC}"
    echo -e "${BLUE}  seen build${NC}"
    echo ""
    echo -e "${BLUE}  # Run your project${NC}"
    echo -e "${BLUE}  seen run${NC}"
    echo ""
    echo "For VS Code support, install the extension:"
    echo -e "${BLUE}  code --install-extension seen-lang.seen-vscode${NC}"
    echo ""
    echo "Documentation: https://docs.seen-lang.org"
    echo "Community: https://discord.gg/seen-lang"
    echo ""
    echo "If you're using a new shell session, you may need to:"
    echo -e "${YELLOW}  source ~/.bashrc${NC}  # or your shell's config file"
    echo ""
}

main() {
    print_header
    
    # Detect system information
    local os=$(detect_os)
    local distro=$(detect_distro)
    local arch=$(detect_architecture)
    
    info "Detected: $distro on $os ($arch)"
    info "Installing Seen $VERSION to $INSTALL_DIR"
    
    # Pre-flight checks
    check_dependencies
    check_permissions
    
    # Create temporary directory
    local temp_dir=$(mktemp -d)
    trap "cleanup $temp_dir" EXIT
    
    # Download and install
    download_release "$VERSION" "$os" "$arch" "$temp_dir"
    verify_download "$temp_dir"
    install_seen "$temp_dir" "$INSTALL_DIR"
    setup_path "$INSTALL_DIR"
    create_symlinks "$INSTALL_DIR"
    
    # Verify and complete
    verify_installation
    show_getting_started
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --install-dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --arch)
            ARCH="$2"
            shift 2
            ;;
        --no-path)
            ADD_TO_PATH="false"
            shift
            ;;
        --no-stdlib)
            INSTALL_STDLIB="false"
            shift
            ;;
        --help)
            echo "Seen Language Installer"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --version VERSION     Install specific version (default: latest)"
            echo "  --install-dir DIR     Installation directory (default: /usr/local)"
            echo "  --arch ARCH           Target architecture (default: auto-detect)"
            echo "  --no-path             Don't modify PATH"
            echo "  --no-stdlib           Don't install standard library"
            echo "  --help                Show this help message"
            echo ""
            echo "Environment variables:"
            echo "  VERSION              Version to install (default: latest)"
            echo "  INSTALL_DIR          Installation directory"
            echo "  ARCH                 Target architecture"
            echo "  ADD_TO_PATH          Add to PATH (true/false)"
            echo "  INSTALL_STDLIB       Install stdlib (true/false)"
            echo ""
            exit 0
            ;;
        *)
            error "Unknown option: $1. Use --help for usage information."
            ;;
    esac
done

# Run main installation
main "$@"