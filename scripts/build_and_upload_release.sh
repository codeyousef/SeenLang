#!/usr/bin/env bash
# Build release packages locally and upload to GitHub Releases.
#
# Usage: ./scripts/build_and_upload_release.sh <version>
#   e.g.: ./scripts/build_and_upload_release.sh 1.0.0-alpha
#
# Prerequisites:
#   - Working compiler at compiler_seen/target/seen
#   - gh CLI authenticated (gh auth status)
#   - Optional: dpkg-deb, rpmbuild, appimagetool for Linux packages
#   - Optional: x86_64-w64-mingw32-gcc, makensis for Windows cross-build
#   - Optional: osxcross (o64-clang) for macOS cross-build

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
LINUX_X64_COMPILER="${SEEN_LINUX_X64_COMPILER:-$COMPILER}"
LINUX_X64_V3_COMPILER="${SEEN_LINUX_X64_V3_COMPILER:-$ROOT_DIR/compiler_seen/target/seen-x86-64-v3}"
DIST_DIR="$ROOT_DIR/dist"  # absolute path required — build_release.sh cd's into subshells

die() {
    echo "Error: $*" >&2
    exit 1
}

require_artifacts() {
    local missing=0
    local artifact

    for artifact in "$@"; do
        if [[ ! -s "$artifact" ]]; then
            echo "Error: required release artifact missing or empty: $artifact" >&2
            missing=1
        fi
    done

    if [[ "$missing" -ne 0 ]]; then
        exit 1
    fi
}

