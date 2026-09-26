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
#   - Optional: prebuilt macOS archives in SEEN_RELEASE_MACOS_INPUT_DIR

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TAG_POLICY="$SCRIPT_DIR/release_tag_policy.sh"
if [[ "${SEEN_RELEASE_CONTAINMENT_IN_SCOPE:-0}" != "1" ]]; then
    exec "$SCRIPT_DIR/run_release_upload.sh" "$@"
fi
if [[ "${SEEN_JOBS:-0}" != "1" || "${SEEN_OPT_JOBS:-0}" != "1" ||
    "${SEEN_PACKAGE_JOBS:-0}" != "1" || "${SEEN_NO_FORK:-0}" != "1" ]]; then

    echo "Error: release compiler, optimizer, and package workers must be serial" >&2
    exit 126
fi
if ! "$SCRIPT_DIR/run_in_hard_memory_scope.sh" --verify-only >/dev/null; then
    echo "Error: release upload is outside a read-back-verified hard scope" >&2
    exit 126
fi
if [[ ! -x "${SEEN_RELEASE_PROJECT_WRAPPER:-}" ]]; then
    echo "Error: release project-artifact wrapper is missing" >&2
    exit 126
fi
BUILD_TRACE_COMMON="$SCRIPT_DIR/build_trace_common.sh"
if [[ -f "$BUILD_TRACE_COMMON" ]]; then
    # shellcheck source=scripts/build_trace_common.sh
    source "$BUILD_TRACE_COMMON"
    seen_build_trace_init "build_and_upload_release"
    trap 'seen_build_trace_summary' EXIT
fi

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
SEEN_PACKAGE_CLIENT_BIN="${SEEN_PACKAGE_CLIENT_BIN:-$(dirname "$LINUX_X64_COMPILER")/seen-pkg}"
export SEEN_PACKAGE_CLIENT_BIN
DIST_DIR="$ROOT_DIR/dist"  # absolute path required — build_release.sh cd's into subshells
MACOS_INPUT_DIR="${SEEN_RELEASE_MACOS_INPUT_DIR:-}"
THREE_PLATFORMS="${SEEN_RELEASE_REQUIRE_THREE_PLATFORMS:-0}"
SKIP_OPTIONAL_CROSS_BUILDS="${SEEN_RELEASE_SKIP_OPTIONAL_CROSS_BUILDS:-0}"
PLATFORM_INPUT_DIR="$ROOT_DIR/.seen/agent-tools/release-platform-inputs/$VERSION"
SIGN_MODE="${SEEN_RELEASE_SIGN_MODE:-}"
DRY_RUN="${SEEN_RELEASE_DRY_RUN:-0}"
SIGN_IDENTITY="${SEEN_RELEASE_SIGN_IDENTITY:-}"
SIGN_ISSUER="${SEEN_RELEASE_SIGN_ISSUER:-https://token.actions.githubusercontent.com}"
RELEASE_REPOSITORY="${GITHUB_REPOSITORY:-codeyousef/SeenLang}"
RELEASE_DRAFT_ID="${SEEN_RELEASE_DRAFT_ID:-}"

die() {
    echo "Error: $*" >&2
    exit 1
}

