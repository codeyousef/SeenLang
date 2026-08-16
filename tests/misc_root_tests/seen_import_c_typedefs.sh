#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-import-c-typedefs
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-import-c-typedefs.XXXXXX")"
HEADER="$TMP_DIR/typedefs.h"
OUT="$TMP_DIR/typedefs_bindings.seen"

cleanup() {
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-import-c-typedefs.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR" ;;
        *) return 1 ;;
    esac
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

bash "$ATTESTED_SEEN" "$COMPILER" import-c "$HEADER" | \
    sed -n '/^\/\/ Auto-generated/,$p' >"$OUT"

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

bash "$ATTESTED_SEEN" "$COMPILER" compile \
    "$OUT" "$TMP_DIR/typedefs_probe" --fast >/dev/null

echo "PASS: import-c emits typedef aliases, repr(C) records, and reused signature types"
