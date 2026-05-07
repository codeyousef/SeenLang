#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_DIR="/tmp/seen_ir_call_shape_preflight"
GOOD_IR="$TMP_DIR/good.ll"
BAD_IR="$TMP_DIR/bad.ll"
BAD_LOG="$TMP_DIR/bad.log"

rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

cat >"$GOOD_IR" <<'IR'
%SeenString = type { i64, ptr }

declare void @takes_i64(i64)
declare void @takes_ptr(ptr)
declare void @takes_string(%SeenString)
declare %SeenString @string_from_int(i64)
declare void @vararg_ok(ptr, ...)

define void @ok(ptr %0, i64 %1, %SeenString %2) {
entry:
  call void @takes_i64(i64 %1)
  call void @takes_ptr(ptr %0)
  call void @takes_string(%SeenString %2)
  %3 = call %SeenString @string_from_int(i64 42)
  call void @takes_string(%SeenString %3)
  call void @vararg_ok(ptr null, i64 7, %SeenString zeroinitializer)
  ret void
}
IR

python3 "$ROOT_DIR/scripts/verify_ir_call_shapes.py" "$GOOD_IR"

cat >"$BAD_IR" <<'IR'
%SeenString = type { i64, ptr }

declare void @takes_i64(i64)
declare void @takes_ptr(ptr)
declare void @takes_string(%SeenString)
declare %SeenString @returns_string(i64)
declare i64 @returns_i64()
declare void @takes_two(i64, i64)

define void @bad(ptr %0, i64 %1, %SeenString %2) {
entry:
  call void @takes_ptr(%SeenString %2)
  call void @takes_string(i64 %1)
  %3 = call i64 @returns_string(i64 %1)
  %4 = call %SeenString @returns_i64()
  call void @takes_two(i64 1)
  ret void
}
IR

if python3 "$ROOT_DIR/scripts/verify_ir_call_shapes.py" "$BAD_IR" >"$BAD_LOG" 2>&1; then
    echo "FAIL: mismatched IR call shapes were accepted"
    cat "$BAD_LOG"
    exit 1
fi

grep -q 'call to @takes_ptr arg 1 is %SeenString, declaration expects ptr' "$BAD_LOG"
grep -q 'call to @takes_string arg 1 is i64, declaration expects %SeenString' "$BAD_LOG"
grep -q 'call to @returns_string returns i64, declaration returns %SeenString' "$BAD_LOG"
grep -q 'call to @returns_i64 returns %SeenString, declaration returns i64' "$BAD_LOG"
grep -q 'call to @takes_two has 1 args, declaration has 2' "$BAD_LOG"

echo "PASS: IR call shape verifier catches mismatches"