if ! [[ "$VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z]+([.-][0-9A-Za-z]+)*)?$ ]]; then
    die "Release version is not a supported semantic version: $VERSION"
fi
ESCAPED_VERSION="${VERSION//./\\.}"
EXPECTED_SIGN_IDENTITY="^https://github\\.com/codeyousef/SeenLang/\\.github/workflows/release\\.yml@refs/tags/v${ESCAPED_VERSION}\$"
if [[ -z "$SIGN_IDENTITY" ]]; then
    SIGN_IDENTITY="$EXPECTED_SIGN_IDENTITY"
fi
[[ "$SIGN_IDENTITY" == "$EXPECTED_SIGN_IDENTITY" ]] ||
    die "SEEN_RELEASE_SIGN_IDENTITY must be the exact anchored release.yml tag identity"
[[ "$SIGN_ISSUER" == "https://token.actions.githubusercontent.com" ]] ||
    die "SEEN_RELEASE_SIGN_ISSUER must be the GitHub Actions OIDC issuer"
[[ "$RELEASE_REPOSITORY" == "codeyousef/SeenLang" ]] ||
    die "GITHUB_REPOSITORY must identify codeyousef/SeenLang"

assert_release_absent() {
    local probe status

    [[ "$RELEASE_REPOSITORY" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] ||
        die "GITHUB_REPOSITORY is invalid"
    set +e
    probe="$(gh api --include "repos/$RELEASE_REPOSITORY/releases/tags/$TAG" 2>&1)"
    status=$?
    set -e
    if [[ "$status" -eq 0 ]]; then
        die "Release $TAG already exists; ordinary publishing never modifies existing releases or assets"
    fi
    if [[ "$status" -ne 1 ]] ||
        ! grep -Eq '^HTTP/[0-9.]+[[:space:]]+404([[:space:]]|$)' <<<"$probe"; then

        die "Could not prove that release $TAG is absent"
    fi
}

assert_staged_draft() {
    [[ "$RELEASE_DRAFT_ID" =~ ^[1-9][0-9]*$ ]] ||
        die "SEEN_RELEASE_DRAFT_ID must identify the numeric unpublished draft"
    python3 "$SCRIPT_DIR/release_draft_api.py" assert-draft \
        --version "$VERSION" --release-id "$RELEASE_DRAFT_ID" \
        --expected-name "seen-$VERSION-macos-arm64.tar.gz" \
        --expected-name "seen-$VERSION-windows-x64.zip" \
        --expected-name "Seen-$VERSION-windows-x64-setup.exe" \
        --expected-name "seen-$VERSION-platform-inputs.json" ||
        die "Could not inspect the exact staged release $TAG"
    python3 "$SCRIPT_DIR/release_platform_inputs.py" verify --root "$ROOT_DIR" \
        --version "$VERSION" --input-dir "$PLATFORM_INPUT_DIR" ||
        die "Staged platform inputs do not match this exact source commit"
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
    local -a sorted_artifact_names=()
    local artifact artifact_dir artifact_name

    for artifact in "$@"; do
        [[ -s "$artifact" ]] || die "Checksum artifact missing or empty: $artifact"
        artifact_dir="$(cd "$(dirname "$artifact")" && pwd -P)"
        if [[ "$artifact_dir" != "$(cd "$DIST_DIR" && pwd -P)" ]]; then
            die "Refusing to checksum an artifact outside $DIST_DIR: $artifact"
        fi
        artifact_names+=("$(basename "$artifact")")
    done

    if [[ "${#artifact_names[@]}" -eq 0 ]]; then
        die "No checksum-eligible artifacts produced in $DIST_DIR"
    fi

    while IFS= read -r artifact_name; do
        sorted_artifact_names+=("$artifact_name")
    done < <(printf '%s\n' "${artifact_names[@]}" | LC_ALL=C sort -u)

    (cd "$DIST_DIR" && sha256sum "${sorted_artifact_names[@]}" > SHA256SUMS)
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

case "$DRY_RUN" in 0|1) ;; *) die "SEEN_RELEASE_DRY_RUN must be 0 or 1" ;; esac
case "$THREE_PLATFORMS" in 0|1) ;; *) die "SEEN_RELEASE_REQUIRE_THREE_PLATFORMS must be 0 or 1" ;; esac
case "$SKIP_OPTIONAL_CROSS_BUILDS" in 0|1) ;; *) die "SEEN_RELEASE_SKIP_OPTIONAL_CROSS_BUILDS must be 0 or 1" ;; esac
if [[ "$THREE_PLATFORMS" == 1 ]]; then
    [[ -z "$MACOS_INPUT_DIR" ]] || die "explicit macOS input conflicts with three-platform draft mode"
    MACOS_INPUT_DIR="$PLATFORM_INPUT_DIR"
    python3 "$SCRIPT_DIR/release_platform_inputs.py" verify --root "$ROOT_DIR" \
        --version "$VERSION" --input-dir "$PLATFORM_INPUT_DIR" ||
        die "Required macOS and Windows inputs are missing"
