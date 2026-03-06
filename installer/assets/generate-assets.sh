#!/usr/bin/env bash
# Generate placeholder branding assets for Seen Language installer
# Creates icons, banners, and other visual assets programmatically

set -e

# Configuration
SEEN_BLUE="#2E86C1"
SEEN_ORANGE="#F39C12"
DARK_GRAY="#2C3E50"
WHITE="#FFFFFF"

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

# Check dependencies
check_dependencies() {
    local deps=("convert" "rsvg-convert")
    local missing=()
    local optional=()
    
    # Check ImageMagick
    if ! command -v convert &> /dev/null; then
        missing+=("ImageMagick (convert)")
    fi
    
    # Check librsvg (optional)
    if ! command -v rsvg-convert &> /dev/null; then
        optional+=("librsvg-bin (rsvg-convert)")
    fi
    
    if [ ${#missing[@]} -gt 0 ]; then
        error "Missing required dependencies: ${missing[*]}"
    fi
    
    if [ ${#optional[@]} -gt 0 ]; then
        info "Optional dependencies missing: ${optional[*]}"
        info "SVG conversion will be limited"
    fi
    
    success "✓ Dependencies checked"
}

# Create directories
setup_directories() {
    info "Creating asset directories..."
    
    local dirs=("icons" "banners" "screenshots" "logos")
    
    for dir in "${dirs[@]}"; do
        mkdir -p "$dir"
    done
    
    success "✓ Directories created"
}

# Generate SVG icon
generate_svg_icon() {
    info "Generating SVG icon..."
    
    cat > icons/seen-icon.svg << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<svg width="256" height="256" viewBox="0 0 256 256" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="gradient" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#2E86C1;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#1B4F72;stop-opacity:1" />
    </linearGradient>
  </defs>
  
  <!-- Background circle -->
  <circle cx="128" cy="128" r="120" fill="url(#gradient)" stroke="#1B4F72" stroke-width="4"/>
  
  <!-- Letter S -->
  <path d="M 80 90 
           Q 80 70, 100 70
           L 156 70
           Q 176 70, 176 90
           Q 176 110, 156 110
           L 120 110
           Q 100 110, 100 130
           Q 100 150, 120 150
           L 156 150
           Q 176 150, 176 170
           Q 176 190, 156 190
           L 100 190
           Q 80 190, 80 170"
        fill="none" 
        stroke="#FFFFFF" 
        stroke-width="12" 
        stroke-linecap="round"/>
  
  <!-- Accent dot -->
  <circle cx="190" cy="80" r="8" fill="#F39C12"/>
</svg>
EOF
    
    success "✓ SVG icon created"
}

# Generate PNG icons from SVG
generate_png_icons() {
    info "Generating PNG icons..."
    
    local sizes=(16 32 48 128 256)
    
    for size in "${sizes[@]}"; do
        if command -v rsvg-convert &> /dev/null; then
            # Use librsvg for better quality
            rsvg-convert -w $size -h $size icons/seen-icon.svg -o icons/seen-icon-${size}.png
        else
            # Fallback to ImageMagick
            convert -background none -size ${size}x${size} icons/seen-icon.svg icons/seen-icon-${size}.png
        fi
        info "  ✓ ${size}x${size} PNG created"
    done
    
    # Create main icon (256px)
    cp icons/seen-icon-256.png icons/seen-icon.png
    
    success "✓ PNG icons created"
}

# Generate Windows ICO file
generate_ico_icon() {
    info "Generating Windows ICO file..."
    
    # Combine multiple PNG sizes into single ICO
    convert icons/seen-icon-16.png icons/seen-icon-32.png icons/seen-icon-48.png icons/seen-icon-256.png icons/seen-icon.ico
    
    success "✓ ICO icon created"
}

# Generate installer banners
generate_banners() {
    info "Generating installer banners..."
    
    # WiX dialog banner (493x58)
    convert -size 493x58 xc:"$SEEN_BLUE" \
            -fill white -font DejaVu-Sans-Bold -pointsize 24 \
            -draw "text 20,35 'Seen Language'" \
            -fill "$SEEN_ORANGE" -font DejaVu-Sans -pointsize 14 \
            -draw "text 20,52 'High-performance systems programming'" \
            banners/banner.bmp
    
    # WiX dialog background (493x312) 
    convert -size 493x312 gradient:"$SEEN_BLUE"-"#AED6F1" \
            -fill white -font DejaVu-Sans-Bold -pointsize 32 \
            -draw "text 50,150 'Welcome to'" \
            -draw "text 50,190 'Seen Language'" \
            -fill "$DARK_GRAY" -font DejaVu-Sans -pointsize 16 \
            -draw "text 50,220 'The world''s most performant'" \
            -draw "text 50,240 'systems programming language'" \
            banners/dialog.bmp
    
    # Web header banner (1200x300)
    convert -size 1200x300 gradient:"$SEEN_BLUE"-"#1B4F72" \
            -fill white -font DejaVu-Sans-Bold -pointsize 48 \
            -draw "text 100,150 'Seen Language'" \
            -fill "$SEEN_ORANGE" -font DejaVu-Sans -pointsize 24 \
            -draw "text 100,200 'Revolutionary systems programming language'" \
            -draw "text 100,230 'JIT <50ms • AOT beats C/Rust • Universal deployment'" \
            banners/header.png
    
    success "✓ Installer banners created"
}

# Generate logos
generate_logos() {
    info "Generating logos..."
    
    # Main logo (SVG)
    cat > logos/seen-logo.svg << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<svg width="400" height="120" viewBox="0 0 400 120" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="logoGradient" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" style="stop-color:#2E86C1;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#1B4F72;stop-opacity:1" />
    </linearGradient>
  </defs>
  
  <!-- Icon -->
  <circle cx="60" cy="60" r="45" fill="url(#logoGradient)"/>
  <path d="M 35 40 
           Q 35 30, 45 30
           L 75 30
           Q 85 30, 85 40
           Q 85 50, 75 50
           L 55 50
           Q 45 50, 45 60
           Q 45 70, 55 70
           L 75 70
           Q 85 70, 85 80
           Q 85 90, 75 90
           L 45 90
           Q 35 90, 35 80"
        fill="none" 
        stroke="#FFFFFF" 
        stroke-width="6" 
        stroke-linecap="round"/>
  
  <!-- Text -->
  <text x="130" y="45" font-family="Arial, sans-serif" font-size="36" font-weight="bold" fill="#2C3E50">Seen</text>
  <text x="130" y="75" font-family="Arial, sans-serif" font-size="18" fill="#7F8C8D">Language</text>
  
  <!-- Tagline -->
  <text x="130" y="95" font-family="Arial, sans-serif" font-size="12" fill="#F39C12">Revolutionary systems programming</text>
</svg>
EOF
    
    # Convert to PNG
    if command -v rsvg-convert &> /dev/null; then
        rsvg-convert -w 400 -h 120 logos/seen-logo.svg -o logos/seen-logo.png
    else
        convert -background none logos/seen-logo.svg logos/seen-logo.png
    fi
    
    # Wordmark (text only)
    convert -size 300x80 xc:none \
            -fill "$SEEN_BLUE" -font DejaVu-Sans-Bold -pointsize 32 \
            -draw "text 10,40 'Seen Language'" \
            -fill "$SEEN_ORANGE" -font DejaVu-Sans -pointsize 14 \
            -draw "text 10,60 'Systems Programming'" \
            logos/seen-wordmark.png
    
    success "✓ Logos created"
}

# Generate sample screenshots
generate_screenshots() {
    info "Generating sample screenshots..."
    
    # IDE integration screenshot
    convert -size 800x600 xc:"#2C3E50" \
            -fill "#ECF0F1" -font DejaVu-Sans-Mono -pointsize 12 \
            -draw "text 20,30 'fn main() {'" \
            -draw "text 40,50 'println!(\"Hello, Seen!\");'" \
            -draw "text 20,70 '}'" \
            -fill "$SEEN_ORANGE" -font DejaVu-Sans -pointsize 10 \
            -draw "text 20,600 'Seen Language - IDE Integration'" \
            screenshots/ide-integration.png
    
    # Terminal usage screenshot
    convert -size 800x400 xc:"#1C1C1C" \
            -fill "#00FF00" -font DejaVu-Sans-Mono -pointsize 14 \
            -draw "text 20,30 '$ seen init my-project'" \
            -draw "text 20,50 '$ cd my-project'" \
            -draw "text 20,70 '$ seen build'" \
            -draw "text 20,90 'Built in 42ms'" \
            -draw "text 20,110 '$ seen run'" \
            -draw "text 20,130 'Hello, Seen!'" \
            -fill "$SEEN_BLUE" -font DejaVu-Sans -pointsize 10 \
            -draw "text 20,390 'Seen Language - Terminal Usage'" \
            screenshots/terminal-usage.png
    
    success "✓ Sample screenshots created"
}

# Main execution
main() {
    header "Seen Language Asset Generation"
    
    # Get script directory
    local script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    cd "$script_dir"
    
    # Check dependencies
    check_dependencies
    
    # Setup directories
    setup_directories
    
    # Generate assets
    generate_svg_icon
    generate_png_icons
    generate_ico_icon
    generate_banners
    generate_logos
    generate_screenshots
    
    success ""
    success "==============================================="
    success "     Asset generation completed!              "
    success "==============================================="
    success ""
    success "Generated assets:"
    success "  Icons: $(ls icons/ | wc -l) files"
    success "  Banners: $(ls banners/ | wc -l) files"
    success "  Logos: $(ls logos/ | wc -l) files"
    success "  Screenshots: $(ls screenshots/ | wc -l) files"
    success ""
    success "Assets are ready for use in installers!"
}

# Show help
show_help() {
    cat << EOF
Seen Language Asset Generator

Usage: $0 [options]

Options:
  --help          Show this help message
  --clean         Remove all generated assets
  --icons-only    Generate only icons
  --banners-only  Generate only banners
  
Examples:
  $0                    # Generate all assets
  $0 --icons-only       # Generate only icons
  $0 --clean           # Clean generated assets

Requirements:
  - ImageMagick (convert command)
  - librsvg-bin (optional, for better SVG conversion)

The script generates placeholder branding assets for the Seen Language
installer system. These can be replaced with official assets when available.

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --help)
            show_help
            exit 0
            ;;
        --clean)
            info "Cleaning generated assets..."
            rm -rf icons banners screenshots logos
            success "✓ Assets cleaned"
            exit 0
            ;;
        --icons-only)
            header "Generating Icons Only"
            check_dependencies
            setup_directories
            generate_svg_icon
            generate_png_icons
            generate_ico_icon
            success "✓ Icons generated"
            exit 0
            ;;
        --banners-only)
            header "Generating Banners Only"
            check_dependencies
            setup_directories
            generate_banners
            success "✓ Banners generated"
            exit 0
            ;;
        *)
            error "Unknown option: $1. Use --help for usage information."
            ;;
    esac
    shift
done

# Run main function
main "$@"