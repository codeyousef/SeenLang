#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_codegen_abi_preflight.XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

mkdir -p "$TMP_DIR/compiler_seen/src/codegen" "$TMP_DIR/compiler_seen/src/parser"

write_owner_modules() {
    cat > "$TMP_DIR/compiler_seen/src/parser/real_parser.seen" <<'SEEN'
class StatementNode {
    var kind: String
    var variableName: String
    var variableType: TypeNode
    var initializer: ParserExpressionNode
    var returnValue: ParserExpressionNode
    var expression: ParserExpressionNode
    var condition: ParserExpressionNode
    var thenBranch: BlockNode
    var elseBranch: BlockNode
    var loopCondition: ParserExpressionNode
    var loopBody: BlockNode

    fun new() r: StatementNode {
        return StatementNode{}
    }
}

class ParserExpressionNode {
    var kind: String
    var operands: Array<ParserExpressionNode>
    var operator: String
    var literalValue: String
    var literalType: String
    var variableName: String
    var callee: String
    var arguments: Array<ParserExpressionNode>

    static fun new() r: ParserExpressionNode {
        return ParserExpressionNode{}
    }
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_field_indices.seen" <<'SEEN'
fun getKnownFieldIndexPrimaryTableImpl(structName: String, fieldName: String)
    r: Int {
    if structName == "StatementNode" {
        if fieldName == "kind" { return 0 }
        if fieldName == "variableName" { return 1 }
        if fieldName == "variableType" { return 2 }
        if fieldName == "initializer" { return 3 }
        if fieldName == "returnValue" { return 4 }
        if fieldName == "expression" { return 5 }
        if fieldName == "condition" { return 6 }
        if fieldName == "thenBranch" { return 7 }
        if fieldName == "elseBranch" { return 8 }
        if fieldName == "loopCondition" { return 9 }
        if fieldName == "loopBody" { return 10 }
    }
    if structName == "ParserExpressionNode" {
        if fieldName == "kind" { return 0 }
        if fieldName == "operands" { return 1 }
        if fieldName == "operator" { return 2 }
        if fieldName == "literalValue" { return 3 }
        if fieldName == "literalType" { return 4 }
        if fieldName == "variableName" { return 5 }
        if fieldName == "callee" { return 6 }
        if fieldName == "arguments" { return 7 }
    }
    return -1
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/type_registry_core.seen" <<'SEEN'
fun getFieldInfoReg(structName: String, fieldName: String,
    structNames: Array<String>, structFieldNames: Array<String>) r: Int {

    if structName == "StatementNode" {
        if fieldName == "kind" { return 0 }
        if fieldName == "variableName" { return 1 }
        if fieldName == "variableType" { return 2 }
        if fieldName == "initializer" { return 3 }
        if fieldName == "returnValue" { return 4 }
        if fieldName == "expression" { return 5 }
        if fieldName == "condition" { return 6 }
        if fieldName == "thenBranch" { return 7 }
        if fieldName == "elseBranch" { return 8 }
        if fieldName == "loopCondition" { return 9 }
        if fieldName == "loopBody" { return 10 }
    }
    if structName == "ParserExpressionNode" {
        if fieldName == "kind" { return 0 }
        if fieldName == "operands" { return 1 }
        if fieldName == "operator" { return 2 }
        if fieldName == "literalValue" { return 3 }
        if fieldName == "literalType" { return 4 }
        if fieldName == "variableName" { return 5 }
        if fieldName == "callee" { return 6 }
        if fieldName == "arguments" { return 7 }
    }
    return -1
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_struct_field_resolution.seen" <<'SEEN'
fun resolveKnownStructFieldImpl(structType: String, fieldName: String)
    r: String {
    let lb = "{"
    let rb = "}"
    if structType == "StatementNode" {
        if fieldName == "kind" { g_resolvedFieldIndex = 0; g_resolvedFieldTypeCode = 3 }
        if fieldName == "variableName" { g_resolvedFieldIndex = 1; g_resolvedFieldTypeCode = 3 }
        if fieldName == "variableType" { g_resolvedFieldIndex = 2; g_resolvedFieldTypeCode = 4 }
        if fieldName == "initializer" { g_resolvedFieldIndex = 3; g_resolvedFieldTypeCode = 0 }
        if fieldName == "returnValue" { g_resolvedFieldIndex = 4; g_resolvedFieldTypeCode = 0 }
        if fieldName == "expression" { g_resolvedFieldIndex = 5; g_resolvedFieldTypeCode = 0 }
        if fieldName == "condition" { g_resolvedFieldIndex = 6; g_resolvedFieldTypeCode = 0 }
        if fieldName == "thenBranch" { g_resolvedFieldIndex = 7; g_resolvedFieldTypeCode = 0 }
        if fieldName == "elseBranch" { g_resolvedFieldIndex = 8; g_resolvedFieldTypeCode = 0 }
        if fieldName == "loopCondition" { g_resolvedFieldIndex = 9; g_resolvedFieldTypeCode = 0 }
        if fieldName == "loopBody" { g_resolvedFieldIndex = 10; g_resolvedFieldTypeCode = 0 }
        return lb + " %SeenString, %SeenString, %TypeNode, i64, i64, i64, i64, i64, i64, i64, i64 " + rb
    }
    if structType == "ParserExpressionNode" {
        if fieldName == "kind" { g_resolvedFieldIndex = 0; g_resolvedFieldTypeCode = 3 }
        if fieldName == "operands" { g_resolvedFieldIndex = 1; g_resolvedFieldTypeCode = 2 }
        if fieldName == "operator" { g_resolvedFieldIndex = 2; g_resolvedFieldTypeCode = 3 }
        if fieldName == "literalValue" { g_resolvedFieldIndex = 3; g_resolvedFieldTypeCode = 3 }
        if fieldName == "literalType" { g_resolvedFieldIndex = 4; g_resolvedFieldTypeCode = 3 }
        if fieldName == "variableName" { g_resolvedFieldIndex = 5; g_resolvedFieldTypeCode = 3 }
        if fieldName == "callee" { g_resolvedFieldIndex = 6; g_resolvedFieldTypeCode = 3 }
        if fieldName == "arguments" { g_resolvedFieldIndex = 7; g_resolvedFieldTypeCode = 2 }
        return lb + " %SeenString, ptr, %SeenString, %SeenString, %SeenString, %SeenString, %SeenString, ptr " + rb
    }
    return ""
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_known_struct_field_types.seen" <<'SEEN'
fun getFieldTypeForKnownStructImpl(structName: String, fieldName: String)
    r: String {
    if structName == "StatementNode" {
        if fieldName == "kind" { return "%SeenString" }
        if fieldName == "variableName" { return "%SeenString" }
        if fieldName == "variableType" { return "%TypeNode" }
        if fieldName == "initializer" { return "i64" }
        if fieldName == "returnValue" { return "i64" }
        if fieldName == "expression" { return "i64" }
        if fieldName == "condition" { return "i64" }
        if fieldName == "thenBranch" { return "i64" }
        if fieldName == "elseBranch" { return "i64" }
        if fieldName == "loopCondition" { return "i64" }
        if fieldName == "loopBody" { return "i64" }
    }
    if structName == "ParserExpressionNode" {
        if fieldName == "kind" { return "%SeenString" }
        if fieldName == "operands" { return "ptr" }
        if fieldName == "operator" { return "%SeenString" }
        if fieldName == "literalValue" { return "%SeenString" }
        if fieldName == "literalType" { return "%SeenString" }
        if fieldName == "variableName" { return "%SeenString" }
        if fieldName == "callee" { return "%SeenString" }
        if fieldName == "arguments" { return "ptr" }
    }
    return ""
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_type_tables.seen" <<'SEEN'
fun registerKnownDataTypesImpl(structNames: Array<String>,
    structLayouts: Array<String>, structFieldNames: Array<String>,
    structFieldTypes: Array<String>, structLlvmFieldTypes: Array<String>,
    structSizes: Array<Int>, structMethodNames: Array<String>,
    structMethodRetTypes: Array<String>) r: Void {

    structNames.push("StatementNode")
    structLayouts.push("{ %SeenString, %SeenString, %TypeNode, i64, i64, i64, i64, i64, i64, i64, i64 }")
    structFieldNames.push("kind,variableName,variableType,initializer,returnValue,expression,condition,thenBranch,elseBranch,loopCondition,loopBody")
    structFieldTypes.push("String|String|TypeNode|ParserExpressionNode|ParserExpressionNode|ParserExpressionNode|ParserExpressionNode|BlockNode|BlockNode|ParserExpressionNode|BlockNode")
    structLlvmFieldTypes.push("")
    structSizes.push(120)
    structMethodNames.push("")
    structMethodRetTypes.push("")

    structNames.push("ParserExpressionNode")
    structLayouts.push("{ %SeenString, ptr, %SeenString, %SeenString, %SeenString, %SeenString, %SeenString, ptr }")
    structFieldNames.push("kind,operands,operator,literalValue,literalType,variableName,callee,arguments")
    structFieldTypes.push("String|Array<ParserExpressionNode>|String|String|String|String|String|Array<ParserExpressionNode>")
    structLlvmFieldTypes.push("")
    structSizes.push(112)
    structMethodNames.push("")
    structMethodRetTypes.push("")
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_codegen_global_state.seen" <<'SEEN'
var generatedFunctions: Array<String> = Array<String>()
var funcNames: Array<String> = Array<String>()
var funcRetTypes: Array<String> = Array<String>()
var structNames: Array<String> = Array<String>()
var structLayouts: Array<String> = Array<String>()
var structFieldNames: Array<String> = Array<String>()
var structFieldTypes: Array<String> = Array<String>()
var structLlvmFieldTypes: Array<String> = Array<String>()
var structSizes: Array<Int> = Array<Int>()
var structMethodNames: Array<String> = Array<String>()
var structMethodRetTypes: Array<String> = Array<String>()
var blockTerminated: Bool = false
var declOrdinalNames: Array<String> = Array<String>()
var declOrdinalCounts: Array<Int> = Array<Int>()
var moduleConstantTypes: Array<String> = Array<String>()
var stringConstantPrefix: String = ""
var currentFunctionReturnType: String = ""
var currentClassName: String = ""
var currentLoopCondLabel: String = ""
var currentLoopEndLabel: String = ""
var pendingArrayLiteralType: String = ""
var activeVarCount: Int = 0
var preAllocatedRegs: Array<String> = Array<String>()
var preAllocatedTypes: Array<String> = Array<String>()
var preAllocatedVars: Array<String> = Array<String>()

fun prepareFunctionGenerationIdentityWithGlobalStateImpl(fn: FunctionNode,
    resolvedFunctionReturnType: String) r: FunctionGenerationIdentitySnapshot {
    return FunctionGenerationIdentitySnapshot.new()
}

fun tryHandleExternFunctionGenerationWithGlobalStateImpl(
    output: StringBuilder, fn: FunctionNode) r: Bool {
    return false
}

fun getBlockTerminatedWithGlobalStateImpl() r: Bool {
    return blockTerminated
}

fun getGlobalFuncNamesImpl() r: Array<String> {
    return funcNames
}

fun getGlobalFuncRetTypesImpl() r: Array<String> {
    return funcRetTypes
}

fun getFunctionReturnTypeWithGlobalStateImpl(name: String) r: String {
    return ""
}

fun getGlobalStructNamesImpl() r: Array<String> {
    return structNames
}

fun getGlobalStructLayoutsImpl() r: Array<String> {
    return structLayouts
}

fun getGlobalStructFieldNamesImpl() r: Array<String> {
    return structFieldNames
}

fun getGlobalStructFieldTypesImpl() r: Array<String> {
    return structFieldTypes
}

fun getGlobalStructLlvmFieldTypesImpl() r: Array<String> {
    return structLlvmFieldTypes
}

fun getGlobalStructSizesImpl() r: Array<Int> {
    return structSizes
}

fun getGlobalStructMethodNamesImpl() r: Array<String> {
    return structMethodNames
}

fun getGlobalStructMethodRetTypesImpl() r: Array<String> {
    return structMethodRetTypes
}

fun setBlockTerminatedWithGlobalStateImpl(value: Bool) r: Void {
    blockTerminated = value
}

fun setStringConstantPrefixWithGlobalStateImpl(prefix: String) r: Void {
    stringConstantPrefix = prefix
}

fun getStringConstantPrefixWithGlobalStateImpl() r: String {
    return stringConstantPrefix
}

fun getCurrentFunctionReturnTypeWithGlobalStateImpl() r: String {
    return currentFunctionReturnType
}

fun setCurrentFunctionReturnTypeWithGlobalStateImpl(value: String) r: Void {
    currentFunctionReturnType = value
}

fun getGlobalCurrentClassNameImpl() r: String {
    return currentClassName
}

fun setGlobalCurrentClassNameImpl(value: String) r: Void {
    currentClassName = value
}

fun getCurrentLoopCondLabelWithGlobalStateImpl() r: String {
    return currentLoopCondLabel
}

fun setCurrentLoopCondLabelWithGlobalStateImpl(value: String) r: Void {
    currentLoopCondLabel = value
}

fun getCurrentLoopEndLabelWithGlobalStateImpl() r: String {
    return currentLoopEndLabel
}

fun setCurrentLoopEndLabelWithGlobalStateImpl(value: String) r: Void {
    currentLoopEndLabel = value
}

fun getPendingArrayLiteralTypeWithGlobalStateImpl() r: String {
    return pendingArrayLiteralType
}

fun setPendingArrayLiteralTypeWithGlobalStateImpl(value: String) r: Void {
    pendingArrayLiteralType = value
}

fun getActiveVarCountWithGlobalStateImpl() r: Int {
    return activeVarCount
}

fun getBoundedActiveVarCountWithGlobalStateImpl(
    varNames: Array<String>) r: Int {

    if activeVarCount < 0 {
        return 0
    }
    if activeVarCount > varNames.length() {
        return varNames.length()
    }
    return activeVarCount
}

fun setActiveVarCountWithGlobalStateImpl(value: Int) r: Void {
    activeVarCount = value
}

fun getPreAllocatedRegsWithGlobalStateImpl() r: Array<String> {
    return preAllocatedRegs
}

fun setPreAllocatedRegsWithGlobalStateImpl(value: Array<String>) r: Void {
    preAllocatedRegs = value
}

fun getPreAllocatedTypesWithGlobalStateImpl() r: Array<String> {
    return preAllocatedTypes
}

fun setPreAllocatedTypesWithGlobalStateImpl(value: Array<String>) r: Void {
    preAllocatedTypes = value
}

fun getPreAllocatedVarsWithGlobalStateImpl() r: Array<String> {
    return preAllocatedVars
}

fun setPreAllocatedVarsWithGlobalStateImpl(value: Array<String>) r: Void {
    preAllocatedVars = value
}

fun prepareLetStatementPlanWithGlobalStateImpl(stmt: StatementNode,
    declaredType: String, initializerType: String) r: LetStatementPlan {
    return LetStatementPlan.new()
}

fun resolveLetPreAllocatedRegWithGlobalStateImpl(preIdx: Int) r: String {
    return ""
}

fun resolveFunctionBodyForInLoopVariableStorageWithGlobalStateImpl(
    output: StringBuilder, regBox: Array<Int>, loopVarName: String,
    loopVarType: String, loopVarLlvmType: String)
    r: FunctionBodyLoopVariableStorage {
    return FunctionBodyLoopVariableStorage.new(loopVarType, "")
}

fun resolveFunctionBodyForInIndexAllocaWithGlobalStateImpl(
    output: StringBuilder, regBox: Array<Int>, idxName: String) r: String {
    return ""
}

fun getModuleConstantTypeWithGlobalStateImpl(idx: Int) r: String {
    return ""
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_codegen_feature_state.seen" <<'SEEN'
var g_traitImplRegistry: String = ""
var g_traitImplCount: Int = 0
var g_dynTraitNames: String = ""
var g_comptimeParamFuncNames: String = ""
var g_comptimeParamCount: Int = 0
var g_funcDefaultsArr: String = ""
var g_dynParamFuncs: String = ""
var g_funcParamCountArr: String = ""
var g_regBox: Array<Int> = Array<Int>()
var g_blockBox: Array<Int> = Array<Int>()
var g_closureDefs: String = ""
var g_initFuncNames: String = ""
var g_initFuncCount: Int = 0
var g_sanitizerMode: String = ""
var g_bitfieldKeys: String = ""
var g_bitfieldWidths: String = ""
var g_bitfieldCount: Int = 0

fun getFeatureBitfieldKeysImpl() r: String {
    return g_bitfieldKeys
}

fun getFeatureBitfieldWidthsImpl() r: String {
    return g_bitfieldWidths
}

fun getFeatureBitfieldCountImpl() r: Int {
    return g_bitfieldCount
}

fun findFeatureBitfieldWidthImpl(bitfieldKey: String) r: Int {
    return 0
}

fun prepareFunctionPreBodyWithFeatureStateImpl(state: CodegenState,
    fn: FunctionNode, implFuncName: String,
    resolvedFunctionReturnType: String) r: Void {
}

fun emitTraitVtableConstantsWithFeatureStateImpl(output: StringBuilder) r: Void {
}

fun emitGeneratedClosuresWithFeatureStateImpl(output: StringBuilder) r: Void {
}

fun emitGlobalConstructorsWithFeatureStateImpl(output: StringBuilder) r: Void {
}

fun emitTBAAMetadataWithFeatureStateImpl(output: StringBuilder) r: Void {
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_codegen_module_state.seen" <<'SEEN'
var g_xmDeclareNames: String = ""
var g_lateXmDeclareNames: String = ""
var g_lateXmDeclareStrs: String = ""
var g_lateXmDeclareCount: Int = 0
var g_moduleConstMutable: String = ""
var g_moduleConstSeenTypes: String = ""
var g_xmConstNames: String = ""
var g_xmConstTypes: String = ""
var g_xmConstMutable: String = ""
var g_xmConstSeenTypes: String = ""
var g_xmConstCount: Int = 0

fun emitLateUserDeclaresWithModuleStateImpl(output: StringBuilder,
    definedFuncs: Array<String>) r: Void {
}

fun findModuleConstantWithModuleStateImpl(name: String) r: Int {
    return -1
}

fun getModuleConstantTypeWithModuleStateImpl(idx: Int) r: String {
    return ""
}

fun getModuleConstSeenTypeWithModuleStateImpl(idx: Int) r: String {
    return ""
}

fun isModuleConstMutableWithModuleStateImpl(idx: Int) r: Bool {
    return false
}

fun moduleConstantInvariantLoadSuffixWithModuleStateImpl(idx: Int) r: String {
    return ""
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_codegen_metrics_state.seen" <<'SEEN'
var g_callCountNamesArr: Array<String> = Array<String>()
var g_selfCallFuncs: String = ""
var g_selfCallNames: String = ""
var g_selfCallCount: Int = 0
var g_mlReplayEnabled: Bool = false
var g_mlReplayLog: String = ""
var g_reductionLoopIds: String = ""
var g_nestedLoopIds: String = ""
var g_loopTileHints: String = ""

fun prepareFunctionPreludeAnalysisWithMetricsStateImpl(output: StringBuilder,
    fn: FunctionNode, fnName: String, funcAttrs: String)
    r: FunctionPreludeAnalysisSnapshot {
    return FunctionPreludeAnalysisSnapshot.new()
}

fun emitLoopMetadataWithMetricsStateImpl(output: StringBuilder,
    options: FunctionLoweringOptions) r: Void {
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/ir_function_entry_exit.seen" <<'SEEN'
fun emitFunctionEntrySetupSnapshotImpl(state: CodegenState, fn: FunctionNode,
    fnName: String, returnType: String, funcAttrs: String)
    r: FunctionEntrySetupSnapshot {
    return FunctionEntrySetupSnapshot.new()
}

fun emitFunctionExitResetSnapshotImpl(state: CodegenState, fn: FunctionNode,
    returnType: String) r: FunctionRuntimeStateSnapshot {
    return FunctionRuntimeStateSnapshot.new()
}
SEEN
    cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun ok(fn: FunctionNode) r: Void {
    let ident = prepareFunctionGenerationIdentityWithGlobalStateImpl(fn,
        resolveFunctionReturnType(fn))
}
SEEN
}

expect_fail() {
    local name="$1"
    if python3 "$ROOT_DIR/scripts/check_codegen_abi_boundaries.py" "$TMP_DIR" >/tmp/"$name".out 2>/tmp/"$name".err; then
        echo "FAIL: $name unexpectedly passed"
        cat /tmp/"$name".out
        cat /tmp/"$name".err
        exit 1
    fi
}

write_owner_modules
python3 "$ROOT_DIR/scripts/check_codegen_abi_boundaries.py" "$TMP_DIR" >/dev/null

cat > "$TMP_DIR/compiler_seen/src/codegen/ir_field_layout.seen" <<'SEEN'
fun generateFieldAccessImpl(output: StringBuilder, regBox: Array<Int>,
    ptrReg: String, structType: String, fieldName: String,
    llvmFieldType: String, structNames: Array<String>,
    structLayouts: Array<String>, structFieldNames: Array<String>,
    structFieldTypes: Array<String>, reprCClassNames: String,
    unionTypes: String, typeAliasNames: String, typeAliasTargets: String,
    typeAliasCount: Int, enumTypeNames: String, bitfieldCount: Int,
    bitfieldKeys: String, bitfieldWidths: String) r: String {
    return ""
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/bad_field_access_arity.seen" <<'SEEN'
fun bad(output: StringBuilder, regBox: Array<Int>) r: String {
    return generateFieldAccessImpl(output, regBox, ptrReg, "Thing",
        "field", structNames, structLayouts, structFieldNames,
        structFieldTypes, reprCClassNames, unionTypes, typeAliasNames,
        typeAliasTargets, typeAliasCount, enumTypeNames, bitfieldCount,
        bitfieldKeys, bitfieldWidths)
}
SEEN
expect_fail field_access_helper_arity
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_field_layout.seen" \
    "$TMP_DIR/compiler_seen/src/codegen/bad_field_access_arity.seen"

cat > "$TMP_DIR/compiler_seen/src/codegen/bad_import.seen" <<'SEEN'
import codegen.ir_codegen_global_state.{generatedFunctions}
SEEN
expect_fail bad_import

rm -f "$TMP_DIR/compiler_seen/src/codegen/bad_import.seen"
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
import codegen.ir_codegen_global_state.{blockTerminated}

fun bad() r: Void {
    if blockTerminated {
        return
    }
}
SEEN
expect_fail block_terminated_direct_facade_import

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    stringConstantPrefix = "M0."
}
SEEN
expect_fail string_prefix_direct_facade_write

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(output: StringBuilder) r: Void {
    emitAdditionalGeneratedStringConstantsImpl(output, strings,
        stringConstantPrefix, 0)
}
SEEN
expect_fail string_prefix_direct_facade_read

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    if blockTerminated {
        return
    }
}
SEEN
expect_fail block_terminated_direct_facade_read

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
import codegen.ir_codegen_global_state.{currentFunctionReturnType}

fun bad() r: Void {
    let retType = mapTypeState(currentFunctionReturnType)
}
SEEN
expect_fail current_return_type_direct_facade_import

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    currentFunctionReturnType = "Int"
}
SEEN
expect_fail current_return_type_direct_facade_write

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    currentLoopCondLabel = "loop.cond"
}
SEEN
expect_fail loop_label_direct_facade_write

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    pendingArrayLiteralType = "Array<Int>"
}
SEEN
expect_fail pending_array_type_direct_facade_write

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    activeVarCount = activeVarCount + 1
}
SEEN
expect_fail active_var_count_direct_facade_write

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
import codegen.ir_codegen_global_state.{getActiveVarCountWithGlobalStateImpl}

class LLVMIRGenerator {
    var varNames: Array<String>

    fun bad() r: Int {
        return getActiveVarCountWithGlobalStateImpl()
    }
}
SEEN
expect_fail active_var_count_unbounded_facade_getter

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
import codegen.ir_codegen_global_state.{preAllocatedTypes}

fun bad() r: Int {
    return preAllocatedTypes.length()
}
SEEN
expect_fail preallocated_types_direct_facade_import

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
import codegen.ir_codegen_global_state.{structNames}

fun bad(name: String) r: Int {
    return findStructReg(name, structNames)
}
SEEN
expect_fail registry_struct_names_direct_facade_import

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(idx: Int) r: String {
    return structLayouts[idx]
}
SEEN
expect_fail registry_struct_layouts_direct_facade_read

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
import codegen.ir_codegen_feature_state.{g_bitfieldWidths}

fun bad() r: Int {
    return findBitfieldWidthImpl("Flags.enabled", "", g_bitfieldWidths)
}
SEEN
expect_fail bitfield_widths_direct_facade_import

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_field_layout.seen" <<'SEEN'
import codegen.ir_bitfield_access.{findBitfieldWidthImpl}

fun bad(bitfieldKeys: String, bitfieldWidths: String) r: Int {
    return findBitfieldWidthImpl("Flags.enabled", bitfieldKeys,
        bitfieldWidths)
}
SEEN
expect_fail bitfield_width_raw_helper
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_field_layout.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_static_dispatch.seen" <<'SEEN'
fun emitPreparedStaticMethodDispatchState(output: StringBuilder,
    regBox: Array<Int>, methodName: String, potentialClassName: String,
    staticClassName: String, funcName: String,
    registeredStaticReturnType: String, inferredStaticReturnType: String,
    argRegs: Array<String>, argTypeStrs: Array<String>,
    structNames: Array<String>, reprCClassNames: String,
    typeAliasNames: String, typeAliasTargets: String, typeAliasCount: Int,
    enumTypeNames: String, lateState: LateUserDeclareState) r: String {
    return ""
}
SEEN
expect_fail static_dispatch_metadata_signature
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_static_dispatch.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_static_dispatch.seen" <<'SEEN'
fun emitPreparedStaticMethodDispatchState(output: StringBuilder,
    regBox: Array<Int>, methodName: String, potentialClassName: String,
    staticClassName: String, funcName: String,
    registeredStaticReturnType: String, inferredStaticReturnType: String,
    argRegs: Array<String>, argTypeStrs: Array<String>,
    lateState: LateUserDeclareState) r: String {
    return ""
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    emitPreparedStaticMethodDispatchState(output, g_regBox, methodName,
        potentialClassName, staticClassName, funcName, retType, inferredType,
        argRegs, argTypeStrs, structNames, g_reprCClassNames,
        g_typeAliasNames, g_typeAliasTargets, g_typeAliasCount,
        g_enumTypeNames, lateState)
}
SEEN
expect_fail static_dispatch_metadata_call
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_static_dispatch.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_finalize.seen" <<'SEEN'
fun emitPreparedFinalMethodDispatchState(output: StringBuilder,
    regBox: Array<Int>, methodName: String, instanceTypeName: String,
    funcName: String, receiverReg: String, receiverType: String,
    registeredMethodReturnType: String, inferredMethodReturnType: String,
    argRegs: Array<String>, argTypeStrs: Array<String>,
    shouldPrepareReceiverAbi: Bool, shouldTrySpecialized: Bool,
    shouldEmitUserMethod: Bool, structNames: Array<String>,
    reprCClassNames: String, typeAliasNames: String,
    typeAliasTargets: String, typeAliasCount: Int, enumTypeNames: String,
    lateState: LateUserDeclareState) r: String {
    return ""
}
SEEN
expect_fail final_dispatch_metadata_signature
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_finalize.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_finalize.seen" <<'SEEN'
fun emitPreparedFinalMethodDispatchState(output: StringBuilder,
    regBox: Array<Int>, methodName: String, instanceTypeName: String,
    funcName: String, receiverReg: String, receiverType: String,
    registeredMethodReturnType: String, inferredMethodReturnType: String,
    argRegs: Array<String>, argTypeStrs: Array<String>,
    shouldPrepareReceiverAbi: Bool, shouldTrySpecialized: Bool,
    shouldEmitUserMethod: Bool, lateState: LateUserDeclareState) r: String {
    return ""
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    emitPreparedFinalMethodDispatchState(output, g_regBox, methodName,
        instanceTypeName, funcName, receiverReg, receiverType, retType,
        inferredType, argRegs, argTypeStrs, true, true, true, structNames,
        g_reprCClassNames, g_typeAliasNames, g_typeAliasTargets,
        g_typeAliasCount, g_enumTypeNames, lateState)
}
SEEN
expect_fail final_dispatch_metadata_call
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_finalize.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_class_method_gen.seen" <<'SEEN'
fun collectClassMethodParameterInfoImpl(methods: Array<FunctionNode>,
    methodIdx: Int, normalizedMethodParamTypes: Array<String>,
    mappedMethodParamTypes: Array<String>,
    quotedMethodArgNames: Array<String>, structNames: Array<String>,
    typeAliasNames: String, typeAliasTargets: String, typeAliasCount: Int,
    enumTypeNames: String) r: Void {
}

fun emitClassMethodParameterBindingsStateImpl(output: StringBuilder,
    regBox: Array<Int>, varNames: Array<String>, varRegs: Array<String>,
    varTypes: Array<String>, activeVarCount: Int, methods: Array<FunctionNode>,
    methodIdx: Int, explicitReceiverName: String,
    normalizedMethodParamTypes: Array<String>,
    quotedMethodArgNames: Array<String>) r: Int {
    return activeVarCount
}
SEEN
expect_fail class_method_param_metadata_signature
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_class_method_gen.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_class_method_gen.seen" <<'SEEN'
fun collectClassMethodParameterInfoImpl(methods: Array<FunctionNode>,
    methodIdx: Int, normalizedMethodParamTypes: Array<String>,
    mappedMethodParamTypes: Array<String>,
    quotedMethodArgNames: Array<String>) r: Void {
}

fun emitClassMethodParameterBindingsStateImpl(output: StringBuilder,
    regBox: Array<Int>, varNames: Array<String>, varRegs: Array<String>,
    varTypes: Array<String>, activeVarCount: Int, methods: Array<FunctionNode>,
    methodIdx: Int, explicitReceiverName: String,
    normalizedMethodParamTypes: Array<String>,
    quotedMethodArgNames: Array<String>) r: Int {
    return activeVarCount
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    collectClassMethodParameterInfoImpl(methods, methodIdx, normalizedTypes,
        mappedTypes, quotedNames, structNames, g_typeAliasNames,
        g_typeAliasTargets, g_typeAliasCount, g_enumTypeNames)
}
SEEN
expect_fail class_method_param_metadata_call
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_class_method_gen.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_function_entry_exit.seen" <<'SEEN'
fun emitPreAllocatedAllocasImpl(output: StringBuilder, regBox: Array<Int>,
    preAllocatedTypes: Array<String>, structNames: Array<String>,
    typeAliasNames: String, typeAliasTargets: String, typeAliasCount: Int,
    enumTypeNames: String, reprCClassNames: String) r: Array<String> {
    return Array<String>()
}
SEEN
expect_fail preallocated_alloca_metadata_signature

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_function_entry_exit.seen" <<'SEEN'
fun emitPreAllocatedAllocasImpl(output: StringBuilder, regBox: Array<Int>,
    preAllocatedLlvmTypes: Array<String>) r: Array<String> {
    return Array<String>()
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    emitPreAllocatedAllocasImpl(output, g_regBox, preAllocatedTypes,
        structNames, g_typeAliasNames, g_typeAliasTargets,
        g_typeAliasCount, g_enumTypeNames, g_reprCClassNames)
}
SEEN
expect_fail preallocated_alloca_metadata_call

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(fn: FunctionNode) r: Void {
    if tryHandleExternFunctionGenerationStateImpl(output, fn,
        getGlobalStructNamesImpl(), g_typeAliasNames, g_typeAliasTargets,
        g_typeAliasCount, g_enumTypeNames, g_reprCClassNames,
        getGlobalFuncNamesImpl(), getGlobalFuncRetTypesImpl()) {
        return
    }
}
SEEN
expect_fail extern_generation_direct_facade_call

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(fn: FunctionNode) r: Void {
    if tryHandleExternFunctionGenerationWithGlobalStateImpl(output, fn,
        getGlobalFuncNamesImpl(), getGlobalFuncRetTypesImpl()) {
        return
    }
}
SEEN
expect_fail extern_generation_owner_facade_state_call

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_lower_emit.seen" <<'SEEN'
fun emitPreparedStringBuilderMethodLowerImpl(output: StringBuilder,
    regBox: Array<Int>, methodName: String, receiverReg: String,
    receiverType: String, sbArg0: String, isFloatFused: Int,
    reprCClassNames: String, structNames: Array<String>) r: String {
    return ""
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_fastpath.seen" <<'SEEN'
fun emitStringBuilderReceiverMethodCallImpl(output: StringBuilder,
    regBox: Array<Int>, methodName: String, receiverReg: String,
    receiverType: String, sbArg0: String, isFloatFused: Int) r: String {
    return ""
}
SEEN
expect_fail string_builder_lower_metadata_signature
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_lower_emit.seen"
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_fastpath.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_lower_emit.seen" <<'SEEN'
fun emitPreparedStringBuilderMethodLowerImpl(output: StringBuilder,
    regBox: Array<Int>, methodName: String, receiverReg: String,
    receiverType: String, sbArg0: String, isFloatFused: Int) r: String {
    return ""
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_fastpath.seen" <<'SEEN'
fun emitStringBuilderReceiverMethodCallImpl(output: StringBuilder,
    regBox: Array<Int>, methodName: String, receiverReg: String,
    receiverType: String, sbArg0: String, isFloatFused: Int) r: String {
    return ""
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    emitPreparedStringBuilderMethodLowerImpl(output, getFeatureRegBoxImpl(),
        methodName, receiverReg, receiverType, sbArg0, isFloatFused,
        g_reprCClassNames, getGlobalStructNamesImpl())
}
SEEN
expect_fail string_builder_lower_metadata_call
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_lower_emit.seen"
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_fastpath.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_lower_emit.seen" <<'SEEN'
fun emitPreparedStringBuilderMethodLowerImpl(output: StringBuilder,
    regBox: Array<Int>, methodName: String, receiverReg: String,
    receiverType: String, sbArg0: String, isFloatFused: Int) r: String {
    return ""
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_method_fastpath.seen" <<'SEEN'
fun emitStringBuilderReceiverMethodCallImpl(output: StringBuilder,
    regBox: Array<Int>, methodName: String, receiverReg: String,
    receiverType: String, sbArg0: String, isFloatFused: Int) r: String {
    let sbPtrReg = convertReceiverToPtrImpl(output, regBox, receiverReg,
        receiverType, g_reprCClassNames, getGlobalStructNamesImpl())
    return ""
}
SEEN
expect_fail string_builder_lower_generic_receiver_conversion
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_lower_emit.seen"
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_method_fastpath.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(stmt: StatementNode) r: Void {
    let plan = prepareLetStatementPlanImpl(stmt, declOrdinalNames,
        declOrdinalCounts, preAllocatedVars, preAllocatedTypes, "Int", "")
}
SEEN
expect_fail declaration_storage_direct_facade_call

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    let count = declOrdinalNames.length()
}
SEEN
expect_fail declaration_storage_direct_facade_read

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(name: String) r: Int {
    let lookup = findModuleConstantSnapshotImpl(name, moduleConstantNames,
        moduleConstantTypes, moduleConstantValues, g_moduleConstMutable,
        g_moduleConstSeenTypes, g_xmConstNames, g_xmConstTypes,
        g_xmConstMutable, g_xmConstSeenTypes, g_xmConstCount)
    return lookup.constIndex
}
SEEN
expect_fail module_constant_direct_snapshot

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(idx: Int) r: String {
    return moduleConstantTypes[idx]
}
SEEN
expect_fail module_constant_direct_array

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(idx: Int) r: Bool {
    return isModuleConstMutableImpl(idx, g_moduleConstMutable)
}
SEEN
expect_fail module_constant_direct_module_state

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(name: String) r: String {
    return getFunctionReturnTypeStateImpl(name, funcNames, funcRetTypes)
}
SEEN
expect_fail function_registry_direct_facade_read

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/bad_func_registry_signature.seen" <<'SEEN'
fun bad(funcNames: String, funcRetTypes: String) r: Void {
}
SEEN
expect_fail function_registry_helper_signature
rm -f "$TMP_DIR/compiler_seen/src/codegen/bad_func_registry_signature.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/bad_late_declare_api.seen" <<'SEEN'
fun recordLateUserDeclareIntoStateImpl(name: String, llvmReturnType: String,
    argTypeStrs: Array<String>, hasImplicitThis: Int,
    funcNames: Array<String>, funcRetTypes: Array<String>,
    xmDeclareNames: String, xmDeclareCount: Int,
    state: LateUserDeclareState) r: Void {
}
SEEN
expect_fail late_declare_deep_stack_api
rm -f "$TMP_DIR/compiler_seen/src/codegen/bad_late_declare_api.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_field_indices.seen" <<'SEEN'
fun getKnownFieldIndexPrimaryTableImpl(structName: String, fieldName: String)
    r: Int {
    if structName == "StatementNode" {
        if fieldName == "returnValue" { return 10 }
    }
    if structName == "ParserExpressionNode" {
        if fieldName == "kind" { return 0 }
        if fieldName == "operands" { return 1 }
        if fieldName == "operator" { return 2 }
        if fieldName == "literalValue" { return 3 }
        if fieldName == "literalType" { return 4 }
        if fieldName == "variableName" { return 5 }
        if fieldName == "callee" { return 6 }
        if fieldName == "arguments" { return 7 }
    }
    return -1
}
SEEN
expect_fail ast_statement_index_drift

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_known_struct_field_types.seen" <<'SEEN'
fun getFieldTypeForKnownStructImpl(structName: String, fieldName: String)
    r: String {
    if structName == "StatementNode" {
        if fieldName == "kind" { return "%SeenString" }
        if fieldName == "variableName" { return "%SeenString" }
        if fieldName == "variableType" { return "%TypeNode" }
        if fieldName == "initializer" { return "i64" }
        if fieldName == "returnValue" { return "i64" }
        if fieldName == "expression" { return "i64" }
        if fieldName == "condition" { return "i64" }
        if fieldName == "thenBranch" { return "i64" }
        if fieldName == "elseBranch" { return "i64" }
        if fieldName == "loopCondition" { return "i64" }
        if fieldName == "loopBody" { return "i64" }
    }
    if structName == "ParserExpressionNode" {
        if fieldName == "kind" { return "%SeenString" }
        if fieldName == "callee" { return "%SeenString" }
        if fieldName == "literalValue" { return "%SeenString" }
        if fieldName == "literalType" { return "%SeenString" }
        if fieldName == "operands" { return "ptr" }
        if fieldName == "operator" { return "%SeenString" }
        if fieldName == "arguments" { return "ptr" }
        if fieldName == "typeArgs" { return "ptr" }
    }
    return ""
}
SEEN
expect_fail ast_expression_type_table_drift

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_codegen_global_state.seen" <<'SEEN'
var generatedFunctions: Array<String> = Array<String>()
var funcNames: Array<String> = Array<String>()
var funcRetTypes: Array<String> = Array<String>()

fun prepareFunctionGenerationIdentityWithGlobalStateImpl(fn: FunctionNode,
    dynTraitNames: String, traitImplRegistry: String, traitImplCount: Int,
    resolvedFunctionReturnType: String) r: FunctionGenerationIdentitySnapshot {
    return FunctionGenerationIdentitySnapshot.new()
}
SEEN
expect_fail identity_signature

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(fn: FunctionNode) r: Void {
    let ident = prepareFunctionGenerationIdentityWithGlobalStateImpl(
        fn, g_dynTraitNames, g_traitImplRegistry, g_traitImplCount,
        resolveFunctionReturnType(fn))
}
SEEN
expect_fail identity_call

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(fn: FunctionNode) r: Void {
    let preBodyState = prepareFunctionPreBodyStateSnapshotImpl(
        getSharedCodegenState(), fn, implFuncName,
        resolvedFunctionReturnType, g_comptimeParamFuncNames,
        g_comptimeParamCount, g_funcDefaultsArr, g_dynParamFuncs,
        g_funcParamCountArr)
    g_dynParamFuncs = preBodyState.dynParamFuncs
}
SEEN
expect_fail prebody_direct_call

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_codegen_feature_state.seen" <<'SEEN'
fun prepareFunctionPreBodyWithFeatureStateImpl(state: CodegenState,
    fn: FunctionNode, implFuncName: String,
    resolvedFunctionReturnType: String, g_dynParamFuncs: String) r: Void {
}
SEEN
expect_fail prebody_owner_signature

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_call_driver.seen" <<'SEEN'
fun generateCallState(ctx: CodegenLoweringContext,
    output: StringBuilder, expr: ParserExpressionNode,
    varNames: Array<String>, varRegs: Array<String>,
    varTypes: Array<String>, stringConstants: Array<String>,
    boundsCheckEnabled: Int, boundsCheckLabelCount: Int,
    comptimeParamCount: Int, comptimeParamFuncNames: String,
    insideUnsafe: Int, currentClassParentName: String,
    asyncFuncNames: String, asyncFuncCount: Int,
    enumVariantNames: Array<String>, enumVariantFieldCounts: Array<Int>,
    enumVariantCount: Int) r: CallDriverResult {

    return CallDriverResult.new("", boundsCheckLabelCount)
}
SEEN
expect_fail call_driver_enum_state_signature

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    returnTypeBox[0] = mapFunctionPreBodyReturnTypeImpl(
        currentFunctionReturnType, structNames, g_typeAliasNames,
        g_typeAliasTargets, g_typeAliasCount, g_enumTypeNames,
        g_reprCClassNames)
}
SEEN
expect_fail prebody_return_type_map

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun prepareFunctionPreBodyAndTryEmitMain(fn: FunctionNode,
    implFuncName: String, resolvedFunctionReturnType: String,
    returnTypeBox: Array<String>) r: Bool {

    prepareFunctionPreBodyWithFeatureStateImpl(getSharedCodegenState(),
        fn, implFuncName, resolvedFunctionReturnType)
    writeBackState()
    returnTypeBox[0] = mapTypeState(currentFunctionReturnType)
    return false
}
SEEN
expect_fail prebody_current_return_type_map

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad() r: Void {
    let mainEntry = emitMainFunctionEntrySnapshotImpl(output, g_regBox,
        returnTypeBox[0], preAllocatedTypes, structNames, g_typeAliasNames,
        g_typeAliasTargets, g_typeAliasCount, g_enumTypeNames,
        g_reprCClassNames)
}
SEEN
expect_fail main_entry_snapshot

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun prepareFunctionPreBodyAndTryEmitMain(fn: FunctionNode,
    implFuncName: String, resolvedFunctionReturnType: String,
    returnTypeBox: Array<String>) r: Bool {

    let mainReturnType = emitMainFunctionEntryAndAllocas(returnTypeBox[0])
    return true
}

fun emitMainFunctionEntryAndAllocas(returnType: String) r: String {
    preAllocatedRegs = Array<String>()
    var i = 0
    while i < preAllocatedTypes.length() {
        i = i + 1
    }
    return returnType
}
SEEN
expect_fail main_entry_facade_helper

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
import codegen.ir_codegen_feature_state.{g_regBox, getFeatureRegBoxImpl}

fun bad(output: StringBuilder) r: Void {
    emitSomething(output, g_regBox)
}
SEEN
expect_fail direct_feature_reg_box

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(output: StringBuilder, fn: FunctionNode, fnName: String,
    funcAttrs: String) r: Void {

    let preludeAnalysis = prepareFunctionPreludeAnalysisSnapshotImpl(
        output, fn, fnName, funcAttrs, g_mlReplayEnabled,
        g_mlReplayLog, g_callCountNamesArr, g_selfCallFuncs,
        g_selfCallNames, g_selfCallCount)
}
SEEN
expect_fail prelude_direct_metrics

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(state: CodegenState, fn: FunctionNode, fnName: String,
    returnType: String, funcAttrs: String) r: Void {

    let entrySetup = emitFunctionEntrySetupSnapshotImpl(state, fn, fnName,
        returnType, funcAttrs, g_typeAliasNames, g_typeAliasTargets,
        g_typeAliasCount, g_enumTypeNames, g_reprCClassNames)
}
SEEN
expect_fail entry_setup_direct_feature_state

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(state: CodegenState, fn: FunctionNode, returnType: String,
    functionEmission: FunctionEmissionState) r: Void {

    let exitState = emitFunctionExitResetSnapshotImpl(state, fn, returnType,
        currentFunctionProfileFlagIntImpl(functionEmission),
        functionEmission.profiledName, 0, functionEmission.asyncCoroId,
        functionEmission.asyncCoroHdl, functionEmission.asyncPromise,
        functionEmission.asyncFinalLabel, functionEmission.asyncCleanupLabel,
        g_typeAliasNames, g_typeAliasTargets, g_typeAliasCount,
        g_enumTypeNames)
}
SEEN
expect_fail exit_reset_direct_feature_state

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(output: StringBuilder, definedFuncs: Array<String>) r: Void {
    emitLateUserDeclaresStateImpl(output, definedFuncs,
        g_lateXmDeclareNames, g_lateXmDeclareStrs, g_lateXmDeclareCount)
}
SEEN
expect_fail late_declares_direct_module_state

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(output: StringBuilder, options: FunctionLoweringOptions,
    blockBox: Array<Int>) r: Void {

    emitLoopMetadataImpl(output, options, blockBox, g_reductionLoopIds,
        g_nestedLoopIds, g_loopTileHints)
}
SEEN
expect_fail loop_metadata_direct_metrics_state

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(output: StringBuilder) r: Void {
    emitTraitVtableConstantsImpl(output, g_traitImplRegistry,
        g_traitImplCount, g_dynTraitNames)
}
SEEN
expect_fail module_tail_direct_trait_state

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(output: StringBuilder) r: Void {
    emitGlobalConstructorsImpl(output, g_initFuncCount, g_initFuncNames)
}
SEEN
expect_fail module_tail_direct_constructor_state

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(output: StringBuilder) r: Void {
    emitTBAAMetadataImpl(output, g_sanitizerMode)
}
SEEN
expect_fail module_tail_direct_tbaa_state

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun bad(fn: FunctionNode) r: Void {
    unreviewedHelper(a, b, c, d, e, generatedFunctions)
}
SEEN
expect_fail facade_owner_call

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/bad_aggregate_helper.seen" <<'SEEN'
class IdentityLikeSnapshot {
    var names: Array<String>
}

fun oversizedIdentityLikeHelper(a0: String, a1: Array<String>,
    a2: Array<Int>, a3: IdentityLikeSnapshot, a4: IdentityLikeSnapshot,
    a5: String, a6: Array<String>, a7: Array<Int>,
    a8: IdentityLikeSnapshot, a9: String, a10: Array<String>,
    a11: Array<Int>, a12: IdentityLikeSnapshot, a13: String,
    a14: Array<String>, a15: Array<Int>, a16: IdentityLikeSnapshot,
    a17: String, a18: Array<String>, a19: Array<Int>,
    a20: IdentityLikeSnapshot) r: Void {
}
SEEN
cat > "$TMP_DIR/compiler_seen/src/codegen/bad_aggregate_call.seen" <<'SEEN'
fun callOversizedIdentityLikeHelper() r: Void {
    oversizedIdentityLikeHelper(s0, a1, a2, snap3, snap4, s5, a6, a7,
        snap8, s9, a10, a11, snap12, s13, a14, a15, snap16, s17,
        a18, a19, snap20)
}
SEEN
expect_fail aggregate_abi_risky_helper
rm -f "$TMP_DIR/compiler_seen/src/codegen/bad_aggregate_helper.seen" \
    "$TMP_DIR/compiler_seen/src/codegen/bad_aggregate_call.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_decl_items.seen" <<'SEEN'
fun registerClassMethodDeclarationsIntoStateImpl(className: String,
    methods: Array<FunctionNode>) r: Void {

    var methodIdx = 0
    while methodIdx < methods.length() {
        let method = methods[methodIdx]
        let methodSeenParamTypes = Array<String>()
        let methodLlvmParamTypes = Array<String>()
        let methodDeclParams = buildDeclareParamsImpl(methodSeenParamTypes,
            methodLlvmParamTypes, not method.isStatic)
        methodIdx = methodIdx + 1
    }
}
SEEN
expect_fail constructor_declare_static_rule
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_decl_items.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/main_compiler.seen" <<'SEEN'
fun bad() r: FunctionNode {
    return FunctionNode.new()
}
SEEN
expect_fail main_compiler_fragile_constructor
rm -f "$TMP_DIR/compiler_seen/src/main_compiler.seen"

write_owner_modules
mkdir -p "$TMP_DIR/compiler_seen/src/bootstrap"
cat > "$TMP_DIR/compiler_seen/src/bootstrap/frontend.seen" <<'SEEN'
fun bad(source: String) r: SeenLexer {
    return SeenLexer.new(source, 1, "en")
}
SEEN
expect_fail frontend_fragile_constructor
rm -f "$TMP_DIR/compiler_seen/src/bootstrap/frontend.seen"

write_owner_modules
mkdir -p "$TMP_DIR/compiler_seen/src/bootstrap"
cat > "$TMP_DIR/compiler_seen/src/bootstrap/frontend.seen" <<'SEEN'
fun bad(file: String, message: String) r: FrontendDiagnostic {
    return FrontendDiagnostic.new(file, 1, 1, "error", message)
}
SEEN
expect_fail frontend_diagnostic_fragile_constructor
rm -f "$TMP_DIR/compiler_seen/src/bootstrap/frontend.seen"

write_owner_modules
mkdir -p "$TMP_DIR/compiler_seen/src/lsp"
cat > "$TMP_DIR/compiler_seen/src/lsp/server.seen" <<'SEEN'
fun bad(file: String, message: String) r: FrontendDiagnostic {
    return FrontendDiagnostic.new(file, 1, 1, "error", message)
}
SEEN
expect_fail lsp_diagnostic_fragile_constructor
rm -f "$TMP_DIR/compiler_seen/src/lsp/server.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_bad_driver.seen" <<'SEEN'
fun badDriverStringLocal(ctx: CodegenLoweringContext,
    expr: ParserExpressionNode) r: String {

    var reg = "0"
    reg = ctx.lowerExpression(expr)
    return reg
}
SEEN
expect_fail driver_string_literal_local
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_bad_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_bad_driver.seen" <<'SEEN'
fun badDriverDynContext(ctx: dyn CodegenLoweringContext,
    expr: ParserExpressionNode) r: String {

    return ctx.lowerExpression(expr)
}
SEEN
expect_fail lowering_context_dyn_param
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_bad_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_bad_driver.seen" <<'SEEN'
fun badDriverTraitCall(ctx: CodegenLoweringContext,
    expr: ParserExpressionNode) r: String {

    return ctx.lowerExpression(expr)
}
SEEN
expect_fail lowering_context_method_call
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_bad_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_lowering_context.seen" <<'SEEN'
trait CodegenLoweringContext {}
SEEN
expect_fail lowering_context_trait_context
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_lowering_context.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
fun lowerExpression(ctxHandle: CodegenLoweringContext,
    expr: ParserExpressionNode) r: String {
    let ctx = ctxHandle as LLVMIRGenerator
    return ctx.generateExpression(expr)
}
SEEN
expect_fail lowering_context_facade_cast
rm -f "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_lowering_context.seen" <<'SEEN'
trait CodegenLoweringContext {
    fun appendLoweringOutput(this: CodegenLoweringContext,
        text: String) r: Void
}
SEEN
expect_fail lowering_context_duplicate_symbol_callback
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_lowering_context.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_bad_driver.seen" <<'SEEN'
fun badClassParentSetter(ctx: CodegenLoweringContext) r: Void {
    setLowerCurrentClassParentName(ctx, "")
}
SEEN
expect_fail lowering_context_feature_setter_call
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_bad_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_module_driver.seen" <<'SEEN'
fun generateSingleState(ctx: CodegenLoweringContext,
    output: StringBuilder, stringConstants: Array<String>,
    program: ProgramNode, moduleIndex: Int) r: String {

    resetLoweringGenerator(ctx)
    resetLoweringSharedModuleScratch(ctx)
    return output.toString()
}
SEEN
expect_fail module_driver_stale_reset
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_module_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_call_args_driver.seen" <<'SEEN'
fun prepareCallArgumentsWithDefaultsState(
    ctx: CodegenLoweringContext, output: StringBuilder,
    funcName: String, arguments: Array<ParserExpressionNode>,
    enableDynBoxing: Bool, enableFloatPromotion: Bool,
    nullifyMovedSources: Bool, moveContext: String,
    varNames: Array<String>, varRegs: Array<String>,
    varTypes: Array<String>, dynParamFuncs: String,
    floatParamFuncNames: Array<String>, floatParamParamIdxs: Array<Int>,
    moveOnlyClasses: String, funcParamCountArr: Array<Int>,
    funcDefaultsArr: Array<String>, stringConstants: Array<String>) r: Void {
}
SEEN
expect_fail call_default_registry_param_types
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_call_args_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_bad_call_driver.seen" <<'SEEN'
fun badCallArgRouting(ctx: CodegenLoweringContext,
    expr: ParserExpressionNode) r: Void {

    prepareLoweredCallArgumentsWithDefaults(ctx, "bad", expr.arguments,
        false, false, false, "")
}
SEEN
expect_fail call_argument_lowering_callback
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_bad_call_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_bad_constructor_driver.seen" <<'SEEN'
fun badReprCConstructorCheck(funcName: String) r: Bool {
    return isReprCConstructorTypeImpl(funcName, getGlobalStructNamesImpl())
}
SEEN
expect_fail reprc_constructor_owner_state
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_bad_constructor_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_call_runtime_driver.seen" <<'SEEN'
fun tryGenerateMathBuiltinCallState(ctx: CodegenLoweringContext,
    output: StringBuilder, expr: ParserExpressionNode,
    funcName: String) r: String {

    let runtimePlan = planRuntimeBuiltinDispatchImpl(funcName,
        expr.arguments.length())
    if not runtimePlan.shouldTryMath {
        return ""
    }
    return "0"
}
SEEN
expect_fail math_runtime_broad_plan
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_call_runtime_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_call_runtime_driver.seen" <<'SEEN'
fun tryGeneratePanicRuntimeBuiltinCallState(
    ctx: CodegenLoweringContext, output: StringBuilder,
    expr: ParserExpressionNode, funcName: String) r: String {

    let argReg = lowerExpression(ctx, expr.arguments[0])
    return emitPanicRuntimeCallImpl(output, getFeatureRegBoxImpl(),
        argReg)
}
SEEN
expect_fail panic_runtime_terminator_state
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_call_runtime_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_binary_short_circuit.seen" <<'SEEN'
fun allocateShortCircuitBinaryLabelsImpl(blockLabelBox: Array<Int>)
    r: ShortCircuitBinaryLabels {

    return ShortCircuitBinaryLabels.new(getNextBlockFn(blockLabelBox),
        getNextBlockFn(blockLabelBox), getNextBlockFn(blockLabelBox),
        getNextBlockFn(blockLabelBox))
}
SEEN
expect_fail short_circuit_bb_label_namespace
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_binary_short_circuit.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_binary_short_circuit_driver.seen" <<'SEEN'
fun generateShortCircuitAndState(ctx: CodegenLoweringContext,
    output: StringBuilder, expr: ParserExpressionNode) r: String {

    let labels = allocateShortCircuitBinaryLabelsImpl(getFeatureRegBoxImpl())
    let leftReg = lowerExpression(ctx, expr.operands[0])
    return leftReg
}
SEEN
expect_fail short_circuit_preallocated_labels
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_binary_short_circuit_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_if_statement_driver.seen" <<'SEEN'
fun generateIfStatementState(ctx: CodegenLoweringContext,
    output: StringBuilder, stmt: StatementNode) r: Void {

    let condBool = lowerExpression(ctx, stmt.condition)
    let branching = emitFunctionBodyIfBranchingImpl(output,
        getFeatureRegBoxImpl(), condBool, "")
}
SEEN
expect_fail branch_label_reg_allocator
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_if_statement_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_member_access_driver.seen" <<'SEEN'
fun generateMemberAccessState(ctx: CodegenLoweringContext,
    output: StringBuilder, expr: ParserExpressionNode,
    varNames: Array<String>, varRegs: Array<String>,
    varTypes: Array<String>) r: String {

    let receiverName = expr.literalValue
    let fieldName = expr.variableName
    let operandCount = expr.operands.length()
    if operandCount > 0 and isExplicitThisReceiverExprState(expr.operands[0]) {
        let explicitThisField = tryGenerateImplicitThisFieldVariableState(
            output, varNames, varRegs, varTypes, fieldName)
        if explicitThisField != "" {
            return explicitThisField
        }
    }
    return "0"
}
SEEN
expect_fail explicit_this_member_access
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_member_access_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_class_method_driver.seen" <<'SEEN'
fun generateClassMethodFromListState(ctx: CodegenLoweringContext,
    output: StringBuilder, className: String, parentName: String,
    methods: Array<FunctionNode>, methodIdx: Int,
    varNames: Array<String>, varRegs: Array<String>,
    varTypes: Array<String>, stringConstants: Array<String>) r: Void {

    resetLoweringLocalCodegenState(ctx)
    prepareClassMethodVariableCollectionState(ctx, className, methods,
        methodIdx, Array<String>(), varNames, varRegs, varTypes)
    setActiveVarCountWithGlobalStateImpl(
        emitClassMethodReceiverBindingStateImpl(output, getFeatureRegBoxImpl(),
        varNames, varRegs, varTypes,
        getBoundedActiveVarCountWithGlobalStateImpl(varNames), className,
        false, ""))
    lowerBlock(ctx, methods[methodIdx].body)
}
SEEN
expect_fail class_method_stale_context_arrays
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_class_method_driver.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen" <<'SEEN'
extern fun LLVMIRGenerator_generateExpression(ctx: CodegenLoweringContext,
    expr: ParserExpressionNode) r: String

class LLVMIRGenerator {
    fun generateExpression(expr: ParserExpressionNode) r: String {
        return "0"
    }
}
SEEN
expect_fail lowering_context_facade_extern_callbacks
rm -f "$TMP_DIR/compiler_seen/src/codegen/llvm_ir_gen.seen"

write_owner_modules
cat > "$TMP_DIR/compiler_seen/src/codegen/ir_binary_driver.seen" <<'SEEN'
fun generateBinaryState(ctx: CodegenLoweringContext,
    output: StringBuilder, expr: ParserExpressionNode,
    boundsCheckLabelCount: Int, panicOnOverflow: Bool,
    inductionVarName: String) r: BinaryDriverResult {

    let leftExpr = expr.operands[0]
    let rightExpr = expr.operands[1]
    let leftOperandCount = leftExpr.operands.length()
    if shouldTryBinaryAssociativeIntFoldImpl(expr.operator,
        false, leftExpr.kind, leftExpr.operator, leftOperandCount) {
        return BinaryDriverResult.new("0", boundsCheckLabelCount, 0, 0)
    }
    return BinaryDriverResult.new("0", boundsCheckLabelCount, 0, 0)
}
SEEN
expect_fail binary_child_operand_guard
rm -f "$TMP_DIR/compiler_seen/src/codegen/ir_binary_driver.seen"

write_owner_modules
mkdir -p "$TMP_DIR/seen_std/src/process"
cat > "$TMP_DIR/seen_std/src/process/process.seen" <<'SEEN'
import str.string.{StringBuilder}

fun shellQuote(text: String) r: String {
    let builder = StringBuilder.new()
    builder.append("\"")
    builder.append(text)
    builder.append("\"")
    return builder.toString()
}
SEEN
expect_fail process_shell_quote_stringbuilder
rm -rf "$TMP_DIR/seen_std"

echo "PASS: codegen ABI preflight regressions"