fi
if [[ "$DRY_RUN" == "0" ]]; then
    if ! command -v gh &>/dev/null; then
        die "gh CLI not found. Install from https://cli.github.com/"
    fi
    if ! gh auth status &>/dev/null 2>&1; then
        die "gh CLI not authenticated. Run: gh auth login"
    fi
    case "$SIGN_MODE" in
        keyless|key|kms) ;;
        *) die "SEEN_RELEASE_SIGN_MODE must be keyless, key, or kms; unsigned uploads are forbidden" ;;
    esac
    if [[ "$THREE_PLATFORMS" == 1 && "$SIGN_MODE" != keyless ]]; then
        die "three-platform publication requires the exact keyless workflow identity"
    fi
    [ -f "$TAG_POLICY" ] && [ ! -L "$TAG_POLICY" ] ||
        die "release tag policy is missing or unsafe"
    HEAD_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)" ||
        die "could not resolve the release commit"
    # shellcheck source=scripts/release_tag_policy.sh
    source "$TAG_POLICY" || die "could not load release tag policy"
    seen_release_verify_published_tag \
        "$ROOT_DIR" "$TAG" "$HEAD_COMMIT" "$RELEASE_REPOSITORY" ||
        die "release tag did not satisfy the published-tag policy"
    if [[ "$THREE_PLATFORMS" == 1 ]]; then
        assert_staged_draft
    else
        assert_release_absent
    fi
fi

if [[ ! -x "$LINUX_X64_COMPILER" ]]; then
    echo "Error: portable Linux x64 compiler not found at $LINUX_X64_COMPILER"
    echo "Build it first with a memory-capped baseline rebuild, for example:"
    echo "  SEEN_LOW_MEMORY=1 SEEN_MAIN_VMEM_KB=8388608 SEEN_OPT_VMEM_KB=2097152 SEEN_RELEASE_CPU_BASELINE=x86-64 ./scripts/safe_rebuild.sh"
    exit 1
fi
if [[ ! -x "$SEEN_PACKAGE_CLIENT_BIN" ]]; then
    die "Version-coupled package client not found at $SEEN_PACKAGE_CLIENT_BIN; run scripts/safe_rebuild.sh --tier full first"
fi

if declare -F seen_build_require_full_release_stamp >/dev/null 2>&1; then
    seen_build_require_full_release_stamp "$ROOT_DIR" "$LINUX_X64_COMPILER"
fi

if [[ -n "${SEEN_APPIMAGE_RUNTIME_FILE:-}" && ! -f "$SEEN_APPIMAGE_RUNTIME_FILE" ]]; then
    die "SEEN_APPIMAGE_RUNTIME_FILE does not exist: $SEEN_APPIMAGE_RUNTIME_FILE"
fi

# Quick smoke test using the supported compile contract and project-confined artifacts.
echo "=== Verifying compiler... ==="
SMOKE_ROOT="$ROOT_DIR/.seen/release-smoke"
rm -rf "$SMOKE_ROOT"
mkdir -p "$SMOKE_ROOT"
TMPFILE="$SMOKE_ROOT/release-smoke.seen"
echo 'fun main() { println("release build ok") }' > "$TMPFILE"
if "$SCRIPT_DIR/run_with_project_artifacts.sh" release-upload-smoke -- \
    "$LINUX_X64_COMPILER" compile "$TMPFILE" "${TMPFILE%.seen}" \
    --target-cpu=x86-64 --no-cache --jobs 1 --opt-jobs 1 --no-fork &>/dev/null; then
    echo "Compiler OK."
else
    echo "Error: Compiler failed smoke test."
    rm -rf "$SMOKE_ROOT"
    exit 1
fi
rm -rf "$SMOKE_ROOT"

# --- Build release packages ---

echo ""
echo "=== Building Linux release packages (v$VERSION)... ==="
if [[ -n "$MACOS_INPUT_DIR" ]]; then
    if [[ ! -d "$MACOS_INPUT_DIR" ]]; then
        die "SEEN_RELEASE_MACOS_INPUT_DIR is not a directory: $MACOS_INPUT_DIR"
    fi
    MACOS_INPUT_DIR="$(cd "$MACOS_INPUT_DIR" && pwd -P)"
    DIST_INPUT_BOUNDARY="$DIST_DIR"
    if [[ -d "$DIST_DIR" ]]; then
        DIST_INPUT_BOUNDARY="$(cd "$DIST_DIR" && pwd -P)"
    fi
    case "$MACOS_INPUT_DIR/" in
        "$DIST_INPUT_BOUNDARY/"*)
            die "SEEN_RELEASE_MACOS_INPUT_DIR must be outside $DIST_DIR"
            ;;
    esac
fi

if [[ "${SEEN_RELEASE_CLEAN_DIST:-0}" == "1" ]]; then
    rm -rf "$DIST_DIR"
fi
mkdir -p "$DIST_DIR"

