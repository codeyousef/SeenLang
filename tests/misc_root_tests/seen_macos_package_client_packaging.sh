#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-macos-package-client-packaging
if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" --platform "$SCOPE" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-platform-active "$SCOPE"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-macos-package-client.XXXXXX")"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-macos-package-client.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) return 1 ;;
    esac
}
trap cleanup EXIT

FIXTURE_ROOT="$TMP_DIR/repo"
FAKE_BIN="$TMP_DIR/bin"
MARKER="$TMP_DIR/pkgbuild-verified"
mkdir -p \
    "$FIXTURE_ROOT/installer/macos" \
    "$FIXTURE_ROOT/scripts" \
    "$FIXTURE_ROOT/compiler_seen/target" \
    "$FIXTURE_ROOT/seen_std/src" \
    "$FIXTURE_ROOT/languages/en" \
    "$FIXTURE_ROOT/seen_runtime" \
    "$FAKE_BIN"
cp "$ROOT_DIR/installer/macos/build-pkg.sh" \
    "$FIXTURE_ROOT/installer/macos/build-pkg.sh"
cp "$ROOT_DIR/scripts/artifact_root.sh" \
    "$FIXTURE_ROOT/scripts/artifact_root.sh"

cat > "$FIXTURE_ROOT/compiler_seen/target/seen" <<'COMPILER_EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--version" ]]; then
    echo 'Seen 0.14.0'
    exit 0
fi
exit 2
COMPILER_EOF

cat > "$FIXTURE_ROOT/compiler_seen/target/seen-pkg" <<'HELPER_EOF'
#!/usr/bin/env bash
if [[ "$*" == '--expect-version 0.14.0 version --machine' ]]; then
    printf 'protocol=SEENPKG1\nversion=0.14.0\n'
    exit 0
fi
exit 2
HELPER_EOF

cat > "$FAKE_BIN/file" <<'FILE_EOF'
#!/usr/bin/env bash
echo "$1: Mach-O 64-bit executable arm64"
FILE_EOF

cat > "$FAKE_BIN/pkgbuild" <<'PKGBUILD_EOF'
#!/usr/bin/env bash
set -euo pipefail
root=""
output=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --root) root="$2"; shift 2 ;;
        --identifier|--version|--scripts|--install-location) shift 2 ;;
        *) output="$1"; shift ;;
    esac
done
[[ -x "$root/usr/local/bin/seen" ]]
[[ -x "$root/usr/local/bin/seen-pkg" ]]
[[ -x "$root/usr/local/bin/seen-lsp" ]]
touch "$SEEN_MACOS_FIXTURE_MARKER"
touch "$output"
PKGBUILD_EOF

cat > "$FAKE_BIN/productbuild" <<'PRODUCTBUILD_EOF'
#!/usr/bin/env bash
set -euo pipefail
output="${!#}"
touch "$output"
PRODUCTBUILD_EOF

chmod +x \
    "$FIXTURE_ROOT/installer/macos/build-pkg.sh" \
    "$FIXTURE_ROOT/compiler_seen/target/seen" \
    "$FIXTURE_ROOT/compiler_seen/target/seen-pkg" \
    "$FAKE_BIN/file" "$FAKE_BIN/pkgbuild" "$FAKE_BIN/productbuild"
touch "$FIXTURE_ROOT/seen_std/src/value.seen"
touch "$FIXTURE_ROOT/languages/en/keywords.toml"
touch "$FIXTURE_ROOT/seen_runtime/seen_runtime.h"

PATH="$SEEN_BOUNDED_TOOLCHAIN_DIR:$FAKE_BIN:$PATH" \
SEEN_ARTIFACT_ROOT="$FIXTURE_ROOT/.seen/agent-tools" \
SEEN_MACOS_FIXTURE_MARKER="$MARKER" \
VERSION=0.14.0 \
bash "$FIXTURE_ROOT/installer/macos/build-pkg.sh" \
    "$FIXTURE_ROOT/compiler_seen/target/seen" >/dev/null

[[ -f "$MARKER" ]]
[[ -f "$FIXTURE_ROOT/installer/macos/seen-0.14.0-macos-arm64.pkg" ]]

cat > "$TMP_DIR/wrong-seen-pkg" <<'WRONG_HELPER_EOF'
#!/usr/bin/env bash
printf 'protocol=SEENPKG1\nversion=9.9.9\n'
exit 0
WRONG_HELPER_EOF
chmod +x "$TMP_DIR/wrong-seen-pkg"

if PATH="$SEEN_BOUNDED_TOOLCHAIN_DIR:$FAKE_BIN:$PATH" \
    SEEN_ARTIFACT_ROOT="$FIXTURE_ROOT/.seen/agent-tools" \
    SEEN_MACOS_FIXTURE_MARKER="$MARKER" \
    SEEN_PACKAGE_CLIENT_BIN="$TMP_DIR/wrong-seen-pkg" \
    VERSION=0.14.0 \
    bash "$FIXTURE_ROOT/installer/macos/build-pkg.sh" \
        "$FIXTURE_ROOT/compiler_seen/target/seen" >/dev/null 2>&1; then
    echo "macOS installer accepted a mismatched package client" >&2
    exit 1
fi

if PATH="$SEEN_BOUNDED_TOOLCHAIN_DIR:$FAKE_BIN:$PATH" \
    SEEN_ARTIFACT_ROOT="$FIXTURE_ROOT/.seen/agent-tools" \
    SEEN_MACOS_FIXTURE_MARKER="$MARKER" \
    VERSION=0.9.0 \
    bash "$FIXTURE_ROOT/installer/macos/build-pkg.sh" \
        "$FIXTURE_ROOT/compiler_seen/target/seen" >/dev/null 2>&1; then
    echo "macOS installer accepted mismatched compiler metadata" >&2
    exit 1
fi

if rg -q 'seen build ' "$ROOT_DIR/installer/macos/build-pkg.sh"; then
    echo "macOS installer still advertises the removed seen build command" >&2
    exit 1
fi

echo "macOS package-client packaging regression passed"
