#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-import-c-unions
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-import-c-unions.XXXXXX")"
HEADER="$TMP_DIR/unions.h"
OUT="$TMP_DIR/unions_bindings.seen"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-import-c-unions.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR" ;;
        *) return 1 ;;
    esac
}

trap cleanup EXIT

cat >"$HEADER" <<'EOF'
typedef union MyValue {
    int i;
    float f;
    void *ptr;
} MyValue;

typedef struct Wrapper {
    MyValue value;
    MyValue *value_ptr;
} Wrapper;

void use_union(MyValue value, MyValue *out_value, Wrapper *wrapper);
EOF

bash "$ATTESTED_SEEN" "$COMPILER" import-c "$HEADER" | \
    sed -n '/^\/\/ Auto-generated/,$p' >"$OUT"

if [ "$(grep -Fxc '@union' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit a Seen union annotation for C unions"
    exit 1
fi

if [ "$(grep -Fxc 'class MyValue {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit a union class for MyValue"
    exit 1
fi

if [ "$(grep -c '^    var i: Int32$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve integer union members"
    exit 1
fi

if [ "$(grep -c '^    var f: Float32$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve float union members"
    exit 1
fi

if [ "$(grep -c '^    var ptr: \*Void$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve pointer union members"
    exit 1
fi

WRAPPER_BLOCK="$(sed -n '/^class Wrapper {$/,/^}$/p' "$OUT")"

if ! printf '%s\n' "$WRAPPER_BLOCK" | grep -Fqx '    var value: MyValue'; then
    echo "FAIL: import-c should preserve union value fields inside repr(C) records"
    exit 1
fi

if ! printf '%s\n' "$WRAPPER_BLOCK" | grep -Fqx '    var value_ptr: *MyValue'; then
    echo "FAIL: import-c should preserve typed pointers to imported unions"
    exit 1
fi

if [ "$(grep -c '^extern fun use_union(arg0: MyValue, arg1: \*MyValue, arg2: \*Wrapper) r: Void$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should reuse imported union types in generated function signatures"
    exit 1
fi

cat >>"$OUT" <<'EOF'

fun main() r: Void {
}
EOF

bash "$ATTESTED_SEEN" "$COMPILER" check "$OUT" >/dev/null

echo "PASS: import-c emits union layouts and preserves union-backed signatures"