# A failed optional builder must not be able to reuse an older artifact from a
# previous attempt at this same version. Remove only outputs this command can
# produce or upload; unrelated versions in dist/ are left untouched.
shopt -s nullglob
VERSION_OUTPUTS=(
    "$DIST_DIR/seen-$VERSION-linux-x64.tar.gz"
    "$DIST_DIR/seen-$VERSION-linux-x64-v3.tar.gz"
    "$DIST_DIR/seen-lang_${VERSION}_amd64.deb"
    "$DIST_DIR/seen-lang-$VERSION-1.x86_64.rpm"
    "$DIST_DIR/seen-lang-devel-$VERSION-1.x86_64.rpm"
    "$DIST_DIR/seen-lang-docs-$VERSION-1.noarch.rpm"
    "$DIST_DIR/SeenLanguage-$VERSION-x86_64.AppImage"
    "$DIST_DIR/seen-compiler-$VERSION-linux-x64"
    "$DIST_DIR/seen-runtime-$VERSION-linux-x64.tar.gz"
    "$DIST_DIR/seen-stdlib-$VERSION-linux-x64.tar.gz"
    "$DIST_DIR/seen-pkg-$VERSION-linux-x64"
    "$DIST_DIR/seen-compiler-$VERSION-linux-x64.sha256"
    "$DIST_DIR/seen-compiler-$VERSION-linux-x64.bundle"
    "$DIST_DIR/seen-runtime-$VERSION-linux-x64.tar.gz.sha256"
    "$DIST_DIR/seen-runtime-$VERSION-linux-x64.tar.gz.bundle"
    "$DIST_DIR/seen-stdlib-$VERSION-linux-x64.tar.gz.sha256"
    "$DIST_DIR/seen-stdlib-$VERSION-linux-x64.tar.gz.bundle"
    "$DIST_DIR/seen-pkg-$VERSION-linux-x64.sha256"
    "$DIST_DIR/seen-pkg-$VERSION-linux-x64.bundle"
    "$DIST_DIR/seen-$VERSION-release-artifacts.json"
    "$DIST_DIR/seen-$VERSION-release-artifacts.json.sha256"
    "$DIST_DIR/seen-$VERSION-release-artifacts.json.bundle"
    "$DIST_DIR/seen-$VERSION-windows-x64.zip"
    "$DIST_DIR/seen-$VERSION-windows-x64.zip.sha256"
    "$DIST_DIR/Seen-$VERSION-windows-x64-setup.exe"
    "$DIST_DIR/Seen-$VERSION-windows-x64-setup.exe.sha256"
    "$DIST_DIR"/seen-"$VERSION"-macos-*.tar.gz
)
shopt -u nullglob
rm -f -- "${VERSION_OUTPUTS[@]}" "$DIST_DIR/SHA256SUMS" \
    "$DIST_DIR/SHA256SUMS.sha256" "$DIST_DIR/SHA256SUMS.bundle" \
    "$DIST_DIR/seen-lang.rb"

if [[ "$THREE_PLATFORMS" == 1 ]]; then
    cp -- "$PLATFORM_INPUT_DIR/seen-$VERSION-windows-x64.zip" \
        "$PLATFORM_INPUT_DIR/Seen-$VERSION-windows-x64-setup.exe" "$DIST_DIR/"
fi

if [[ -n "$MACOS_INPUT_DIR" ]]; then
    shopt -s nullglob
    MACOS_INPUTS=("$MACOS_INPUT_DIR"/seen-"$VERSION"-macos-*.tar.gz)
    shopt -u nullglob
    if [[ "${#MACOS_INPUTS[@]}" -eq 0 ]]; then
        die "No seen-$VERSION-macos-*.tar.gz archives found in $MACOS_INPUT_DIR"
    fi
    for artifact in "${MACOS_INPUTS[@]}"; do
        [[ -s "$artifact" ]] || die "macOS input artifact missing or empty: $artifact"
        cp -f -- "$artifact" "$DIST_DIR/"
    done
fi

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

if [[ "$THREE_PLATFORMS" == 1 ]]; then
    echo "Using exact-commit Windows ZIP and installer from the staged draft."
elif [[ "$SKIP_OPTIONAL_CROSS_BUILDS" == 0 ]] && command -v x86_64-w64-mingw32-gcc &>/dev/null; then
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

