#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_DIR="$(mktemp -d /tmp/seen_import_c_typedefs.XXXXXX)"
HEADER="$TMP_DIR/typedefs.h"
OUT="$TMP_DIR/typedefs_bindings.seen"

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT

cat >"$HEADER" <<'EOF'
typedef unsigned int MyFlags;
typedef struct MyHandle_T* MyHandle;
typedef struct MyInfo MyInfo;
typedef void (*MyCallback)(MyHandle h, const MyInfo* info);

MyFlags do_thing(MyHandle handle, MyFlags flags, MyCallback callback, const MyInfo* info, MyHandle* out_handle);
EOF

"$COMPILER" import-c "$HEADER" | sed -n '/^\/\/ Auto-generated/,$p' >"$OUT"

if [ "$(grep -c '^type MyFlags = Int$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit MyFlags typedef alias"
    exit 1
fi

if [ "$(grep -c '^type MyHandle = Ptr$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit MyHandle opaque pointer alias"
    exit 1
fi

if [ "$(grep -c '^type MyInfo = Ptr$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit MyInfo opaque record alias"
    exit 1
fi

if [ "$(grep -c '^type MyCallback = Ptr$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit MyCallback function-pointer alias"
    exit 1
fi

if [ "$(grep -c '^extern fun do_thing(arg0: MyHandle, arg1: MyFlags, arg2: MyCallback, arg3: MyInfo, arg4: \*MyHandle) r: MyFlags$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should reuse typedef aliases in generated function signatures"
    exit 1
fi

cat >>"$OUT" <<'EOF'

fun main() r: Void {
}
EOF

"$COMPILER" check "$OUT" >/dev/null

echo "PASS: import-c emits typedef aliases and reuses them in signatures"
