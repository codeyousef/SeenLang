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
typedef float MyRgba[4];
typedef struct MyInfo {
    MyFlags flags;
    MyHandle handle;
    const char* name;
    MyRgba rgba;
} MyInfo;
typedef void (*MyCallback)(MyHandle h, const MyInfo* info);

MyFlags do_thing(MyHandle handle, MyFlags flags, MyCallback callback, const MyInfo* info, MyHandle* out_handle);
EOF

"$COMPILER" import-c "$HEADER" | sed -n '/^\/\/ Auto-generated/,$p' >"$OUT"

if [ "$(grep -c '^type MyFlags = UInt32$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit MyFlags typedef alias"
    exit 1
fi

if [ "$(grep -c '^type MyHandle = Ptr$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit MyHandle opaque pointer alias"
    exit 1
fi

if [ "$(grep -c '^type MyRgba = Float32\[4\]$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit fixed-array typedef aliases"
    exit 1
fi

if [ "$(grep -Fxc 'class MyInfo {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit a repr(C) class for MyInfo"
    exit 1
fi

if [ "$(grep -Fxc 'type MyCallback = fn(MyHandle, *MyInfo) -> Void' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit a typed MyCallback function-pointer alias"
    exit 1
fi

if [ "$(grep -c '^    var flags: MyFlags$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve typedef aliases in generated struct fields"
    exit 1
fi

if [ "$(grep -c '^    var handle: MyHandle$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve opaque handle aliases in generated struct fields"
    exit 1
fi

if [ "$(grep -c '^    var name: \*Char$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit raw pointer fields for C string members"
    exit 1
fi

if [ "$(grep -c '^    var rgba: MyRgba$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve fixed-array typedef aliases in generated struct fields"
    exit 1
fi

if [ "$(grep -c '^extern fun do_thing(arg0: MyHandle, arg1: MyFlags, arg2: MyCallback, arg3: \*MyInfo, arg4: \*MyHandle) r: MyFlags$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should reuse typedefs and record pointers in generated function signatures"
    exit 1
fi

cat >>"$OUT" <<'EOF'

fun main() r: Void {
}
EOF

"$COMPILER" compile "$OUT" "$TMP_DIR/typedefs_probe" --fast >/dev/null

echo "PASS: import-c emits typedef aliases, repr(C) records, and reused signature types"
