#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_DIR="/tmp/seen_fix_ir_stage2_patterns"
IR_FILE="$TMP_DIR/stage2_patterns.ll"
OTHER_IR_FILE="$TMP_DIR/stage2_patterns_other.ll"
BC_FILE="$TMP_DIR/stage2_patterns.bc"
OPT_FILE="$TMP_DIR/stage2_patterns.opt.ll"

rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

cat >"$IR_FILE" <<'IR'
%SeenString = type { i64, ptr }
%SeenArray = type { i64, i64, i64, ptr }
%TypeNode = type { i64, ptr }
%/ = type { i8 }

declare void @takes_i64(i64)
declare void @takes_ptr(ptr)
declare void @takes_float(float)
declare void @takes_double(double)
declare void @takes_seen_string(%SeenString)
declare ptr @returns_ptr()
declare i64 @side_effect()
declare void @VoidHelper(ptr)
declare i64 @Map_put(i64, %SeenString, i64)
declare i1 @Map_containsKey(i64, %SeenString)
declare %SeenArray @Map_keys(i64)
declare %SeenString @mapTypeImpl(%SeenString, ptr, %SeenString, %SeenString, i64, %SeenString)
declare i1 @llvm.xxx(ptr, ptr, %SeenString, %SeenString, ptr, i64, %SeenString, %SeenString)
declare void @llvm.prefetch.p0(ptr, i32, i32, i32)
declare double @llvm.fmuladd.f64(double, double, double)
declare %SeenString @seen_float_to_string(double)
declare i64 @dyn_lowerExpression(ptr, i64)
declare %SeenArray @__ReadFileBytes(i64, i64)
declare i64 @__WriteFileBytes(i64, %SeenArray)
declare i64 @Ok(i64)
declare i64 @abort(i64)
declare void @__panic(i64, ptr) noreturn nounwind
declare ptr @prepareFunctionPreludeAnalysisWithMetricsStateImpl(i64, i64, %SeenString, %SeenString)
declare ptr @prepareFunctionGenerationIdentityWithGlobalStateImpl(i64, %SeenString)
declare void @prepareFunctionPreBodyWithFeatureStateImpl(ptr, i64, %SeenString, %SeenString)
declare void @emitLoopMetadataWithMetricsStateImpl(i64, ptr)
declare void @emitLoopMetadataImpl(i64, ptr, ptr, %SeenString, %SeenString, %SeenString)
declare void @resetFunctionLoweringOptionsStateImpl(ptr)
declare void @resetFunctionHighPressureImpl(ptr)
declare void @markFunctionHighPressureImpl(ptr)
declare i1 @isFunctionHighPressureImpl(ptr)
declare i64 @currentFunctionAlignToImpl(ptr)
declare i64 @currentFunctionRegionSizeBytesImpl(ptr)
declare i1 @isCurrentFunctionAsyncLoweringImpl(ptr)
declare void @markTailCallPositionImpl(ptr)
declare i1 @takeTailCallPositionImpl(ptr)
declare i64 @emitFunctionEntrySetupSnapshotImpl(ptr, i64, %SeenString, %SeenString, %SeenString)
declare i64 @emitFunctionExitResetSnapshotImpl(ptr, i64, %SeenString)
declare ptr @scanFunctionBodyDeadStorePatternsSnapshotImpl(i64, i64, i64, %SeenString)
declare %SeenString @generateLiteralFree(ptr, i64)
declare ptr @prepareFunctionGenerationIdentitySnapshotImpl(i64, ptr, %SeenString, ptr, ptr, %SeenString, i64, %SeenString, ptr, ptr, ptr)
declare void @configureFunctionLoweringOptionsImpl(i64, ptr)
declare i64 @snapshotNestedScratchStateImpl(ptr)
declare void @resetLocalCodegenStateImpl(ptr)
declare void @clearActiveBindingsImpl(ptr)
declare void @beginNestedScratchStateImpl(ptr, %SeenString)
declare void @restoreNestedScratchStateImpl(ptr, i64)
declare ptr @emitFunctionEntrySetupStateImpl(ptr, i64, %SeenString, %SeenString, %SeenString, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr)
declare void @emitFunctionExitAndResetStateImpl(ptr, i64, %SeenString, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr)
declare ptr @currentLoweringFunctionOptions(i64)
declare i64 @prepareFunctionPreBodyStateSnapshotImpl(ptr, i64, %SeenString, %SeenString, %SeenString, i64, %SeenString, %SeenString, %SeenString)
declare ptr @emitFunctionBodyTrailingDeadCodeNoticeSnapshotImpl(i64, i64, i64, i64)
declare void @syncCodegenStateBridgeImpl(ptr, i64, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, %SeenString, %SeenString, %SeenString, i1, %SeenString, %SeenString, ptr, ptr, ptr, ptr, ptr, i64, %SeenString, ptr, ptr, %SeenString, ptr, ptr, ptr)
declare i64 @captureCodegenStateBridgeSnapshotImpl(ptr)
declare i64 @resetLLVMIRGeneratorModuleStateImpl(ptr)
declare ptr @beginLoweringNestedScratch(i64, %SeenString)
declare void @restoreLoweringNestedScratch(i64, ptr)
declare i64 @beginNestedScratchWithLoweringContext(ptr, %SeenString)
declare void @restoreNestedScratchWithLoweringContext(ptr, i64)
declare void @resetLocalCodegenWithLoweringContext(ptr)
declare void @clearActiveBindingsWithLoweringContext(ptr)
declare ptr @prepareClassTypeDecoratorScanWithFeatureStateImpl(i64, i64, ptr, ptr, ptr)
declare ptr @prepareClassTypeAliasWithFeatureStateImpl(i64)
declare ptr @prepareLetStatementPlanWithGlobalStateImpl(i64, %SeenString, %SeenString)
declare ptr @CrossModuleStateHelper(ptr)
declare i64 @run_frontend(%SeenString, %SeenString, ptr)
declare i64 @ParserExpressionNode_new(ptr)
declare i64 @Token_new(ptr, i64, %SeenString, i64, i64, i64, i64)
declare i64 @FrontendDiagnostic_new(ptr, %SeenString, i64, i64, %SeenString, %SeenString)
declare i64 @Type_new(%SeenString, i1)
declare i64 @Environment_new(i64)
declare i64 @FunctionType_new(ptr, i64, i1)
declare i64 @JsonValue_bool(i1)
declare %SeenString @ContentLengthReader_readMessage(ptr)
declare i64 @parseJson(%SeenString)
declare i64 @LspError_unwrap(ptr)
declare i64 @Document_unwrap(ptr)
declare i1 @isReprCConstructorTypeImpl(%SeenString, %SeenString)
declare %SeenString @resolveLiteralMethodReceiverTypeImpl(%SeenString, %SeenString, %SeenString, i64, %SeenString, ptr, ptr, ptr, ptr, ptr)
declare %SeenString @coerceValueForLlvmTargetImpl(i64, ptr, %SeenString, %SeenString, %SeenString, %SeenString)
declare i1 @finalizeConditionalEndBlockImpl(i64, %SeenString, i1, i1)
declare %SeenString @badDynContextDeclare(ptr, void, i64)

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
  call void @takes_double(double 0)
  ret void
}

