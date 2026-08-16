#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-import-c-enums
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-import-c-enums.XXXXXX")"
HEADER="$TMP_DIR/import_enums.h"
OUT="$TMP_DIR/import.out"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-import-c-enums.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR" ;;
        *) return 1 ;;
    esac
}
trap cleanup EXIT

mkdir -p "$TMP_DIR"

cat >"$HEADER" <<'EOF'
enum VkResultLike {
    VK_FOO = 0,
    VK_BAR = -4,
    VK_BIG = 1000001003,
};
EOF

bash "$ATTESTED_SEEN" "$COMPILER" import-c "$HEADER" >"$OUT"

grep -q '^let VK_FOO: Int = 0$' "$OUT" || {
    echo "FAIL: import-c did not emit VK_FOO enum constant"
    exit 1
}

grep -q '^let VK_BAR: Int = -4$' "$OUT" || {
    echo "FAIL: import-c did not emit VK_BAR enum constant"
    exit 1
}

grep -q '^let VK_BIG: Int = 1000001003$' "$OUT" || {
    echo "FAIL: import-c did not emit VK_BIG enum constant"
    exit 1
}

echo "PASS: import-c emits enum constants"
