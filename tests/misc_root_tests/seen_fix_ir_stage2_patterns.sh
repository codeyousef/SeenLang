#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_DIR="/tmp/seen_fix_ir_stage2_patterns"
IR_FILE="$TMP_DIR/stage2_patterns.ll"
BC_FILE="$TMP_DIR/stage2_patterns.bc"
OPT_FILE="$TMP_DIR/stage2_patterns.opt.ll"

rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

cat >"$IR_FILE" <<'IR'
%SeenString = type { i64, ptr }
%TypeNode = type { i64, ptr }

declare void @takes_i64(i64)
declare void @takes_ptr(ptr)

If no } found before this leaked parser diagnostic should be a comment

define i64 @missing_i64_operand() {
entry:
  %1 = add i64 %999, 1
  ret i64 %1
}

define ptr @missing_ptr_operand() {
entry:
  %1 = getelementptr i8, ptr %888, i64 0
  ret ptr %1
}

define %SeenString @missing_aggregate_operand() {
entry:
  %1 = insertvalue %SeenString %777, i64 4, 0
  ret %SeenString %1
}

define void @scalar_literal_repairs(ptr %0) {
entry:
  store ptr 8, ptr %0
  %1 = fcmp olt double 0, 1.0
  ret void
}

define void @typed_call_coercions(ptr %0) {
entry:
  %1 = icmp eq ptr %0, null
  call void @takes_i64(i64 %1)
  %2 = ptrtoint ptr %0 to i64
  call void @takes_ptr(ptr %2)
  ret void
}

define void @aggregate_store_from_handle(i64 %0, ptr %1) {
entry:
  %2 = add i64 %0, 0
  store %TypeNode %2, ptr %1
  ret void
}

define void @stale_generator_layout_indices(ptr %0) {
entry:
  %1 = getelementptr inbounds { i64, ptr, i64, i64, ptr, ptr, ptr, i64, i64, %SeenString, %SeenString, i1, %SeenString, %SeenString, ptr, ptr, %SeenString, ptr, ptr, i1, i1, %SeenString, ptr, ptr, ptr }, ptr %0, i32 0, i32 32
  %2 = getelementptr inbounds { i64, ptr, i64, i64, ptr, ptr, ptr, i64, i64, %SeenString, %SeenString, i1, %SeenString, %SeenString, ptr, ptr, %SeenString, ptr, ptr, i1, i1, %SeenString, ptr, ptr, ptr }, ptr %0, i32 0, i32 30
  ret void
}

define i1 @ret_i64_as_bool() {
entry:
  %1 = add i64 0, 1
  ret i1 %1
}
IR

python3 "$ROOT_DIR/scripts/fix_ir.py" "$IR_FILE"

grep -q 'add i64 0, 1' "$IR_FILE"
grep -q 'getelementptr i8, ptr null' "$IR_FILE"
grep -q 'insertvalue %SeenString zeroinitializer' "$IR_FILE"
grep -q 'store i64 8, ptr' "$IR_FILE"
grep -q 'fcmp olt double 0.0, 1.0' "$IR_FILE"
grep -q '= zext i1 %1 to i64' "$IR_FILE"
grep -q '= inttoptr i64 %2 to ptr' "$IR_FILE"
grep -q '= load %TypeNode, ptr' "$IR_FILE"
grep -q '^; If no } found before' "$IR_FILE"
grep -q 'i32 0, i32 22' "$IR_FILE"
grep -q 'i32 0, i32 21' "$IR_FILE"
grep -q '= icmp ne i64 %1, 0' "$IR_FILE"
grep -q 'ret i1 %2' "$IR_FILE"

llvm-as "$IR_FILE" -o "$BC_FILE"
opt -O2 -S "$BC_FILE" -o "$OPT_FILE"

echo "PASS: Stage2 fix_ir pattern repairs"
