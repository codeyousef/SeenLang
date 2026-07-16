#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_release_upload_scope.XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

FIXTURE_ROOT="$TMP_DIR/repo"
FIXTURE_BIN="$FIXTURE_ROOT/bin"
MIN_PATH="$TMP_DIR/path"
OPTIONAL_PATH="$TMP_DIR/optional-path"
DIST_DIR="$FIXTURE_ROOT/dist"
STALE_ARTIFACT="$DIST_DIR/seen-0.8.0-linux-x64.tar.gz"
CURRENT_ARTIFACT="$DIST_DIR/seen-0.10.0-linux-x64.tar.gz"
STALE_OPTIONAL_ARTIFACT="$DIST_DIR/seen-lang_0.10.0_amd64.deb"
IMPLICIT_MACOS_ARTIFACT="$DIST_DIR/seen-0.10.0-macos-arm64.tar.gz"

mkdir -p "$FIXTURE_ROOT/scripts" "$FIXTURE_BIN" "$MIN_PATH" "$OPTIONAL_PATH" "$DIST_DIR"
cp "$ROOT_DIR/scripts/build_and_upload_release.sh" "$FIXTURE_ROOT/scripts/"

cat > "$FIXTURE_BIN/seen" <<'SEEN_EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "build" ]]; then
    exit 0
fi
exit 2
SEEN_EOF

cat > "$FIXTURE_BIN/seen-pkg" <<'PKG_EOF'
#!/usr/bin/env bash
exit 0
PKG_EOF

cat > "$FIXTURE_ROOT/scripts/build_release.sh" <<'BUILD_EOF'
#!/usr/bin/env bash
set -euo pipefail
version=""
output_dir=""
artifact_suffix=""
while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --version)
            version="$2"
            shift 2
            ;;
        --output-dir)
            output_dir="$2"
            shift 2
            ;;
        --artifact-suffix)
            artifact_suffix="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done
mkdir -p "$output_dir"
printf 'current release artifact\n' > "$output_dir/seen-$version-$artifact_suffix.tar.gz"
BUILD_EOF

cat > "$FIXTURE_BIN/git" <<'GIT_EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "-C" ]]; then
    shift 2
fi
case "${1:-}" in
    rev-parse|rev-list)
        printf 'fixture-commit\n'
        ;;
    push|tag)
        ;;
    *)
        echo "unexpected git invocation: $*" >&2
        exit 2
        ;;
esac
GIT_EOF

cat > "$FIXTURE_BIN/gh" <<'GH_EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-} ${2:-}" in
    "auth status")
        exit 0
        ;;
    "release view")
        exit "${GH_RELEASE_VIEW_STATUS:?}"
        ;;
    "release upload"|"release create")
        printf '%s\0' "$@" > "${GH_CAPTURE:?}"
        ;;
    *)
        echo "unexpected gh invocation: $*" >&2
        exit 2
        ;;
esac
GH_EOF

chmod 755 \
    "$FIXTURE_BIN/seen" \
    "$FIXTURE_BIN/seen-pkg" \
    "$FIXTURE_BIN/git" \
    "$FIXTURE_BIN/gh" \
    "$FIXTURE_ROOT/scripts/build_release.sh"

for tool in bash basename cp dirname ls mkdir mktemp rm sha256sum sort; do
    ln -s "$(command -v "$tool")" "$MIN_PATH/$tool"
done

cat > "$OPTIONAL_PATH/dpkg-deb" <<'DPKG_EOF'
#!/usr/bin/env bash
exit 0
DPKG_EOF
chmod 755 "$OPTIONAL_PATH/dpkg-deb"

assert_checksum_scope() {
    local expected_macos="${1:-}"
    if ! grep -Fq 'seen-0.10.0-linux-x64.tar.gz' "$DIST_DIR/SHA256SUMS"; then
        echo "SHA256SUMS omitted the current release artifact" >&2
        exit 1
    fi
    if grep -Fq 'seen-0.8.0-linux-x64.tar.gz' "$DIST_DIR/SHA256SUMS"; then
        echo "SHA256SUMS included a stale release artifact" >&2
        exit 1
    fi
    if [[ -z "$expected_macos" ]] && grep -Fq "$(basename "$IMPLICIT_MACOS_ARTIFACT")" "$DIST_DIR/SHA256SUMS"; then
        echo "SHA256SUMS included an implicit stale macOS artifact" >&2
        exit 1
    fi
    if [[ -n "$expected_macos" ]] && ! grep -Fq "$(basename "$expected_macos")" "$DIST_DIR/SHA256SUMS"; then
        echo "SHA256SUMS omitted the explicit macOS input" >&2
        exit 1
    fi
}

