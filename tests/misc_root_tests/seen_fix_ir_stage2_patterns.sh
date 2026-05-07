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
%/ = type { i8 }

declare void @takes_i64(i64)
declare void @takes_ptr(ptr)
declare void @takes_float(float)
declare void @takes_seen_string(%SeenString)
declare ptr @returns_ptr()
declare i1 @llvm.xxx(ptr, ptr, %SeenString, %SeenString, ptr, i64, %SeenString, %SeenString)
declare void @llvm.prefetch.p0(ptr, i32, i32, i32)
declare %SeenString @seen_float_to_string(double)

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

define void @float_call_literal_repairs() {
entry:
  %1 = call %SeenString @seen_float_to_string(double 0)
  call void @takes_float(float 0)
  ret void
}

define void @invalid_named_type_repair(ptr %0) {
entry:
  %1 = load %/, ptr %0
  ret void
}

define void @misplaced_global_def() {
entry:
@.str.hoisted = private unnamed_addr constant [4 x i8] c"hey\00"
  %1 = getelementptr [4 x i8], ptr @.str.hoisted, i64 0, i64 0
  ret void
}

define void @ret_void_with_value() {
entry:
  %1 = call %SeenString @seen_float_to_string(double 0.0)
  ret void %1
}

define void @seen_string_call_arg_from_i64() {
entry:
  %1 = sub i64 0, 0
  call void @takes_seen_string(%SeenString %1)
  ret void
}

define i64 @arith_ptr_as_i64() {
entry:
  %1 = call ptr @returns_ptr()
  %2 = add i64 %1, 1
  ret i64 %2
}

define i64 @load_from_seen_string_ptr() {
entry:
  %1 = call %SeenString @seen_float_to_string(double 0.0)
  %2 = load i64, ptr %1
  ret i64 %2
}

define i64 @load_from_i64_ptr() {
entry:
  %1 = sub i64 0, 0
  %2 = load i64, ptr %1
  ret i64 %2
}

define i64 @load_from_literal_ptr() {
entry:
  %1 = load i64, ptr 32
  ret i64 %1
}

define ptr @gep_from_i64_base() {
entry:
  %1 = sub i64 0, 0
  %2 = getelementptr i64, ptr %1, i64 0
  ret ptr %2
}

define ptr @gep_from_i1_index(ptr %0) {
entry:
  %1 = icmp eq i64 0, 0
  %2 = getelementptr i64, ptr %0, i64 %1
  ret ptr %2
}

define %SeenString @insertvalue_bad_seen_string() {
entry:
  %1 = sub i64 0, 0
  %2 = insertvalue %SeenString %1, ptr %1, 1
  ret %SeenString %2
}

define i64 @zext_i64_marked_i1() {
entry:
  %1 = sub i64 0, 0
  %2 = zext i1 %1 to i64
  ret i64 %2
}

define ptr @inttoptr_bool_marked_i64() {
entry:
  %1 = icmp eq i64 0, 0
  %2 = inttoptr i64 %1 to ptr
  ret ptr %2
}

define i64 @nondominating_typed_use() {
entry:
  br i1 false, label %bb_a, label %bb_b
bb_a:
  %1 = add i64 40, 2
  br label %bb_b
bb_b:
  %2 = add i64 %1, 1
  ret i64 %2
}

define i1 @icmp_seen_string_as_i64() {
entry:
  %1 = call %SeenString @seen_float_to_string(double 0.0)
  %2 = icmp ne i64 %1, 0
  ret i1 %2
}

define i1 @ssa_param_collision(ptr %0, ptr %1, %SeenString %2, %SeenString %3, ptr %4, i64 %5, %SeenString %6, %SeenString %7) {
entry:
  %1 = call i1 @llvm.xxx(ptr null, ptr %1, %SeenString zeroinitializer, %SeenString zeroinitializer, ptr null, i64 0, %SeenString zeroinitializer, %SeenString zeroinitializer)
  ret i1 %1
}

define void @dotted_call_ptr_arg(i64 %0) {
entry:
  call void @llvm.prefetch.p0(ptr %0, i32 0, i32 3, i32 1)
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
grep -q 'call %SeenString @seen_float_to_string(double 0.0)' "$IR_FILE"
grep -q 'call void @takes_float(float 0.0)' "$IR_FILE"
grep -q '%"/" = type { i8 }' "$IR_FILE"
grep -q 'load %"/", ptr' "$IR_FILE"
grep -q '^@.str.hoisted = private unnamed_addr constant' "$IR_FILE"
! grep -q 'ret void %' "$IR_FILE"
grep -q '= insertvalue %SeenString zeroinitializer, i64 %1, 0' "$IR_FILE"
grep -q 'call void @takes_seen_string(%SeenString %2)' "$IR_FILE"
grep -q '= ptrtoint ptr %1 to i64' "$IR_FILE"
grep -q '= add i64 %2, 1' "$IR_FILE"
grep -q '= extractvalue %SeenString %1, 1' "$IR_FILE"
grep -q '= inttoptr i64 %1 to ptr' "$IR_FILE"
grep -q '= inttoptr i64 32 to ptr' "$IR_FILE"
grep -q 'getelementptr i64, ptr %2, i64 0' "$IR_FILE"
grep -q 'getelementptr i64, ptr %0, i64 %2' "$IR_FILE"
grep -q 'insertvalue %SeenString zeroinitializer, ptr %2, 1' "$IR_FILE"
grep -q '= add i64 %1, 0' "$IR_FILE"
grep -q '= zext i1 %1 to i64' "$IR_FILE"
grep -q '= add i64 0, 1' "$IR_FILE"
grep -q '= extractvalue %SeenString %1, 0' "$IR_FILE"
grep -q '%8 = call i1 @llvm.xxx(ptr null, ptr %1' "$IR_FILE"
grep -q 'ret i1 %8' "$IR_FILE"
grep -q 'call void @llvm.prefetch.p0(ptr %1, i32 0, i32 3, i32 1)' "$IR_FILE"
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