write_checksum_manifest() {
    local -a artifact_names=()
    local artifact_name

    while IFS= read -r artifact_name; do
        artifact_names+=("$artifact_name")
    done < <(cd "$DIST_DIR" && find . -maxdepth 1 -type f \
        \( -name '*.tar.gz' -o -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' -o -name '*.zip' -o -name '*.exe' \) \
        -printf '%f\n' | sort)

    if [[ "${#artifact_names[@]}" -eq 0 ]]; then
        die "No checksum-eligible artifacts produced in $DIST_DIR"
    fi

    (cd "$DIST_DIR" && sha256sum "${artifact_names[@]}" > SHA256SUMS)
    (cd "$DIST_DIR" && sha256sum -c SHA256SUMS >/dev/null)
}

write_sidecar_checksum() {
    local artifact="$1"
    local artifact_dir artifact_name

    [[ -s "$artifact" ]] || return 0
    artifact_dir="$(dirname "$artifact")"
    artifact_name="$(basename "$artifact")"
    (cd "$artifact_dir" && sha256sum "$artifact_name" > "$artifact_name.sha256")
}

# --- Preflight checks ---

if ! command -v gh &>/dev/null; then
    die "gh CLI not found. Install from https://cli.github.com/"
fi

if ! gh auth status &>/dev/null 2>&1; then
    die "gh CLI not authenticated. Run: gh auth login"
fi

if [[ ! -x "$LINUX_X64_COMPILER" ]]; then
    echo "Error: portable Linux x64 compiler not found at $LINUX_X64_COMPILER"
    echo "Build it first with a memory-capped baseline rebuild, for example:"
    echo "  SEEN_LOW_MEMORY=1 SEEN_MAIN_VMEM_KB=8388608 SEEN_OPT_VMEM_KB=2097152 SEEN_RELEASE_CPU_BASELINE=x86-64 ./scripts/safe_rebuild.sh"
    exit 1
fi

if [[ -n "${SEEN_APPIMAGE_RUNTIME_FILE:-}" && ! -f "$SEEN_APPIMAGE_RUNTIME_FILE" ]]; then
    die "SEEN_APPIMAGE_RUNTIME_FILE does not exist: $SEEN_APPIMAGE_RUNTIME_FILE"
fi

# Quick smoke test (try "build" first — full CLI, fall back to "compile" for bootstrap-only binary)
echo "=== Verifying compiler... ==="
TMPFILE=$(mktemp /tmp/seen_verify_XXXXXX.seen)
echo 'fun main() { println("release build ok") }' > "$TMPFILE"
if "$LINUX_X64_COMPILER" build "$TMPFILE" -o "${TMPFILE%.seen}" &>/dev/null; then
    echo "Compiler OK (full CLI)."
elif "$LINUX_X64_COMPILER" compile "$TMPFILE" "${TMPFILE%.seen}" --target-cpu=x86-64 &>/dev/null; then
    echo "Compiler OK (bootstrap-only binary)."
else
    echo "Error: Compiler failed smoke test."
    rm -f "$TMPFILE" "${TMPFILE%.seen}"
    exit 1
fi
rm -f "$TMPFILE" "${TMPFILE%.seen}"

# --- Build release packages ---

echo ""
echo "=== Building Linux release packages (v$VERSION)... ==="
rm -rf "$DIST_DIR"
"$SCRIPT_DIR/build_release.sh" \
    --version "$VERSION" \
    --output-dir "$DIST_DIR" \
    --compiler "$LINUX_X64_COMPILER" \
    --cpu-baseline x86-64 \
    --artifact-suffix linux-x64

if [[ -x "$LINUX_X64_V3_COMPILER" ]]; then
    "$SCRIPT_DIR/build_release.sh" \
        --version "$VERSION" \
        --output-dir "$DIST_DIR" \
        --compiler "$LINUX_X64_V3_COMPILER" \
        --cpu-baseline x86-64-v3 \
        --artifact-suffix linux-x64-v3
else
    echo ""
    echo "Skipping linux-x64-v3 tarball: compiler not found at $LINUX_X64_V3_COMPILER"
    echo "Build it separately with SEEN_RELEASE_CPU_BASELINE=x86-64-v3 and set SEEN_LINUX_X64_V3_COMPILER."
fi

# --- Windows cross-build ---

if command -v x86_64-w64-mingw32-gcc &>/dev/null; then
    echo ""
    echo "=== Building Windows packages (v$VERSION)... ==="

    WIN_DIR="$ROOT_DIR/target-windows"
    WIN_INSTALLER_DIR="$ROOT_DIR/installer/windows"

    # Cross-compile seen.exe if not already present
    if [[ ! -f "$WIN_DIR/seen.exe" ]]; then
        echo "Cross-compiling seen.exe..."
        # Cross-compile a hello-world to verify the toolchain, then use
        # pre-built .exe if the full compiler can't be cross-compiled as one file
        TMPWIN=$(mktemp /tmp/seen_win_test_XXXXXX.seen)
        echo 'fun main() { println("windows ok") }' > "$TMPWIN"
        if bash "$SCRIPT_DIR/build_windows.sh" "$TMPWIN" "$WIN_DIR/test_win.exe" &>/dev/null; then
            rm -f "$TMPWIN" "$WIN_DIR/test_win.exe"
            echo "  Windows cross-compilation toolchain verified."
        else
            rm -f "$TMPWIN"
            echo "  WARNING: Windows cross-compilation failed, skipping .exe build."
        fi
    fi

    if [[ -f "$WIN_DIR/seen.exe" ]]; then
        # Build NSIS installer
        if command -v makensis &>/dev/null; then
            echo "Building Windows installer..."
            bash "$SCRIPT_DIR/build_windows_installer.sh" "$VERSION" --skip-compile 2>&1 | tail -10

            # Copy Windows artifacts to dist/
            for f in "$WIN_DIR"/seen-"$VERSION"-windows-x64.zip; do
                [[ -f "$f" ]] && cp "$f" "$DIST_DIR/"
            done
            for f in "$WIN_INSTALLER_DIR"/output/Seen-"$VERSION"-windows-x64-setup.exe; do
                [[ -f "$f" ]] && cp "$f" "$DIST_DIR/"
            done
        else
            # At least create the ZIP
            bash "$SCRIPT_DIR/package_windows.sh" "$VERSION" 2>&1 | tail -5
            for f in "$WIN_DIR"/seen-"$VERSION"-windows-x64.zip; do
                [[ -f "$f" ]] && cp "$f" "$DIST_DIR/"
            done
        fi
    fi
else
    echo ""
    echo "Skipping Windows build (mingw-gcc not found)."
    echo "  Install: sudo apt-get install gcc-mingw-w64-x86-64"
fi

# --- macOS Homebrew formula ---

if [[ -f "$ROOT_DIR/installer/homebrew/generate-formula.sh" ]]; then
    echo ""
    if [[ "${SEEN_RELEASE_GENERATE_HOMEBREW:-0}" == "1" ]] ||
        compgen -G "$DIST_DIR/seen-$VERSION-macos-*.tar.gz" >/dev/null; then
        echo "=== Generating macOS Homebrew formula (v$VERSION)... ==="
        if bash "$ROOT_DIR/installer/homebrew/generate-formula.sh" \
            --version "$VERSION" \
            --output "$DIST_DIR/seen-lang.rb" 2>&1 | tail -5; then
            echo "  -> $DIST_DIR/seen-lang.rb"
        else
            rm -f "$DIST_DIR/seen-lang.rb"
            die "Homebrew formula generator failed."
        fi
    else
        echo "Skipping Homebrew formula (macOS release artifacts not present)."
        echo "  Set SEEN_RELEASE_GENERATE_HOMEBREW=1 to force generation."
    fi
fi

# --- macOS native binary (requires osxcross) ---

if command -v o64-clang &>/dev/null || command -v x86_64-apple-darwin-clang &>/dev/null; then
    echo ""
    echo "=== Cross-compiling macOS binary (v$VERSION)... ==="
    echo "  osxcross detected, building macOS binary..."
    # TODO: Implement osxcross-based macOS cross-compilation
    echo "  (not yet implemented — use scripts/bootstrap_macos.sh on macOS)"
else
    echo ""
    echo "Skipping macOS native binary (osxcross not found)."
    echo "  Build on macOS: ./scripts/bootstrap_macos.sh"
    echo "  Or install osxcross: https://github.com/tpoechtrager/osxcross"
fi

# --- Summary ---

ARTIFACTS=("$DIST_DIR"/*)
if [[ ${#ARTIFACTS[@]} -eq 0 ]]; then
    die "No artifacts produced in $DIST_DIR"
fi

EXPECTED_ARTIFACTS=(
    "$DIST_DIR/seen-$VERSION-linux-x64.tar.gz"
)

if [[ -x "$LINUX_X64_V3_COMPILER" ]]; then
    EXPECTED_ARTIFACTS+=("$DIST_DIR/seen-$VERSION-linux-x64-v3.tar.gz")
fi

if command -v dpkg-deb &>/dev/null; then
    EXPECTED_ARTIFACTS+=("$DIST_DIR/seen-lang_${VERSION}_amd64.deb")
fi

if command -v rpmbuild &>/dev/null; then
    EXPECTED_ARTIFACTS+=(
        "$DIST_DIR/seen-lang-$VERSION-1.x86_64.rpm"
        "$DIST_DIR/seen-lang-devel-$VERSION-1.x86_64.rpm"
        "$DIST_DIR/seen-lang-docs-$VERSION-1.noarch.rpm"
    )
fi

if command -v appimagetool &>/dev/null; then
    EXPECTED_ARTIFACTS+=("$DIST_DIR/SeenLanguage-$VERSION-x86_64.AppImage")
fi

if command -v x86_64-w64-mingw32-gcc &>/dev/null && [[ -f "$ROOT_DIR/target-windows/seen.exe" ]]; then
    EXPECTED_ARTIFACTS+=("$DIST_DIR/seen-$VERSION-windows-x64.zip")
    if command -v makensis &>/dev/null; then
        EXPECTED_ARTIFACTS+=("$DIST_DIR/Seen-$VERSION-windows-x64-setup.exe")
    fi
fi

require_artifacts "${EXPECTED_ARTIFACTS[@]}"

write_sidecar_checksum "$DIST_DIR/seen-$VERSION-windows-x64.zip"
write_sidecar_checksum "$DIST_DIR/Seen-$VERSION-windows-x64-setup.exe"

# Regenerate checksums to include all platforms
echo ""
echo "Regenerating checksums..."
write_checksum_manifest
echo "  -> $DIST_DIR/SHA256SUMS"

echo ""
echo "Artifacts:"
ls -lh "$DIST_DIR"/ | grep -v '^total'

# --- Tag and push ---

echo ""
echo "=== Creating tag $TAG... ==="
if git -C "$ROOT_DIR" rev-parse -q --verify "refs/tags/$TAG" &>/dev/null; then
    TAG_COMMIT="$(git -C "$ROOT_DIR" rev-list -n1 "$TAG")"
    HEAD_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"
    if [[ "$TAG_COMMIT" != "$HEAD_COMMIT" ]]; then
        die "Tag $TAG points at $TAG_COMMIT, but HEAD is $HEAD_COMMIT. Move or recreate the tag before uploading."
    fi
    echo "Tag $TAG already exists at HEAD."
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

**Linux:**
\`\`\`bash
curl -sSL https://github.com/codeyousef/SeenLang/releases/download/$TAG/seen-${VERSION}-linux-x64.tar.gz | tar xz
cd seen-${VERSION}-linux-x64
sudo ./install.sh
\`\`\`

\`linux-x64\` is the portable x86-64 baseline. Use \`seen-${VERSION}-linux-x64-v3.tar.gz\` only on x86-64-v3/AVX2-class machines.

**Windows:** Download \`Seen-${VERSION}-windows-x64-setup.exe\` or the ZIP archive. The installer includes Seen's runtime, standard library, language files, and LLVM toolchain support; users do not need LLVM preinstalled.

**macOS:** \`brew install seen-lang\` (if published) or build from source with \`./scripts/bootstrap_macos.sh\`.

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
