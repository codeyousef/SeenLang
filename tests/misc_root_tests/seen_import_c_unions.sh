#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_DIR="$(mktemp -d /tmp/seen_import_c_unions.XXXXXX)"
HEADER="$TMP_DIR/unions.h"
OUT="$TMP_DIR/unions_bindings.seen"

cleanup() {
    rm -rf "$TMP_DIR"
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

"$COMPILER" import-c "$HEADER" | sed -n '/^\/\/ Auto-generated/,$p' >"$OUT"

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

"$COMPILER" check "$OUT" >/dev/null

echo "PASS: import-c emits union layouts and preserves union-backed signatures"