define double @fma_call_literal_repairs(double %0) {
entry:
  %1 = call double @llvm.fmuladd.f64(double 0, double 0, double %0)
  ret double %1
}

define double @float_binop_literal_repairs(double %0) {
entry:
  %1 = fadd fast double 0, %0
  %2 = fsub fast double %1, 0
  %3 = fmul fast double 0, %2
  %4 = fdiv fast double %3, 1
  %5 = frem fast double %4, 2
  ret double %5
}

define i64 @float_unary_literal_repairs(ptr %0) {
entry:
  store double 0, ptr %0
  %1 = fneg fast double 0
  %2 = fptosi double 0 to i64
  %3 = bitcast double 0 to i64
  %4 = add i64 %2, %3
  ret i64 %4
}

define void @ptr_null_call_literal_repair() {
entry:
  call void @takes_ptr(ptr 0)
  ret void
}

define i64 @void_dyn_context_param_repair(ptr noalias nonnull %ctx.arg, void %CodegenLoweringContext.arg, i64 %expr.arg) {
entry:
  %1 = alloca void
  store void %CodegenLoweringContext.arg, ptr %1
  ret i64 %expr.arg
}

define %SeenString @void_dyn_context_declare_repair(ptr %0, i64 %1) {
entry:
  %2 = call %SeenString @badDynContextDeclare(ptr %0, i64 %1)
  ret %SeenString %2
}

