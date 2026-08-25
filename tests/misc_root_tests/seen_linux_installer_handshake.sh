#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
# shellcheck source=scripts/artifact_root.sh
source "$ROOT_DIR/scripts/artifact_root.sh"
seen_artifact_root_init "$ROOT_DIR"
TEST_SCOPE=$(seen_artifact_scope_init linux-installer-handshake-test)
TEST_ROOT=$(seen_artifact_mktemp_dir "$TEST_SCOPE" fixture)

cleanup_test_root() {
    case "${TEST_ROOT:-}" in
        "$TEST_SCOPE"/fixture.*)
            if [ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ]; then
                rm -rf -- "$TEST_ROOT"
            fi
            ;;
        "") ;;
        *) printf 'refusing to clean unexpected test root: %s\n' "$TEST_ROOT" >&2 ;;
    esac
}
trap cleanup_test_root EXIT

SOURCE_DIR="$TEST_ROOT/source files"
FAKE_BIN="$TEST_ROOT/fake tools"
OUTPUT_DIR="$TEST_ROOT/output files"
PACKAGER_LOG="$TEST_ROOT/packager.log"
PACKAGE_CLIENT_LOG="$TEST_ROOT/package-client.log"
BUILDER_ARTIFACT_ROOT="$TEST_ROOT/builder artifacts"
mkdir -p "$SOURCE_DIR" "$FAKE_BIN" "$OUTPUT_DIR" "$BUILDER_ARTIFACT_ROOT"
cp "$ROOT_DIR/releases/compatibility-manifest.json" \
    "$SOURCE_DIR/compatibility-manifest.json"

cat > "$SOURCE_DIR/seen" <<'COMPILER_EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--version" && $# -eq 1 ]]; then
    printf 'Seen %s\nLanguage: Seen\nEntrypoint: fixture\n' \
        "${FAKE_COMPILER_VERSION:-0.12.0}"
    exit 0
fi
exit 2
COMPILER_EOF

