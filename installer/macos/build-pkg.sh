#!/bin/bash
# Build a macOS .pkg installer for the Seen language
#
# This creates an installer that places:
#   /usr/local/bin/seen          - Compiler + LSP (seen lsp)
#   /usr/local/bin/seen-lsp      - Symlink to seen (convenience)
#   /usr/local/lib/seen/         - Standard library sources
#   /usr/local/share/seen/       - Language configs, runtime headers
#
# Usage:
#   ./installer/macos/build-pkg.sh              # uses compiler_seen/target/seen
#   ./installer/macos/build-pkg.sh /path/to/seen # uses custom binary
#
# Requirements: macOS with pkgbuild + productbuild (included in Xcode CLI tools)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
VERSION="${VERSION:-0.1.0-alpha}"
IDENTIFIER="org.seen-lang.seen"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()    { echo -e "${CYAN}$1${NC}"; }
success() { echo -e "${GREEN}$1${NC}"; }
warn()    { echo -e "${YELLOW}$1${NC}"; }
die()     { echo -e "${RED}Error: $1${NC}" >&2; exit 1; }

# --- Locate the Seen binary ---
SEEN_BIN="${1:-$PROJECT_ROOT/compiler_seen/target/seen}"

if [ ! -x "$SEEN_BIN" ]; then
    die "Seen binary not found at $SEEN_BIN. Build it first with ./scripts/safe_rebuild.sh"
fi

ARCH=$(file "$SEEN_BIN" | grep -o 'arm64\|x86_64' | head -1)
[ -z "$ARCH" ] && die "Cannot determine architecture of $SEEN_BIN"

info "=== Seen macOS Installer Builder ==="
info "Binary:  $SEEN_BIN ($ARCH)"
info "Version: $VERSION"
echo ""

# --- Set up staging area ---
WORK_DIR=$(mktemp -d /tmp/seen-pkg-build.XXXXXX)
trap "rm -rf '$WORK_DIR'" EXIT

PAYLOAD="$WORK_DIR/payload"
SCRIPTS="$WORK_DIR/scripts"
PKG_OUT="$PROJECT_ROOT/installer/macos/seen-${VERSION}-macos-${ARCH}.pkg"

mkdir -p "$PAYLOAD/usr/local/bin"
mkdir -p "$PAYLOAD/usr/local/lib/seen"
mkdir -p "$PAYLOAD/usr/local/share/seen/languages"
mkdir -p "$PAYLOAD/usr/local/share/seen/runtime"
mkdir -p "$SCRIPTS"

# --- 1. Install compiler binary (includes LSP) ---
info "[1/5] Packaging compiler binary..."
cp "$SEEN_BIN" "$PAYLOAD/usr/local/bin/seen"
chmod 755 "$PAYLOAD/usr/local/bin/seen"

# Create seen-lsp convenience symlink (points to same binary, runs `seen lsp`)
# We use a tiny wrapper script so editors can invoke seen-lsp directly
cat > "$PAYLOAD/usr/local/bin/seen-lsp" << 'LSPWRAPPER'
#!/bin/sh
# Convenience wrapper: invokes the Seen compiler's built-in LSP server.
# Editors can call this directly or use `seen lsp` — both work identically.
exec "$(dirname "$0")/seen" lsp "$@"
LSPWRAPPER
chmod 755 "$PAYLOAD/usr/local/bin/seen-lsp"

# --- 2. Install standard library ---
info "[2/5] Packaging standard library..."
if [ -d "$PROJECT_ROOT/seen_std/src" ]; then
    cp -R "$PROJECT_ROOT/seen_std/src/." "$PAYLOAD/usr/local/lib/seen/"
    STD_COUNT=$(find "$PAYLOAD/usr/local/lib/seen" -name '*.seen' | wc -l | tr -d ' ')
    info "    $STD_COUNT stdlib modules packaged"
else
    warn "    Standard library not found, skipping"
fi