HOMEBREW_FORMULA=""
if [[ -f "$ROOT_DIR/installer/homebrew/generate-formula.sh" ]]; then
    echo ""
    if [[ "$THREE_PLATFORMS" == 1 ]]; then
        echo "Skipping Homebrew formula: no macOS x64 archive is certified."
    elif [[ "${SEEN_RELEASE_GENERATE_HOMEBREW:-0}" == "1" ]] ||
        compgen -G "$DIST_DIR/seen-$VERSION-macos-*.tar.gz" >/dev/null; then
        echo "=== Generating macOS Homebrew formula (v$VERSION)... ==="
        if bash "$ROOT_DIR/installer/homebrew/generate-formula.sh" \
            --version "$VERSION" \
            --output "$DIST_DIR/seen-lang.rb" 2>&1 | tail -5; then
            HOMEBREW_FORMULA="$DIST_DIR/seen-lang.rb"
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

if [[ "$THREE_PLATFORMS" == 1 ]]; then
    echo "macOS arm64 archive supplied by exact-commit staged draft."
elif [[ "$SKIP_OPTIONAL_CROSS_BUILDS" == 0 ]] &&
    (command -v o64-clang &>/dev/null || command -v x86_64-apple-darwin-clang &>/dev/null); then
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

EXPECTED_ARTIFACTS=(
    "$DIST_DIR/seen-$VERSION-linux-x64.tar.gz"
    "$DIST_DIR/seen-compiler-$VERSION-linux-x64"
    "$DIST_DIR/seen-runtime-$VERSION-linux-x64.tar.gz"
    "$DIST_DIR/seen-stdlib-$VERSION-linux-x64.tar.gz"
    "$DIST_DIR/seen-pkg-$VERSION-linux-x64"
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

if [[ "$THREE_PLATFORMS" != 1 && "$SKIP_OPTIONAL_CROSS_BUILDS" == 0 ]] &&
    command -v x86_64-w64-mingw32-gcc &>/dev/null &&
    [[ -f "$ROOT_DIR/target-windows/seen.exe" ]]; then
    EXPECTED_ARTIFACTS+=("$DIST_DIR/seen-$VERSION-windows-x64.zip")
    if command -v makensis &>/dev/null; then
        EXPECTED_ARTIFACTS+=("$DIST_DIR/Seen-$VERSION-windows-x64-setup.exe")
    fi
fi
if [[ "$THREE_PLATFORMS" == 1 ]]; then
    EXPECTED_ARTIFACTS+=(
        "$DIST_DIR/seen-$VERSION-macos-arm64.tar.gz"
        "$DIST_DIR/seen-$VERSION-windows-x64.zip"
        "$DIST_DIR/Seen-$VERSION-windows-x64-setup.exe"
    )
fi

require_artifacts "${EXPECTED_ARTIFACTS[@]}"
"$SCRIPT_DIR/verify_stdlib_component_payload.sh" \
    "$DIST_DIR/seen-stdlib-$VERSION-linux-x64.tar.gz"

# Refuse to publish if any Linux installer/package embeds a compiler other
# than the exact standalone component that will be signed below.
"$SCRIPT_DIR/verify_linux_delivery_compiler_identity.sh" \
    "$VERSION" "$DIST_DIR"

CHECKSUM_ARTIFACTS=("${EXPECTED_ARTIFACTS[@]}")

# macOS archives can only reach dist/ through the explicit cross-host input
# directory prepared above. Include only archives for this exact version.
for artifact in "$DIST_DIR"/seen-"$VERSION"-macos-*.tar.gz; do
    [[ -s "$artifact" ]] || continue
    CHECKSUM_ARTIFACTS+=("$artifact")
done

write_sidecar_checksum "$DIST_DIR/seen-$VERSION-windows-x64.zip"
write_sidecar_checksum "$DIST_DIR/Seen-$VERSION-windows-x64-setup.exe"

# Regenerate checksums to include all platforms
echo ""
echo "Regenerating checksums..."
write_checksum_manifest "${CHECKSUM_ARTIFACTS[@]}"
echo "  -> $DIST_DIR/SHA256SUMS"

RELEASE_ARTIFACTS=("${CHECKSUM_ARTIFACTS[@]}" "$DIST_DIR/SHA256SUMS")
for artifact in \
    "$DIST_DIR/seen-$VERSION-windows-x64.zip.sha256" \
    "$DIST_DIR/Seen-$VERSION-windows-x64-setup.exe.sha256"; do
    [[ -s "$artifact" ]] && RELEASE_ARTIFACTS+=("$artifact")
done
if [[ -n "$HOMEBREW_FORMULA" ]]; then
    RELEASE_ARTIFACTS+=("$HOMEBREW_FORMULA")
fi

if [[ "$DRY_RUN" == "1" ]]; then
    echo ""
    echo "=== Local release packaging dry run passed; signing and upload were not attempted. ==="
    ls -lh -- "${RELEASE_ARTIFACTS[@]}"
    exit 0
fi

COMPONENT_ARTIFACTS=(
    "$DIST_DIR/seen-compiler-$VERSION-linux-x64"
    "$DIST_DIR/seen-runtime-$VERSION-linux-x64.tar.gz"
    "$DIST_DIR/seen-stdlib-$VERSION-linux-x64.tar.gz"
    "$DIST_DIR/seen-pkg-$VERSION-linux-x64"
)
SOURCE_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"
SOURCE_DIGEST="$(git -C "$ROOT_DIR" archive --format=tar HEAD | sha256sum | awk '{print $1}')"
MANIFEST="$DIST_DIR/seen-$VERSION-release-artifacts.json"
SIGN_ARGS=()
case "$SIGN_MODE" in
    keyless) SIGN_ARGS+=(--keyless) ;;
    key)
        [[ -n "${SEEN_COSIGN_KEY:-}" ]] || die "SEEN_COSIGN_KEY is required for key signing"
        SIGN_ARGS+=(--key "$SEEN_COSIGN_KEY")
        ;;
    kms)
        [[ -n "${SEEN_COSIGN_KMS_URI:-}" ]] || die "SEEN_COSIGN_KMS_URI is required for KMS signing"
        SIGN_ARGS+=(--kms "$SEEN_COSIGN_KMS_URI")
        ;;