cat > "$SOURCE_DIR/seen-pkg" <<'PACKAGE_CLIENT_EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$SEEN_INSTALLER_PACKAGE_CLIENT_LOG"
if [[ "${1:-}" != "--expect-version" || "${2:-}" != "0.12.0" ||
      "${3:-}" != "version" || "${4:-}" != "--machine" || $# -ne 4 ]]; then
    exit 64
fi
if [[ "${FAKE_PACKAGE_CLIENT_FAILURE:-0}" == "1" ]]; then
    exit 1
fi
printf 'protocol=%s\nversion=%s\n' \
    "${FAKE_PACKAGE_PROTOCOL:-SEENPKG1}" \
    "${FAKE_PACKAGE_VERSION:-0.12.0}"
PACKAGE_CLIENT_EOF

cat > "$FAKE_BIN/package-tool" <<'PACKAGE_TOOL_EOF'
#!/usr/bin/env bash
printf '%s\n' "$(basename "$0")" >> "$SEEN_INSTALLER_PACKAGER_LOG"
exit 73
PACKAGE_TOOL_EOF

cat > "$FAKE_BIN/convert" <<'CONVERT_EOF'
#!/usr/bin/env bash
output="${!#}"
: > "$output"
CONVERT_EOF

chmod +x "$SOURCE_DIR/seen" "$SOURCE_DIR/seen-pkg" \
    "$FAKE_BIN/package-tool" "$FAKE_BIN/convert"
for tool in dpkg-deb rpmbuild appimagetool; do
    ln -s package-tool "$FAKE_BIN/$tool"
done

export SEEN_INSTALLER_PACKAGER_LOG="$PACKAGER_LOG"
export SEEN_INSTALLER_PACKAGE_CLIENT_LOG="$PACKAGE_CLIENT_LOG"

assert_no_packager_call() {
    if [ -s "$PACKAGER_LOG" ]; then
        printf 'packager was invoked before source validation completed:\n' >&2
        cat "$PACKAGER_LOG" >&2
        exit 1
    fi
}

assert_handshake_invocation() {
    if ! grep -Fxq -- '--expect-version 0.12.0 version --machine' \
        "$PACKAGE_CLIENT_LOG"; then
        printf 'package-client handshake did not use the required arguments\n' >&2
        cat "$PACKAGE_CLIENT_LOG" >&2
        exit 1
    fi
}

assert_no_staging_directory() {
    local leftover
    leftover=$(find "$BUILDER_ARTIFACT_ROOT" -type d -name 'package.*' \
        -print -quit)
    if [ -n "$leftover" ]; then
        printf 'installer left its staging directory behind: %s\n' "$leftover" >&2
        exit 1
    fi
}

run_builder_failure() {
    local builder=$1
    local arch=$2
    local expected=$3
    local log_file=$4
    shift 4

    if env \
        PATH="$FAKE_BIN:$PATH" \
        SEEN_ARTIFACT_ROOT="$BUILDER_ARTIFACT_ROOT" \
        "$@" \
        bash "$ROOT_DIR/installer/linux/$builder" \
        0.12.0 "$arch" \
        --source-dir "$SOURCE_DIR" \
        --output-dir "$OUTPUT_DIR" >"$log_file" 2>&1; then
        printf '%s unexpectedly succeeded\n' "$builder" >&2
        exit 1
    fi
    if ! grep -Fq -- "$expected" "$log_file"; then
        printf '%s did not report expected failure: %s\n' "$builder" "$expected" >&2
        cat "$log_file" >&2
        exit 1
    fi
}

case_number=0
for builder_arch in \
    'build-deb.sh amd64 dpkg-deb' \
    'build-rpm.sh x86_64 rpmbuild' \
    'build-appimage.sh x86_64 appimagetool'; do
    read -r builder arch package_tool <<< "$builder_arch"

    case_number=$((case_number + 1))
    : > "$PACKAGER_LOG"
    : > "$PACKAGE_CLIENT_LOG"
    run_builder_failure "$builder" "$arch" \
        'does not match package version 0.12.0' \
        "$TEST_ROOT/case-$case_number.log" \
        FAKE_COMPILER_VERSION=9.9.9
    assert_no_packager_call
    if [ -s "$PACKAGE_CLIENT_LOG" ]; then
        printf '%s invoked seen-pkg after a compiler version mismatch\n' "$builder" >&2
        exit 1
    fi

    case_number=$((case_number + 1))
    : > "$PACKAGER_LOG"
    : > "$PACKAGE_CLIENT_LOG"
    run_builder_failure "$builder" "$arch" \
        'returned an invalid version handshake' \
        "$TEST_ROOT/case-$case_number.log" \
        FAKE_PACKAGE_PROTOCOL=SEENPKG9
    assert_no_packager_call
    assert_handshake_invocation

    case_number=$((case_number + 1))
    : > "$PACKAGER_LOG"
    : > "$PACKAGE_CLIENT_LOG"
    run_builder_failure "$builder" "$arch" \
        'does not match Seen 0.12.0' \
        "$TEST_ROOT/case-$case_number.log" \
        FAKE_PACKAGE_CLIENT_FAILURE=1
    assert_no_packager_call
    assert_handshake_invocation

    case_number=$((case_number + 1))
    : > "$PACKAGER_LOG"
    : > "$PACKAGE_CLIENT_LOG"
    run_builder_failure "$builder" "$arch" \
        'returned an invalid version handshake' \
        "$TEST_ROOT/case-$case_number.log" \
        FAKE_PACKAGE_VERSION=9.9.9
    assert_no_packager_call
    assert_handshake_invocation

    case_number=$((case_number + 1))
    : > "$PACKAGER_LOG"
    : > "$PACKAGE_CLIENT_LOG"
    run_builder_failure "$builder" "$arch" \
        "Configuration:" \
        "$TEST_ROOT/case-$case_number.log"
    assert_handshake_invocation
    if ! grep -Fxq -- "$package_tool" "$PACKAGER_LOG"; then
        printf '%s did not reach its mocked packaging tool\n' "$builder" >&2
        cat "$PACKAGER_LOG" >&2
        exit 1
    fi
    assert_no_staging_directory
done

if find "$OUTPUT_DIR" -type f -print -quit | grep -q .; then
    printf 'mocked installer validation unexpectedly produced a package\n' >&2
    exit 1
fi

printf 'Linux installer version handshake and artifact-scope regression passed\n'