# --- 3. Install language configurations ---
info "[3/5] Packaging language configurations..."
if [ -d "$PROJECT_ROOT/languages" ]; then
    cp -R "$PROJECT_ROOT/languages/." "$PAYLOAD/usr/local/share/seen/languages/"
    LANG_COUNT=$(ls -d "$PAYLOAD/usr/local/share/seen/languages"/*/ 2>/dev/null | wc -l | tr -d ' ')
    info "    $LANG_COUNT language packs (en, ar, es, ru, zh, ...)"
else
    warn "    Language configurations not found, skipping"
fi

# --- 4. Install runtime headers ---
info "[4/5] Packaging runtime headers..."
if [ -d "$PROJECT_ROOT/seen_runtime" ]; then
    cp "$PROJECT_ROOT"/seen_runtime/*.h "$PAYLOAD/usr/local/share/seen/runtime/" 2>/dev/null || true
    cp "$PROJECT_ROOT"/seen_runtime/*.c "$PAYLOAD/usr/local/share/seen/runtime/" 2>/dev/null || true
    RT_COUNT=$(ls "$PAYLOAD/usr/local/share/seen/runtime/" 2>/dev/null | wc -l | tr -d ' ')
    info "    $RT_COUNT runtime files packaged"
else
    warn "    Runtime not found, skipping"
fi

# --- 5. Create postinstall script ---
info "[5/5] Creating installer scripts..."
cat > "$SCRIPTS/postinstall" << 'POSTINSTALL'
#!/bin/bash
# Post-installation: verify the install and print a welcome message

SEEN=/usr/local/bin/seen

# Verify binary works
if [ -x "$SEEN" ]; then
    echo ""
    echo "Seen Language installed successfully!"
    echo ""
    echo "  Compiler:  $SEEN"
    echo "  LSP:       /usr/local/bin/seen-lsp  (or: seen lsp)"
    echo "  Stdlib:    /usr/local/lib/seen/"
    echo "  Languages: /usr/local/share/seen/languages/"
    echo ""
    echo "Quick start:"
    echo "  echo 'fun main() { println(\"Hello, Seen!\") }' > hello.seen"
    echo "  seen build hello.seen"
    echo "  ./hello"
    echo ""
    echo "VS Code extension:"
    echo "  code --install-extension seen-lang.seen-vscode"
    echo ""
fi

exit 0
POSTINSTALL
chmod 755 "$SCRIPTS/postinstall"

# --- Build the .pkg ---
echo ""
info "Building .pkg installer..."

# Step 1: build component pkg
pkgbuild \
    --root "$PAYLOAD" \
    --identifier "$IDENTIFIER" \
    --version "$VERSION" \
    --scripts "$SCRIPTS" \
    --install-location "/" \
    "$WORK_DIR/seen-component.pkg"

# Step 2: create distribution XML for productbuild (nicer installer UI)
cat > "$WORK_DIR/distribution.xml" << DISTXML
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>Seen Language</title>
    <welcome file="welcome.html"/>
    <conclusion file="conclusion.html"/>
    <options customize="never" require-scripts="false" hostArchitectures="arm64,x86_64"/>
    <domains enable_localSystem="true"/>
    <choices-outline>
        <line choice="default">
            <line choice="org.seen-lang.seen"/>
        </line>
    </choices-outline>
    <choice id="default"/>
    <choice id="org.seen-lang.seen" visible="false">
        <pkg-ref id="org.seen-lang.seen"/>
    </choice>
    <pkg-ref id="org.seen-lang.seen"
             version="$VERSION"
             onConclusion="none">seen-component.pkg</pkg-ref>
</installer-gui-script>
DISTXML

# Create welcome HTML
mkdir -p "$WORK_DIR/resources"
cat > "$WORK_DIR/resources/welcome.html" << 'WELCOME'
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><style>
body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; }
h1 { font-size: 24px; }
.feature { margin: 4px 0; }
code { background: #f0f0f0; padding: 2px 6px; border-radius: 3px; font-size: 13px; }
</style></head>
<body>
<h1>Seen Language</h1>
<p>A high-performance systems programming language with multi-language support.</p>

<p>This installer will place the following on your system:</p>
<div class="feature"><code>/usr/local/bin/seen</code> &mdash; Compiler, build system, and formatter</div>
<div class="feature"><code>/usr/local/bin/seen-lsp</code> &mdash; Language Server for editor integration</div>
<div class="feature"><code>/usr/local/lib/seen/</code> &mdash; Standard library</div>
<div class="feature"><code>/usr/local/share/seen/</code> &mdash; Language configs and runtime</div>

<p style="margin-top: 16px;">Requires: LLVM (install via <code>brew install llvm</code>)</p>
</body>
</html>
WELCOME

cat > "$WORK_DIR/resources/conclusion.html" << 'CONCLUSION'
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><style>
body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; }
h1 { font-size: 24px; color: #28a745; }
code { background: #f0f0f0; padding: 2px 6px; border-radius: 3px; font-size: 13px; }
pre { background: #f8f8f8; padding: 12px; border-radius: 6px; font-size: 13px; }
</style></head>
<body>
<h1>Installation Complete</h1>
<p>Seen has been installed. Open a new terminal and try:</p>

<pre>echo 'fun main() { println("Hello!") }' > hello.seen
seen build hello.seen
./hello</pre>

<p><b>VS Code extension:</b></p>
<pre>code --install-extension seen-lang.seen-vscode</pre>

<p><b>LSP for other editors:</b> point your editor at <code>seen-lsp</code> (stdio transport).</p>
</body>
</html>
CONCLUSION

# Step 3: build product pkg (with UI resources)
productbuild \
    --distribution "$WORK_DIR/distribution.xml" \
    --resources "$WORK_DIR/resources" \
    --package-path "$WORK_DIR" \
    "$PKG_OUT"

echo ""
success "=== macOS Installer Built ==="
success "Output: $PKG_OUT"
echo ""
ls -lh "$PKG_OUT"
echo ""
info "To install: open $PKG_OUT"
info "Or from terminal: sudo installer -pkg $PKG_OUT -target /"