assert_release_args() {
    local capture="$1"
    local expected_action="$2"
    local expected_macos="${3:-}"
    local arg saw_current=0 saw_checksums=0 saw_macos=0
    local -a args=()

    mapfile -d '' -t args < "$capture"
    if [[ "${args[0]:-}" != "release" || "${args[1]:-}" != "$expected_action" ||
        "${args[2]:-}" != "v0.10.0" ]]; then
        echo "unexpected gh release $expected_action prefix" >&2
        exit 1
    fi

    for arg in "${args[@]}"; do
        if [[ "$arg" == "$STALE_ARTIFACT" || "$arg" == *seen-0.8.0-linux-x64.tar.gz* ]]; then
            echo "gh release $expected_action included a stale release artifact" >&2
            exit 1
        fi
        if [[ "$arg" == "$STALE_OPTIONAL_ARTIFACT" ]]; then
            echo "gh release $expected_action included a stale same-version optional artifact" >&2
            exit 1
        fi
        [[ "$arg" == "$CURRENT_ARTIFACT" ]] && saw_current=1
        [[ "$arg" == "$DIST_DIR/SHA256SUMS" ]] && saw_checksums=1
        [[ -n "$expected_macos" && "$arg" == "$expected_macos" ]] && saw_macos=1
        if [[ "$arg" == "$DIST_DIR/"*.tar.gz && "$arg" != "$CURRENT_ARTIFACT" && "$arg" != "$expected_macos" ]]; then
            echo "gh release $expected_action included an unexpected tarball: $arg" >&2
            exit 1
        fi
    done

    if [[ "$saw_current" -ne 1 || "$saw_checksums" -ne 1 ]]; then
        echo "gh release $expected_action omitted the scoped artifact set" >&2
        exit 1
    fi
    if [[ -n "$expected_macos" && "$saw_macos" -ne 1 ]]; then
        echo "gh release $expected_action omitted the explicit macOS input" >&2
        exit 1
    fi
}

run_release_case() {
    local view_status="$1"
    local action="$2"
    local capture="$TMP_DIR/gh-$action.args"

    printf 'stale release artifact\n' > "$STALE_ARTIFACT"
    printf 'implicit stale macOS artifact\n' > "$IMPLICIT_MACOS_ARTIFACT"
    PATH="$FIXTURE_BIN:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_VIEW_STATUS="$view_status" \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.0 >/dev/null

    assert_checksum_scope
    assert_release_args "$capture" "$action"
}

run_failed_optional_case() {
    local capture="$TMP_DIR/gh-failed-optional.args"
    local output status

    printf 'stale release artifact\n' > "$STALE_ARTIFACT"
    printf 'stale same-version optional artifact\n' > "$STALE_OPTIONAL_ARTIFACT"
    printf 'stale checksums\n' > "$DIST_DIR/SHA256SUMS"
    printf 'stale formula\n' > "$DIST_DIR/seen-lang.rb"
    printf 'stale sidecar\n' > "$DIST_DIR/seen-0.10.0-windows-x64.zip.sha256"
    set +e
    output="$(PATH="$FIXTURE_BIN:$OPTIONAL_PATH:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_VIEW_STATUS=0 \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.0 2>&1)"
    status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        echo "stale same-version optional artifact satisfied the release gate" >&2
        exit 1
    fi
    if [[ -e "$STALE_OPTIONAL_ARTIFACT" ]]; then
        echo "release preparation retained a stale same-version optional artifact" >&2
        exit 1
    fi
    if [[ ! -s "$STALE_ARTIFACT" ]]; then
        echo "version-scoped cleanup removed an unrelated release artifact" >&2
        exit 1
    fi
    for generated in \
        "$DIST_DIR/SHA256SUMS" \
        "$DIST_DIR/seen-lang.rb" \
        "$DIST_DIR/seen-0.10.0-windows-x64.zip.sha256"; do
        if [[ -e "$generated" ]]; then
            echo "release preparation retained stale generated output: $generated" >&2
            exit 1
        fi
    done
    if ! grep -Fq "required release artifact missing or empty: $STALE_OPTIONAL_ARTIFACT" <<<"$output"; then
        echo "$output" >&2
        echo "release did not fail closed on the skipped optional artifact" >&2
        exit 1
    fi
    if [[ -e "$capture" ]]; then
        echo "release reached gh with a failed optional artifact build" >&2
        exit 1
    fi
}

run_explicit_macos_case() {
    local input_dir="$TMP_DIR/macos-input"
    local input_artifact="$input_dir/seen-0.10.0-macos-arm64.tar.gz"
    local copied_artifact="$DIST_DIR/$(basename "$input_artifact")"
    local capture="$TMP_DIR/gh-explicit-macos.args"

    mkdir -p "$input_dir"
    printf 'explicit macOS release artifact\n' > "$input_artifact"
    PATH="$FIXTURE_BIN:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_VIEW_STATUS=0 \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_MACOS_INPUT_DIR="$input_dir" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.0 >/dev/null

    if [[ ! -s "$input_artifact" || ! -s "$copied_artifact" ]]; then
        echo "explicit macOS input was not preserved and staged" >&2
        exit 1
    fi
    assert_checksum_scope "$copied_artifact"
    assert_release_args "$capture" upload "$copied_artifact"
}

run_failed_optional_case
run_release_case 0 upload
run_release_case 1 create
run_explicit_macos_case

echo "release upload artifact scope test passed"
