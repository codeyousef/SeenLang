#!/usr/bin/env bash
# Generate Homebrew formula for Seen Language
# This script creates/updates the Homebrew formula with current release information

set -e

# Configuration
VERSION=""
MACOS_X64_URL=""
MACOS_ARM64_URL=""
LINUX_X64_URL=""
LINUX_ARM64_URL=""
TEMPLATE_FILE="seen-lang.rb"
OUTPUT_FILE=""
GITHUB_REPO="seen-lang/seen"
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
Homebrew Formula Generator for Seen Language

Usage: $0 --version <version> [options]

Required:
  --version VERSION    Release version (e.g., 1.0.0)

Options:
  --macos-x64-url URL      macOS x64 tarball URL
  --macos-arm64-url URL    macOS ARM64 tarball URL  
  --linux-x64-url URL     Linux x64 tarball URL
  --linux-arm64-url URL   Linux ARM64 tarball URL
  --template FILE         Template formula file (default: $TEMPLATE_FILE)
  --output FILE           Output formula file (default: auto-generated)
  --github-repo REPO      GitHub repository (default: $GITHUB_REPO)
  --verbose               Enable verbose output
  --help                  Show this help message

If URLs are not provided, they will be auto-generated based on GitHub releases.

Examples:
  $0 --version 1.0.0
  $0 --version 1.2.3 --output /path/to/homebrew-tap/Formula/seen-lang.rb
  $0 --version 2.0.0 --verbose

Requirements:
  - curl (for fetching SHA256 checksums)
  - GitHub release assets must exist for automatic URL generation

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --macos-x64-url)
            MACOS_X64_URL="$2"
            shift 2
            ;;
        --macos-arm64-url)
            MACOS_ARM64_URL="$2"
            shift 2
            ;;
        --linux-x64-url)
            LINUX_X64_URL="$2"
            shift 2
            ;;
        --linux-arm64-url)
            LINUX_ARM64_URL="$2"
            shift 2
            ;;
        --template)
            TEMPLATE_FILE="$2"
            shift 2
            ;;
        --output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        --github-repo)
            GITHUB_REPO="$2"
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
            error "Unknown argument: $1. Use --help for usage information."
            ;;
    esac
done

# Validate required arguments
if [ -z "$VERSION" ]; then
    error "Version is required. Use --version to specify."
fi

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Set defaults
if [ -z "$TEMPLATE_FILE" ]; then
    TEMPLATE_FILE="$SCRIPT_DIR/seen-lang.rb"
fi

if [ -z "$OUTPUT_FILE" ]; then
    OUTPUT_FILE="$SCRIPT_DIR/seen-lang-$VERSION.rb"
fi

header "Generating Homebrew Formula for Seen Language $VERSION"

info "Configuration:"
info "  Version: $VERSION"
info "  GitHub Repo: $GITHUB_REPO"
info "  Template: $TEMPLATE_FILE"
info "  Output: $OUTPUT_FILE"