esac
if [[ "$THREE_PLATFORMS" == 1 ]]; then
    SIGN_ARGS+=(--checksum-list "$DIST_DIR/SHA256SUMS")
fi
"$SCRIPT_DIR/sign_release.sh" "${SIGN_ARGS[@]}" --version "$VERSION" \
    --source-commit "$SOURCE_COMMIT" --source-digest "$SOURCE_DIGEST" \
    --manifest "$MANIFEST" --signer-identity "$SIGN_IDENTITY" --signer-issuer "$SIGN_ISSUER" \
    --artifact compiler="${COMPONENT_ARTIFACTS[0]}" \
    --artifact runtime="${COMPONENT_ARTIFACTS[1]}" \
    --artifact stdlib="${COMPONENT_ARTIFACTS[2]}" \
    --artifact package-client="${COMPONENT_ARTIFACTS[3]}"
for artifact in "${COMPONENT_ARTIFACTS[@]}"; do
    RELEASE_ARTIFACTS+=("$artifact.sha256" "$artifact.bundle")
done
RELEASE_ARTIFACTS+=("$MANIFEST" "$MANIFEST.sha256" "$MANIFEST.bundle")

if [[ "$THREE_PLATFORMS" == 1 ]]; then
    # The four components and the cross-platform checksum list all use the
    # same bounded signing/retry/verification path.
    RELEASE_ARTIFACTS+=("$DIST_DIR/SHA256SUMS.sha256" "$DIST_DIR/SHA256SUMS.bundle")
fi

echo ""
echo "Artifacts:"
ls -lh -- "${RELEASE_ARTIFACTS[@]}"

# --- Create GitHub Release ---

echo ""
echo "=== Uploading to GitHub Releases... ==="

PRERELEASE_FLAG=""
if [[ "$VERSION" == *alpha* || "$VERSION" == *beta* || "$VERSION" == *rc* ]]; then
    PRERELEASE_FLAG="--prerelease"
fi

NOTES="## Seen Language $VERSION

### Highlights

- See CHANGELOG.md for the exact compiler and runtime changes in this version.
- The Linux x64, macOS arm64, and Windows x64 archives are bound to the same
  source commit. SHA256SUMS is signed by the release workflow identity.

### Installation

