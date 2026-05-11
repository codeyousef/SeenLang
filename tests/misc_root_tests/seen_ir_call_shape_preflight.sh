#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_DIR="/tmp/seen_ir_call_shape_preflight"
GOOD_IR="$TMP_DIR/good.ll"
BAD_IR="$TMP_DIR/bad.ll"
BAD_PTR_ZERO_IR="$TMP_DIR/bad_ptr_zero.ll"
MISSING_IMPL_A_IR="$TMP_DIR/missing_impl_a.ll"
MISSING_IMPL_B_IR="$TMP_DIR/missing_impl_b.ll"
MISSING_DYN_A_IR="$TMP_DIR/missing_dyn_a.ll"
MISSING_DYN_B_IR="$TMP_DIR/missing_dyn_b.ll"
BAD_LOG="$TMP_DIR/bad.log"
BAD_PTR_ZERO_LOG="$TMP_DIR/bad_ptr_zero.log"
MISSING_IMPL_LOG="$TMP_DIR/missing_impl.log"
MISSING_DYN_LOG="$TMP_DIR/missing_dyn.log"

rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

cat >"$GOOD_IR" <<'IR'
%SeenString = type { i64, ptr }

declare void @takes_i64(i64)
declare void @takes_ptr(ptr)
declare void @takes_string(%SeenString)
declare %SeenString @string_from_int(i64)
declare void @vararg_ok(ptr, ...)
declare i64 @externalCrossModuleHelperImpl(i64)

@.embedded_ir = private unnamed_addr constant [36 x i8] c"  call void @takes_i64()\0A\00"

define void @ok(ptr %0, i64 %1, %SeenString %2) {
entry:
  call void @takes_i64(i64 %1)
  call void @takes_ptr(ptr %0)
  call void @takes_string(%SeenString %2)
  %3 = call %SeenString @string_from_int(i64 42)
  call void @takes_string(%SeenString %3)
  call void @vararg_ok(ptr null, i64 7, %SeenString zeroinitializer)
  %4 = call i64 @externalCrossModuleHelperImpl(i64 %1)
  ret void
}
IR

python3 "$ROOT_DIR/scripts/verify_ir_call_shapes.py" "$GOOD_IR"

cat >"$MISSING_IMPL_A_IR" <<'IR'
declare i64 @missingSeededHelperImpl(i64)

define i64 @caller(i64 %0) {
entry:
  %1 = call i64 @missingSeededHelperImpl(i64 %0)
  ret i64 %1
}
IR

cat >"$MISSING_IMPL_B_IR" <<'IR'
define i64 @other(i64 %0) {
entry:
  ret i64 %0
}
IR

if python3 "$ROOT_DIR/scripts/verify_ir_call_shapes.py" \
    "$MISSING_IMPL_A_IR" "$MISSING_IMPL_B_IR" >"$MISSING_IMPL_LOG" 2>&1; then
    echo "FAIL: missing cross-module Impl definition was accepted"
    cat "$MISSING_IMPL_LOG"
    exit 1
fi

grep -q '@missingSeededHelperImpl is declared and called but has no definition' "$MISSING_IMPL_LOG"

cat >"$MISSING_DYN_A_IR" <<'IR'
%SeenString = type { i64, ptr }

declare i64 @dyn_appendLoweringOutput(ptr, %SeenString)

define i64 @dyn_caller(ptr %0, %SeenString %1) {
entry:
  %2 = call i64 @dyn_appendLoweringOutput(ptr %0, %SeenString %1)
  ret i64 %2
}
IR

cat >"$MISSING_DYN_B_IR" <<'IR'
define i64 @other_dyn(i64 %0) {
entry:
  ret i64 %0
}
IR

if python3 "$ROOT_DIR/scripts/verify_ir_call_shapes.py" \
    "$MISSING_DYN_A_IR" "$MISSING_DYN_B_IR" >"$MISSING_DYN_LOG" 2>&1; then
    echo "FAIL: missing dyn dispatch definition was accepted"
    cat "$MISSING_DYN_LOG"
    exit 1
fi

grep -q '@dyn_appendLoweringOutput is declared and called but has no definition' "$MISSING_DYN_LOG"

cat >"$BAD_IR" <<'IR'
%SeenString = type { i64, ptr }

declare void @takes_i64(i64)
declare void @takes_ptr(ptr)
declare void @takes_string(%SeenString)
declare %SeenString @returns_string(i64)
declare i64 @returns_i64()
declare void @takes_two(i64, i64)
declare i64 @Int_append(i64, %SeenString)

define void @bad(ptr %0, i64 %1, %SeenString %2) {
entry:
  call void @takes_ptr(%SeenString %2)
  call void @takes_string(i64 %1)
  %3 = call i64 @returns_string(i64 %1)
  %4 = call %SeenString @returns_i64()
  call void @takes_two(i64 1)
  %5 = call i64 @Int_append(i64 %1, %SeenString %2)
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
grep -q '@Int_append is declared and called but has no definition' "$BAD_LOG"

cat >"$BAD_PTR_ZERO_IR" <<'IR'
@.embedded_ir = private unnamed_addr constant [41 x i8] c"  call void @takes_ptr(ptr 0)\0A\00"

declare void @takes_ptr(ptr)

define void @bad_ptr_zero() {
entry:
  call void @takes_ptr(ptr 0)
  ret void
}
IR

if python3 "$ROOT_DIR/scripts/verify_ir_call_shapes.py" "$BAD_PTR_ZERO_IR" >"$BAD_PTR_ZERO_LOG" 2>&1; then
    echo "FAIL: ptr 0 operand was accepted"
    cat "$BAD_PTR_ZERO_LOG"
    exit 1
fi

grep -q 'integer zero cannot be used as a ptr operand; use ptr null' "$BAD_PTR_ZERO_LOG"
if grep -q 'embedded_ir' "$BAD_PTR_ZERO_LOG"; then
    echo "FAIL: ptr 0 verifier inspected quoted embedded IR text"
    cat "$BAD_PTR_ZERO_LOG"
    exit 1
fi

echo "PASS: IR call shape verifier catches mismatches"
