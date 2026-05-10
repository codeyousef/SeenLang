#!/usr/bin/env python3
"""Cheap source preflight for bootstrap-sensitive codegen ABI boundaries.

This catches failure classes that type-check in Seen source but can corrupt
self-hosted runtime values only during an expensive Stage2 smoke.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


OWNER_MODULES = (
    "ir_codegen_global_state",
    "ir_codegen_feature_state",
    "ir_codegen_module_state",
)

OWNER_IMPORT_ALLOWLIST = {
    "llvm_ir_gen",
    "ir_codegen_global_state",
    "ir_codegen_feature_state",
    "ir_codegen_module_state",
    "ir_codegen_state_bridge",
    "ir_codegen_reset_state",
}

OWNER_STATE_NAMES = {
    "generatedFunctions",
    "generatedClasses",
    "funcNames",
    "funcRetTypes",
    "structNames",
    "structLayouts",
    "structFieldNames",
    "structFieldTypes",
    "structLlvmFieldTypes",
    "structSizes",
    "structMethodNames",
    "structMethodRetTypes",
    "moduleConstantNames",
    "moduleConstantValues",
    "moduleConstantTypes",
    "preAllocatedVars",
    "preAllocatedTypes",
    "preAllocatedRegs",
    "activeVarCount",
}

KNOWN_FACADE_OWNER_CALLS = {
    "registerFunctionStateImpl",
    "registerStructMethodReg",
    "tryHandleExternFunctionGenerationStateImpl",
    "emitGeneratedClassDecoratorMethodsImpl",
    "emitInheritedClassMethodThunksImpl",
    "collectClassMethodParameterInfoImpl",
    "emitClassMethodParameterBindingsStateImpl",
    "emitPreAllocatedAllocasImpl",
    "emitClassMethodConstructorSetupStateImpl",
    "mapFunctionPreBodyReturnTypeImpl",
    "emitUnusedResultCallWarningImpl",
    "emitImplicitThisFieldStoreImpl",
    "emitImplicitThisMemberAssignmentImpl",
    "emitPreparedIndexAssignmentStoreImpl",
    "resolveMethodFieldPathTypeImpl",
    "coerceMethodCallHandleAssignmentImpl",
    "emitClassArrayFieldInitializersImpl",
    "tryGeneratePreparedExplicitReceiverFastPathImpl",
    "tryResolveImplicitThisDottedMethodReceiverStateImpl",
    "tryGeneratePreparedLocalVariableReceiverFastPathImpl",
    "tryResolveImplicitThisLiteralMethodReceiverStateImpl",
    "emitPreparedStringBuilderMethodLowerImpl",
    "findModuleConstantSnapshotImpl",
    "storeActiveVariableBindingImpl",
    "emitClassMethodReceiverBindingStateImpl",
    "tryEmitClassMethodConstructorReturnStateImpl",
    "emitMainFunctionEntrySnapshotImpl",
    "tryHandleFunctionIntrinsicDriverGlobalStateImpl",
    "bindFunctionBodyCatchVariableImpl",
    "prepareLetStatementPlanImpl",
    "emitIfLetBindingStateImpl",
    "resolveFunctionBodyForInLoopVariableStorageImpl",
    "emitValueReceiverMemberAssignmentImpl",
    "emitVariableReceiverMemberAssignmentImpl",
    "emitPreparedExpressionReceiverMemberAccessImpl",
    "emitPreparedVariableReceiverMemberAccessImpl",
    "emitPreparedModuleConstReceiverMemberAccessImpl",
    "emitPreparedBracketPathMemberAccessImpl",
    "resolveChainedPathSuffixTypeImpl",
    "emitPreparedChainedReceiverMemberAccessImpl",
    "emitPreparedImplicitThisReceiverMemberAccessImpl",
    "generateFieldAccessImpl",
    "generateFieldAccessPtrImpl",
    "resolveOffsetofBuiltinValueImpl",
    "planLowLevelBuiltinCallImpl",
    "resolveClassConstructorFieldOffsetImpl",
    "emitClassHeapAllocationImpl",
    "recordRegularLateDeclareIfNeededState",
    "emitPreparedStaticMethodDispatchState",
    "emitPreparedArrayFreeMutatorMethodStateImpl",
    "emitPreparedArrayPushMutatorMethodStateImpl",
    "emitPreparedStringLikeMethodLowerImpl",
    "emitPreparedFinalMethodDispatchState",
    "emitWhenPatternBindingsState",
}
AGGREGATE_ABI_PARAM_THRESHOLD = 20
AGGREGATE_ABI_AGGREGATE_THRESHOLD = 12
AGGREGATE_ABI_SIGNATURE_ALLOWLIST = {
    # Existing reviewed bridge/helper shapes. New helpers over the threshold
    # should prefer a state/context object or a smaller focused helper.
    "ir_assignment_gen.emitImplicitThisMemberAssignmentImpl",
    "ir_assignment_gen.emitResolvedMemberAssignmentStoreImpl",
    "ir_assignment_gen.emitValueReceiverMemberAssignmentImpl",
    "ir_assignment_gen.emitVariableReceiverMemberAssignmentImpl",
    "ir_class_type_decorators.new",
    "ir_class_type_decorators.prepareClassTypeDecoratorScanSnapshotImpl",
    "ir_codegen_state_bridge.syncCodegenStateBridgeImpl",
    "ir_decl_registry.captureDeclarationPassStateImpl",
    "ir_decl_registry.new",
    "ir_member_access_gen.emitBracketPathMemberAccessImpl",
    "ir_member_access_gen.emitChainedReceiverMemberAccessImpl",
    "ir_member_access_gen.emitImplicitThisReceiverMemberAccessImpl",
    "ir_member_access_gen.emitModuleConstReceiverMemberAccessImpl",
    "ir_member_access_gen.emitPreparedBracketPathMemberAccessImpl",
    "ir_member_access_gen.emitPreparedChainedReceiverMemberAccessImpl",
    "ir_member_access_gen.emitPreparedImplicitThisReceiverMemberAccessImpl",
    "ir_member_access_gen.emitPreparedModuleConstReceiverMemberAccessImpl",
    "ir_method_receiver_lookup_gen.emitImplicitDottedMethodReceiverLoadImpl",
    "ir_method_receiver_lookup_gen.emitImplicitThisMethodReceiverLoadImpl",
    "ir_method_receiver_lookup_gen.tryResolveImplicitThisDottedMethodReceiverStateImpl",
    "ir_method_receiver_lookup_gen.tryResolveImplicitThisLiteralMethodReceiverStateImpl",
}
AGGREGATE_ABI_PRIMITIVE_TYPES = {
    "Bool",
    "Byte",
    "Char",
    "Double",
    "Float",
    "Float32",
    "Float64",
    "I32",
    "I64",
    "Int",
    "UInt",
    "UInt32",
    "UInt64",
    "U8",
    "Void",
}
MAIN_COMPILER_BOOTSTRAP_FRAGILE_CONSTRUCTORS = (
    "FrontendResult.new",
    "FrontendDiagnostic.new",
    "ProgramNode.new",
    "EnumNode.new",
    "FunctionNode.new",
    "ClassNode.new",
    "BlockNode.new",
    "LLVMIRGenerator.new",
    "CImportGenerator.new",
    "run_frontend",
    "run_frontend_declarations",
)
FRONTEND_BOOTSTRAP_FRAGILE_CALLS = (
    "FrontendDiagnostic.new",
    "ProgramNode.new",
    "FunctionNode.new",
    "ClassNode.new",
    "SeenLexer.new",
    "RealParser.new",
    "TypeChecker.new",
    "MacroExpander.new",
)
LSP_BOOTSTRAP_FRAGILE_CALLS = (
    "FrontendDiagnostic.new",
)

IDENTITY_HELPER = "prepareFunctionGenerationIdentityWithGlobalStateImpl"
IDENTITY_FORBIDDEN_PARAMS = {
    "dynTraitNames",
    "traitImplRegistry",
    "traitImplCount",
}

PREBODY_HELPER = "prepareFunctionPreBodyStateSnapshotImpl"
PREBODY_OWNER_HELPER = "prepareFunctionPreBodyWithFeatureStateImpl"
PREBODY_FACADE_HELPER = "prepareFunctionPreBodyAndTryEmitMain"
MAIN_ENTRY_FACADE_HELPER = "emitMainFunctionEntryAndAllocas"
PREBODY_FORBIDDEN_STATE = {
    "g_comptimeParamFuncNames",
    "g_comptimeParamCount",
    "g_funcDefaultsArr",
    "g_dynParamFuncs",
    "g_funcParamCountArr",
}
FEATURE_BOX_GLOBALS = {
    "g_defaultFillArgRegs",
    "g_defaultFillArgTypeStrs",
    "g_regBox",
    "g_blockBox",
}
FEATURE_STATE_GLOBAL_ACCESSORS = {
    "g_bitfieldKeys": "getFeatureBitfieldKeysImpl",
    "g_bitfieldWidths": "getFeatureBitfieldWidthsImpl",
    "g_bitfieldCount": "getFeatureBitfieldCountImpl",
}
BITFIELD_WIDTH_RAW_HELPER = "findBitfieldWidthImpl"
BITFIELD_WIDTH_OWNER_HELPER = "findFeatureBitfieldWidthImpl"
BITFIELD_WIDTH_RAW_HELPER_ALLOWLIST = {
    "ir_bitfield_access",
    "ir_codegen_feature_state",
}

FUNCTION_PREBODY_DIRECT_HELPERS = {
    "mapFunctionPreBodyReturnTypeImpl": "use mapTypeState/current owner-state getters instead",
    "emitMainFunctionEntrySnapshotImpl": "emit main entry through the small state-aware facade path",
}
PRELUDE_HELPER = "prepareFunctionPreludeAnalysisSnapshotImpl"
PRELUDE_OWNER_HELPER = "prepareFunctionPreludeAnalysisWithMetricsStateImpl"
PRELUDE_FORBIDDEN_STATE = {
    "g_mlReplayEnabled",
    "g_mlReplayLog",
    "g_callCountNamesArr",
    "g_selfCallFuncs",
    "g_selfCallNames",
    "g_selfCallCount",
}
ENTRY_SETUP_HELPER = "emitFunctionEntrySetupSnapshotImpl"
ENTRY_SETUP_MAX_ARGS = 5
ENTRY_SETUP_FORBIDDEN_STATE = {
    "g_typeAliasNames",
    "g_typeAliasTargets",
    "g_typeAliasCount",
    "g_enumTypeNames",
    "g_reprCClassNames",
}
EXIT_RESET_HELPER = "emitFunctionExitResetSnapshotImpl"
EXIT_RESET_MAX_ARGS = 3
EXIT_RESET_FORBIDDEN_STATE = ENTRY_SETUP_FORBIDDEN_STATE
LATE_DECLARE_HELPER = "emitLateUserDeclaresStateImpl"
LATE_DECLARE_OWNER_HELPER = "emitLateUserDeclaresWithModuleStateImpl"
LATE_DECLARE_OWNER_MAX_ARGS = 2
LATE_DECLARE_FORBIDDEN_STATE = {
    "g_lateXmDeclareNames",
    "g_lateXmDeclareStrs",
    "g_lateXmDeclareCount",
}
LOOP_METADATA_HELPER = "emitLoopMetadataImpl"
LOOP_METADATA_OWNER_HELPER = "emitLoopMetadataWithMetricsStateImpl"
LOOP_METADATA_OWNER_MAX_ARGS = 2
LOOP_METADATA_FORBIDDEN_STATE = {
    "g_reductionLoopIds",
    "g_nestedLoopIds",
    "g_loopTileHints",
}
MODULE_TAIL_HELPERS = {
    "emitTraitVtableConstantsImpl": "emitTraitVtableConstantsWithFeatureStateImpl",
    "emitGeneratedClosuresImpl": "emitGeneratedClosuresWithFeatureStateImpl",
    "emitGlobalConstructorsImpl": "emitGlobalConstructorsWithFeatureStateImpl",
    "emitTBAAMetadataImpl": "emitTBAAMetadataWithFeatureStateImpl",
}
MODULE_TAIL_FORBIDDEN_STATE = {
    "g_traitImplRegistry",
    "g_traitImplCount",
    "g_dynTraitNames",
    "g_closureDefs",
    "g_initFuncCount",
    "g_initFuncNames",
    "g_sanitizerMode",
}
BLOCK_TERMINATED_GETTER = "getBlockTerminatedWithGlobalStateImpl"
BLOCK_TERMINATED_SETTER = "setBlockTerminatedWithGlobalStateImpl"
PER_FUNCTION_GLOBAL_STATE_ACCESSORS = {
    "currentFunctionReturnType": (
        "getCurrentFunctionReturnTypeWithGlobalStateImpl",
        "setCurrentFunctionReturnTypeWithGlobalStateImpl",
    ),
    "currentClassName": (
        "getGlobalCurrentClassNameImpl",
        "setGlobalCurrentClassNameImpl",
    ),
    "currentLoopCondLabel": (
        "getCurrentLoopCondLabelWithGlobalStateImpl",
        "setCurrentLoopCondLabelWithGlobalStateImpl",
    ),
    "currentLoopEndLabel": (
        "getCurrentLoopEndLabelWithGlobalStateImpl",
        "setCurrentLoopEndLabelWithGlobalStateImpl",
    ),
    "pendingArrayLiteralType": (
        "getPendingArrayLiteralTypeWithGlobalStateImpl",
        "setPendingArrayLiteralTypeWithGlobalStateImpl",
    ),
    "activeVarCount": (
        "getActiveVarCountWithGlobalStateImpl",
        "setActiveVarCountWithGlobalStateImpl",
    ),
    "preAllocatedRegs": (
        "getPreAllocatedRegsWithGlobalStateImpl",
        "setPreAllocatedRegsWithGlobalStateImpl",
    ),
    "preAllocatedTypes": (
        "getPreAllocatedTypesWithGlobalStateImpl",
        "setPreAllocatedTypesWithGlobalStateImpl",
    ),
    "preAllocatedVars": (
        "getPreAllocatedVarsWithGlobalStateImpl",
        "setPreAllocatedVarsWithGlobalStateImpl",
    ),
}
ACTIVE_VAR_COUNT_UNBOUNDED_GETTER = "getActiveVarCountWithGlobalStateImpl"
ACTIVE_VAR_COUNT_BOUNDED_GETTER = "getBoundedActiveVarCountWithGlobalStateImpl"
REGISTRY_GLOBAL_STATE_ACCESSORS = {
    "structNames": "getGlobalStructNamesImpl",
    "structLayouts": "getGlobalStructLayoutsImpl",
    "structFieldNames": "getGlobalStructFieldNamesImpl",
    "structFieldTypes": "getGlobalStructFieldTypesImpl",
    "structLlvmFieldTypes": "getGlobalStructLlvmFieldTypesImpl",
    "structSizes": "getGlobalStructSizesImpl",
    "structMethodNames": "getGlobalStructMethodNamesImpl",
    "structMethodRetTypes": "getGlobalStructMethodRetTypesImpl",
}
DECL_STORAGE_OWNER_HELPERS = {
    "prepareLetStatementPlanWithGlobalStateImpl",
    "resolveLetPreAllocatedRegWithGlobalStateImpl",
    "resolveFunctionBodyForInLoopVariableStorageWithGlobalStateImpl",
    "resolveFunctionBodyForInIndexAllocaWithGlobalStateImpl",
}
DECL_STORAGE_DIRECT_HELPERS = {
    "prepareLetStatementPlanImpl": "prepareLetStatementPlanWithGlobalStateImpl",
    "resolveLetPreAllocatedRegImpl": "resolveLetPreAllocatedRegWithGlobalStateImpl",
    "resolveFunctionBodyForInLoopVariableStorageImpl": "resolveFunctionBodyForInLoopVariableStorageWithGlobalStateImpl",
    "resolveFunctionBodyForInIndexAllocaImpl": "resolveFunctionBodyForInIndexAllocaWithGlobalStateImpl",
}
DECL_STORAGE_FORBIDDEN_STATE = {
    "declOrdinalNames",
    "declOrdinalCounts",
}
MODULE_CONST_OWNER_HELPERS = {
    "findModuleConstantWithModuleStateImpl",
    "getModuleConstantTypeWithModuleStateImpl",
    "getModuleConstSeenTypeWithModuleStateImpl",
    "isModuleConstMutableWithModuleStateImpl",
    "moduleConstantInvariantLoadSuffixWithModuleStateImpl",
}
MODULE_CONST_DIRECT_HELPERS = {
    "findModuleConstantSnapshotImpl": "findModuleConstantWithModuleStateImpl",
    "tryInferModuleConstantVariableTypeSnapshotImpl": "tryInferModuleConstantVariableTypeWithModuleStateImpl",
    "isModuleConstMutableImpl": "isModuleConstMutableWithModuleStateImpl",
    "getModuleConstSeenTypeImpl": "getModuleConstSeenTypeWithModuleStateImpl",
    "moduleConstantInvariantLoadSuffixImpl": "moduleConstantInvariantLoadSuffixWithModuleStateImpl",
}
MODULE_CONST_FORBIDDEN_GLOBAL_STATE = {
    "moduleConstantNames",
    "moduleConstantValues",
    "moduleConstantTypes",
}
MODULE_CONST_FORBIDDEN_MODULE_STATE = {
    "g_moduleConstMutable",
    "g_moduleConstSeenTypes",
    "g_xmConstNames",
    "g_xmConstTypes",
    "g_xmConstMutable",
    "g_xmConstSeenTypes",
    "g_xmConstCount",
}
FUNCTION_REGISTRY_GETTERS = (
    "getGlobalFuncNamesImpl",
    "getGlobalFuncRetTypesImpl",
    "getFunctionReturnTypeWithGlobalStateImpl",
)
FUNCTION_REGISTRY_FORBIDDEN_STATE = {
    "funcNames",
    "funcRetTypes",
}
FUNCTION_REGISTRY_PARAM_TYPES = {
    "funcNames": "Array<String>",
    "funcRetTypes": "Array<String>",
}
CALL_DEFAULT_REGISTRY_PARAM_TYPES = {
    "funcParamCountArr": "String",
    "funcDefaultsArr": "String",
}
LATE_DECLARE_STACK_HELPER_MAX_ARGS = {
    "recordLateUserDeclareIntoStateImpl": 5,
    "recordRegularLateDeclareIfNeededState": 4,
}
LATE_DECLARE_ROUTING_HELPERS = {
    "recordLateUserDeclareIntoStateImpl",
    "recordRegularLateDeclareIfNeededState",
    "emitPreparedStaticMethodDispatchState",
    "emitPreparedFinalMethodDispatchState",
}
LATE_DECLARE_UNUSED_STATE_PARAMS = {
    "funcNames",
    "funcRetTypes",
    "xmDeclareNames",
    "xmDeclareCount",
}
STATIC_METHOD_DISPATCH_HELPER = "emitPreparedStaticMethodDispatchState"
STATIC_METHOD_DISPATCH_MAX_ARGS = 11
STATIC_METHOD_DISPATCH_FORBIDDEN_STATE = {
    "structNames",
    "getGlobalStructNamesImpl",
    "g_reprCClassNames",
    "reprCClassNames",
    "g_typeAliasNames",
    "typeAliasNames",
    "g_typeAliasTargets",
    "typeAliasTargets",
    "g_typeAliasCount",
    "typeAliasCount",
    "g_enumTypeNames",
    "enumTypeNames",
}
FINAL_METHOD_DISPATCH_HELPER = "emitPreparedFinalMethodDispatchState"
FINAL_METHOD_DISPATCH_MAX_ARGS = 15
FINAL_METHOD_DISPATCH_FORBIDDEN_STATE = STATIC_METHOD_DISPATCH_FORBIDDEN_STATE
CLASS_METHOD_METADATA_HELPERS = {
    "collectClassMethodParameterInfoImpl": 5,
    "emitClassMethodParameterBindingsStateImpl": 11,
}
CLASS_METHOD_METADATA_FORBIDDEN_STATE = STATIC_METHOD_DISPATCH_FORBIDDEN_STATE
PREALLOCATED_ALLOCA_HELPER = "emitPreAllocatedAllocasImpl"
PREALLOCATED_ALLOCA_MAX_ARGS = 3
PREALLOCATED_ALLOCA_FORBIDDEN_STATE = STATIC_METHOD_DISPATCH_FORBIDDEN_STATE
EXTERN_GENERATION_HELPER = "tryHandleExternFunctionGenerationStateImpl"
EXTERN_GENERATION_OWNER_HELPER = "tryHandleExternFunctionGenerationWithGlobalStateImpl"
EXTERN_GENERATION_OWNER_MAX_ARGS = 2
EXTERN_GENERATION_FORBIDDEN_CALL_STATE = {
    "getGlobalFuncNamesImpl",
    "getGlobalFuncRetTypesImpl",
    "funcNames",
    "funcRetTypes",
    "getGlobalStructNamesImpl",
    "structNames",
    "g_reprCClassNames",
    "g_typeAliasNames",
    "g_typeAliasTargets",
    "g_typeAliasCount",
    "g_enumTypeNames",
}
STRING_BUILDER_METHOD_LOWER_HELPER = "emitPreparedStringBuilderMethodLowerImpl"
STRING_BUILDER_METHOD_LOWER_MAX_ARGS = 7
STRING_BUILDER_RECEIVER_HELPER = "emitStringBuilderReceiverMethodCallImpl"
STRING_BUILDER_RECEIVER_MAX_ARGS = 7
STRING_BUILDER_FORBIDDEN_STATE = {
    "getGlobalStructNamesImpl",
    "structNames",
    "g_reprCClassNames",
    "reprCClassNames",
    "g_typeAliasNames",
    "g_typeAliasTargets",
    "g_typeAliasCount",
    "g_enumTypeNames",
}
STRICT_HELPER_ARITY_NAMES = {
    "generateFieldAccessImpl",
    "generateFieldAccessPtrImpl",
}
AST_LAYOUT_STRUCTS = (
    "StatementNode",
    "ParserExpressionNode",
)
AST_INDEX_TABLES = {
    "ir_field_indices.seen": ("structName", "return"),
    "ir_struct_field_resolution.seen": ("structType", "g_resolvedFieldIndex"),
    "type_registry_core.seen": ("structName", "return"),
}
AST_TYPE_TABLES = (
    "ir_known_struct_field_types.seen",
)

CALL_KEYWORDS = {
    "if",
    "while",
    "for",
    "return",
    "fun",
    "static",
    "class",
    "new",
}

COMPILER_IMPORT_ROOTS = {
    "bootstrap",
    "codegen",
    "errors",
    "ffi",
    "ir",
    "lexer",
    "lsp",
    "macro",
    "main_compiler",
    "optimization",
    "parser",
    "reactive",
    "runtime",
    "testing",
    "tools",
    "typechecker",
    "types",
}
MAX_IMPORT_CYCLE_FINDINGS = 20
KNOWN_LEGACY_IMPORT_CYCLE_SETS = {
    frozenset(
        {
            "codegen.ir_decl_items",
            "codegen.ir_decl_registry",
            "codegen.ir_decl_scan",
        }
    ),
    frozenset(
        {
            "codegen.ir_decl_items",
            "codegen.ir_decl_registry",
            "codegen.ir_decl_scan",
            "codegen.ir_module_emit",
        }
    ),
}
BOOTSTRAP_DIRECT_IMPORT_ALLOWLIST = {
    # Long-standing low-level modules are pulled in through older bootstrap
    # roots. New extracted codegen helpers must be directly imported from
    # main_compiler.seen or the frozen compiler may emit declares without
    # definitions during Stage2 recovery/link.
    "codegen.ir_declarations",
    "codegen.ir_optimization",
    "codegen.ir_type_info",
    "codegen.ir_type_tables",
}
LOWERING_CONTEXT_BUILDER_RETURN = "currentLoweringOutput"
LOWERING_CONTEXT_FORBIDDEN_CALLBACKS = {
    "appendLoweringOutput",
    "currentLoweringOutputText",
}
LOWERING_CONTEXT_CALLBACK_NAMES = {
    "beginLoweringNestedScratch",
    "clearLoweringActiveBindings",
    "collectLoweringVariableDeclarations",
    "currentLoweringFunctionOptions",
    "inferLoweredExpressionType",
    "isLoweringBlockTerminated",
    "lowerBlock",
    "lowerCallExpression",
    "lowerClassMethodFromList",
    "lowerClassNode",
    "lowerEnumConstructor",
    "lowerExpression",
    "lowerFunctionNode",
    "lowerPlainLargeClass",
    "lowerProgramClasses",
    "lowerProgramTopLevelFunctions",
    "lowerStatement",
    "prepareLoweredCallArgumentsWithDefaults",
    "resetLoweringGenerator",
    "resetLoweringLocalCodegenState",
    "resetLoweringSharedModuleScratch",
    "restoreLoweringNestedScratch",
    "setLowerCurrentClassParentName",
    "syncLoweringState",
    "writeBackLoweringState",
}


class Finding:
    def __init__(self, path: Path, line: int, message: str) -> None:
        self.path = path
        self.line = line
        self.message = message

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.message}"


def strip_triple_slash_blocks(text: str) -> str:
    lines: list[str] = []
    in_block = False
    for line in text.splitlines():
        if line.strip() == "///":
            in_block = not in_block
            lines.append("")
            continue
        if in_block:
            lines.append("")
        else:
            lines.append(line)
    return "\n".join(lines) + ("\n" if text.endswith("\n") else "")


def strip_line_comment(line: str) -> str:
    in_string = False
    escaped = False
    i = 0
    while i + 1 < len(line):
        ch = line[i]
        if in_string:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
        else:
            if ch == '"':
                in_string = True
            elif ch == "/" and line[i + 1] == "/":
                return line[:i]
        i += 1
    return line


def source_lines(path: Path) -> list[str]:
    text = strip_triple_slash_blocks(path.read_text(errors="ignore"))
    return [strip_line_comment(line) for line in text.splitlines()]


def compiler_src_root(root: Path) -> Path:
    return root / "compiler_seen" / "src"


def normalize_import_module(module: str) -> str:
    module = module.strip().rstrip(".")
    prefix = "compiler_seen.src."
    if module.startswith(prefix):
        module = module[len(prefix) :]
    return module


def compiler_module_path(root: Path, module: str) -> Path | None:
    module = normalize_import_module(module)
    if not module:
        return None
    parts = module.split(".")
    if parts[0] not in COMPILER_IMPORT_ROOTS:
        return None
    src_root = compiler_src_root(root)
    for end in range(len(parts), 0, -1):
        candidate = src_root.joinpath(*parts[:end]).with_suffix(".seen")
        if candidate.exists():
            return candidate
        candidate_dir = src_root.joinpath(*parts[:end])
        if candidate_dir.is_dir():
            mod_file = candidate_dir / "mod.seen"
            if mod_file.exists():
                return mod_file
            package_file = candidate_dir / f"{parts[end - 1]}.seen"
            if package_file.exists():
                return package_file
            return candidate_dir
    return None


def expected_compiler_module_path(root: Path, module: str) -> Path:
    module = normalize_import_module(module)
    parts = module.split(".")
    candidate_dir = compiler_src_root(root).joinpath(*parts)
    if candidate_dir.is_dir():
        return candidate_dir / "mod.seen"
    return candidate_dir.with_suffix(".seen")


def compiler_module_id(root: Path, path: Path) -> str:
    rel = path.relative_to(compiler_src_root(root)).with_suffix("")
    return ".".join(rel.parts)


def import_statements(path: Path) -> list[tuple[int, str]]:
    imports: list[tuple[int, str]] = []
    for line_no, line in enumerate(source_lines(path), 1):
        match = re.match(r"\s*import\s+([A-Za-z_][A-Za-z0-9_.]*)", line)
        if match:
            imports.append((line_no, match.group(1)))
    return imports


def compiler_import_integrity_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    src_root = compiler_src_root(root)
    if not src_root.exists():
        findings.append(Finding(src_root, 1, "missing compiler source root"))
        return findings
    for path in sorted(src_root.rglob("*.seen")):
        for line_no, module in import_statements(path):
            normalized = normalize_import_module(module)
            top = normalized.split(".", 1)[0]
            if top not in COMPILER_IMPORT_ROOTS:
                continue
            if compiler_module_path(root, normalized) is None:
                findings.append(
                    Finding(
                        path,
                        line_no,
                        "missing imported compiler module "
                        f"`{module}`; expected {expected_compiler_module_path(root, normalized)}",
                    )
                )
    return findings


def main_compiler_direct_bootstrap_import_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "main_compiler.seen"
    if not path.exists():
        return []
    text = strip_triple_slash_blocks(path.read_text(errors="ignore"))
    direct_imports = {
        normalize_import_module(match.group(1))
        for match in re.finditer(r"^\s*import\s+([A-Za-z_][A-Za-z0-9_.]*)", text, re.M)
    }
    pushed_modules: list[tuple[int, str]] = []
    for match in re.finditer(r'modules\.push\("([A-Za-z_][A-Za-z0-9_.]*)"\)', text):
        module = normalize_import_module(match.group(1))
        if module.startswith("codegen."):
            pushed_modules.append((line_for_offset(text, match.start()), module))

    findings: list[Finding] = []
    for line_no, module in pushed_modules:
        if module in BOOTSTRAP_DIRECT_IMPORT_ALLOWLIST:
            continue
        if module not in direct_imports:
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"bootstrap module `{module}` is seeded but not directly "
                    "imported by main_compiler.seen; frozen bootstrap scans can "
                    "miss its definitions and leave Stage2 link-only undefined symbols",
                )
            )
    return findings


def lowering_context_builder_return_findings(root: Path) -> list[Finding]:
    codegen = root / "compiler_seen" / "src" / "codegen"
    findings: list[Finding] = []
    for path in sorted(codegen.glob("*.seen")):
        if path.name in {"ir_lowering_context.seen", "llvm_ir_gen.seen"}:
            continue
        for line_no, line in enumerate(source_lines(path), 1):
            if f".{LOWERING_CONTEXT_BUILDER_RETURN}(" in line:
                findings.append(
                    Finding(
                        path,
                        line_no,
                        "dyn lowering context returns StringBuilder here; "
                        "frozen bootstrap lowers that class return as an Int "
                        "handle, which later emits undefined Int_append. Pass "
                        "the output builder explicitly through the helper",
                    )
                )
    return findings


def compiler_import_cycle_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    src_root = compiler_src_root(root)
    graph: dict[str, set[str]] = {}
    paths: dict[str, Path] = {}
    for path in sorted(src_root.rglob("*.seen")):
        module_id = compiler_module_id(root, path)
        paths[module_id] = path
        deps: set[str] = set()
        for _, imported in import_statements(path):
            imported_path = compiler_module_path(root, imported)
            if imported_path is None:
                continue
            if imported_path.is_dir():
                continue
            try:
                dep_id = compiler_module_id(root, imported_path)
            except ValueError:
                continue
            if dep_id != module_id:
                deps.add(dep_id)
        graph[module_id] = deps

    visiting: list[str] = []
    visited: set[str] = set()
    reported: set[tuple[str, ...]] = set()

    def is_known_legacy_cycle(cycle: list[str]) -> bool:
        cycle_set = frozenset(cycle[:-1] if cycle and cycle[0] == cycle[-1] else cycle)
        if any(module_id.startswith("tools.c_import_") for module_id in cycle_set):
            return True
        return cycle_set in KNOWN_LEGACY_IMPORT_CYCLE_SETS

    def visit(module_id: str) -> None:
        if len(findings) >= MAX_IMPORT_CYCLE_FINDINGS:
            return
        if module_id in visiting:
            cycle = visiting[visiting.index(module_id) :] + [module_id]
            if is_known_legacy_cycle(cycle):
                return
            key = tuple(cycle)
            if key not in reported:
                reported.add(key)
                findings.append(
                    Finding(
                        paths.get(module_id, src_root),
                        1,
                        "compiler import cycle: " + " -> ".join(cycle),
                    )
                )
            return
        if module_id in visited:
            return
        visiting.append(module_id)
        for dep_id in sorted(graph.get(module_id, ())):
            visit(dep_id)
        visiting.pop()
        visited.add(module_id)

    for module_id in sorted(graph):
        visit(module_id)
        if len(findings) >= MAX_IMPORT_CYCLE_FINDINGS:
            break
    return findings


def raw_float_literal_source_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_dir = root / "compiler_seen" / "src" / "codegen"
    malformed_literal = re.compile(r"\b(?:float|double)\s+0(?=\s*[,)\"])")
    raw_float_to_string = re.compile(
        r"@seen_float_to_string\(double\s+\"\s*\+\s*([A-Za-z_][A-Za-z0-9_]*)"
    )
    for path in sorted(codegen_dir.rglob("*.seen")):
        lines = source_lines(path)
        for line_no, line in enumerate(lines, 1):
            if malformed_literal.search(line):
                findings.append(
                    Finding(
                        path,
                        line_no,
                        "raw LLVM float/double zero literal in source; emit 0.0",
                    )
                )
        for index, line in enumerate(lines):
            snippet = " ".join(lines[index : index + 4])
            match = raw_float_to_string.search(snippet)
            if not match:
                continue
            context = " ".join(lines[max(0, index - 3) : index + 4])
            if "normalizeDoubleLiteralForLlvmImpl" in context or (
                "normalizeRuntimeDoubleOperandImpl" in context
            ):
                continue
            if "declare %SeenString @seen_float_to_string(double)" in context:
                continue
            findings.append(
                Finding(
                    path,
                    index + 1,
                    "seen_float_to_string call appends a raw double operand; "
                    "normalize literal operands before emission",
                )
            )
            if len(findings) >= 50:
                break
        if len(findings) >= 50:
            break
    return findings


def synthetic_parser_expression_literal_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_dir = root / "compiler_seen" / "src" / "codegen"
    for path in sorted(codegen_dir.rglob("*.seen")):
        for line_no, line in enumerate(source_lines(path), 1):
            if "ParserExpressionNode{" not in line:
                continue
            findings.append(
                Finding(
                    path,
                    line_no,
                    "codegen must build synthetic ParserExpressionNode values "
                    "with ParserExpressionNode.new() plus field assignments; "
                    "partial aggregate literals can leave omitted String/Array "
                    "fields ABI-corrupt during self-hosted lowering",
                )
            )
    return findings


def feature_getter_return_type_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_feature_state.seen"
    if not path.exists():
        return []

    lines = source_lines(path)
    global_types: dict[str, str] = {}
    for line in lines:
        match = re.match(
            r"\s*var\s+(g_[A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^=]+?)\s*=",
            line,
        )
        if match:
            global_types[match.group(1)] = " ".join(match.group(2).split())

    findings: list[Finding] = []
    getter = re.compile(
        r"\s*fun\s+(getFeature[A-Za-z0-9_]*Impl)\(\)\s+r:\s*([^{]+?)\s*"
        r"\{\s*return\s+(g_[A-Za-z_][A-Za-z0-9_]*)\s*\}"
    )
    for line_no, line in enumerate(lines, 1):
        match = getter.match(line)
        if not match:
            continue
        func_name = match.group(1)
        return_type = " ".join(match.group(2).split())
        global_name = match.group(3)
        global_type = global_types.get(global_name)
        if global_type is None or return_type == global_type:
            continue
        findings.append(
            Finding(
                path,
                line_no,
                f"{func_name} returns {return_type} but {global_name} is "
                f"{global_type}; owner-state accessors must preserve ABI type",
            )
        )
    return findings


def owner_vars(root: Path, module: str) -> set[str]:
    path = root / "compiler_seen" / "src" / "codegen" / f"{module}.seen"
    if not path.exists():
        return set()
    names: set[str] = set()
    for line in source_lines(path):
        match = re.match(r"\s*var\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", line)
        if match:
            names.add(match.group(1))
    return names


def find_owner_import_violations(root: Path) -> list[Finding]:
    codegen = root / "compiler_seen" / "src" / "codegen"
    findings: list[Finding] = []
    module_vars = {module: owner_vars(root, module) for module in OWNER_MODULES}
    for path in sorted(codegen.glob("*.seen")):
        if path.stem in OWNER_IMPORT_ALLOWLIST:
            continue
        lines = source_lines(path)
        for line_no, line in enumerate(lines, 1):
            for module, names in module_vars.items():
                marker = f"import codegen.{module}"
                if marker not in line or "{" not in line or "}" not in line:
                    continue
                imported = line.split("{", 1)[1].rsplit("}", 1)[0]
                bad = sorted(
                    name.strip()
                    for name in imported.split(",")
                    if name.strip() in names
                )
                if bad:
                    findings.append(
                        Finding(
                            path,
                            line_no,
                            "direct mutable owner-state import from "
                            f"{module}: {', '.join(bad)}",
                        )
                    )
    return findings


def collect_balanced(text: str, open_index: int) -> tuple[str, int]:
    depth = 0
    in_string = False
    escaped = False
    i = open_index
    while i < len(text):
        ch = text[i]
        if in_string:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
        else:
            if ch == '"':
                in_string = True
            elif ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
                if depth == 0:
                    return text[open_index + 1 : i], i + 1
        i += 1
    return text[open_index + 1 :], len(text)


def split_top_level_args(params: str) -> list[str]:
    args: list[str] = []
    start = 0
    depth = 0
    in_string = False
    escaped = False
    for i, ch in enumerate(params):
        if in_string:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
            continue
        if ch == '"':
            in_string = True
        elif ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            if depth > 0:
                depth -= 1
        elif ch == "," and depth == 0:
            args.append(params[start:i].strip())
            start = i + 1
    tail = params[start:].strip()
    if tail:
        args.append(tail)
    if len(args) == 1 and args[0] == "":
        return []
    return args


def line_for_offset(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def find_function_definition(text: str, name: str) -> tuple[int, list[str]] | None:
    pattern = re.compile(rf"\bfun\s+{re.escape(name)}\s*\(")
    match = pattern.search(text)
    if not match:
        return None
    open_index = text.find("(", match.start())
    params, _ = collect_balanced(text, open_index)
    return line_for_offset(text, match.start()), split_top_level_args(params)


def find_function_body(text: str, name: str) -> tuple[int, str] | None:
    pattern = re.compile(rf"\bfun\s+{re.escape(name)}\s*\(")
    match = pattern.search(text)
    if not match:
        return None
    params_open = text.find("(", match.start())
    _, params_end = collect_balanced(text, params_open)
    body_open = text.find("{", params_end)
    if body_open < 0:
        return None
    body, _ = collect_balanced_curly(text, body_open)
    return line_for_offset(text, body_open), body


def collect_balanced_curly(text: str, open_index: int) -> tuple[str, int]:
    depth = 0
    in_string = False
    escaped = False
    i = open_index
    while i < len(text):
        ch = text[i]
        if in_string:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
        else:
            if ch == '"':
                in_string = True
            elif ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    return text[open_index + 1 : i], i + 1
        i += 1
    return text[open_index + 1 :], len(text)


def find_class_body(text: str, name: str) -> tuple[int, str] | None:
    pattern = re.compile(rf"\bclass\s+{re.escape(name)}\b")
    match = pattern.search(text)
    if not match:
        return None
    body_open = text.find("{", match.end())
    if body_open < 0:
        return None
    body, _ = collect_balanced_curly(text, body_open)
    return line_for_offset(text, body_open), body


def parse_class_field_order(text: str, name: str) -> list[str]:
    body = find_class_body(text, name)
    if body is None:
        return []
    _, body_text = body
    fields: list[str] = []
    for line in body_text.splitlines():
        if re.match(r"\s*(static\s+)?fun\b", line):
            break
        match = re.match(r"\s*var\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", line)
        if match:
            fields.append(match.group(1))
    return fields


def find_struct_if_block(text: str, selector: str, struct_name: str) -> tuple[int, str] | None:
    pattern = re.compile(
        rf"\bif\s+{re.escape(selector)}\s*==\s*\"{re.escape(struct_name)}\"\s*\{{"
    )
    match = pattern.search(text)
    if not match:
        return None
    body_open = text.find("{", match.start())
    body, _ = collect_balanced_curly(text, body_open)
    return line_for_offset(text, match.start()), body


def ast_layout_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    parser_path = root / "compiler_seen" / "src" / "parser" / "real_parser.seen"
    if not parser_path.exists():
        findings.append(Finding(parser_path, 1, "missing parser source for AST layout check"))
        return findings
    parser_text = "\n".join(source_lines(parser_path))
    expected: dict[str, list[str]] = {}
    for struct_name in AST_LAYOUT_STRUCTS:
        fields = parse_class_field_order(parser_text, struct_name)
        if not fields:
            findings.append(
                Finding(
                    parser_path,
                    1,
                    f"could not read {struct_name} field order from parser source",
                )
            )
        expected[struct_name] = fields

    codegen_dir = root / "compiler_seen" / "src" / "codegen"
    for filename, (selector, index_expr) in sorted(AST_INDEX_TABLES.items()):
        path = codegen_dir / filename
        text = "\n".join(source_lines(path))
        for struct_name, fields in expected.items():
            block = find_struct_if_block(text, selector, struct_name)
            if block is None:
                findings.append(
                    Finding(path, 1, f"missing {struct_name} layout block in {filename}")
                )
                continue
            block_line, body = block
            for expected_idx, field_name in enumerate(fields):
                if index_expr == "return":
                    pattern = rf'fieldName\s*==\s*"{re.escape(field_name)}"\s*\{{[^}}]*\breturn\s+{expected_idx}\b'
                else:
                    pattern = rf'fieldName\s*==\s*"{re.escape(field_name)}"\s*\{{[^}}]*{re.escape(index_expr)}\s*=\s*{expected_idx}\b'
                if not re.search(pattern, body):
                    findings.append(
                        Finding(
                            path,
                            block_line,
                            f"{struct_name}.{field_name} has stale or missing "
                            f"field index in {filename}; parser order requires "
                            f"index {expected_idx}",
                        )
                    )
            stale_fields = sorted(
                set(re.findall(r'fieldName\s*==\s*"([^"]+)"', body)) - set(fields)
            )
            if stale_fields:
                findings.append(
                    Finding(
                        path,
                        block_line,
                        f"{struct_name} layout block contains fields not in parser "
                        "class: " + ", ".join(stale_fields),
                    )
                )

    type_table_path = codegen_dir / "ir_type_tables.seen"
    type_table_text = "\n".join(source_lines(type_table_path))
    for struct_name, fields in expected.items():
        pattern = re.compile(
            rf'structNames\.push\("{re.escape(struct_name)}"\)(.*?)structMethodRetTypes\.push\(',
            re.S,
        )
        match = pattern.search(type_table_text)
        if not match:
            findings.append(
                Finding(type_table_path, 1, f"missing {struct_name} registration")
            )
            continue
        expected_csv = ",".join(fields)
        if f'structFieldNames.push("{expected_csv}")' not in match.group(1):
            findings.append(
                Finding(
                    type_table_path,
                    line_for_offset(type_table_text, match.start()),
                    f"{struct_name} registered field order is stale; expected "
                    f"{expected_csv}",
                )
            )

    for filename in AST_TYPE_TABLES:
        path = codegen_dir / filename
        text = "\n".join(source_lines(path))
        for struct_name, fields in expected.items():
            block = find_struct_if_block(text, "structName", struct_name)
            if block is None:
                findings.append(
                    Finding(path, 1, f"missing {struct_name} type block in {filename}")
                )
                continue
            block_line, body = block
            present = set(re.findall(r'fieldName\s*==\s*"([^"]+)"', body))
            missing = [field for field in fields if field not in present]
            stale = sorted(present - set(fields))
            if missing:
                findings.append(
                    Finding(
                        path,
                        block_line,
                        f"{struct_name} type block is missing parser fields: "
                        + ", ".join(missing),
                    )
                )
            if stale:
                findings.append(
                    Finding(
                        path,
                        block_line,
                        f"{struct_name} type block contains stale parser fields: "
                        + ", ".join(stale),
                    )
                )
    return findings


def find_calls(text: str, name: str | None = None) -> list[tuple[int, str, list[str], str]]:
    calls: list[tuple[int, str, list[str], str]] = []
    pattern = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(")
    for match in pattern.finditer(text):
        callee = match.group(1)
        if name is not None and callee != name:
            continue
        if callee in CALL_KEYWORDS:
            continue
        line_start = text.rfind("\n", 0, match.start()) + 1
        line_prefix = text[line_start : match.start()].strip()
        if line_prefix.startswith("fun ") or line_prefix.startswith("static fun "):
            continue
        if match.start() > 0 and text[match.start() - 1] == ".":
            continue
        open_index = text.find("(", match.start())
        params, end = collect_balanced(text, open_index)
        calls.append(
            (
                line_for_offset(text, match.start()),
                callee,
                split_top_level_args(params),
                text[match.start() : end],
            )
        )
    return calls


def strict_helper_arity_findings(root: Path) -> list[Finding]:
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    findings: list[Finding] = []
    definitions: dict[str, tuple[Path, int, int]] = {}
    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        for helper_name in STRICT_HELPER_ARITY_NAMES:
            definition = find_function_definition(text, helper_name)
            if definition is None:
                continue
            line_no, params = definition
            definitions[helper_name] = (path, line_no, len(params))

    for helper_name, (definition_path, definition_line, expected_args) in sorted(
        definitions.items()
    ):
        if expected_args <= 0:
            findings.append(
                Finding(
                    definition_path,
                    definition_line,
                    f"{helper_name} has no parsed parameters; arity guard cannot run",
                )
            )
            continue
        for path in sorted(codegen_path.glob("*.seen")):
            text = "\n".join(source_lines(path))
            for line_no, _, args, _ in find_calls(text, helper_name):
                if len(args) != expected_args:
                    findings.append(
                        Finding(
                            path,
                            line_no,
                            f"{helper_name} call has {len(args)} args; "
                            f"definition requires {expected_args}. Missing "
                            "state/helper arguments corrupt Stage2 ABI values.",
                        )
                    )
    return findings


def static_method_dispatch_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    helper_path = root / "compiler_seen" / "src" / "codegen" / "ir_method_static_dispatch.seen"
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    if not helper_path.exists() or not facade_path.exists():
        return findings
    helper_text = "\n".join(source_lines(helper_path))
    definition = find_function_definition(helper_text, STATIC_METHOD_DISPATCH_HELPER)
    if definition is not None:
        line_no, params = definition
        if len(params) > STATIC_METHOD_DISPATCH_MAX_ARGS:
            findings.append(
                Finding(
                    helper_path,
                    line_no,
                    f"{STATIC_METHOD_DISPATCH_HELPER} has {len(params)} args; "
                    "keep static dispatch metadata owner-backed so Stage3 does "
                    "not pass alias/reprC strings through a high-arity helper",
                )
            )
        joined_params = " ".join(params)
        leaked = sorted(
            name
            for name in STATIC_METHOD_DISPATCH_FORBIDDEN_STATE
            if re.search(rf"\b{re.escape(name)}\b", joined_params)
        )
        if leaked:
            findings.append(
                Finding(
                    helper_path,
                    line_no,
                    f"{STATIC_METHOD_DISPATCH_HELPER} takes bootstrap-sensitive "
                    "type metadata directly: " + ", ".join(leaked),
                )
            )

    facade_text = "\n".join(source_lines(facade_path))
    for line_no, _, args, _ in find_calls(facade_text, STATIC_METHOD_DISPATCH_HELPER):
        joined_args = " ".join(args)
        leaked = sorted(
            name
            for name in STATIC_METHOD_DISPATCH_FORBIDDEN_STATE
            if re.search(rf"\b{re.escape(name)}\b", joined_args)
        )
        if leaked:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{STATIC_METHOD_DISPATCH_HELPER} call passes "
                    "bootstrap-sensitive type metadata directly: "
                    + ", ".join(leaked),
                )
            )
    return findings


def final_method_dispatch_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    helper_path = root / "compiler_seen" / "src" / "codegen" / "ir_method_finalize.seen"
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    if not helper_path.exists() or not facade_path.exists():
        return findings
    helper_text = "\n".join(source_lines(helper_path))
    definition = find_function_definition(helper_text, FINAL_METHOD_DISPATCH_HELPER)
    if definition is not None:
        line_no, params = definition
        if len(params) > FINAL_METHOD_DISPATCH_MAX_ARGS:
            findings.append(
                Finding(
                    helper_path,
                    line_no,
                    f"{FINAL_METHOD_DISPATCH_HELPER} has {len(params)} args; "
                    "keep final dispatch metadata owner-backed so Stage3 does "
                    "not pass alias/reprC strings through the late-declare path",
                )
            )
        joined_params = " ".join(params)
        leaked = sorted(
            name
            for name in FINAL_METHOD_DISPATCH_FORBIDDEN_STATE
            if re.search(rf"\b{re.escape(name)}\b", joined_params)
        )
        if leaked:
            findings.append(
                Finding(
                    helper_path,
                    line_no,
                    f"{FINAL_METHOD_DISPATCH_HELPER} takes bootstrap-sensitive "
                    "type metadata directly: " + ", ".join(leaked),
                )
            )

    facade_text = "\n".join(source_lines(facade_path))
    for line_no, _, args, _ in find_calls(facade_text, FINAL_METHOD_DISPATCH_HELPER):
        joined_args = " ".join(args)
        leaked = sorted(
            name
            for name in FINAL_METHOD_DISPATCH_FORBIDDEN_STATE
            if re.search(rf"\b{re.escape(name)}\b", joined_args)
        )
        if leaked:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{FINAL_METHOD_DISPATCH_HELPER} call passes "
                    "bootstrap-sensitive type metadata directly: "
                    + ", ".join(leaked),
                )
            )
    return findings


def class_method_metadata_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    helper_path = root / "compiler_seen" / "src" / "codegen" / "ir_class_method_gen.seen"
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    if not helper_path.exists() or not facade_path.exists():
        return findings
    helper_text = "\n".join(source_lines(helper_path))
    for helper_name, max_args in sorted(CLASS_METHOD_METADATA_HELPERS.items()):
        definition = find_function_definition(helper_text, helper_name)
        if definition is not None:
            line_no, params = definition
            if len(params) > max_args:
                findings.append(
                    Finding(
                        helper_path,
                        line_no,
                        f"{helper_name} has {len(params)} args; keep class "
                        "method parameter mapping owner-backed so Stage3 "
                        "does not loop in mapTypeImpl on fragile metadata",
                    )
                )
            joined_params = " ".join(params)
            leaked = sorted(
                name
                for name in CLASS_METHOD_METADATA_FORBIDDEN_STATE
                if re.search(rf"\b{re.escape(name)}\b", joined_params)
            )
            if leaked:
                findings.append(
                    Finding(
                        helper_path,
                        line_no,
                        f"{helper_name} takes bootstrap-sensitive type "
                        "metadata directly: " + ", ".join(leaked),
                    )
                )

    facade_text = "\n".join(source_lines(facade_path))
    for helper_name in sorted(CLASS_METHOD_METADATA_HELPERS):
        for line_no, _, args, _ in find_calls(facade_text, helper_name):
            joined_args = " ".join(args)
            leaked = sorted(
                name
                for name in CLASS_METHOD_METADATA_FORBIDDEN_STATE
                if re.search(rf"\b{re.escape(name)}\b", joined_args)
            )
            if leaked:
                findings.append(
                    Finding(
                        facade_path,
                        line_no,
                        f"{helper_name} call passes bootstrap-sensitive "
                        "type metadata directly: " + ", ".join(leaked),
                    )
                )
    return findings


def preallocated_alloca_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    helper_path = codegen_path / "ir_function_entry_exit.seen"
    if not helper_path.exists():
        return findings

    helper_text = "\n".join(source_lines(helper_path))
    definition = find_function_definition(helper_text, PREALLOCATED_ALLOCA_HELPER)
    if definition is not None:
        line_no, params = definition
        if len(params) > PREALLOCATED_ALLOCA_MAX_ARGS:
            findings.append(
                Finding(
                    helper_path,
                    line_no,
                    f"{PREALLOCATED_ALLOCA_HELPER} has {len(params)} args; "
                    "map preallocated Seen types before this alloca emitter "
                    "so Stage3 does not pass alias/reprC strings through it",
                )
            )
        joined_params = " ".join(params)
        leaked = sorted(
            name
            for name in PREALLOCATED_ALLOCA_FORBIDDEN_STATE
            if re.search(rf"\b{re.escape(name)}\b", joined_params)
        )
        if leaked:
            findings.append(
                Finding(
                    helper_path,
                    line_no,
                    f"{PREALLOCATED_ALLOCA_HELPER} takes bootstrap-sensitive "
                    "type metadata directly: " + ", ".join(leaked),
                )
            )

    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        for line_no, _, args, call_text in find_calls(text, PREALLOCATED_ALLOCA_HELPER):
            if len(args) > PREALLOCATED_ALLOCA_MAX_ARGS:
                findings.append(
                    Finding(
                        path,
                        line_no,
                        f"{PREALLOCATED_ALLOCA_HELPER} call has {len(args)} "
                        "args; pass mapped LLVM alloca types only",
                    )
                )
            leaked = sorted(
                name
                for name in PREALLOCATED_ALLOCA_FORBIDDEN_STATE
                if re.search(rf"\b{re.escape(name)}\b", call_text)
            )
            if leaked:
                findings.append(
                    Finding(
                        path,
                        line_no,
                        f"{PREALLOCATED_ALLOCA_HELPER} call passes "
                        "bootstrap-sensitive type metadata directly: "
                        + ", ".join(leaked),
                    )
                )
    return findings


def extern_generation_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    global_path = codegen_path / "ir_codegen_global_state.seen"
    facade_path = codegen_path / "llvm_ir_gen.seen"
    if not global_path.exists() or not facade_path.exists():
        return findings

    global_text = "\n".join(source_lines(global_path))
    definition = find_function_definition(global_text, EXTERN_GENERATION_OWNER_HELPER)
    if definition is None:
        findings.append(
            Finding(
                global_path,
                1,
                f"missing {EXTERN_GENERATION_OWNER_HELPER}; extern "
                "function registration must mutate function registry arrays "
                "inside their owner module",
            )
        )
    else:
        line_no, params = definition
        if len(params) > EXTERN_GENERATION_OWNER_MAX_ARGS:
            findings.append(
                Finding(
                    global_path,
                    line_no,
                    f"{EXTERN_GENERATION_OWNER_HELPER} has {len(params)} "
                    "args; keep extern registration owner-backed",
                )
            )

    facade_text = "\n".join(source_lines(facade_path))
    for line_no, _, args, call_text in find_calls(facade_text, EXTERN_GENERATION_HELPER):
        leaked = sorted(
            name
            for name in EXTERN_GENERATION_FORBIDDEN_CALL_STATE
            if re.search(rf"\b{re.escape(name)}\b", call_text)
        )
        details = ""
        if leaked:
            details = ": " + ", ".join(leaked)
        findings.append(
            Finding(
                facade_path,
                line_no,
                f"facade calls {EXTERN_GENERATION_HELPER}; use "
                f"{EXTERN_GENERATION_OWNER_HELPER} so Stage3 does not pass "
                "function-registry arrays through a deep extern helper"
                + details,
            )
        )

    for line_no, _, args, call_text in find_calls(
        facade_text, EXTERN_GENERATION_OWNER_HELPER
    ):
        if len(args) > EXTERN_GENERATION_OWNER_MAX_ARGS:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{EXTERN_GENERATION_OWNER_HELPER} call has {len(args)} "
                    "args; pass output and function only",
                )
            )
        leaked = sorted(
            name
            for name in EXTERN_GENERATION_FORBIDDEN_CALL_STATE
            if re.search(rf"\b{re.escape(name)}\b", call_text)
        )
        if leaked:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{EXTERN_GENERATION_OWNER_HELPER} call passes "
                    "bootstrap-sensitive state directly: " + ", ".join(leaked),
                )
            )
    return findings


def string_builder_method_lower_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    lower_path = codegen_path / "ir_method_lower_emit.seen"
    fastpath_path = codegen_path / "ir_method_fastpath.seen"
    facade_path = codegen_path / "llvm_ir_gen.seen"
    if not lower_path.exists() or not fastpath_path.exists() or not facade_path.exists():
        return findings

    lower_text = "\n".join(source_lines(lower_path))
    definition = find_function_definition(lower_text, STRING_BUILDER_METHOD_LOWER_HELPER)
    if definition is not None:
        line_no, params = definition
        if len(params) > STRING_BUILDER_METHOD_LOWER_MAX_ARGS:
            findings.append(
                Finding(
                    lower_path,
                    line_no,
                    f"{STRING_BUILDER_METHOD_LOWER_HELPER} has {len(params)} "
                    "args; StringBuilder runtime lowering should not carry "
                    "type-registry or reprC metadata through Stage3",
                )
            )
        joined_params = " ".join(params)
        leaked = sorted(
            name
            for name in STRING_BUILDER_FORBIDDEN_STATE
            if re.search(rf"\b{re.escape(name)}\b", joined_params)
        )
        if leaked:
            findings.append(
                Finding(
                    lower_path,
                    line_no,
                    f"{STRING_BUILDER_METHOD_LOWER_HELPER} takes bootstrap-sensitive "
                    "type metadata directly: " + ", ".join(leaked),
                )
            )

    fastpath_text = "\n".join(source_lines(fastpath_path))
    receiver_definition = find_function_definition(
        fastpath_text, STRING_BUILDER_RECEIVER_HELPER
    )
    if receiver_definition is not None:
        line_no, params = receiver_definition
        if len(params) > STRING_BUILDER_RECEIVER_MAX_ARGS:
            findings.append(
                Finding(
                    fastpath_path,
                    line_no,
                    f"{STRING_BUILDER_RECEIVER_HELPER} has {len(params)} args; "
                    "StringBuilder receivers already have a known handle ABI",
                )
            )
        joined_params = " ".join(params)
        leaked = sorted(
            name
            for name in STRING_BUILDER_FORBIDDEN_STATE
            if re.search(rf"\b{re.escape(name)}\b", joined_params)
        )
        if leaked:
            findings.append(
                Finding(
                    fastpath_path,
                    line_no,
                    f"{STRING_BUILDER_RECEIVER_HELPER} takes bootstrap-sensitive "
                    "type metadata directly: " + ", ".join(leaked),
                )
            )
    body = find_function_body(fastpath_text, STRING_BUILDER_RECEIVER_HELPER)
    if body is not None:
        body_line, body_text = body
        if "convertReceiverToPtrImpl" in body_text or "isClassTypeImpl" in body_text:
            findings.append(
                Finding(
                    fastpath_path,
                    body_line,
                    f"{STRING_BUILDER_RECEIVER_HELPER} must not ask the "
                    "global type registry whether StringBuilder is class-like; "
                    "emit the known i64-handle-to-ptr conversion directly",
                )
            )

    facade_text = "\n".join(source_lines(facade_path))
    for line_no, _, args, call_text in find_calls(
        facade_text, STRING_BUILDER_METHOD_LOWER_HELPER
    ):
        if len(args) > STRING_BUILDER_METHOD_LOWER_MAX_ARGS:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{STRING_BUILDER_METHOD_LOWER_HELPER} call has {len(args)} "
                    "args; pass output, reg box, receiver, argument and fuse "
                    "state only",
                )
            )
        leaked = sorted(
            name
            for name in STRING_BUILDER_FORBIDDEN_STATE
            if re.search(rf"\b{re.escape(name)}\b", call_text)
        )
        if leaked:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{STRING_BUILDER_METHOD_LOWER_HELPER} call passes "
                    "bootstrap-sensitive type metadata directly: "
                    + ", ".join(leaked),
                )
            )
    return findings


def identity_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    global_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_global_state.seen"
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    global_text = "\n".join(source_lines(global_path))
    facade_text = "\n".join(source_lines(facade_path))

    definition = find_function_definition(global_text, IDENTITY_HELPER)
    if definition is None:
        findings.append(Finding(global_path, 1, f"missing {IDENTITY_HELPER}"))
    else:
        line_no, params = definition
        if len(params) > 2:
            findings.append(
                Finding(
                    global_path,
                    line_no,
                    f"{IDENTITY_HELPER} has {len(params)} params; keep it <= 2 "
                    "so String/Array values stay out of fragile stack ABI slots",
                )
            )
        forbidden = [
            name
            for name in IDENTITY_FORBIDDEN_PARAMS
            if any(re.search(rf"\b{name}\b", param) for param in params)
        ]
        if forbidden:
            findings.append(
                Finding(
                    global_path,
                    line_no,
                    f"{IDENTITY_HELPER} takes feature-owner state directly: "
                    + ", ".join(forbidden),
                )
            )

    for line_no, callee, args, _ in find_calls(facade_text, IDENTITY_HELPER):
        if len(args) > 2:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{IDENTITY_HELPER} call has {len(args)} args; route "
                    "feature/global state through owner accessors instead",
                )
            )
        joined = "\n".join(args)
        forbidden = [
            "g_dynTraitNames",
            "g_traitImplRegistry",
            "g_traitImplCount",
            "generatedFunctions",
            "funcNames",
            "funcRetTypes",
        ]
        passed = [name for name in forbidden if re.search(rf"\b{name}\b", joined)]
        if passed:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{IDENTITY_HELPER} call passes owner state directly: "
                    + ", ".join(passed),
                )
            )
    if re.search(r"g_traitImplRegistry\s*=\s*functionIdentity", facade_text):
        findings.append(
            Finding(
                facade_path,
                1,
                "facade writes g_traitImplRegistry from identity snapshot; "
                "use feature-state owner APIs",
            )
        )
    if re.search(r"g_traitImplCount\s*=\s*functionIdentity", facade_text):
        findings.append(
            Finding(
                facade_path,
                1,
                "facade writes g_traitImplCount from identity snapshot; "
                "use feature-state owner APIs",
            )
        )
    return findings


def prebody_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    feature_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_feature_state.seen"
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    feature_text = "\n".join(source_lines(feature_path))
    facade_text = "\n".join(source_lines(facade_path))

    definition = find_function_definition(feature_text, PREBODY_OWNER_HELPER)
    if definition is None:
        findings.append(Finding(feature_path, 1, f"missing {PREBODY_OWNER_HELPER}"))
    else:
        line_no, params = definition
        if len(params) > 4:
            findings.append(
                Finding(
                    feature_path,
                    line_no,
                    f"{PREBODY_OWNER_HELPER} has {len(params)} params; keep it "
                    "<= 4 so function pre-body feature state stays behind "
                    "the owner module",
                )
            )

    for line_no, _, args, call_text in find_calls(facade_text, PREBODY_HELPER):
        passed = [
            name
            for name in sorted(PREBODY_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        detail = ""
        if passed:
            detail = ": " + ", ".join(passed)
        findings.append(
            Finding(
                facade_path,
                line_no,
                f"facade calls {PREBODY_HELPER} directly{detail}; use "
                f"{PREBODY_OWNER_HELPER} so String feature-state values do "
                "not spill into fragile stack ABI slots",
            )
        )

    for line_no, _, args, call_text in find_calls(facade_text, PREBODY_OWNER_HELPER):
        if len(args) > 4:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{PREBODY_OWNER_HELPER} call has {len(args)} args; keep "
                    "it <= 4",
                )
            )
        passed = [
            name
            for name in sorted(PREBODY_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        if passed:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{PREBODY_OWNER_HELPER} call passes feature-owner state "
                    "directly: " + ", ".join(passed),
                )
            )

    for name in sorted(PREBODY_FORBIDDEN_STATE):
        if re.search(rf"\b{name}\s*=\s*preBodyState\.", facade_text):
            findings.append(
                Finding(
                    facade_path,
                    1,
                    f"facade writes {name} from preBodyState; use "
                    f"{PREBODY_OWNER_HELPER}",
                )
            )

    body = find_function_body(facade_text, PREBODY_FACADE_HELPER)
    if body is not None:
        body_line, body_text = body
        match = re.search(r"mapTypeState\s*\(\s*currentFunctionReturnType\s*\)", body_text)
        if match:
            findings.append(
                Finding(
                    facade_path,
                    body_line + line_for_offset(body_text, match.start()) - 1,
                    f"{PREBODY_FACADE_HELPER} maps currentFunctionReturnType "
                    "directly after owner-state writeback; map the resolved "
                    "return type parameter instead",
                )
            )
        for line_no, _, _, _ in find_calls(body_text, MAIN_ENTRY_FACADE_HELPER):
            findings.append(
                Finding(
                    facade_path,
                    body_line + line_no - 1,
                    f"{PREBODY_FACADE_HELPER} calls facade-local "
                    f"{MAIN_ENTRY_FACADE_HELPER}; route main entry/allocation "
                    "emission through the global-state owner helper",
                )
            )
    if find_function_definition(facade_text, MAIN_ENTRY_FACADE_HELPER) is not None:
        line_no, _ = find_function_definition(facade_text, MAIN_ENTRY_FACADE_HELPER)
        findings.append(
            Finding(
                facade_path,
                line_no,
                f"facade defines {MAIN_ENTRY_FACADE_HELPER}; keep main "
                "entry/preallocated-allocation emission in the global-state "
                "owner module so preAllocated* globals are not lowered as "
                "stale facade fields",
            )
        )
    for helper, replacement in sorted(FUNCTION_PREBODY_DIRECT_HELPERS.items()):
        for line_no, _, _, _ in find_calls(facade_text, helper):
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"facade calls {helper} directly; {replacement}",
                )
            )
    return findings


def facade_feature_box_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    findings: list[Finding] = []
    for line_no, line in enumerate(source_lines(path), 1):
        used = [
            name
            for name in sorted(FEATURE_BOX_GLOBALS)
            if re.search(rf"\b{name}\b", line)
        ]
        if not used:
            continue
        findings.append(
            Finding(
                path,
                line_no,
                "facade directly references mutable feature box global(s): "
                + ", ".join(used)
                + "; use getFeatureRegBoxImpl()/getFeatureBlockBoxImpl() "
                "so reset-time array replacement stays behind the owner module",
            )
        )
    return findings


def feature_state_metadata_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_dir = root / "compiler_seen" / "src" / "codegen"
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    feature_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_feature_state.seen"
    facade_lines = source_lines(facade_path)
    feature_text = "\n".join(source_lines(feature_path))

    for state_name, getter in sorted(FEATURE_STATE_GLOBAL_ACCESSORS.items()):
        if find_function_definition(feature_text, getter) is None:
            findings.append(Finding(feature_path, 1, f"missing {getter}"))
    if find_function_definition(feature_text, BITFIELD_WIDTH_OWNER_HELPER) is None:
        findings.append(Finding(feature_path, 1, f"missing {BITFIELD_WIDTH_OWNER_HELPER}"))

    state_pattern = re.compile(
        r"(?<!\.)\b("
        + "|".join(re.escape(name) for name in sorted(FEATURE_STATE_GLOBAL_ACCESSORS))
        + r")\b"
    )
    for line_no, line in enumerate(facade_lines, 1):
        used = sorted(set(state_pattern.findall(line)))
        if not used:
            continue
        findings.append(
            Finding(
                facade_path,
                line_no,
                "facade references bootstrap-sensitive feature metadata directly: "
                + ", ".join(used)
                + "; use feature-state getter helpers so Stage3 member access "
                "does not receive stale facade/global string values",
            )
        )
    raw_helper_pattern = re.compile(rf"(?<!\.)\b{BITFIELD_WIDTH_RAW_HELPER}\b")
    for path in sorted(codegen_dir.rglob("*.seen")):
        if path.stem in BITFIELD_WIDTH_RAW_HELPER_ALLOWLIST:
            continue
        for line_no, line in enumerate(source_lines(path), 1):
            if raw_helper_pattern.search(line):
                findings.append(
                    Finding(
                        path,
                        line_no,
                        f"{BITFIELD_WIDTH_RAW_HELPER} bypasses feature-state "
                        f"metadata ownership; use {BITFIELD_WIDTH_OWNER_HELPER} "
                        "so Stage3 member/assignment lowering does not pass "
                        "bitfield metadata through high-arity call chains",
                    )
                )
    return findings


def prelude_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    metrics_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_metrics_state.seen"
    facade_text = "\n".join(source_lines(facade_path))
    metrics_text = "\n".join(source_lines(metrics_path))

    if find_function_definition(metrics_text, PRELUDE_OWNER_HELPER) is None:
        findings.append(Finding(metrics_path, 1, f"missing {PRELUDE_OWNER_HELPER}"))

    for line_no, _, _, call_text in find_calls(facade_text, PRELUDE_HELPER):
        passed = [
            name
            for name in sorted(PRELUDE_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        detail = ""
        if passed:
            detail = ": " + ", ".join(passed)
        findings.append(
            Finding(
                facade_path,
                line_no,
                f"facade calls {PRELUDE_HELPER} directly{detail}; use "
                f"{PRELUDE_OWNER_HELPER} so metrics-owner arrays stay behind "
                "the owner module",
            )
        )

    for line_no, line in enumerate(source_lines(facade_path), 1):
        used = [
            name
            for name in sorted(PRELUDE_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", line)
        ]
        if "import codegen.ir_codegen_metrics_state" not in line:
            used = [name for name in used if name == "g_callCountNamesArr"]
        if used:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade directly references prelude metrics-owner state: "
                    + ", ".join(used),
                )
            )
    return findings


def entry_setup_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    entry_path = root / "compiler_seen" / "src" / "codegen" / "ir_function_entry_exit.seen"
    facade_text = "\n".join(source_lines(facade_path))
    entry_text = "\n".join(source_lines(entry_path))

    definition = find_function_definition(entry_text, ENTRY_SETUP_HELPER)
    if definition is None:
        findings.append(Finding(entry_path, 1, f"missing {ENTRY_SETUP_HELPER}"))
    else:
        line_no, params = definition
        if len(params) > ENTRY_SETUP_MAX_ARGS:
            findings.append(
                Finding(
                    entry_path,
                    line_no,
                    f"{ENTRY_SETUP_HELPER} has {len(params)} params; keep it "
                    f"<= {ENTRY_SETUP_MAX_ARGS} so feature-state strings and "
                    "counts stay behind owner accessors",
                )
            )
        joined_params = "\n".join(params)
        passed = [
            name
            for name in sorted(ENTRY_SETUP_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", joined_params)
        ]
        if passed:
            findings.append(
                Finding(
                    entry_path,
                    line_no,
                    f"{ENTRY_SETUP_HELPER} takes feature-owner state directly: "
                    + ", ".join(passed),
                )
            )

    for line_no, _, args, call_text in find_calls(facade_text, ENTRY_SETUP_HELPER):
        if len(args) > ENTRY_SETUP_MAX_ARGS:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{ENTRY_SETUP_HELPER} call has {len(args)} args; route "
                    "type-mapping and async state through owner helpers",
                )
            )
        passed = [
            name
            for name in sorted(ENTRY_SETUP_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        if passed:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{ENTRY_SETUP_HELPER} call passes feature-owner state "
                    "directly: " + ", ".join(passed),
                )
            )
    return findings


def exit_reset_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    entry_path = root / "compiler_seen" / "src" / "codegen" / "ir_function_entry_exit.seen"
    facade_text = "\n".join(source_lines(facade_path))
    entry_text = "\n".join(source_lines(entry_path))

    definition = find_function_definition(entry_text, EXIT_RESET_HELPER)
    if definition is None:
        findings.append(Finding(entry_path, 1, f"missing {EXIT_RESET_HELPER}"))
    else:
        line_no, params = definition
        if len(params) > EXIT_RESET_MAX_ARGS:
            findings.append(
                Finding(
                    entry_path,
                    line_no,
                    f"{EXIT_RESET_HELPER} has {len(params)} params; keep it "
                    f"<= {EXIT_RESET_MAX_ARGS} so feature-state strings and "
                    "async/profile strings stay behind owner accessors",
                )
            )
        joined_params = "\n".join(params)
        passed = [
            name
            for name in sorted(EXIT_RESET_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", joined_params)
        ]
        if passed:
            findings.append(
                Finding(
                    entry_path,
                    line_no,
                    f"{EXIT_RESET_HELPER} takes feature-owner state directly: "
                    + ", ".join(passed),
                )
            )

    for line_no, _, args, call_text in find_calls(facade_text, EXIT_RESET_HELPER):
        if len(args) > EXIT_RESET_MAX_ARGS:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{EXIT_RESET_HELPER} call has {len(args)} args; route "
                    "type-mapping and async/profile state through owner helpers",
                )
            )
        passed = [
            name
            for name in sorted(EXIT_RESET_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        if passed:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{EXIT_RESET_HELPER} call passes feature-owner state "
                    "directly: " + ", ".join(passed),
                )
            )
    return findings


def late_declare_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    module_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_module_state.seen"
    facade_text = "\n".join(source_lines(facade_path))
    module_text = "\n".join(source_lines(module_path))

    definition = find_function_definition(module_text, LATE_DECLARE_OWNER_HELPER)
    if definition is None:
        findings.append(Finding(module_path, 1, f"missing {LATE_DECLARE_OWNER_HELPER}"))
    else:
        line_no, params = definition
        if len(params) > LATE_DECLARE_OWNER_MAX_ARGS:
            findings.append(
                Finding(
                    module_path,
                    line_no,
                    f"{LATE_DECLARE_OWNER_HELPER} has {len(params)} params; keep it "
                    f"<= {LATE_DECLARE_OWNER_MAX_ARGS} so late-declare pipe "
                    "state stays behind the module-state owner",
                )
            )

    for line_no, _, _, call_text in find_calls(facade_text, LATE_DECLARE_HELPER):
        passed = [
            name
            for name in sorted(LATE_DECLARE_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        detail = ""
        if passed:
            detail = ": " + ", ".join(passed)
        findings.append(
            Finding(
                facade_path,
                line_no,
                f"facade calls {LATE_DECLARE_HELPER} directly{detail}; use "
                f"{LATE_DECLARE_OWNER_HELPER} so late-declare String/count "
                "state stays behind the module-state owner",
            )
        )

    for line_no, _, args, call_text in find_calls(facade_text, LATE_DECLARE_OWNER_HELPER):
        if len(args) > LATE_DECLARE_OWNER_MAX_ARGS:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{LATE_DECLARE_OWNER_HELPER} call has {len(args)} args; "
                    f"keep it <= {LATE_DECLARE_OWNER_MAX_ARGS}",
                )
            )
        passed = [
            name
            for name in sorted(LATE_DECLARE_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        if passed:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{LATE_DECLARE_OWNER_HELPER} call passes module-state "
                    "directly: " + ", ".join(passed),
                )
            )

    for line_no, line in enumerate(source_lines(facade_path), 1):
        used = [
            name
            for name in sorted(LATE_DECLARE_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", line)
        ]
        if used:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade directly references late-declare module state: "
                    + ", ".join(used),
                )
            )
    return findings


def loop_metadata_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    metrics_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_metrics_state.seen"
    facade_text = "\n".join(source_lines(facade_path))
    metrics_text = "\n".join(source_lines(metrics_path))

    definition = find_function_definition(metrics_text, LOOP_METADATA_OWNER_HELPER)
    if definition is None:
        findings.append(Finding(metrics_path, 1, f"missing {LOOP_METADATA_OWNER_HELPER}"))
    else:
        line_no, params = definition
        if len(params) > LOOP_METADATA_OWNER_MAX_ARGS:
            findings.append(
                Finding(
                    metrics_path,
                    line_no,
                    f"{LOOP_METADATA_OWNER_HELPER} has {len(params)} params; "
                    f"keep it <= {LOOP_METADATA_OWNER_MAX_ARGS} so loop "
                    "metadata pipe strings stay behind the metrics owner",
                )
            )

    for line_no, _, _, call_text in find_calls(facade_text, LOOP_METADATA_HELPER):
        passed = [
            name
            for name in sorted(LOOP_METADATA_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        detail = ""
        if passed:
            detail = ": " + ", ".join(passed)
        findings.append(
            Finding(
                facade_path,
                line_no,
                f"facade calls {LOOP_METADATA_HELPER} directly{detail}; use "
                f"{LOOP_METADATA_OWNER_HELPER} so loop metadata String state "
                "stays behind the metrics owner",
            )
        )

    for line_no, _, args, call_text in find_calls(facade_text, LOOP_METADATA_OWNER_HELPER):
        if len(args) > LOOP_METADATA_OWNER_MAX_ARGS:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{LOOP_METADATA_OWNER_HELPER} call has {len(args)} args; "
                    f"keep it <= {LOOP_METADATA_OWNER_MAX_ARGS}",
                )
            )
        passed = [
            name
            for name in sorted(LOOP_METADATA_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", call_text)
        ]
        if passed:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"{LOOP_METADATA_OWNER_HELPER} call passes metrics state "
                    "directly: " + ", ".join(passed),
                )
            )

    for line_no, line in enumerate(source_lines(facade_path), 1):
        used = [
            name
            for name in sorted(LOOP_METADATA_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", line)
        ]
        if used:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade directly references loop metadata metrics state: "
                    + ", ".join(used),
                )
            )
    return findings


def module_tail_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    feature_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_feature_state.seen"
    facade_text = "\n".join(source_lines(facade_path))
    feature_text = "\n".join(source_lines(feature_path))

    for helper, owner_helper in sorted(MODULE_TAIL_HELPERS.items()):
        definition = find_function_definition(feature_text, owner_helper)
        if definition is None:
            findings.append(Finding(feature_path, 1, f"missing {owner_helper}"))
        else:
            line_no, params = definition
            if len(params) > 1:
                findings.append(
                    Finding(
                        feature_path,
                        line_no,
                        f"{owner_helper} has {len(params)} params; keep module "
                        "tail feature state behind the feature owner",
                    )
                )

        for line_no, _, _, call_text in find_calls(facade_text, helper):
            passed = [
                name
                for name in sorted(MODULE_TAIL_FORBIDDEN_STATE)
                if re.search(rf"\b{name}\b", call_text)
            ]
            detail = ""
            if passed:
                detail = ": " + ", ".join(passed)
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"facade calls {helper} directly{detail}; use "
                    f"{owner_helper}",
                )
            )

        for line_no, _, args, call_text in find_calls(facade_text, owner_helper):
            if len(args) > 1:
                findings.append(
                    Finding(
                        facade_path,
                        line_no,
                        f"{owner_helper} call has {len(args)} args; pass only output",
                    )
                )
            passed = [
                name
                for name in sorted(MODULE_TAIL_FORBIDDEN_STATE)
                if re.search(rf"\b{name}\b", call_text)
            ]
            if passed:
                findings.append(
                    Finding(
                        facade_path,
                        line_no,
                        f"{owner_helper} call passes feature state directly: "
                        + ", ".join(passed),
                    )
                )
    return findings


def block_terminated_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    global_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_global_state.seen"
    facade_lines = source_lines(facade_path)
    global_text = "\n".join(source_lines(global_path))

    if find_function_definition(global_text, BLOCK_TERMINATED_GETTER) is None:
        findings.append(Finding(global_path, 1, f"missing {BLOCK_TERMINATED_GETTER}"))
    if find_function_definition(global_text, BLOCK_TERMINATED_SETTER) is None:
        findings.append(Finding(global_path, 1, f"missing {BLOCK_TERMINATED_SETTER}"))

    for line_no, line in enumerate(facade_lines, 1):
        if "import codegen.ir_codegen_global_state" in line and re.search(
            r"\{\s*[^}]*\bblockTerminated\b", line
        ):
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade imports blockTerminated directly; use "
                    f"{BLOCK_TERMINATED_GETTER}/{BLOCK_TERMINATED_SETTER} "
                    "so function pre-body resets cannot leave a stale facade copy",
                )
            )
        if re.search(r"\bblockTerminated\b", line):
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade references blockTerminated directly; use "
                    "isBlockTerminated()/setBlockTerminated() owner-backed helpers",
                )
            )
    return findings


def per_function_global_state_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    global_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_global_state.seen"
    facade_lines = source_lines(facade_path)
    global_text = "\n".join(source_lines(global_path))

    for state_name, (getter, setter) in sorted(PER_FUNCTION_GLOBAL_STATE_ACCESSORS.items()):
        if find_function_definition(global_text, getter) is None:
            findings.append(Finding(global_path, 1, f"missing {getter}"))
        if find_function_definition(global_text, setter) is None:
            findings.append(Finding(global_path, 1, f"missing {setter}"))
    if find_function_definition(global_text, ACTIVE_VAR_COUNT_BOUNDED_GETTER) is None:
        findings.append(Finding(global_path, 1, f"missing {ACTIVE_VAR_COUNT_BOUNDED_GETTER}"))

    state_pattern = re.compile(
        r"(?<!\.)\b("
        + "|".join(
            re.escape(name)
            for name in sorted(PER_FUNCTION_GLOBAL_STATE_ACCESSORS)
        )
        + r")\b"
    )
    for line_no, line in enumerate(facade_lines, 1):
        if "import codegen.ir_codegen_global_state" in line:
            imported = sorted(set(state_pattern.findall(line)))
            if ACTIVE_VAR_COUNT_UNBOUNDED_GETTER in line:
                findings.append(
                    Finding(
                        facade_path,
                        line_no,
                        "facade imports unbounded active-var-count getter; use "
                        f"{ACTIVE_VAR_COUNT_BOUNDED_GETTER}(this.varNames) so "
                        "corrupt bootstrap state cannot drive unbounded scans",
                    )
                )
            if imported:
                findings.append(
                    Finding(
                        facade_path,
                        line_no,
                        "facade imports per-function global state directly: "
                        + ", ".join(imported)
                        + "; use global-state getter/setter helpers",
                    )
                )
            continue

        if re.search(rf"\b{re.escape(ACTIVE_VAR_COUNT_UNBOUNDED_GETTER)}\s*\(", line):
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade uses unbounded active-var-count getter; use "
                    f"{ACTIVE_VAR_COUNT_BOUNDED_GETTER}(this.varNames) before "
                    "passing counts into variable scans or binding storage",
                )
            )

        used = sorted(set(state_pattern.findall(line)))
        if not used:
            continue
        findings.append(
            Finding(
                facade_path,
                line_no,
                "facade references per-function global state directly: "
                + ", ".join(used)
                + "; use owner-backed getter/setter helpers so class methods "
                "do not read bogus facade object offsets",
            )
        )
    return findings


def registry_global_state_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    global_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_global_state.seen"
    facade_lines = source_lines(facade_path)
    global_text = "\n".join(source_lines(global_path))

    for state_name, getter in sorted(REGISTRY_GLOBAL_STATE_ACCESSORS.items()):
        if find_function_definition(global_text, getter) is None:
            findings.append(Finding(global_path, 1, f"missing {getter} for {state_name}"))

    state_pattern = re.compile(
        r"(?<!\.)\b("
        + "|".join(
            re.escape(name)
            for name in sorted(REGISTRY_GLOBAL_STATE_ACCESSORS)
        )
        + r")\b"
    )
    for line_no, line in enumerate(facade_lines, 1):
        if "import codegen.ir_codegen_global_state" in line:
            imported = sorted(set(state_pattern.findall(line)))
            if imported:
                findings.append(
                    Finding(
                        facade_path,
                        line_no,
                        "facade imports bootstrap registry globals directly: "
                        + ", ".join(imported)
                        + "; use global-state getter helpers",
                    )
                )
            continue

        used = sorted(set(state_pattern.findall(line)))
        if not used:
            continue
        findings.append(
            Finding(
                facade_path,
                line_no,
                "facade references bootstrap registry globals directly: "
                + ", ".join(used)
                + "; use owner-backed getter helpers so old bootstrap "
                "compilers cannot lower them as LLVMIRGenerator fields",
            )
        )
    return findings


def declaration_storage_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    global_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_global_state.seen"
    facade_text = "\n".join(source_lines(facade_path))
    facade_lines = source_lines(facade_path)
    global_text = "\n".join(source_lines(global_path))

    for helper in sorted(DECL_STORAGE_OWNER_HELPERS):
        if find_function_definition(global_text, helper) is None:
            findings.append(Finding(global_path, 1, f"missing {helper}"))

    for helper, replacement in sorted(DECL_STORAGE_DIRECT_HELPERS.items()):
        for line_no, _, _, call_text in find_calls(facade_text, helper):
            passed = [
                name
                for name in sorted(DECL_STORAGE_FORBIDDEN_STATE)
                if re.search(rf"\b{name}\b", call_text)
            ]
            detail = ""
            if passed:
                detail = ": " + ", ".join(passed)
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"facade calls {helper} directly{detail}; use "
                    f"{replacement} so declaration ordinal/preallocation "
                    "arrays stay behind the global-state owner",
                )
            )

    for line_no, line in enumerate(facade_lines, 1):
        used = [
            name
            for name in sorted(DECL_STORAGE_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", line)
        ]
        if used:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade directly references declaration ordinal state: "
                    + ", ".join(used),
                )
            )
    return findings


def module_constant_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    module_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_module_state.seen"
    global_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_global_state.seen"
    facade_text = "\n".join(source_lines(facade_path))
    facade_lines = source_lines(facade_path)
    module_text = "\n".join(source_lines(module_path))
    global_text = "\n".join(source_lines(global_path))

    for helper in sorted(MODULE_CONST_OWNER_HELPERS):
        if find_function_definition(module_text, helper) is None:
            findings.append(Finding(module_path, 1, f"missing {helper}"))
    if find_function_definition(global_text, "getModuleConstantTypeWithGlobalStateImpl") is None:
        findings.append(
            Finding(global_path, 1, "missing getModuleConstantTypeWithGlobalStateImpl")
        )

    for helper, replacement in sorted(MODULE_CONST_DIRECT_HELPERS.items()):
        for line_no, _, _, call_text in find_calls(facade_text, helper):
            passed = [
                name
                for name in sorted(
                    MODULE_CONST_FORBIDDEN_GLOBAL_STATE
                    | MODULE_CONST_FORBIDDEN_MODULE_STATE
                )
                if re.search(rf"\b{name}\b", call_text)
            ]
            detail = ""
            if passed:
                detail = ": " + ", ".join(passed)
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    f"facade calls {helper} directly{detail}; use "
                    f"{replacement} so module-constant arrays and pipe "
                    "strings stay behind their owner modules",
                )
            )

    for line_no, line in enumerate(facade_lines, 1):
        used_global = [
            name
            for name in sorted(MODULE_CONST_FORBIDDEN_GLOBAL_STATE)
            if re.search(rf"\b{name}\b", line)
        ]
        used_module = [
            name
            for name in sorted(MODULE_CONST_FORBIDDEN_MODULE_STATE)
            if re.search(rf"\b{name}\b", line)
        ]
        if used_global:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade directly references module-constant global state: "
                    + ", ".join(used_global),
                )
            )
        if used_module:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade directly references module-constant module state: "
                    + ", ".join(used_module),
                )
            )
    return findings


def function_registry_boundary_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    facade_path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    global_path = root / "compiler_seen" / "src" / "codegen" / "ir_codegen_global_state.seen"
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    facade_lines = source_lines(facade_path)
    global_text = "\n".join(source_lines(global_path))

    for getter in FUNCTION_REGISTRY_GETTERS:
        if find_function_definition(global_text, getter) is None:
            findings.append(Finding(global_path, 1, f"missing {getter}"))

    for line_no, line in enumerate(facade_lines, 1):
        used = [
            name
            for name in sorted(FUNCTION_REGISTRY_FORBIDDEN_STATE)
            if re.search(rf"\b{name}\b", line)
        ]
        if used:
            findings.append(
                Finding(
                    facade_path,
                    line_no,
                    "facade directly references function-registry state: "
                    + ", ".join(used)
                    + "; use global-state function registry accessors",
                )
            )

    function_pattern = re.compile(
        r"\b(?:static\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\("
    )
    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        for match in function_pattern.finditer(text):
            open_index = text.find("(", match.start())
            params, _ = collect_balanced(text, open_index)
            for param in split_top_level_args(params):
                param_match = re.match(
                    r"\s*(funcNames|funcRetTypes)\s*:\s*([^=]+?)\s*$",
                    param,
                    re.S,
                )
                if not param_match:
                    continue
                param_name = param_match.group(1)
                actual_type = " ".join(param_match.group(2).split())
                expected_type = FUNCTION_REGISTRY_PARAM_TYPES[param_name]
                if actual_type != expected_type:
                    findings.append(
                        Finding(
                            path,
                            line_for_offset(text, match.start()),
                            f"{match.group(1)} parameter {param_name} has "
                            f"type {actual_type}; function registry values "
                            f"must stay {expected_type} to avoid self-hosted "
                            "String/Array ABI mismatch",
                        )
                    )
    return findings


def call_default_registry_param_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    function_pattern = re.compile(
        r"\b(?:static\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\("
    )

    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        for match in function_pattern.finditer(text):
            open_index = text.find("(", match.start())
            params, _ = collect_balanced(text, open_index)
            for param in split_top_level_args(params):
                param_match = re.match(
                    r"\s*(funcParamCountArr|funcDefaultsArr)\s*:\s*([^=]+?)\s*$",
                    param,
                    re.S,
                )
                if not param_match:
                    continue
                param_name = param_match.group(1)
                actual_type = " ".join(param_match.group(2).split())
                expected_type = CALL_DEFAULT_REGISTRY_PARAM_TYPES[param_name]
                if actual_type != expected_type:
                    findings.append(
                        Finding(
                            path,
                            line_for_offset(text, match.start()),
                            f"{match.group(1)} parameter {param_name} has "
                            f"type {actual_type}; default-argument registries "
                            f"must stay {expected_type} to avoid self-hosted "
                            "String/Array ABI mismatch in regular calls",
                        )
                    )
    return findings


def call_argument_lowering_callback_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    callback_re = re.compile(r"\bprepareLoweredCallArgumentsWithDefaults\s*\(")
    allowed_files = {
        "ir_lowering_context.seen",
        "ir_lowering_context_bridge.seen",
        "llvm_ir_gen.seen",
    }

    for path in sorted(codegen_path.glob("*.seen")):
        if path.name in allowed_files:
            continue
        for lineno, line in enumerate(source_lines(path), start=1):
            if callback_re.search(line):
                findings.append(
                    Finding(
                        path,
                        lineno,
                        "call argument lowering must use "
                        "prepareCallArgumentsWithDefaultsState directly; "
                        "routing ParserExpressionNode arrays through the "
                        "lowering-context callback has caused Stage3 ABI "
                        "corruption",
                    )
                )
    return findings


def reprc_constructor_owner_state_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    helper_re = re.compile(r"\bisReprCConstructorTypeImpl\s*\(")
    allowed_files = {
        "ir_constructor_call_plan.seen",
        "ir_codegen_feature_state.seen",
    }

    for path in sorted(codegen_path.glob("*.seen")):
        if path.name in allowed_files:
            continue
        for lineno, line in enumerate(source_lines(path), start=1):
            if helper_re.search(line):
                findings.append(
                    Finding(
                        path,
                        lineno,
                        "ReprC constructor checks must go through "
                        "isReprCConstructorTypeWithFeatureStateImpl; passing "
                        "struct-name arrays where pipe-delimited ReprC strings "
                        "are expected can corrupt Stage3 string ABI calls",
                    )
                )
    return findings


def late_declare_stack_api_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_path = root / "compiler_seen" / "src" / "codegen"

    function_pattern = re.compile(
        r"\b(?:static\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\("
    )
    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        for match in function_pattern.finditer(text):
            helper_name = match.group(1)
            if helper_name not in LATE_DECLARE_ROUTING_HELPERS:
                continue
            open_index = text.find("(", match.start())
            params_text, _ = collect_balanced(text, open_index)
            params = split_top_level_args(params_text)
            max_args = LATE_DECLARE_STACK_HELPER_MAX_ARGS.get(helper_name)
            if max_args is not None and len(params) > max_args:
                findings.append(
                    Finding(
                        path,
                        line_for_offset(text, match.start()),
                        f"{helper_name} has {len(params)} params; keep it "
                        f"<= {max_args} so late-declare state is not read "
                        "from fragile deep stack slots",
                    )
                )
            forbidden = sorted(
                name
                for name in LATE_DECLARE_UNUSED_STATE_PARAMS
                if re.search(rf"\b{name}\s*:", params_text)
            )
            if forbidden:
                findings.append(
                    Finding(
                        path,
                        line_for_offset(text, match.start()),
                        f"{helper_name} carries unused late-declare routing "
                        "state: " + ", ".join(forbidden),
                    )
                )

    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        for helper_name, max_args in sorted(LATE_DECLARE_STACK_HELPER_MAX_ARGS.items()):
            for line_no, _, args, call_text in find_calls(text, helper_name):
                if len(args) > max_args:
                    findings.append(
                        Finding(
                            path,
                            line_no,
                            f"{helper_name} call has {len(args)} args; keep "
                            f"late-declare calls <= {max_args} args",
                        )
                    )
                passed = [
                    name
                    for name in sorted(LATE_DECLARE_UNUSED_STATE_PARAMS)
                    if re.search(rf"\b{name}\b", call_text)
                ]
                if passed:
                    findings.append(
                        Finding(
                            path,
                            line_no,
                            f"{helper_name} call passes unused late-declare "
                            "routing state: " + ", ".join(passed),
                        )
                    )
    return findings


def param_declared_type(param: str) -> str:
    if ":" not in param:
        return ""
    type_text = param.split(":", 1)[1].split("=", 1)[0].strip()
    # Drop a possible return marker if a malformed parse included it.
    if ") r:" in type_text:
        type_text = type_text.split(") r:", 1)[0].strip()
    return type_text


def is_aggregate_abi_type(type_name: str) -> bool:
    if not type_name:
        return False
    if type_name == "String" or type_name == "StringBuilder":
        return True
    if type_name.startswith("Array<"):
        return True
    if type_name.endswith("Snapshot") or type_name.endswith("State"):
        return True
    if type_name[0].isupper() and type_name not in AGGREGATE_ABI_PRIMITIVE_TYPES:
        return True
    return False


def aggregate_abi_signature_findings(root: Path) -> list[Finding]:
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    findings: list[Finding] = []
    risky_defs: dict[str, tuple[Path, int, int, int]] = {}
    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        pattern = re.compile(r"\bfun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
        for match in pattern.finditer(text):
            name = match.group(1)
            open_index = text.find("(", match.start())
            params_text, _ = collect_balanced(text, open_index)
            params = split_top_level_args(params_text)
            aggregate_params = [
                param
                for param in params
                if is_aggregate_abi_type(param_declared_type(param))
            ]
            if len(params) < AGGREGATE_ABI_PARAM_THRESHOLD:
                continue
            if len(aggregate_params) < AGGREGATE_ABI_AGGREGATE_THRESHOLD:
                continue
            key = f"{path.stem}.{name}"
            if key in AGGREGATE_ABI_SIGNATURE_ALLOWLIST:
                continue
            line_no = line_for_offset(text, match.start())
            risky_defs[name] = (path, line_no, len(params), len(aggregate_params))
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"{name} has {len(params)} params with "
                    f"{len(aggregate_params)} aggregate-like values; use a "
                    "state/context object, owner wrapper, or split helper "
                    f"(allowlist key: {key})",
                )
            )

    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        for line_no, callee, args, _ in find_calls(text):
            risky = risky_defs.get(callee)
            if risky is None:
                continue
            def_path, _, param_count, aggregate_count = risky
            if def_path == path:
                continue
            if len(args) < AGGREGATE_ABI_PARAM_THRESHOLD:
                continue
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"cross-module call to aggregate-heavy helper {callee} "
                    f"passes {len(args)} args; definition has {param_count} "
                    f"params and {aggregate_count} aggregate-like values",
                )
            )
    return findings


def call_driver_enum_state_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_call_driver.seen"
    if not path.exists():
        return []

    text = "\n".join(source_lines(path))
    match = re.search(r"\bfun\s+generateCallState\s*\(", text)
    if not match:
        return []

    params_text, _ = collect_balanced(text, text.find("(", match.start()))
    expected = {
        "enumVariantNames": "String",
        "enumVariantFieldCounts": "String",
    }
    findings: list[Finding] = []
    for param in split_top_level_args(params_text):
        if ":" not in param:
            continue
        name = param.split(":", 1)[0].strip()
        declared = param_declared_type(param)
        wanted = expected.get(name)
        if wanted is None or declared == wanted:
            continue
        findings.append(
            Finding(
                path,
                line_for_offset(text, match.start()),
                f"generateCallState {name} must be {wanted}; feature enum "
                "state is pipe-encoded String data, and treating it as an "
                "Array corrupts Stage2 call lowering",
            )
        )
    return findings


def enum_feature_state_param_findings(root: Path) -> list[Finding]:
    codegen_path = root / "compiler_seen" / "src" / "codegen"
    expected_string_params = {
        "enumVariantNames",
        "enumVariantTags",
        "enumVariantFieldTypes",
        "enumVariantFieldCounts",
        "enumVariantParent",
    }
    findings: list[Finding] = []
    for path in sorted(codegen_path.glob("*.seen")):
        text = "\n".join(source_lines(path))
        for match in re.finditer(r"\bfun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", text):
            params_text, _ = collect_balanced(text, text.find("(", match.start()))
            for param in split_top_level_args(params_text):
                if ":" not in param:
                    continue
                name = param.split(":", 1)[0].strip()
                if name not in expected_string_params:
                    continue
                declared = param_declared_type(param)
                if declared == "String":
                    continue
                findings.append(
                    Finding(
                        path,
                        line_for_offset(text, match.start()),
                        f"{match.group(1)} parameter {name} must be String; "
                        "feature enum state is pipe-encoded owner data, not "
                        f"{declared}",
                    )
                )
    return findings


def constructor_declaration_static_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_decl_items.seen"
    if not path.exists():
        return []

    findings: list[Finding] = []
    lines = source_lines(path)
    for idx, line in enumerate(lines):
        if "buildDeclareParamsImpl" not in line:
            continue
        window = "\n".join(lines[max(0, idx - 4) : idx + 5])
        if "not method.isStatic" in window and 'method.name == "new"' not in window:
            findings.append(
                Finding(
                    path,
                    idx + 1,
                    "class method declarations must treat `new` as static; "
                    "use a methodIsStatic value that includes "
                    'method.name == "new"',
                )
            )
    return findings


def main_compiler_bootstrap_constructor_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "main_compiler.seen"
    if not path.exists():
        return []

    findings: list[Finding] = []
    for line_no, line in enumerate(source_lines(path), 1):
        for ctor in MAIN_COMPILER_BOOTSTRAP_FRAGILE_CONSTRUCTORS:
            if f"{ctor}(" in line:
                findings.append(
                    Finding(
                        path,
                        line_no,
                        f"{ctor} is fragile when frozen bootstrap emits stale "
                        "cross-module constructor declarations; use a local "
                        "struct-literal helper in main_compiler.seen",
                    )
                )
    return findings


def frontend_bootstrap_constructor_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "bootstrap" / "frontend.seen"
    if not path.exists():
        return []

    findings: list[Finding] = []
    for line_no, line in enumerate(source_lines(path), 1):
        for call in FRONTEND_BOOTSTRAP_FRAGILE_CALLS:
            if f"{call}(" in line:
                findings.append(
                    Finding(
                        path,
                        line_no,
                        f"{call} is fragile when frozen bootstrap emits stale "
                        "cross-module constructor declarations; use a local "
                        "literal helper or top-level compat wrapper",
                    )
                )
    return findings


def lsp_bootstrap_constructor_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "lsp" / "server.seen"
    if not path.exists():
        return []

    findings: list[Finding] = []
    for line_no, line in enumerate(source_lines(path), 1):
        for call in LSP_BOOTSTRAP_FRAGILE_CALLS:
            if f"{call}(" in line:
                findings.append(
                    Finding(
                        path,
                        line_no,
                        f"{call} is fragile when frozen bootstrap emits stale "
                        "cross-module constructor declarations; use a local "
                        "literal helper or top-level compat wrapper",
                    )
                )
    return findings


def facade_owner_call_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    text = "\n".join(source_lines(path))
    findings: list[Finding] = []
    owner_pattern = re.compile(
        r"\b(" + "|".join(re.escape(name) for name in sorted(OWNER_STATE_NAMES)) + r")\b"
    )
    for line_no, callee, args, call_text in find_calls(text):
        if len(args) < 6:
            continue
        owner_names = sorted(set(owner_pattern.findall(call_text)))
        if not owner_names:
            continue
        if callee == IDENTITY_HELPER or "generatedFunctions" in owner_names:
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"fragile facade owner-state call to {callee}: "
                    + ", ".join(owner_names),
                )
            )
            continue
        if callee not in KNOWN_FACADE_OWNER_CALLS:
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"new unreviewed large facade call passes owner-state to "
                    f"{callee}: {', '.join(owner_names)}",
                )
            )
    return findings


def facade_string_prefix_owner_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    findings: list[Finding] = []
    for line_no, line in enumerate(source_lines(path), 1):
        if re.search(r"\bstringConstantPrefix\s*=", line):
            findings.append(
                Finding(
                    path,
                    line_no,
                    "facade writes stringConstantPrefix directly; use "
                    "setStringConstantPrefixWithGlobalStateImpl() so "
                    "state-based literal lowering sees the module prefix",
                )
            )
        if re.search(r"[, (]stringConstantPrefix[, )]", line):
            findings.append(
                Finding(
                    path,
                    line_no,
                    "facade reads stringConstantPrefix directly; use "
                    "getStringConstantPrefixWithGlobalStateImpl() so "
                    "prefixed string constants and literal uses stay aligned",
                )
            )
    return findings


def driver_string_literal_local_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_root = root / "compiler_seen" / "src" / "codegen"
    if not codegen_root.exists():
        return findings

    bad_var_re = re.compile(r"\bvar\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\"")
    for path in sorted(codegen_root.glob("ir_*driver.seen")):
        for line_no, line in enumerate(source_lines(path), 1):
            match = bad_var_re.search(line)
            if not match:
                continue
            findings.append(
                Finding(
                    path,
                    line_no,
                    "driver local string literal needs an explicit "
                    "`: String` annotation; old-bootstrap inference can "
                    "lower it as i64 and later build a null-data SeenString",
                )
            )
    return findings


def lowering_context_dyn_param_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_root = root / "compiler_seen" / "src" / "codegen"
    if not codegen_root.exists():
        return findings

    bad_param_re = re.compile(r"\bctx\s*:\s*dyn\s+CodegenLoweringContext\b")
    for path in sorted(codegen_root.glob("*.seen")):
        for line_no, line in enumerate(source_lines(path), 1):
            if not bad_param_re.search(line):
                continue
            findings.append(
                Finding(
                    path,
                    line_no,
                    "bootstrap declaration scanning emits a phantom `void` "
                    "parameter for `dyn CodegenLoweringContext`; use "
                    "`ctx: CodegenLoweringContext` in codegen driver helpers",
                )
            )
    return findings


def lowering_context_alias_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    path = root / "compiler_seen" / "src" / "codegen" / "ir_lowering_context.seen"
    if path.exists():
        text = "\n".join(source_lines(path))
        if not re.search(r"\btype\s+CodegenLoweringContext\s*=\s*Int\b", text):
            findings.append(
                Finding(
                    path,
                    1,
                    "CodegenLoweringContext must stay an opaque Int handle; "
                    "trait/class context types use unstable Stage2 trait/object ABI",
                )
            )
        for line_no, line in enumerate(text.splitlines(), 1):
            if re.search(r"\b(trait|class)\s+CodegenLoweringContext\b", line):
                findings.append(
                    Finding(
                        path,
                        line_no,
                        "CodegenLoweringContext must not be a trait/class; "
                        "use `type CodegenLoweringContext = Int`",
                    )
                )

    facade = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    if facade.exists():
        for line_no, line in enumerate(source_lines(facade), 1):
            if re.search(r"\bimpl\s+CodegenLoweringContext\s+for\b", line):
                findings.append(
                    Finding(
                        facade,
                        line_no,
                        "LLVMIRGenerator must not implement CodegenLoweringContext; "
                        "the lowering context is an opaque Int handle, not a trait",
                    )
                )
    return findings


def lowering_context_facade_cast_findings(root: Path) -> list[Finding]:
    facade = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    if not facade.exists():
        return []

    findings: list[Finding] = []
    bad_cast_re = re.compile(r"\bctxHandle\s+as\s+LLVMIRGenerator\b")
    for line_no, line in enumerate(source_lines(facade), 1):
        if not bad_cast_re.search(line):
            continue
        findings.append(
            Finding(
                facade,
                line_no,
                "`ctxHandle as LLVMIRGenerator` inside facade callbacks "
                "mis-mangles under the frozen compiler into Int_/String_ "
                "receiver calls; forward through explicit LLVMIRGenerator_* "
                "ABI wrappers instead",
            )
        )
    return findings


def lowering_context_facade_extern_callback_findings(root: Path) -> list[Finding]:
    facade = root / "compiler_seen" / "src" / "codegen" / "llvm_ir_gen.seen"
    if not facade.exists():
        return []

    findings: list[Finding] = []
    bad_extern_re = re.compile(r"\bextern\s+fun\s+LLVMIRGenerator_[A-Za-z0-9_]*\s*\(")
    for line_no, line in enumerate(source_lines(facade), 1):
        if not bad_extern_re.search(line):
            continue
        findings.append(
            Finding(
                facade,
                line_no,
                "LLVMIRGenerator_* extern callbacks must not live in "
                "llvm_ir_gen.seen; the same module defines the class methods "
                "and Stage3 rejects the duplicate declare/define symbol. Put "
                "the extern callback bridge in ir_lowering_context_bridge.seen",
            )
        )
    return findings


def lowering_context_duplicate_symbol_callback_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_lowering_context.seen"
    if not path.exists():
        return []

    findings: list[Finding] = []
    for line_no, line in enumerate(source_lines(path), 1):
        match = re.match(r"\s*fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", line)
        if not match:
            continue
        name = match.group(1)
        if name not in LOWERING_CONTEXT_FORBIDDEN_CALLBACKS:
            continue
        findings.append(
            Finding(
                path,
                line_no,
                f"`{name}` in CodegenLoweringContext emits a trait stub that "
                "collides with the LLVMIRGenerator implementation at Stage2 link; "
                "thread output/state through explicit helper parameters instead",
            )
        )
    return findings


def lowering_context_method_call_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_root = root / "compiler_seen" / "src" / "codegen"
    if not codegen_root.exists():
        return findings

    callback_pattern = "|".join(sorted(map(re.escape, LOWERING_CONTEXT_CALLBACK_NAMES)))
    bad_call_re = re.compile(rf"\bctx\.({callback_pattern})\s*\(")
    for path in sorted(codegen_root.glob("*.seen")):
        if path.name == "llvm_ir_gen.seen":
            continue
        for line_no, line in enumerate(source_lines(path), 1):
            match = bad_call_re.search(line)
            if not match:
                continue
            findings.append(
                Finding(
                    path,
                    line_no,
                    f"`ctx.{match.group(1)}(...)` emits broken Stage2 "
                    "trait-call ABI through the frozen compiler; call the "
                    "imported lowering-context callback function instead",
                )
            )
    return findings


def lowering_context_feature_setter_call_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_root = root / "compiler_seen" / "src" / "codegen"
    if not codegen_root.exists():
        return findings

    bad_call_re = re.compile(r"\bsetLowerCurrentClassParentName\s*\(")
    for path in sorted(codegen_root.glob("*.seen")):
        if path.name in {"ir_lowering_context.seen", "ir_lowering_context_bridge.seen", "llvm_ir_gen.seen"}:
            continue
        for line_no, line in enumerate(source_lines(path), 1):
            code = strip_line_comment(line)
            if not bad_call_re.search(code):
                continue
            findings.append(
                Finding(
                    path,
                    line_no,
                    "`setLowerCurrentClassParentName(...)` emits a plain "
                    "lowering-context link symbol under the frozen compiler; "
                    "call setFeatureCurrentClassParentNameImpl(...) from "
                    "ir_codegen_feature_state instead",
                )
            )
    return findings


def module_driver_stale_reset_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_module_driver.seen"
    if not path.exists():
        return []

    findings: list[Finding] = []
    text = "\n".join(source_lines(path))
    bad_calls = ("resetLoweringGenerator", "resetLoweringSharedModuleScratch")
    for function_name in ("generateMultipleState", "generateSingleState"):
        body = find_function_body(text, function_name)
        if body is None:
            continue
        body_line, body_text = body
        for bad_call in bad_calls:
            match = re.search(rf"\b{re.escape(bad_call)}\s*\(", body_text)
            if not match:
                continue
            findings.append(
                Finding(
                    path,
                    body_line + body_text.count("\n", 0, match.start()),
                    f"{function_name} calls {bad_call} after the facade has "
                    "already passed output/stringConstants into the state "
                    "driver; reset in the facade first so driver locals do "
                    "not keep stale Array/StringBuilder objects into Stage2",
                )
            )
    return findings


def math_runtime_broad_plan_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_call_runtime_driver.seen"
    if not path.exists():
        return []

    text = "\n".join(source_lines(path))
    body = find_function_body(text, "tryGenerateMathBuiltinCallState")
    if body is None:
        return []

    body_line, body_text = body
    match = re.search(r"\bplanRuntimeBuiltinDispatchImpl\s*\(", body_text)
    if not match:
        return []
    return [
        Finding(
            path,
            body_line + body_text.count("\n", 0, match.start()),
            "tryGenerateMathBuiltinCallState builds the full runtime dispatch "
            "plan for ordinary user calls; use direct math predicates so Stage2 "
            "does not exercise the fragile RuntimeBuiltinDispatchPlan class "
            "return on every regular call",
        )
    ]


def panic_runtime_terminator_state_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_call_runtime_driver.seen"
    if not path.exists():
        return []

    text = "\n".join(source_lines(path))
    body = find_function_body(text, "tryGeneratePanicRuntimeBuiltinCallState")
    if body is None:
        return []

    body_line, body_text = body
    if "emitPanicRuntimeCallImpl" not in body_text:
        return []
    if "setBlockTerminatedWithGlobalStateImpl(true)" in body_text:
        return []
    return [
        Finding(
            path,
            body_line,
            "panic runtime lowering emits an LLVM terminator; mark the "
            "current block terminated so if/loop drivers do not append a "
            "merge branch after unreachable",
        )
    ]


def short_circuit_label_namespace_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_root = root / "compiler_seen" / "src" / "codegen"
    short_circuit_path = codegen_root / "ir_binary_short_circuit.seen"
    driver_path = codegen_root / "ir_binary_short_circuit_driver.seen"

    if short_circuit_path.exists():
        lines = source_lines(short_circuit_path)
        for line_no, line in enumerate(lines, 1):
            if "getNextBlockFn" in line:
                findings.append(
                    Finding(
                        short_circuit_path,
                        line_no,
                        "short-circuit labels must use a distinct sc.* "
                        "namespace, not ordinary bbN labels, or recursive "
                        "child lowering can collide with reserved labels",
                    )
                )

        text = "\n".join(lines)
        body = find_function_body(text, "allocateShortCircuitBinaryLabelsImpl")
        if body is not None:
            body_line, _body_text = body
            if '"sc."' not in text or "getNextRegFn" not in text:
                findings.append(
                    Finding(
                        short_circuit_path,
                        body_line,
                        "allocateShortCircuitBinaryLabelsImpl should derive "
                        "sc.* labels from the register allocator so labels "
                        "cannot collide with normal bbN control-flow labels",
                    )
                )

    if driver_path.exists():
        text = "\n".join(source_lines(driver_path))
        for function_name in (
            "generateShortCircuitAndState",
            "generateShortCircuitOrState",
        ):
            body = find_function_body(text, function_name)
            if body is None:
                continue
            body_line, body_text = body
            alloc = body_text.find("allocateShortCircuitBinaryLabelsImpl")
            lower_left = body_text.find("let leftReg = lowerExpression")
            if alloc >= 0 and lower_left >= 0 and alloc < lower_left:
                findings.append(
                    Finding(
                        driver_path,
                        body_line + body_text.count("\n", 0, alloc),
                        f"{function_name} reserves short-circuit labels before "
                        "lowering the left operand; allocate after left "
                        "lowering so child control-flow labels cannot collide",
                    )
                )
    return findings


def branch_label_allocator_findings(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    codegen_root = root / "compiler_seen" / "src" / "codegen"
    if not codegen_root.exists():
        return findings

    for path in sorted(codegen_root.glob("*.seen")):
        lines = source_lines(path)
        for index, line in enumerate(lines):
            if "emitFunctionBodyIfBranchingImpl" not in line:
                continue
            snippet = "\n".join(lines[index : index + 4])
            if "getFeatureRegBoxImpl()" not in snippet:
                continue
            findings.append(
                Finding(
                    path,
                    index + 1,
                    "if/if-let branch labels must be allocated from the "
                    "block-label box, not the register box; using the "
                    "register counter can duplicate ordinary bbN labels "
                    "inside loops and break Stage3 LLVM verification",
                )
            )
    return findings


def explicit_this_member_access_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_member_access_driver.seen"
    if not path.exists():
        return []
    text = "\n".join(source_lines(path))
    body = find_function_body(text, "generateMemberAccessState")
    if body is None:
        return []
    body_line, body_text = body
    if 'receiverName == "this"' in body_text and (
        "tryGenerateImplicitThisFieldVariableState" in body_text
    ):
        return []
    return [
        Finding(
            path,
            body_line,
            "generateMemberAccessState must handle explicit this.field "
            "member access before route planning; otherwise class methods "
            "can lower fields to literal 0 and emit invalid ptr 0 IR",
        )
    ]


def class_method_context_array_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_class_method_driver.seen"
    if not path.exists():
        return []
    text = "\n".join(source_lines(path))
    body = find_function_body(text, "generateClassMethodFromListState")
    if body is None:
        return []
    body_line, body_text = body
    findings: list[Finding] = []
    if (
        "let collectionState = getSharedCodegenState()" not in body_text
        or "let methodState = getSharedCodegenState()" not in body_text
    ):
        findings.append(
            Finding(
                path,
                body_line,
                "class-method lowering must reacquire current generator "
                "variable arrays after context reset/clear callbacks; stale "
                "array references make this/parameter bindings invisible to "
                "recursive expression lowering",
            )
        )

    stale_binding_patterns = (
        "emitClassMethodReceiverBindingStateImpl(output, getFeatureRegBoxImpl(),\n"
        "        varNames, varRegs, varTypes",
        "emitClassMethodParameterBindingsStateImpl(output, getFeatureRegBoxImpl(),\n"
        "        varNames, varRegs, varTypes",
        "emitClassMethodConstructorSetupStateImpl(output, getFeatureRegBoxImpl(),\n"
        "        varNames, varRegs, varTypes",
        "tryEmitClassMethodConstructorReturnStateImpl(output,\n"
        "        getFeatureRegBoxImpl(), varNames, varRegs",
    )
    for pattern in stale_binding_patterns:
        if pattern in body_text:
            offset = body_text.find(pattern)
            findings.append(
                Finding(
                    path,
                    body_line + body_text.count("\n", 0, offset),
                    "class-method receiver/parameter/constructor bindings "
                    "must use reacquired methodVar* arrays, not the stale "
                    "arrays captured before resetLoweringLocalCodegenState",
                )
            )
    return findings


def binary_driver_operand_guard_findings(root: Path) -> list[Finding]:
    path = root / "compiler_seen" / "src" / "codegen" / "ir_binary_driver.seen"
    if not path.exists():
        return []
    text = "\n".join(source_lines(path))
    body = find_function_body(text, "generateBinaryState")
    if body is None:
        return []
    body_line, body_text = body
    findings: list[Finding] = []
    unsafe_patterns = (
        "leftExpr.operands.length()",
        "rightExpr.operands.length()",
    )
    for pattern in unsafe_patterns:
        offset = body_text.find(pattern)
        if offset >= 0:
            findings.append(
                Finding(
                    path,
                    body_line + body_text.count("\n", 0, offset),
                    "binary peephole/FMA planning must not read child "
                    "operands arrays before proving the child is a Binary "
                    "expression; leaf parser nodes may have uninitialized "
                    "operands arrays and crash Stage2/Stage3 codegen",
                )
            )
    return findings


def process_shell_quote_bootstrap_findings(root: Path) -> list[Finding]:
    path = root / "seen_std" / "src" / "process" / "process.seen"
    if not path.exists():
        return []
    text = "\n".join(source_lines(path))
    body = find_function_body(text, "shellQuote")
    if body is None:
        return []
    body_line, body_text = body
    findings: list[Finding] = []
    if "StringBuilder" in text or "StringBuilder.new" in body_text:
        findings.append(
            Finding(
                path,
                body_line,
                "process.shellQuote must stay on direct string operations; "
                "package prebuild runs it inside the compiler binary before "
                "artifact compilation, and the StringBuilder path can lower to "
                "the quoted string length instead of the quoted string",
            )
        )
    return findings


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "root",
        nargs="?",
        default=".",
        help="repository root (default: current directory)",
    )
    args = parser.parse_args()
    root = Path(args.root).resolve()

    findings: list[Finding] = []
    findings.extend(compiler_import_integrity_findings(root))
    findings.extend(main_compiler_direct_bootstrap_import_findings(root))
    findings.extend(compiler_import_cycle_findings(root))
    findings.extend(lowering_context_builder_return_findings(root))
    findings.extend(raw_float_literal_source_findings(root))
    findings.extend(synthetic_parser_expression_literal_findings(root))
    findings.extend(feature_getter_return_type_findings(root))
    findings.extend(find_owner_import_violations(root))
    findings.extend(static_method_dispatch_boundary_findings(root))
    findings.extend(final_method_dispatch_boundary_findings(root))
    findings.extend(class_method_metadata_boundary_findings(root))
    findings.extend(preallocated_alloca_boundary_findings(root))
    findings.extend(extern_generation_boundary_findings(root))
    findings.extend(string_builder_method_lower_boundary_findings(root))
    findings.extend(strict_helper_arity_findings(root))
    findings.extend(identity_boundary_findings(root))
    findings.extend(prebody_boundary_findings(root))
    findings.extend(facade_feature_box_findings(root))
    findings.extend(feature_state_metadata_boundary_findings(root))
    findings.extend(prelude_boundary_findings(root))
    findings.extend(entry_setup_boundary_findings(root))
    findings.extend(exit_reset_boundary_findings(root))
    findings.extend(late_declare_boundary_findings(root))
    findings.extend(loop_metadata_boundary_findings(root))
    findings.extend(module_tail_boundary_findings(root))
    findings.extend(block_terminated_boundary_findings(root))
    findings.extend(per_function_global_state_boundary_findings(root))
    findings.extend(registry_global_state_boundary_findings(root))
    findings.extend(declaration_storage_boundary_findings(root))
    findings.extend(module_constant_boundary_findings(root))
    findings.extend(function_registry_boundary_findings(root))
    findings.extend(call_default_registry_param_findings(root))
    findings.extend(call_argument_lowering_callback_findings(root))
    findings.extend(reprc_constructor_owner_state_findings(root))
    findings.extend(late_declare_stack_api_findings(root))
    findings.extend(ast_layout_boundary_findings(root))
    findings.extend(aggregate_abi_signature_findings(root))
    findings.extend(call_driver_enum_state_findings(root))
    findings.extend(enum_feature_state_param_findings(root))
    findings.extend(constructor_declaration_static_findings(root))
    findings.extend(main_compiler_bootstrap_constructor_findings(root))
    findings.extend(frontend_bootstrap_constructor_findings(root))
    findings.extend(lsp_bootstrap_constructor_findings(root))
    findings.extend(facade_owner_call_findings(root))
    findings.extend(facade_string_prefix_owner_findings(root))
    findings.extend(driver_string_literal_local_findings(root))
    findings.extend(lowering_context_dyn_param_findings(root))
    findings.extend(lowering_context_alias_findings(root))
    findings.extend(lowering_context_facade_cast_findings(root))
    findings.extend(lowering_context_facade_extern_callback_findings(root))
    findings.extend(lowering_context_duplicate_symbol_callback_findings(root))
    findings.extend(lowering_context_method_call_findings(root))
    findings.extend(lowering_context_feature_setter_call_findings(root))
    findings.extend(module_driver_stale_reset_findings(root))
    findings.extend(math_runtime_broad_plan_findings(root))
    findings.extend(panic_runtime_terminator_state_findings(root))
    findings.extend(short_circuit_label_namespace_findings(root))
    findings.extend(branch_label_allocator_findings(root))
    findings.extend(explicit_this_member_access_findings(root))
    findings.extend(class_method_context_array_findings(root))
    findings.extend(binary_driver_operand_guard_findings(root))
    findings.extend(process_shell_quote_bootstrap_findings(root))

    if findings:
        print("codegen ABI boundary preflight failed:", file=sys.stderr)
        for finding in findings:
            print(f"ERROR: {finding}", file=sys.stderr)
        return 1

    print("PASS: codegen ABI boundary preflight")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