define %SeenString @lowerExpression(i64 %this.arg, i64 %expr.arg) {
entry:
  %1 = insertvalue %SeenString zeroinitializer, i64 %expr.arg, 0
  ret %SeenString %1
}

define %SeenString @lowerExpression(ptr noalias nonnull %this.arg, i64 %expr.arg) alwaysinline norecurse #0 {
entry:
  %1 = alloca ptr
  store ptr %this.arg, ptr %1
  %2 = alloca i64
  store i64 %expr.arg, ptr %2
  ret %SeenString zeroinitializer
}

define %SeenString @dyn_lowering_context_repair(ptr %0, i64 %1) {
entry:
  %2 = call i64 @dyn_lowerExpression(ptr %0, i64 %1)
  ret %SeenString %2
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

define void @unassigned_nonvoid_call_slot(ptr %0) {
entry:
  %1 = alloca i64, align 8
  call i64 @side_effect()
  %2 = load i64, ptr %1
  ret void
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

define i64 @result_ok_seen_string() {
entry:
  %1 = call %SeenString @seen_float_to_string(double 0.0)
  %2 = tail call i64 @Ok(%SeenString %1)
  ret i64 %2
}

define i64 @result_ok_ptr() {
entry:
  %1 = call ptr @returns_ptr()
  %2 = tail call i64 @Ok(ptr %1)
  ret i64 %2
}

define ptr @file_byte_runtime_read(i64 %0, i64 %1) {
entry:
  %2 = call ptr @__ReadFileBytes(i64 %0, i64 %1)
  ret ptr %2
}

define i64 @file_byte_runtime_write(i64 %0, ptr %1) {
entry:
  %2 = call i64 @__WriteFileBytes(i64 %0, ptr %1)
  ret i64 %2
}

define i64 @abort_seen_string() {
entry:
  %1 = call %SeenString @seen_float_to_string(double 0.0)
  %2 = call i64 @abort(%SeenString %1)
  ret i64 %2
}

define i64 @state_snapshot_handle_abi(i64 %0, i64 %1) {
entry:
  %2 = call %SeenString @seen_float_to_string(double 0.0)
  call void @resetFunctionLoweringOptionsStateImpl(i64 %0)
  call void @resetFunctionHighPressureImpl(i64 %0)
  call void @markFunctionHighPressureImpl(i64 %0)
  call void @prepareFunctionPreBodyWithFeatureStateImpl(i64 %0, i64 %1, %SeenString %2, %SeenString %2)
  %3 = call i64 @prepareFunctionGenerationIdentityWithGlobalStateImpl(i64 %1, %SeenString %2)
  %4 = call i64 @prepareFunctionPreludeAnalysisWithMetricsStateImpl(i64 %0, i64 %1, %SeenString %2, %SeenString %2)
  %5 = call i64 @emitFunctionEntrySetupSnapshotImpl(i64 %0, i64 %1, %SeenString %2, %SeenString %2, %SeenString %2)
  %6 = call i64 @emitFunctionExitResetSnapshotImpl(i64 %0, i64 %1, %SeenString %2)
  %7 = call i64 @scanFunctionBodyDeadStorePatternsSnapshotImpl(i64 %0, i64 %1, i64 %3, %SeenString %2)
  %8 = add i64 %4, %5
  %9 = add i64 %8, %6
  %10 = add i64 %9, %7
  ret i64 %10
}

define i64 @constructor_zero_arg_abi() {
entry:
  %1 = call i64 @ParserExpressionNode_new()
  ret i64 %1
}

define i64 @frontend_language_decl_abi() {
entry:
  %1 = call i64 @run_frontend(%SeenString zeroinitializer, %SeenString zeroinitializer, %SeenString zeroinitializer)
  ret i64 %1
}

define i64 @map_put_bool_value_abi(ptr %0) {
entry:
  %1 = call i64 @Map_put(ptr %0, %SeenString zeroinitializer, i1 true)
  ret i64 %1
}

define i64 @map_put_string_value_abi(ptr %0) {
entry:
  %1 = call i64 @Map_put(ptr %0, %SeenString zeroinitializer, %SeenString zeroinitializer)
  ret i64 %1
}

define %SeenString @map_type_short_call_abi() {
entry:
  %1 = call %SeenString @mapTypeImpl(%SeenString zeroinitializer)
  ret %SeenString %1
}

define ptr @map_keys_ptr_abi(ptr %0) {
entry:
  %1 = call ptr @Map_keys(ptr %0)
  ret ptr %1
}

define i1 @map_contains_key_ptr_abi(ptr %0) {
entry:
  %1 = call i1 @Map_containsKey(ptr %0, %SeenString zeroinitializer)
  ret i1 %1
}

define i64 @default_constructor_args_abi() {
entry:
  %1 = call i64 @Type_new(%SeenString zeroinitializer)
  %2 = call i64 @Environment_new()
  %3 = call i64 @FunctionType_new(ptr null, i64 %1)
  %4 = add i64 %2, %3
  ret i64 %4
}

define i64 @json_bool_literal_abi() {
entry:
  %1 = call i64 @JsonValue_bool(i64 1)
  %2 = call i64 @JsonValue_bool(i64 0)
  %3 = add i64 %1, %2
  ret i64 %3
}

define i64 @lsp_json_string_abi(ptr %0) {
entry:
  %1 = alloca i64, align 8
  %2 = call i64 @ContentLengthReader_readMessage(ptr %0)
  store i64 %2, ptr %1
  %3 = load i64, ptr %1
  %4 = call i64 @parseJson(i64 %3)
  ret i64 %4
}

define i64 @lsp_error_unwrap_handle_abi(ptr %0) {
entry:
  %1 = call i64 @LspError_unwrap(ptr %0)
  ret i64 %1
}

define i64 @lsp_option_unwrap_handle_abi(ptr %0) {
entry:
  %1 = call i64 @Document_unwrap(ptr %0)
  ret i64 %1
}

define i1 @legacy_driver_arity_repairs(%SeenString %0, ptr %1) {
entry:
  %2 = call i1 @isReprCConstructorTypeImpl(%SeenString %0)
  %3 = call %SeenString @resolveLiteralMethodReceiverTypeImpl(%SeenString %0, ptr %1, ptr %1, ptr %1)
  %4 = call %SeenString @coerceValueForLlvmTargetImpl(%SeenString %0, i64 0, %SeenString %3)
  %5 = call i1 @finalizeConditionalEndBlockImpl(i64 0, i1 true, %SeenString %4)
  %6 = and i1 %2, %5
  ret i1 %6
}

define i64 @constructor_stale_receiver_abi() {
entry:
  %1 = call i64 @Token_new(i64 6, %SeenString zeroinitializer, i64 1, i64 2, i64 5, i64 7)
  ret i64 %1
}

define void @store_declared_function_pointer(ptr %0) {
entry:
  store ptr @returns_ptr, ptr %0
  ret void
}

define void @assigned_void_call(ptr %0) {
entry:
  %1 = call ptr @VoidHelper(ptr %0)
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

cat >"$OTHER_IR_FILE" <<'IR'
define i64 @CrossModuleStateHelper(i64 %state.arg) {
entry:
  ret i64 %state.arg
}
IR

python3 "$ROOT_DIR/scripts/fix_ir.py" "$IR_FILE" "$OTHER_IR_FILE"

grep -q 'add i64 0, 1' "$IR_FILE"
grep -q 'getelementptr i8, ptr null' "$IR_FILE"
grep -q 'insertvalue %SeenString zeroinitializer' "$IR_FILE"
grep -q 'store i64 8, ptr' "$IR_FILE"
grep -q 'fcmp olt double 0.0, 1.0' "$IR_FILE"
grep -q 'call %SeenString @seen_float_to_string(double 0.0)' "$IR_FILE"
grep -q 'call void @takes_float(float 0.0)' "$IR_FILE"
grep -q 'call void @takes_double(double 0.0)' "$IR_FILE"
grep -q 'call double @llvm.fmuladd.f64(double 0.0, double 0.0, double %0)' "$IR_FILE"
grep -q 'fadd fast double 0.0, %0' "$IR_FILE"
grep -q 'fsub fast double %1, 0.0' "$IR_FILE"
grep -q 'fmul fast double 0.0, %2' "$IR_FILE"
grep -q 'fdiv fast double %3, 1.0' "$IR_FILE"
grep -q 'frem fast double %4, 2.0' "$IR_FILE"
grep -q 'store double 0.0, ptr %0' "$IR_FILE"
grep -q 'fneg fast double 0.0' "$IR_FILE"
grep -q 'fptosi double 0.0 to i64' "$IR_FILE"
grep -q 'bitcast double 0.0 to i64' "$IR_FILE"
grep -q 'call void @takes_ptr(ptr null)' "$IR_FILE"
! grep -q 'ptr 0' "$IR_FILE"
grep -q 'declare %SeenString @badDynContextDeclare(ptr, i64)' "$IR_FILE"
grep -q 'define i64 @void_dyn_context_param_repair(ptr noalias nonnull %ctx.arg, i64 %expr.arg)' "$IR_FILE"
! grep -q 'alloca void' "$IR_FILE"
! grep -q 'store void' "$IR_FILE"
! grep -q 'define %SeenString @lowerExpression(ptr noalias nonnull' "$IR_FILE"
grep -q '= ptrtoint ptr %0 to i64' "$IR_FILE"
grep -q 'call %SeenString @lowerExpression(i64 %2, i64 %1)' "$IR_FILE"
grep -q 'ret %SeenString %3' "$IR_FILE"
grep -q '%"/" = type { i8 }' "$IR_FILE"
grep -q 'load %"/", ptr' "$IR_FILE"
grep -q '^@.str.hoisted = private unnamed_addr constant' "$IR_FILE"
! grep -q 'ret void %' "$IR_FILE"
grep -q '%2 = call i64 @side_effect()' "$IR_FILE"
grep -q '%3 = load i64, ptr %1' "$IR_FILE"
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
grep -q 'declare ptr @__ReadFileBytes(i64, i64)' "$IR_FILE"
grep -q 'declare i64 @__WriteFileBytes(i64, ptr)' "$IR_FILE"
grep -q 'call ptr @seen_checked_malloc(i64 16)' "$IR_FILE"
grep -q 'store %SeenString %1, ptr' "$IR_FILE"
grep -q '= ptrtoint ptr %1 to i64' "$IR_FILE"
! grep -q 'call i64 @Ok(%SeenString' "$IR_FILE"
! grep -q 'call i64 @Ok(ptr' "$IR_FILE"
grep -q 'call void @__panic(i64' "$IR_FILE"
! grep -q 'call i64 @abort(%SeenString' "$IR_FILE"
grep -q 'declare i64 @prepareFunctionPreludeAnalysisWithMetricsStateImpl(i64, i64, %SeenString, %SeenString)' "$IR_FILE"
grep -q 'declare i64 @prepareFunctionGenerationIdentityWithGlobalStateImpl(i64, %SeenString)' "$IR_FILE"
grep -q 'declare void @prepareFunctionPreBodyWithFeatureStateImpl(i64, i64, %SeenString, %SeenString)' "$IR_FILE"
grep -q 'declare void @emitLoopMetadataWithMetricsStateImpl(i64, i64)' "$IR_FILE"
grep -q 'declare void @emitLoopMetadataImpl(i64, i64, ptr, %SeenString, %SeenString, %SeenString)' "$IR_FILE"
grep -q 'declare void @resetFunctionLoweringOptionsStateImpl(i64)' "$IR_FILE"
grep -q 'declare void @resetFunctionHighPressureImpl(i64)' "$IR_FILE"
grep -q 'declare void @markFunctionHighPressureImpl(i64)' "$IR_FILE"
grep -q 'declare i1 @isFunctionHighPressureImpl(i64)' "$IR_FILE"
grep -q 'declare i64 @currentFunctionAlignToImpl(i64)' "$IR_FILE"
grep -q 'declare i64 @currentFunctionRegionSizeBytesImpl(i64)' "$IR_FILE"
grep -q 'declare i1 @isCurrentFunctionAsyncLoweringImpl(i64)' "$IR_FILE"
grep -q 'declare void @markTailCallPositionImpl(i64)' "$IR_FILE"
grep -q 'declare i1 @takeTailCallPositionImpl(i64)' "$IR_FILE"
grep -q 'declare i64 @emitFunctionEntrySetupSnapshotImpl(i64, i64, %SeenString, %SeenString, %SeenString)' "$IR_FILE"
grep -q 'declare i64 @emitFunctionExitResetSnapshotImpl(i64, i64, %SeenString)' "$IR_FILE"
grep -q 'declare i64 @scanFunctionBodyDeadStorePatternsSnapshotImpl(i64, i64, i64, %SeenString)' "$IR_FILE"
grep -q 'declare %SeenString @generateLiteralFree(i64, i64)' "$IR_FILE"
grep -q 'declare i64 @prepareFunctionGenerationIdentitySnapshotImpl(i64, ptr, %SeenString, ptr, ptr, %SeenString, i64, %SeenString, ptr, ptr, ptr)' "$IR_FILE"
grep -q 'declare void @configureFunctionLoweringOptionsImpl(i64, i64)' "$IR_FILE"
grep -q 'declare i64 @snapshotNestedScratchStateImpl(i64)' "$IR_FILE"
grep -q 'declare void @resetLocalCodegenStateImpl(i64)' "$IR_FILE"
grep -q 'declare void @clearActiveBindingsImpl(i64)' "$IR_FILE"
grep -q 'declare void @beginNestedScratchStateImpl(i64, %SeenString)' "$IR_FILE"
grep -q 'declare void @restoreNestedScratchStateImpl(i64, i64)' "$IR_FILE"
grep -q 'declare ptr @emitFunctionEntrySetupStateImpl(i64, i64, %SeenString, %SeenString, %SeenString, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr)' "$IR_FILE"
grep -q 'declare void @emitFunctionExitAndResetStateImpl(i64, i64, %SeenString, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr)' "$IR_FILE"
grep -q 'declare i64 @currentLoweringFunctionOptions(i64)' "$IR_FILE"
grep -q 'declare i64 @prepareFunctionPreBodyStateSnapshotImpl(i64, i64, %SeenString, %SeenString, %SeenString, i64, %SeenString, %SeenString, %SeenString)' "$IR_FILE"
grep -q 'declare i64 @emitFunctionBodyTrailingDeadCodeNoticeSnapshotImpl(i64, i64, i64, i64)' "$IR_FILE"
grep -q 'declare void @syncCodegenStateBridgeImpl(i64, i64, ptr, ptr, ptr, ptr, ptr, ptr, i64, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, ptr, %SeenString, %SeenString, %SeenString, i1, %SeenString, %SeenString, ptr, ptr, ptr, ptr, ptr, i64, %SeenString, ptr, ptr, %SeenString, ptr, ptr, ptr)' "$IR_FILE"
grep -q 'declare i64 @captureCodegenStateBridgeSnapshotImpl(i64)' "$IR_FILE"
grep -q 'declare i64 @resetLLVMIRGeneratorModuleStateImpl(i64)' "$IR_FILE"
grep -q 'declare i64 @beginLoweringNestedScratch(i64, %SeenString)' "$IR_FILE"
grep -q 'declare void @restoreLoweringNestedScratch(i64, i64)' "$IR_FILE"
grep -q 'declare i64 @beginNestedScratchWithLoweringContext(i64, %SeenString)' "$IR_FILE"
grep -q 'declare void @restoreNestedScratchWithLoweringContext(i64, i64)' "$IR_FILE"
grep -q 'declare void @resetLocalCodegenWithLoweringContext(i64)' "$IR_FILE"
grep -q 'declare void @clearActiveBindingsWithLoweringContext(i64)' "$IR_FILE"
grep -q 'declare i64 @prepareClassTypeDecoratorScanWithFeatureStateImpl(i64, i64, ptr, ptr, ptr)' "$IR_FILE"
grep -q 'declare i64 @prepareClassTypeAliasWithFeatureStateImpl(i64)' "$IR_FILE"
grep -q 'declare i64 @prepareLetStatementPlanWithGlobalStateImpl(i64, %SeenString, %SeenString)' "$IR_FILE"
grep -q 'declare i64 @CrossModuleStateHelper(i64)' "$IR_FILE"
grep -q 'declare i64 @run_frontend(%SeenString, %SeenString, %SeenString)' "$IR_FILE"
grep -q 'declare i64 @Map_put(ptr, %SeenString, i64)' "$IR_FILE"
grep -q '= zext i1 true to i64' "$IR_FILE"
grep -q 'store %SeenString zeroinitializer, ptr' "$IR_FILE"
grep -q 'declare i1 @Map_containsKey(ptr, %SeenString)' "$IR_FILE"
grep -q 'declare ptr @Map_keys(ptr)' "$IR_FILE"
grep -q 'declare ptr @LspError_unwrap(ptr)' "$IR_FILE"
grep -q '= call ptr @LspError_unwrap(ptr %0)' "$IR_FILE"
grep -q '= ptrtoint ptr %.* to i64' "$IR_FILE"
grep -q 'declare ptr @Document_unwrap(ptr)' "$IR_FILE"
grep -q '= call ptr @Document_unwrap(ptr %0)' "$IR_FILE"
grep -q 'call i1 @isReprCConstructorTypeImpl(%SeenString %0, %SeenString zeroinitializer)' "$IR_FILE"
grep -q 'call %SeenString @resolveLiteralMethodReceiverTypeImpl(%SeenString %0, %SeenString zeroinitializer, %SeenString zeroinitializer, i64 -1, %SeenString zeroinitializer, ptr null, ptr null, ptr null, ptr null, ptr null)' "$IR_FILE"
grep -q 'call %SeenString @coerceValueForLlvmTargetImpl(i64 0, ptr null, %SeenString %0,' "$IR_FILE"
grep -q 'call i1 @finalizeConditionalEndBlockImpl(i64 0, %SeenString %5, i1 true, i1 false)' "$IR_FILE"
grep -q 'call i64 @Type_new(%SeenString zeroinitializer, i1 false)' "$IR_FILE"
grep -q 'call i64 @Environment_new(i64 0)' "$IR_FILE"
grep -q 'call i64 @FunctionType_new(ptr null, i64 %1, i1 false)' "$IR_FILE"
grep -q 'call i64 @JsonValue_bool(i1 true)' "$IR_FILE"
grep -q 'call i64 @JsonValue_bool(i1 false)' "$IR_FILE"
grep -q 'call %SeenString @ContentLengthReader_readMessage(ptr %0)' "$IR_FILE"
grep -q 'call i64 @parseJson(%SeenString %3)' "$IR_FILE"
grep -q 'declare %SeenString @mapTypeImpl(%SeenString)' "$IR_FILE"
grep -q 'declare i64 @ParserExpressionNode_new()' "$IR_FILE"
grep -q 'declare i64 @Token_new(i64, %SeenString, i64, i64, i64, i64)' "$IR_FILE"
grep -q 'declare i64 @FrontendDiagnostic_new(%SeenString, i64, i64, %SeenString, %SeenString)' "$IR_FILE"
! grep -q '^@returns_ptr = external global ptr' "$IR_FILE"
grep -q 'call void @VoidHelper(ptr %0)' "$IR_FILE"
! grep -q '= call ptr @VoidHelper' "$IR_FILE"
grep -q '= load %TypeNode, ptr' "$IR_FILE"
grep -q '^; If no } found before' "$IR_FILE"
grep -q 'i32 0, i32 22' "$IR_FILE"
grep -q 'i32 0, i32 21' "$IR_FILE"
grep -q '= icmp ne i64 %1, 0' "$IR_FILE"
grep -q 'ret i1 %2' "$IR_FILE"

llvm-as "$IR_FILE" -o "$BC_FILE"
opt -O2 -S "$BC_FILE" -o "$OPT_FILE"

echo "PASS: Stage2 fix_ir pattern repairs"