**Linux:**
\`\`\`bash
curl -sSL https://github.com/codeyousef/SeenLang/releases/download/$TAG/seen-${VERSION}-linux-x64.tar.gz | tar xz
cd seen-${VERSION}-linux-x64
pkexec ./install.sh
\`\`\`

\`linux-x64\` is the portable x86-64 baseline. Use \`seen-${VERSION}-linux-x64-v3.tar.gz\` only on x86-64-v3/AVX2-class machines.

The macOS arm64 archive and Windows x64 ZIP/installer are required assets.
The Homebrew formula is omitted until a macOS x64 archive is certified.

### Checksums

Download \`SHA256SUMS\` from this release and verify files with
\`sha256sum -c SHA256SUMS\`."

# Create a new release with only the exact version-scoped set assembled above.
# The read-only preflight rejected an existing release, and this command has no
# asset-overwrite mode. A race that creates the release first therefore fails.
CURRENT_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)" ||
    die "could not re-resolve the release commit"
seen_release_verify_published_tag \
    "$ROOT_DIR" "$TAG" "$CURRENT_COMMIT" "$RELEASE_REPOSITORY" ||
    die "release tag changed or remote main advanced during release preparation"
if [[ "$THREE_PLATFORMS" == 1 ]]; then
    assert_staged_draft
    UPLOAD_ARTIFACTS=()
    for artifact in "${RELEASE_ARTIFACTS[@]}"; do
        case "$(basename "$artifact")" in
            "seen-$VERSION-macos-arm64.tar.gz"|\
            "seen-$VERSION-windows-x64.zip"|\
            "Seen-$VERSION-windows-x64-setup.exe") continue ;;
        esac
        UPLOAD_ARTIFACTS+=("$artifact")
    done
    upload_args=()
    for artifact in "${UPLOAD_ARTIFACTS[@]}"; do
        upload_args+=(--file "$artifact")
    done
    python3 "$SCRIPT_DIR/release_draft_api.py" upload \
        --version "$VERSION" --release-id "$RELEASE_DRAFT_ID" \
        "${upload_args[@]}" || die "Could not upload the exact signed release artifacts"
    audit_dir="$ROOT_DIR/.seen/agent-tools/release-draft-audit/$VERSION"
    [[ ! -e "$audit_dir" ]] || die "draft audit directory already exists"
    mkdir -p "$audit_dir"
    expected_names=("seen-$VERSION-platform-inputs.json")
    expected_names+=("seen-$VERSION-macos-arm64.tar.gz")
    expected_names+=("seen-$VERSION-windows-x64.zip")
    expected_names+=("Seen-$VERSION-windows-x64-setup.exe")
    for artifact in "${RELEASE_ARTIFACTS[@]}"; do
        expected_names+=("$(basename "$artifact")")
    done
    mapfile -t sorted_expected_names < <(printf '%s\n' "${expected_names[@]}" | LC_ALL=C sort -u)
    audit_args=()
    for name in "${sorted_expected_names[@]}"; do
        audit_args+=(--expected-name "$name")
    done
    python3 "$SCRIPT_DIR/release_draft_api.py" download-all \
        --version "$VERSION" --release-id "$RELEASE_DRAFT_ID" \
        --output-dir "$audit_dir" "${audit_args[@]}" ||
        die "Could not download and verify the complete staged release"
    mapfile -t actual_names < <(find "$audit_dir" -maxdepth 1 -type f -printf '%f\n' | LC_ALL=C sort)
    [[ "${actual_names[*]}" == "${sorted_expected_names[*]}" ]] ||
        die "draft release asset set differs from the complete required set"
    cmp -- "$PLATFORM_INPUT_DIR/seen-$VERSION-platform-inputs.json" \
        "$audit_dir/seen-$VERSION-platform-inputs.json" || die "draft provenance manifest changed"
    for artifact in "${RELEASE_ARTIFACTS[@]}"; do
        cmp -- "$artifact" "$audit_dir/$(basename "$artifact")" ||
            die "draft asset changed: $(basename "$artifact")"
    done
    python3 "$SCRIPT_DIR/release_draft_api.py" publish \
        --version "$VERSION" --release-id "$RELEASE_DRAFT_ID" \
        "${audit_args[@]}" --title "Seen Language $VERSION" \
        --notes "$NOTES" $PRERELEASE_FLAG ||
        die "Could not publish the fully audited three-platform draft"
else
    assert_release_absent
    gh release create "$TAG" "${RELEASE_ARTIFACTS[@]}" \
        --repo "$RELEASE_REPOSITORY" \
        --verify-tag \
        --title "Seen Language $VERSION" \
        --notes "$NOTES" \
        $PRERELEASE_FLAG
fi

echo ""
echo "=== Done! ==="
echo "Release: https://github.com/codeyousef/SeenLang/releases/tag/$TAG"
