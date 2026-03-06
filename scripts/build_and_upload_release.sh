#!/usr/bin/env bash
# Build release packages locally and upload to GitHub Releases.
#
# Usage: ./scripts/build_and_upload_release.sh <version>
#   e.g.: ./scripts/build_and_upload_release.sh 1.0.0-alpha
#
# Prerequisites:
#   - Working compiler at compiler_seen/target/seen
#   - gh CLI authenticated (gh auth status)
#   - Optional: dpkg-deb, rpmbuild, appimagetool for extra packages

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 <version>"
    echo "  e.g.: $0 1.0.0-alpha"
    exit 1
fi

TAG="v$VERSION"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
DIST_DIR="$ROOT_DIR/dist"  # absolute path required — build_release.sh cd's into subshells

# --- Preflight checks ---

if ! command -v gh &>/dev/null; then
    echo "Error: gh CLI not found. Install from https://cli.github.com/"
    exit 1
fi

if ! gh auth status &>/dev/null 2>&1; then
    echo "Error: gh CLI not authenticated. Run: gh auth login"
    exit 1
fi

if [[ ! -x "$COMPILER" ]]; then
    echo "Error: Compiler not found at $COMPILER"
    echo "Run ./scripts/safe_rebuild.sh first."
    exit 1
fi

# Quick smoke test (try "build" first — full CLI, fall back to "compile" for bootstrap-only binary)
echo "=== Verifying compiler... ==="
TMPFILE=$(mktemp /tmp/seen_verify_XXXXXX.seen)
echo 'fun main() { println("release build ok") }' > "$TMPFILE"
if "$COMPILER" build "$TMPFILE" -o "${TMPFILE%.seen}" &>/dev/null; then
    echo "Compiler OK (full CLI)."
elif "$COMPILER" compile "$TMPFILE" "${TMPFILE%.seen}" &>/dev/null; then
    echo "Compiler OK (bootstrap-only binary)."
else
    echo "Error: Compiler failed smoke test."
    rm -f "$TMPFILE" "${TMPFILE%.seen}"
    exit 1
fi
rm -f "$TMPFILE" "${TMPFILE%.seen}"

# --- Build release packages ---

echo ""
echo "=== Building release packages (v$VERSION)... ==="
rm -rf "$DIST_DIR"
"$SCRIPT_DIR/build_release.sh" --version "$VERSION" --output-dir "$DIST_DIR"

ARTIFACTS=("$DIST_DIR"/*)
if [[ ${#ARTIFACTS[@]} -eq 0 ]]; then
    echo "Error: No artifacts produced in $DIST_DIR"
    exit 1
fi

echo ""
echo "Artifacts:"
ls -lh "$DIST_DIR"/ | grep -v '^total'

# --- Tag and push ---

echo ""
echo "=== Creating tag $TAG... ==="
if git -C "$ROOT_DIR" rev-parse "$TAG" &>/dev/null; then
    echo "Tag $TAG already exists, skipping creation."
else
    git -C "$ROOT_DIR" tag "$TAG"
    echo "Created tag $TAG."
fi

echo "Pushing tag $TAG..."
git -C "$ROOT_DIR" push origin "$TAG"

# --- Create GitHub Release ---

echo ""
echo "=== Uploading to GitHub Releases... ==="

PRERELEASE_FLAG=""
if [[ "$VERSION" == *alpha* || "$VERSION" == *beta* || "$VERSION" == *rc* ]]; then
    PRERELEASE_FLAG="--prerelease"
fi

NOTES="## Seen Language $VERSION

### Installation

**Quick install (Linux):**
\`\`\`bash
curl -sSL https://github.com/codeyousef/SeenLang/releases/download/$TAG/seen-${VERSION}-linux-x64.tar.gz | tar xz
cd seen-${VERSION}-linux-x64
sudo ./install.sh
\`\`\`

**Or download individual packages below.**

### Verification

All artifacts are signed with [Sigstore](https://www.sigstore.dev/) keyless signing (signatures added by CI after upload).

\`\`\`bash
cosign verify-blob --bundle <artifact>.bundle <artifact>
\`\`\`

### Checksums

See \`SHA256SUMS\` for file integrity verification."

# Create or update the release, uploading all artifacts
if gh release view "$TAG" &>/dev/null 2>&1; then
    echo "Release $TAG exists, uploading artifacts..."
    gh release upload "$TAG" "$DIST_DIR"/* --clobber
else
    gh release create "$TAG" "$DIST_DIR"/* \
        --title "Seen Language $VERSION" \
        --notes "$NOTES" \
        $PRERELEASE_FLAG
fi

echo ""
echo "=== Done! ==="
echo "Release: https://github.com/codeyousef/SeenLang/releases/tag/$TAG"
echo ""
echo "CI will automatically sign artifacts when the tag is processed."