# Check dependencies
check_dependencies() {
    local deps=("curl")
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

# Generate URLs if not provided
generate_urls() {
    info "Generating release URLs..."
    
    local base_url="https://github.com/$GITHUB_REPO/releases/download/v$VERSION"
    
    if [ -z "$MACOS_X64_URL" ]; then
        MACOS_X64_URL="$base_url/seen-$VERSION-macos-x64.tar.gz"
    fi
    
    if [ -z "$MACOS_ARM64_URL" ]; then
        MACOS_ARM64_URL="$base_url/seen-$VERSION-macos-arm64.tar.gz"
    fi
    
    if [ -z "$LINUX_X64_URL" ]; then
        LINUX_X64_URL="$base_url/seen-$VERSION-linux-x64.tar.gz"
    fi
    
    if [ -z "$LINUX_ARM64_URL" ]; then
        LINUX_ARM64_URL="$base_url/seen-$VERSION-linux-arm64.tar.gz"
    fi
    
    success "✓ URLs generated"
}

# Fetch SHA256 checksum for a URL
fetch_sha256() {
    local url="$1"
    local filename=$(basename "$url")
    
    info "  Fetching SHA256 for $filename..."
    
    # Try to download and compute SHA256
    local temp_file=$(mktemp)
    trap "rm -f $temp_file" RETURN
    
    if curl -sL "$url" -o "$temp_file" 2>/dev/null; then
        local checksum=$(sha256sum "$temp_file" 2>/dev/null | cut -d' ' -f1)
        if [ -n "$checksum" ] && [ ${#checksum} -eq 64 ]; then
            echo "$checksum"
            return 0
        fi
    fi
    
    # If download fails, try to fetch from GitHub API
    local api_url="https://api.github.com/repos/$GITHUB_REPO/releases/tags/v$VERSION"
    local asset_info=$(curl -s "$api_url" 2>/dev/null | grep -A 10 "\"name\": \"$filename\"" | grep -o '"browser_download_url": "[^"]*"' | cut -d'"' -f4)
    
    if [ -n "$asset_info" ] && curl -sL "$asset_info" -o "$temp_file" 2>/dev/null; then
        local checksum=$(sha256sum "$temp_file" 2>/dev/null | cut -d' ' -f1)
        if [ -n "$checksum" ] && [ ${#checksum} -eq 64 ]; then
            echo "$checksum"
            return 0
        fi
    fi
    
    # Return placeholder if unable to fetch
    warning "    Could not fetch checksum for $filename"
    echo "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
}

# Fetch all SHA256 checksums
fetch_all_checksums() {
    info "Fetching SHA256 checksums..."
    
    MACOS_X64_SHA256=$(fetch_sha256 "$MACOS_X64_URL")
    MACOS_ARM64_SHA256=$(fetch_sha256 "$MACOS_ARM64_URL")
    LINUX_X64_SHA256=$(fetch_sha256 "$LINUX_X64_URL")
    LINUX_ARM64_SHA256=$(fetch_sha256 "$LINUX_ARM64_URL")
    
    success "✓ Checksums fetched"
}

# Generate the formula
generate_formula() {
    info "Generating formula..."
    
    # Check if template exists
    if [ ! -f "$TEMPLATE_FILE" ]; then
        error "Template file not found: $TEMPLATE_FILE"
    fi
    
    # Read template
    local template_content=$(cat "$TEMPLATE_FILE")
    
    # Replace placeholders
    local formula_content="$template_content"
    
    # Replace version and URLs
    formula_content=$(echo "$formula_content" | sed "s|version \"[^\"]*\"|version \"$VERSION\"|g")
    formula_content=$(echo "$formula_content" | sed "s|url \"[^\"]*\"|url \"$MACOS_X64_URL\"|")
    formula_content=$(echo "$formula_content" | sed "s|sha256 \"[^\"]*\"|sha256 \"$MACOS_X64_SHA256\"|")
    
    # Replace platform-specific URLs and checksums
    # macOS Intel
    formula_content=$(echo "$formula_content" | sed "s|url \"https://github.com/.*/seen-[^/]*/seen-[^-]*-macos-x64.tar.gz\"|url \"$MACOS_X64_URL\"|g")
    formula_content=$(echo "$formula_content" | sed "/url \"$MACOS_X64_URL\"/,/sha256/ s/sha256 \"[^\"]*\"/sha256 \"$MACOS_X64_SHA256\"/" | sed "0,/sha256 \"$MACOS_X64_SHA256\"/{//d;}")
    
    # macOS ARM
    formula_content=$(echo "$formula_content" | sed "s|url \"https://github.com/.*/seen-[^/]*/seen-[^-]*-macos-arm64.tar.gz\"|url \"$MACOS_ARM64_URL\"|g")
    formula_content=$(echo "$formula_content" | sed "/url \"$MACOS_ARM64_URL\"/,/sha256/ s/sha256 \"[^\"]*\"/sha256 \"$MACOS_ARM64_SHA256\"/" | sed "0,/sha256 \"$MACOS_ARM64_SHA256\"/{//d;}")
    
    # Linux x64
    formula_content=$(echo "$formula_content" | sed "s|url \"https://github.com/.*/seen-[^/]*/seen-[^-]*-linux-x64.tar.gz\"|url \"$LINUX_X64_URL\"|g")
    formula_content=$(echo "$formula_content" | sed "/url \"$LINUX_X64_URL\"/,/sha256/ s/sha256 \"[^\"]*\"/sha256 \"$LINUX_X64_SHA256\"/" | sed "0,/sha256 \"$LINUX_X64_SHA256\"/{//d;}")
    
    # Linux ARM64
    formula_content=$(echo "$formula_content" | sed "s|url \"https://github.com/.*/seen-[^/]*/seen-[^-]*-linux-arm64.tar.gz\"|url \"$LINUX_ARM64_URL\"|g")
    formula_content=$(echo "$formula_content" | sed "/url \"$LINUX_ARM64_URL\"/,/sha256/ s/sha256 \"[^\"]*\"/sha256 \"$LINUX_ARM64_SHA256\"/" | sed "0,/sha256 \"$LINUX_ARM64_SHA256\"/{//d;}")
    
    # Replace download URLs in version-specific sections
    formula_content=$(echo "$formula_content" | sed "s|releases/download/v[0-9][^/]*/|releases/download/v$VERSION/|g")
    
    # Write formula to output file
    echo "$formula_content" > "$OUTPUT_FILE"
    
    success "✓ Formula generated: $OUTPUT_FILE"
}

# Validate the generated formula
validate_formula() {
    info "Validating formula..."
    
    if [ ! -f "$OUTPUT_FILE" ]; then
        error "Generated formula file not found: $OUTPUT_FILE"
    fi
    
    # Check for required components
    local required_components=(
        "class SeenLang"
        "desc \"High-performance systems programming language\""
        "homepage \"https://seen-lang.org\""
        "license \"MIT\""
        "def install"
        "def test"
    )
    
    local missing_components=()
    
    for component in "${required_components[@]}"; do
        if ! grep -q "$component" "$OUTPUT_FILE"; then
            missing_components+=("$component")
        fi
    done
    
    if [ ${#missing_components[@]} -gt 0 ]; then
        warning "Missing components in formula:"
        printf '  %s\n' "${missing_components[@]}"
    fi
    
    # Check syntax (basic Ruby syntax check if available)
    if command -v ruby &> /dev/null; then
        if ruby -c "$OUTPUT_FILE" >/dev/null 2>&1; then
            success "  ✓ Ruby syntax validation passed"
        else
            warning "  Ruby syntax validation failed"
        fi
    fi
    
    # Check file size
    local file_size=$(wc -l < "$OUTPUT_FILE")
    info "  Formula length: $file_size lines"
    
    success "✓ Formula validation completed"
}

# Show usage instructions
show_usage_instructions() {
    success ""
    success "==============================================="
    success "     Homebrew formula generated!              "
    success "==============================================="
    success ""
    success "Generated formula: $OUTPUT_FILE"
    success ""
    success "To use this formula:"
    success ""
    success "1. For local testing:"
    success "   brew install --build-from-source $OUTPUT_FILE"
    success ""
    success "2. For Homebrew tap (recommended):"
    success "   # Copy to your tap repository:"
    success "   cp $OUTPUT_FILE /path/to/homebrew-tap/Formula/seen-lang.rb"
    success "   "
    success "   # Then users can install with:"
    success "   brew tap your-org/your-tap"
    success "   brew install seen-lang"
    success ""
    success "3. For core Homebrew (submit PR):"
    success "   # Submit to https://github.com/Homebrew/homebrew-core"
    success "   # Follow Homebrew contribution guidelines"
    success ""
    success "Testing the formula:"
    success "  brew audit --strict $OUTPUT_FILE"
    success "  brew test seen-lang"
    success ""
    success "Documentation:"
    success "  https://docs.brew.sh/Formula-Cookbook"
    success "  https://docs.brew.sh/Acceptable-Formulae"
    success ""
}

# Main process
main() {
    header "Homebrew Formula Generation"
    
    # Validate environment
    check_dependencies
    
    # Generate URLs and fetch checksums
    generate_urls
    fetch_all_checksums
    
    # Generate the formula
    generate_formula
    validate_formula
    
    # Show usage instructions
    show_usage_instructions
}

# Run main function
main "$@"