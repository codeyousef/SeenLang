# Seen Language Branding Assets

This directory contains branding assets used by the installer system.

## Directory Structure

```
assets/
├── icons/                 # Application icons
│   ├── seen-icon.ico      # Windows icon (256x256)
│   ├── seen-icon.icns     # macOS icon bundle
│   ├── seen-icon.png      # PNG icon (multiple sizes)
│   └── seen-icon.svg      # Vector icon (scalable)
│
├── banners/               # Installer banners
│   ├── dialog.bmp         # WiX dialog background (493x312)
│   ├── banner.bmp         # WiX banner (493x58)
│   └── header.png         # Web installer header
│
├── screenshots/           # Application screenshots
│   ├── ide-integration.png
│   ├── terminal-usage.png
│   └── project-structure.png
│
└── logos/                 # Company/project logos
    ├── seen-logo.svg      # Main logo (vector)
    ├── seen-logo.png      # Logo PNG (transparent)
    └── seen-wordmark.png  # Text-based logo
```

## Icon Requirements

### Windows (.ico)
- Multiple sizes: 16x16, 32x32, 48x48, 256x256
- 32-bit color depth with alpha channel
- Used in MSI installer and executables

### macOS (.icns)
- Icon bundle with multiple resolutions
- Retina display support (2x variants)
- Used in DMG installer and .app bundles

### Linux (.png)
- Standard sizes: 16x16, 32x32, 48x48, 128x128, 256x256
- PNG format with transparency
- Used in DEB/RPM packages and desktop entries

### Web (.svg/.png)
- Scalable SVG for responsive designs
- High-resolution PNG fallbacks
- Used in documentation and websites

## Banner Requirements

### WiX Installer Banners
- **Dialog Background**: 493x312 pixels, BMP format
- **Top Banner**: 493x58 pixels, BMP format
- 24-bit color depth (no transparency)
- Professional appearance with Seen branding

### Web Banners
- **Header**: 1200x300 pixels, PNG format
- **Social**: 1200x628 pixels (OpenGraph)
- **GitHub**: 1280x640 pixels (repository banner)

## Color Palette

### Primary Colors
- **Seen Blue**: #2E86C1
- **Dark Blue**: #1B4F72
- **Light Blue**: #AED6F1

### Secondary Colors  
- **Seen Orange**: #F39C12
- **Dark Orange**: #D68910
- **Light Orange**: #F8C471

### Neutral Colors
- **Dark Gray**: #2C3E50
- **Medium Gray**: #7F8C8D
- **Light Gray**: #ECF0F1
- **White**: #FFFFFF

## Typography

### Primary Font
- **Name**: Inter
- **Usage**: Headlines, UI elements
- **Weights**: Regular (400), Medium (500), Bold (700)

### Secondary Font  
- **Name**: JetBrains Mono
- **Usage**: Code examples, technical content
- **Weights**: Regular (400), Bold (700)

## Usage Guidelines

### Logo Usage
- Maintain clear space equal to the height of the "S" in "Seen"
- Don't modify colors, proportions, or add effects
- Use PNG on colored backgrounds, SVG for scalable needs

### Color Usage
- Primary blue for main elements and CTAs
- Orange for accents and highlights  
- Neutral grays for text and backgrounds
- Ensure sufficient contrast for accessibility

### Icon Usage
- Use appropriate size for context (16px-256px)
- Maintain consistent visual style
- Include transparency for overlays

## File Formats

### Vector Graphics (.svg)
- Scalable for any size
- Small file size
- Ideal for web and print

### Raster Graphics (.png)
- Transparency support
- Good compression
- Wide compatibility

### Windows Specific (.ico/.bmp)
- Native Windows formats
- Required for MSI installers
- Multiple sizes in single file

## Creating Assets

### Tools Recommended
- **Vector**: Adobe Illustrator, Inkscape, Figma
- **Raster**: Adobe Photoshop, GIMP, Canva
- **Conversion**: ImageMagick, GIMP batch processing

### Automation
Use the provided scripts to generate assets:
```bash
# Generate all icon sizes from SVG
./generate-icons.sh seen-icon.svg

# Create installer banners
./generate-banners.sh

# Optimize all images
./optimize-assets.sh
```

## Placeholder Assets

Until official branding assets are available, placeholder assets are generated programmatically:

- Simple geometric icons using ImageMagick
- Basic color schemes with brand colors
- Minimal banners with text overlays

## Asset Guidelines

### Quality Standards
- **Vector**: Use vectors when possible for scalability
- **Resolution**: Minimum 2x for Retina displays
- **Compression**: Optimize without quality loss
- **Naming**: Use consistent naming conventions

### Accessibility
- **Contrast**: Minimum 4.5:1 for normal text
- **Visibility**: Icons should be recognizable at 16px
- **Color**: Don't rely solely on color for information

### Platform Consistency
- Follow platform-specific design guidelines
- Use native formats when required
- Maintain brand recognition across platforms

## License

All branding assets are proprietary to the Seen Language project. Usage outside of official Seen Language installations and documentation requires permission.