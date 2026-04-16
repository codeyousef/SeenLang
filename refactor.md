# LLVM IR Generator Refactor Plan

## Goal

Refactor `compiler_seen/src/codegen/llvm_ir_gen.seen` and the surrounding LLVM codegen area so responsibilities are separated cleanly, duplication is reduced, and no source file is longer than 500 lines unless there is a strong, explicit reason.

This started as an investigation and proposed plan. It now also tracks which refactor slices have already been completed.

## Completed So Far (2026-04-16)

### Current Snapshot

- This document reflects the current working tree, not just committed history.
- `llvm_ir_gen.seen` has been reduced from the plan baseline of `16,086` lines to `13,908` lines.
- New extracted helper modules now in tree:
  - `ir_module_emit.seen`
  - `ir_decl_scan.seen`
  - `ir_async_registry.seen`
  - `ir_trait_registry.seen`
  - `ir_call_fixups.seen`
  - `ir_method_finalize.seen`
  - `ir_field_layout.seen`
  - `ir_path_expr.seen`
  - `ir_member_access.seen`
  - `ir_method_receiver.seen`
  - `ir_binary_expr.seen`
  - `ir_class_method_gen.seen`
  - `ir_variable_gen.seen`
- `main_compiler.seen` bootstrap module registration has been updated for each new helper module added so far.
- The latest continuation also pushes more final instance-call cleanup into `ir_method_finalize.seen`, which is now `206` lines and owns shared return-type fallback, receiver ABI preparation, and `Option.unwrap()` specialization logic used by `emitNormalizedInstanceMethodCall()`.
- Current large-method snapshot:
  - `generateFunction()` is down to about `420` lines.
  - `generateMultiple()` is down to about `74` lines.
  - `generateSingle()` is down to about `195` lines.
  - `generateCall()` is down to about `139` lines.
  - `generateMethodCall()` is down to about `57` lines.
  - `generateBinary()` is down to about `339` lines.
  - `emitClassType()` is down to about `29` lines.
  - `generateClass()` is down to about `48` lines.
  - `generatePlainLargeClass()` is down to about `23` lines.
  - `tryGenerateTraitClass()` is down to about `23` lines.
  - `generateClassMethodFromList()` is down to about `32` lines.
  - `inferExpressionType()` is down to about `193` lines.
  - `generateAssignmentExpr()` is down to about `16` lines.
  - `generateAssignment()` is down to about `21` lines.
  - `generateVariable()` is down to about `36` lines.
  - `generateBlock()` is down to about `29` lines.
  - `generateStatement()` is down to about `16` lines.
  - `generateWhileStatement()` is down to about `32` lines.
  - `generateForInStatement()` is down to about `33` lines.
  - `generateExpression()` is down to about `44` lines.
  - `generateIfStatement()` is down to about `38` lines.
  - `generateIfLetStatement()` is down to about `32` lines.
  - `generateIfExpression()` is down to about `34` lines.
  - `generateLetStatement()` is down to about `9` lines.
  - `generateReturnStatement()` is down to about `29` lines.
  - `generateAwaitExpression()` is down to about `6` lines.
  - `generateUnary()` is down to about `24` lines.
  - `generateArrayLiteral()` is down to about `19` lines.
  - `generateElvis()` is down to about `23` lines.
  - `generateSafeNavigation()` is down to about `26` lines.
  - `generateStructLiteral()` is down to about `34` lines.
  - `generateStringInterpolation()` is down to about `20` lines.
  - `generateWhenExpression()` is down to about `57` lines.
  - `generateEnumConstructor()` is down to about `10` lines.
  - `generateMemberAccess()` is down to about `51` lines.
  - `generateShortCircuitAnd()` is down to about `49` lines.
  - `generateShortCircuitOr()` is down to about `45` lines.
  - `generateFieldAccess()` is down to about `30` lines.
  - `generateFieldAccessPtr()` is down to about `14` lines.
  - `generateMemberAssignment()` is down to about `31` lines.
  - `generateIndexAssignment()` is down to about `36` lines.
  - `resolveChainedPathType()` is down to about `26` lines.
  - Function-pipeline helpers inside `llvm_ir_gen.seen` are now split into:
    - `shouldSkipFunctionByCfg()` at about `50` lines.
    - `emitFunctionDecoratorMetadataComments()` at about `18` lines.
    - `tryHandleIntrinsicFunctionGeneration()` at about `47` lines.
    - `registerFunctionTraitImplDecorators()` at about `16` lines.
    - `tryHandleGpuShaderFunctionGeneration()` at about `31` lines.
    - `shouldSkipSpecialFunctionBody()` at about `12` lines.
    - `tryHandleExternFunctionGeneration()` at about `31` lines.
    - `resolveImplementationFunctionName()` at about `21` lines.
    - `resetFunctionGenerationState()` at about `20` lines.
    - `preRegisterFunctionParameters()` at about `48` lines.
    - `tryEmitMainFunction()` at about `28` lines.
    - `emitFunctionParameterAllocas()` at about `39` lines.
  - Module-entry helpers inside `llvm_ir_gen.seen` are now split into:
    - `resetSharedModuleGenerationScratchState()` at about `16` lines.
    - `collectProgramsStringConstants()` at about `8` lines.
    - `emitProgramClassTypes()` at about `11` lines.
    - `emitProgramsClassTypes()` at about `8` lines.
    - `emitProgramsGlobalVariables()` at about `8` lines.
    - `generateProgramClasses()` at about `15` lines.
    - `generateProgramsClasses()` at about `8` lines.
    - `emitCImportHeaderDeclares()` at about `66` lines.
    - `tryHandleSpecialTopLevelFunction()` at about `22` lines.
    - `shouldSkipStandaloneTopLevelFunction()` at about `7` lines.
    - `emitProgramTopLevelFunctions()` at about `37` lines.
    - `emitProgramsTopLevelFunctions()` at about `8` lines.
    - `collectProgramDefinedSymbols()` at about `27` lines.
    - `emitAdditionalGeneratedStringConstants()` at about `16` lines.
    - `emitGeneratedClosures()` at about `6` lines.
    - `emitOptimizationStatisticsSummary()` at about `68` lines.
  - Statement-pipeline helpers inside `llvm_ir_gen.seen` are now split into:
    - `isReadModifyWriteDeadStore()` at about `19` lines.
    - `tryMarkDeadStoreElimination()` at about `32` lines.
    - `scanBlockDeadStorePatterns()` at about `8` lines.
    - `emitTrailingDeadCodeNotice()` at about `8` lines.
    - `emitBlockDeferredCleanup()` at about `11` lines.
    - `tryGenerateAssignmentLikeExpression()` at about `16` lines.
    - `warnUnusedResultCall()` at about `15` lines.
    - `generateExpressionStatement()` at about `8` lines.
    - `emitContinueStatement()` at about `18` lines.
    - `emitBreakStatement()` at about `10` lines.
    - `generateDeferredStatement()` at about `12` lines.
    - `generateInlineBlockStatement()` at about `6` lines.
    - `generateUnsafeStatement()` at about `10` lines.
    - `generateTryCatchStatement()` at about `43` lines.
    - `tryGenerateBasicStatement()` at about `40` lines.
    - `tryGenerateLoopControlStatement()` at about `12` lines.
    - `tryGenerateScopedStatement()` at about `24` lines.
    - `tryGenerateMetaStatement()` at about `26` lines.
  - Receiver-preparation helpers inside `llvm_ir_gen.seen` are now split into:
    - `resolveRebuiltLiteralPathMethodReceiver()` at about `28` lines.
    - `resolveMethodFieldPathType()` at about `27` lines.
    - `resolveLiteralMethodReceiverType()` at about `33` lines.
    - `tryResolveEnumLiteralMethodReceiver()` at about `15` lines.
    - `tryResolveImplicitThisChainedLiteralMethodReceiver()` at about `33` lines.
    - `tryGenerateExplicitDynTraitMethodCall()` at about `30` lines.
    - `normalizeExplicitMethodReceiverType()` at about `9` lines.
    - `tryGenerateExplicitOptionMethodCall()` at about `15` lines.
    - `tryGenerateExplicitHotReloadMethodCall()` at about `21` lines.
    - `tryGenerateExplicitCollectionMethodCall()` at about `20` lines.
    - `tryPrepareExplicitMethodReceiver()` at about `43` lines.
    - `resolveChainedLiteralMethodReceiver()` at about `16` lines.
    - `tryGenerateSimpleVariableDynTraitMethodCall()` at about `30` lines.
    - `tryPrepareLocalVariableMethodReceiver()` at about `27` lines.
    - `fallbackModuleConstantMethodReceiverType()` at about `12` lines.
    - `tryPrepareModuleConstantMethodReceiver()` at about `19` lines.
    - `tryPrepareLLVMIRGeneratorFieldMethodReceiver()` at about `13` lines.
    - `tryPrepareStructImplicitThisFieldMethodReceiver()` at about `16` lines.
    - `tryPrepareKnownImplicitThisFieldMethodReceiver()` at about `18` lines.
    - `tryPrepareImplicitThisFieldMethodReceiver()` at about `32` lines.
    - `tryResolveSimpleLiteralMethodReceiver()` at about `29` lines.
    - `tryPrepareMethodCallReceiver()` at about `14` lines.
  - Method-call emission helpers inside `llvm_ir_gen.seen` are now split into:
    - `tryGenerateLengthLikeMethodCall()` at about `23` lines.
    - `tryGenerateStringBuilderReceiverMethodCall()` at about `33` lines.
    - `tryGeneratePrimitiveConversionMethodCall()` at about `19` lines.
    - `tryGenerateNumericReceiverMethodCall()` at about `21` lines.
    - `tryGenerateStringLikeReceiverMethodCall()` at about `40` lines.
    - `tryGenerateBuiltinMethodCall()` at about `26` lines.
    - `resolveMethodCallInstanceTypeName()` at about `17` lines.
    - `tryGenerateHotReloadInstanceMethodCall()` at about `16` lines.
    - `resolveMethodCallReturnType()` at about `5` lines.
    - `prepareMethodCallReceiverAbi()` at about `8` lines.
    - `tryGenerateSpecializedInstanceMethodCall()` at about `7` lines.
    - `emitNormalizedInstanceMethodCall()` at about `57` lines.
  - Method-call fallback helpers inside `llvm_ir_gen.seen` are now split into:
    - `tryGenerateOptionResolvedReceiverMethodCall()` at about `15` lines.
    - `tryGenerateSmallVecResolvedReceiverMethodCall()` at about `16` lines.
    - `tryGenerateCollectionResolvedReceiverMethodCall()` at about `23` lines.
    - `tryGenerateArrayResolvedReceiverMethodCall()` at about `18` lines.
    - `tryGenerateResolvedReceiverFastPathMethodCall()` at about `21` lines.
    - `resolveStaticMethodClassNames()` at about `17` lines.
    - `isStaticMethodReceiverReference()` at about `3` lines.
    - `emitStaticHashMapWithCapacityCall()` at about `28` lines.
    - `tryGenerateStaticFactoryMethodCall()` at about `25` lines.
    - `resolveStaticMethodReturnType()` at about `11` lines.
    - `emitPreparedStaticMethodCall()` at about `20` lines.
    - `tryGenerateUppercaseStaticMethodCall()` at about `26` lines.
    - `emitFallbackStaticMethodCall()` at about `10` lines.
    - `tryGenerateStaticMethodCall()` at about `22` lines.
    - `shouldLowerUnresolvedMethodAsCall()` at about `10` lines.
    - `tryGenerateStandaloneParserFallbackMethodCall()` at about `20` lines.
    - `tryGenerateUnresolvedReceiverFallbackMethod()` at about `38` lines.
    - `applyUnresolvedReceiverDefault()` at about `12` lines.
    - `tryHandleUnresolvedMethodReceiver()` at about `23` lines.
  - Call-dispatch helpers inside `llvm_ir_gen.seen` are now split into:
    - `applyComptimeParamSpecialization()` at about `34` lines.
    - `tryGenerateMetaBuiltinCall()` at about `131` lines.
    - `tryGenerateLowLevelBuiltinCall()` at about `160` lines.
    - `resolveArrayConstructorElementType()` at about `7` lines.
    - `tryGenerateArrayConstructorCall()` at about `36` lines.
    - `tryGenerateSmallVecConstructorCall()` at about `11` lines.
    - `tryGenerateCollectionConstructorCall()` at about `15` lines.
    - `isReprCConstructorType()` at about `3` lines.
    - `tryGenerateReprCClassConstructorCall()` at about `31` lines.
    - `allocateClassConstructorStorage()` at about `19` lines.
    - `emitClassConstructorArrayFieldInitializers()` at about `56` lines.
    - `emitClassConstructorArgumentStores()` at about `44` lines.
    - `tryGenerateHeapClassConstructorCall()` at about `13` lines.
    - `tryGenerateClassConstructorCall()` at about `13` lines.
    - `tryGenerateConstructorLikeCall()` at about `19` lines.
    - `tryGenerateOptionRuntimeBuiltinCall()` at about `18` lines.
    - `tryGenerateSuperRuntimeBuiltinCall()` at about `16` lines.
    - `tryGenerateEmptyRuntimeBuiltinCall()` at about `12` lines.
    - `tryGeneratePrintRuntimeBuiltinCall()` at about `16` lines.
    - `tryGenerateIoRuntimeBuiltinCall()` at about `31` lines.
    - `tryGeneratePanicRuntimeBuiltinCall()` at about `16` lines.
    - `tryGenerateRuntimeBuiltinCall()` at about `29` lines.
    - `tryGenerateImplicitThisCall()` at about `62` lines.
    - `tryGenerateMathBuiltinCall()` at about `25` lines.
  - Expression-dispatch helpers inside `llvm_ir_gen.seen` are now split into:
    - `tryGenerateConstructionExpression()` at about `11` lines.
    - `tryGenerateConditionalValueExpression()` at about `17` lines.
    - `tryGenerateSpecialExpression()` at about `20` lines.
  - Type-inference helpers inside `llvm_ir_gen.seen` are now split into:
    - `inferVariableExprTypeLocal()` at about `24` lines.
    - `inferBinaryExprTypeLocal()` at about `66` lines.
    - `resolveMethodCallReceiverType()` at about `7` lines.
    - `tryInferImplicitOrFreeMethodCallType()` at about `25` lines.
    - `tryInferStaticMethodCallType()` at about `38` lines.
    - `tryInferBuiltinReceiverMethodCallType()` at about `43` lines.
    - `tryInferReceiverRegistryMethodCallType()` at about `15` lines.
    - `tryInferMethodCallBoolSuffix()` at about `15` lines.
    - `inferMethodCallExprTypeLocal()` at about `45` lines.
    - `inferCallExprTypeLocal()` at about `57` lines.
    - `inferMemberAccessExprTypeLocal()` at about `56` lines.

### Handoff Snapshot

- Latest committed refactor commit available from a clean checkout is `ef24b17` with message `Extract final instance call helpers`.
- The continuation from `596169c` to `ef24b17` touched:
  - `compiler_seen/src/codegen/ir_method_finalize.seen`
  - `compiler_seen/src/codegen/llvm_ir_gen.seen`
  - `refactor.md`
- That continuation keeps expanding the shared method-call helper surface without adding another bootstrap compiler module:
  - shared identifier quoting, receiver-type helpers, receiver-pointer normalization, prepared dyn-trait dispatch, builtin receiver emission, static class-literal dispatch, and unresolved-receiver fallback helpers now live in `ir_method_receiver.seen`.
  - shared final instance-call helpers for return-type fallback, receiver ABI preparation, and `Option.unwrap()` specialization now also route through `ir_method_finalize.seen`.
  - `llvm_ir_gen.seen` keeps the AST-facing orchestration wrappers, but shared helper logic for `inferTypeRegistryFieldType()`, `getSemanticFieldType()`, `resolveMethodFieldPathType()`, `tryResolveEnumLiteralMethodReceiver()`, `normalizeExplicitMethodReceiverType()`, module-constant receiver type fallback, pointer normalization, prepared dyn-trait emission, builtin receiver emission, static-method receiver-name parsing, static factory dispatch, static return-type fallback, unresolved-call lowering checks, unresolved fallback emission, and unresolved default-receiver selection now routes through the extracted helper module.
  - `main_compiler.seen` bootstrap registration still only needs `codegen.ir_method_receiver`, which keeps the helper extraction inside the existing compiler-module count.
  - follow-up bootstrap-hardening fixes in the same working tree now also:
    - route `getCurrentFunctionOptions()` through shared `CodegenState` instead of a local `FunctionLoweringOptions` cache.
    - store `g_sharedCodegenState` as an `Int` handle in `type_registry.seen` so the frozen bootstrap compiler does not lower it as a `ptr` global.
    - mark `runtimeDeclsLoaded = true` immediately after `emitCoreRuntimeDecls(...)` so `emitTypecheckerDecls(...)` does not emit a second conflicting `Result_*` declaration block.
    - avoid direct indexed nested imported-type accesses in the most fragile spots by first binding the indexed node to a local (`computeJsonObjectSize()`, `generateJsonDeserializeImpl()` init field loop, `collectDefaultParamStringConstantsFromFunction()`, `resolveMethodReturnTypeFromList()`).
    - add `lastIndexOf(...)` to the runtime shim and runtime-backed string-compat list so bootstrap builds no longer die at final link when `llvm_ir_gen.seen` lowers `ciLine.lastIndexOf(\"'\")`.
- Last known validation for the latest continuation used a `MemTotal / 4` cap of `16229809` KB:
  - passed on the latest continuation: `./bootstrap/stage1_frozen check compiler_seen/src/main_compiler.seen`.
  - passed on the latest continuation: `./bootstrap/stage1_frozen check compiler_seen/src/codegen/ir_method_finalize.seen`.
  - passed: `./bootstrap/stage1_frozen check compiler_seen/src/test_ir_method_receiver_import.seen` while the temporary harness existed, which confirmed the extracted helper module resolved cleanly under the frozen bootstrap compiler.
  - passed after the follow-up bootstrap-hardening fixes: repeated `./bootstrap/stage1_frozen check compiler_seen/src/main_compiler.seen`.
  - reached and cleared `Optimization stats (module 5)` for `compiler_seen/src/codegen/llvm_ir_gen.seen`, then cleared the next frozen-bootstrap blockers in `type_registry.seen` (module 37), `ir_decl_features.seen` (module 11), `parser/real_parser.seen` (module 6), and the final link step (`lastIndexOf`) during bounded `./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen ... --fast --no-cache --no-fork` runs.
  - the `store %SeenString 0, ptr %4` failure previously seen at `/tmp/seen_module_5.ll:84130` inside `LLVMIRGenerator_collectDefaultParamStringConstantsFromFunction` was fixed by rewriting that indexed imported-type access to bind the parameter node first.
  - after that rewrite, a fresh bounded stage1 compile again got through `Optimization stats (module 5)` and into Pass 2b, and a direct bounded `opt -O3 /tmp/seen_module_5.ll` repro also succeeded, which confirms the old module-5 `String`-collapse failure is gone.
  - after the runtime `lastIndexOf(...)` fix, a bounded `./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen /tmp/seen_refactor_continue_stage1_rerun --fast --no-cache --no-fork` completed successfully under the same cap.
  - after folding the static/unresolved dispatch extraction into `ir_method_receiver.seen` instead of adding a new compiler module, a second bounded `./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen /tmp/seen_refactor_receiver_dispatch_merge --fast --no-cache --no-fork` also completed successfully under the same cap.
  - current validation frontier after re-running both the `596169c` baseline and the new `ir_method_finalize.seen` extraction is again a bounded Pass 2b failure with the same signatures in both runs:
    - duplicate `Result_isOkay` declarations across multiple `/tmp/seen_module_*.ll` files (for example `/tmp/seen_module_7.ll`, `/tmp/seen_module_11.ll`, `/tmp/seen_module_19.ll`).
    - malformed integer-style SDL constants lowered as floating-point globals in `/tmp/seen_module_9.ll` and `/tmp/seen_module_33.ll` (`floating point constant invalid for type`).
  - because the same frontier reproduced while re-checking the pre-change `596169c` checkpoint, the current method-finalize extraction is not yet isolated as the cause of that bounded full-compile failure.
- If resuming in the same area, the cleanest next slices are:
  - keep moving the remaining receiver-preparation wrappers out of `llvm_ir_gen.seen` into `ir_method_receiver.seen`, especially the implicit-`this` field receiver path and the rebuilt chained-literal receiver path.
  - keep pushing final instance-call glue from `llvm_ir_gen.seen` into `ir_method_finalize.seen`, especially the traced-unwrap receiver-type cleanup and the hot-reload receiver preparation path.
  - collapse the still-duplicated call-argument preparation/fill-default plumbing shared by free-function calls, parser-workaround calls, static calls, and receiver method calls, but do it in smaller slices than the reverted helper-plumbing experiment.
  - keep moving the remaining array-mutator and user/static-call emission helpers out of `llvm_ir_gen.seen` now that the static/unresolved dispatch logic already routes through `ir_method_receiver.seen`.
  - Shared loop analysis is now routed through `ir_control_flow.seen` for:
    - memcpy/memmove pattern detection.
    - literal loop-bound extraction and tile-size computation.
    - nested-loop, reduction, induction-variable, break-on-flag, and GCD-pattern detection.
  - Shared indexed-loop scaffolding is now routed through `ir_stmt_gen.seen` for:
    - range-based `for-in` loop headers.
    - array/string indexed `for-in` loop headers.
  - Shared conditional-branch scaffolding is now routed through `ir_stmt_gen.seen` for:
    - condition normalization to `i1`.
    - branch label allocation and `br i1` emission for `if` / `if let`.
  - `if`-statement helpers inside `llvm_ir_gen.seen` are now split into:
    - `tryEmitConstantIfStatement()` at about `21` lines.
    - `tryEmitPairedAssignmentSelectIfStatement()` at about `69` lines.
    - `tryEmitSingleArmSelectIfStatement()` at about `59` lines.
    - `tryEmitSelectOptimizedIfStatement()` at about `6` lines.
    - `inferIfConditionType()` at about `20` lines.
    - `computeIfBranchWeightMetadata()` at about `9` lines.
    - `finalizeConditionalEndBlock()` at about `10` lines.
  - Declaration/initializer helpers inside `llvm_ir_gen.seen` are now split into:
    - `declarationTypeNeedsFallback()` at about `3` lines.
    - `resolveDeclarationBaseType()` at about `7` lines.
    - `resolveLetStatementType()` at about `10` lines.
    - `resolveLetStatementAlloca()` at about `10` lines.
    - `shouldSkipDeadLiteralInitializer()` at about `16` lines.
    - `emitLetStatementInitializer()` at about `15` lines.
  - `if let` binding inside `llvm_ir_gen.seen` is now split into:
    - `emitIfLetBinding()` at about `7` lines.
  - Return-statement helpers inside `llvm_ir_gen.seen` are now split into:
    - `emitPendingReturnDefers()` at about `10` lines.
    - `prepareErrdeferReturnStatement()` at about `33` lines.
    - `emitReturnRegionAndProfileCleanup()` at about `11` lines.
    - `tryEmitAsyncReturnStatement()` at about `21` lines.
    - `emitDefaultEmptyReturnStatement()` at about `10` lines.
    - `tryEmitCharLiteralReturnFastPath()` at about `14` lines.
    - `resolveReturnStatementValueReg()` at about `15` lines.
    - `normalizeReturnStatementValueReg()` at about `26` lines.
  - Shared assignment lowering is now routed through `ir_assignment_gen.seen` for:
    - expression-result receiver pointer preparation.
    - local-variable receiver pointer preparation.
    - assignment field-type lookup.
    - union field stores and bitfield writeback.
    - final field stores.
    - indexed-assignment bounds-check emission.
    - primitive array element inline stores.
    - generic boxed `Array_set(...)` stores.
  - The remaining while-loop helpers in `llvm_ir_gen.seen` are now just the IR-emission layer:
    - `emitLiteralBoundWhileHints()`
    - `emitWhileLoopInvariantAnnotations()`
    - `emitGcdPatternWhileHint()`
    - `emitWhileConditionBranch()`
    - `tryEmitMemcpyOptimizedWhileLoop()`
  - Loop-lowering helpers inside `llvm_ir_gen.seen` are now split into:
    - `enterLoopContext()` at about `8` lines.
    - `restoreLoopContext()` at about `5` lines.
    - `emitLoopEndBlock()` at about `6` lines.
    - `emitWhileLoopAnalyses()` at about `33` lines.
    - `emitWhileLoopBody()` at about `19` lines.
    - `resolveForInLoopVariableStorage()` at about `23` lines.
    - `resolveForInIndexAlloca()` at about `13` lines.
    - `emitRangeForInLoop()` at about `18` lines.
    - `tryEmitIteratorProtocolForInLoop()` at about `54` lines.
    - `emitStringForInElement()` at about `10` lines.
    - `emitArrayLikeForInLoop()` at about `55` lines.
    - `finalizeForInLoop()` at about `15` lines.
  - Member-assignment helpers inside `llvm_ir_gen.seen` are now split into:
    - `emitResolvedMemberAssignment()` at about `47` lines.
    - `tryGenerateExpressionReceiverMemberAssignment()` at about `13` lines.
    - `tryGenerateSimpleReceiverMemberAssignment()` at about `15` lines.
    - `tryGenerateImplicitThisMemberAssignment()` at about `29` lines.
  - Array-literal helpers inside `llvm_ir_gen.seen` are now split into:
    - `resolveArrayLiteralElementSeenType()` at about `8` lines.
    - `resolveArrayLiteralElementType()` at about `12` lines.
    - `emitArrayLiteralAllocation()` at about `12` lines.
    - `generateArrayLiteralElementValue()` at about `10` lines.
    - `emitArrayLiteralElementPush()` at about `28` lines.
  - Construction-expression helpers inside `llvm_ir_gen.seen` are now split into:
    - `inferStructLiteralFieldTypeFromValueExpr()` at about `13` lines.
    - `buildInferredStructLiteralLayoutInfo()` at about `24` lines.
    - `resolveStructLiteralLayoutInfo()` at about `24` lines.
    - `allocateStructLiteralStorage()` at about `10` lines.
    - `resolveStructLiteralFieldValue()` at about `15` lines.
    - `resolveStructLiteralFieldInfo()` at about `32` lines.
    - `normalizeSeenStringValueReg()` at about `5` lines.
    - `emitSeenStringConcat()` at about `5` lines.
    - `convertInterpolatedExprToString()` at about `27` lines.
    - `allocateEnumConstructorObject()` at about `8` lines.
    - `emitEnumConstructorFieldStores()` at about `12` lines.
  - Special-expression helpers inside `llvm_ir_gen.seen` are now split into:
    - `emitCastSpecialExpression()` at about `5` lines.
    - `emitIsExpressionTest()` at about `16` lines.
    - `emitTryPropagateExpression()` at about `25` lines.
    - `resolveAwaitExpressionResultLlvmType()` at about `8` lines.
    - `emitAwaitPollLoop()` at about `17` lines.
    - `emitAwaitResultValue()` at about `13` lines.
    - `tryEmitOverloadedUnary()` at about `27` lines.
    - `emitUnaryNegation()` at about `11` lines.
    - `tryEmitNegatedComparison()` at about `26` lines.
    - `emitUnaryLogicalNot()` at about `16` lines.
    - `emitUnaryAddressOf()` at about `24` lines.
    - `emitUnaryDereference()` at about `11` lines.
    - `isUnaryBitwiseNotOperator()` at about `5` lines.
    - `emitUnaryBitwiseNot()` at about `5` lines.
  - Conditional-expression helpers inside `llvm_ir_gen.seen` are now split into:
    - `emitIfExpressionSideEffects()` at about `11` lines.
    - `emitIfExpressionBranchValue()` at about `7` lines.
    - `emitPhiMergeValue()` at about `5` lines.
    - `emitNullishCheckComparison()` at about `18` lines.
    - `defaultValueForLlvmType()` at about `9` lines.
    - `buildSafeNavigationMemberExpr()` at about `6` lines.
    - `emitWhenLiteralPatternBranch()` at about `23` lines.
    - `emitWhenIsPatternBranch()` at about `15` lines.
    - `emitWhenPatternBranch()` at about `15` lines.
    - `emitWhenArmGuard()` at about `16` lines.
    - `emitWhenPatternBindings()` at about `34` lines.
    - `normalizeWhenArmStoreValue()` at about `8` lines.
    - `emitWhenArmResult()` at about `28` lines.
  - Shared index-assignment helpers now live in `ir_assignment_gen.seen`:
    - `emitIndexAssignmentBoundsCheckImpl()` at about `15` lines.
    - `emitPrimitiveIndexAssignmentStoreImpl()` at about `25` lines.
    - `emitGenericIndexAssignmentStoreImpl()` at about `35` lines.
  - Variable-resolution helpers inside `llvm_ir_gen.seen` are now split into:
    - `generateAssignmentValuePreservingPendingType()` at about `7` lines.
    - `generateAssignmentValueForSeenType()` at about `12` lines.
    - `tryGenerateModuleConstantAssignment()` at about `11` lines.
    - `tryGenerateImplicitThisFieldAssignment()` at about `25` lines.
    - `tryGenerateFallbackAssignmentTarget()` at about `7` lines.
    - `emitDistinctAssignmentTypeWarning()` at about `10` lines.
    - `emitLocalVariableAssignment()` at about `7` lines.
    - `applyMoveAssignmentSemantics()` at about `28` lines.
    - `emitTriviallyCopyableAssignmentNote()` at about `5` lines.
    - `tryEmitMovedVariableRead()` at about `8` lines.
    - `tryGenerateComptimeVariable()` at about `10` lines.
    - `tryGenerateImplicitThisVariable()` at about `30` lines.
    - `tryGenerateFallbackVariable()` at about `24` lines.
    - `mapSimpleLlvmTypeToSeenType()` at about `16` lines.
    - `tryInferModuleConstantVariableType()` at about `12` lines.
    - `tryInferImplicitThisVariableType()` at about `26` lines.
  - Shared variable load/store helpers now live in `ir_variable_gen.seen` for:
    - expression variable-name extraction.
    - use-after-move trap emission.
    - comptime constant load emission.
    - module-constant load emission.
    - function-pointer interop loads.
    - local load/store emission.
    - implicit-`this` field stores.
    - move-source nullification.
    - `@trivially_copyable` assignment notes.
  - Member/field-access helpers inside `llvm_ir_gen.seen` are now split into:
    - `shouldRebuildMemberAccessReceiver()` at about `10` lines.
    - `tryRebuildMemberAccessReceiver()` at about `16` lines.
    - `tryGenerateEnumMemberAccess()` at about `19` lines.
    - `tryGenerateSimdSwizzleMemberAccess()` at about `14` lines.
    - `tryGenerateExpressionReceiverMemberAccess()` at about `12` lines.
    - `tryGenerateVariableReceiverMemberAccess()` at about `15` lines.
    - `tryGenerateModuleConstReceiverMemberAccess()` at about `26` lines.
    - `tryGenerateBracketPathMemberAccess()` at about `47` lines.
    - `walkMemberAccessChainPointer()` at about `26` lines.
    - `tryGenerateChainedReceiverMemberAccess()` at about `46` lines.
    - `tryGenerateImplicitThisReceiverMemberAccess()` at about `34` lines.
    - `tryGenerateReprCFieldAccess()` at about `15` lines.
    - `tryGenerateSpecialFieldAccessChain()` at about `18` lines.
    - `tryGenerateChainedFieldAccess()` at about `27` lines.
    - `tryGenerateUnionFieldAccess()` at about `8` lines.
    - `resolveFieldAccessLayoutInfo()` at about `13` lines.
    - `emitResolvedFieldAccessLoad()` at about `28` lines.
    - `resolveFieldAccessPtrLayoutInfo()` at about `3` lines.
    - `tryInferJsonValueMemberAccessType()` at about `23` lines.
    - `tryInferMemberFieldType()` at about `11` lines.
    - `tryInferLiteralReceiverMemberAccessType()` at about `29` lines.
  - Shared member-access helpers now live in `ir_member_access.seen` for:
    - SIMD swizzle result-type inference.
    - extracting the tail field from nested member names.
    - indexed array element pointer emission.
    - shared field GEP emission.
    - shared field load emission.
    - shared bitfield read masking.

### Implemented Slices

1. Phase 1 state extraction
- Introduced explicit `FunctionLoweringOptions` and shared `CodegenState`.
- Wired `syncState()` / `writeBackState()` so extracted helpers can use shared lowering state.
- Replaced the main loop-metadata `indent` encoding path with explicit function-lowering state.
- Moved `generateLiteral()` onto the shared state-backed lowering path.

2. Phase 2 orchestration dedup
- Extracted shared module emission logic into `ir_module_emit.seen`.
- `generateMultiple()` and `generateSingle()` now share helpers for global emission, cross-module constant declares, trait vtable constants, and `@llvm.global_ctors`.
- Extracted shared call-argument preparation into `ir_call_dispatch.seen` / `prepareCallArguments(...)`.
- Unified repeated argument adaptation for free calls, implicit `this` calls, static calls, parser workaround calls, and receiver method calls.

3. Phase 3 declaration and registry work
- Extracted cross-module declare recording, enum variant registration, and cross-module constant registration into `ir_decl_scan.seen`.
- Split `registerDeclarations()` into smaller helpers for class pre-registration, class declaration items, data declaration items, and function declaration items.
- Moved shared declare-string / declare-param builders and declaration predicates into `ir_decl_scan.seen`.
- Extracted the async function name/return-type registry into `ir_async_registry.seen`, keeping `llvm_ir_gen.seen` as a thin wrapper around the registry state.
- Moved late-discovered user declare lookup, declaration-string building, registry append, duplicate filtering, and emit helpers into `ir_decl_scan.seen`.
- Extracted dyn-trait name registration plus explicit and auto-detected trait-impl registry append logic into `ir_trait_registry.seen`.

4. Phase 4 function pipeline
- Extracted shared function signature emission, default-return emission, async coroutine preamble/epilogue emission, intrinsic-wrapper emission, and unused-parameter scanning into `ir_function_gen.seen`.
- Reduced `generateFunction()` by moving setup and epilogue details behind small focused helpers while preserving the `LLVMIRGenerator` facade.
- Split `generateFunction()` further into focused entry/setup helpers for cfg guards, decorator metadata comments, intrinsic and GPU-special handling, extern dispatch, implementation-name resolution, transient-state reset, parameter pre-registration, `main` emission, and parameter alloca materialization.

5. Phase 5 call pipeline kickoff
- `generateCall()` now uses the shared `emitUserFunctionCallImpl(...)` path for final free-function emission instead of hand-rolling void/non-void argument loops inline.
- Implicit `this` calls and the RealParser misplaced-method bootstrap workaround now share `emitUserMethodCallImpl(...)` for final call emission, including tail-position handling for non-void returns.
- Extracted the RealParser bootstrap workaround table into `ir_call_fixups.seen` so parser-specific call fixups no longer live inline inside `generateCall()`.
- Extracted the final instance-method-call normalization path into `ir_method_finalize.seen`, including receiver type cleanup, hot-reload synthesized method interception, receiver ABI adaptation, and `Option<T>.unwrap()` inner-type parsing.
- `generateMethodCall()` now delegates its final user-method lowering path to those helpers instead of carrying that ABI/detail logic inline.
- Split `generateMethodCall()` further into class-local sub-pipeline helpers for resolved-receiver fast paths, static method lowering, and unresolved-receiver fallback handling.
- Moved the standalone parser workaround classification/return-type table behind shared helpers in `ir_call_fixups.seen` so the method dispatcher no longer owns those lists directly.
- Extracted array mutator lowering (`free`, `push`, `pop`, `swap`) behind `tryGenerateArrayMutatorMethodCall(...)` so those structural mutations no longer live inline in `generateMethodCall()`.
- Split receiver preparation out of `generateMethodCall()` into focused helpers for rebuilt chained paths, explicit receiver fast paths, chained literal fallback, and simple literal receiver lookup. This makes `generateMethodCall()` read as a dispatcher pipeline instead of a mixed resolver/emitter blob.
- Split `generateCall()` into focused helper phases for comptime specialization, meta builtins, low-level builtins, constructor-like calls, normalized runtime builtins, implicit `this` dispatch, and math builtins. This also removed the duplicated `print` / `println` formatting path behind a shared emitter helper.
- Split `inferExpressionType()` into focused helpers for variable lookup, binary operator inference, method-call inference, free-call inference, and member-access inference. That turns the main inference entrypoint into a compact dispatcher and mirrors the same pipeline shape now used by `generateCall()` and `generateMethodCall()`.
- Removed a dead duplicate `StructLiteral` branch from `inferExpressionType()` after the split so the fallback path stays unambiguous.
- Split `generateWhileStatement()` into focused helpers for literal-bound loop hints, LICM/nested-loop annotations, GCD-pattern unroll hints, reduction detection, induction-variable detection, break-flag early-exit lowering, and memcpy/memmove fast-path detection. This turns the while emitter into a compact control-flow orchestrator instead of a mixed optimizer/emitter monolith.
- Removed the duplicated loop-analysis implementation from `llvm_ir_gen.seen` and rewired the while-loop pipeline to reuse the shared analyzers already living in `ir_control_flow.seen`. This is the first slice that converts a class-local refactor into an actual cross-file architectural consolidation.
- Reused `ir_stmt_gen.seen` for shared indexed-loop header emission so the range and array/string branches inside `generateForInStatement()` no longer hand-roll their own cond/body/end scaffolding inline. This extends the same architectural consolidation pattern from `while` loops into `for-in` lowering.
- Reused `ir_stmt_gen.seen` for shared condition normalization and branch skeleton emission inside `generateIfStatement()` and `generateIfLetStatement()`. That moves more of the statement pipeline toward shared emitters instead of keeping each statement form responsible for its own low-level block plumbing.
- Extracted shared struct field layout/index resolution into `ir_field_layout.seen` so `generateFieldAccess()` and `generateFieldAccessPtr()` no longer carry duplicate hardcoded layout tables for registry-backed and known bootstrap structs.
- Extracted chained-path parsing, expression rebuilding, first-segment splitting, bracket-path splitting, and suffix-type walking into `ir_path_expr.seen`, then rewired `generateMemberAccess()`, `generateMethodCall()`, and `resolveChainedPathType()` to reuse that shared path logic.
- Extracted member-access receiver adaptation and SIMD swizzle emission into `ir_member_access.seen`, then rewired `generateMemberAccess()` to reuse shared helpers for expression receivers, local-variable receivers, module-constant receivers, and vector swizzles.
- Extracted binary-expression pointer arithmetic, SIMD arithmetic/comparison emission, operator-suffix mapping, and overloaded operator call emission into `ir_binary_expr.seen`, then rewired `generateBinary()` to use those shared emitters.
- Extracted boolean-result classification, scalar-to-bool coercion, and short-circuit phi emission into `ir_binary_expr.seen`, then rewired `generateShortCircuitAnd()` / `generateShortCircuitOr()` to use those shared helpers instead of open-coding the same `icmp`/`phi` scaffolding twice.
- Expanded `ir_class_method_gen.seen` from an Option-only special case into a real class-method helper module that now owns method-attribute synthesis, explicit receiver detection, shared parameter-signature emission, constructor allocation/Array-List field bootstrap, and constructor return emission. `generateClassMethodFromList()` is now a much thinner orchestrator around those shared helpers.
- Expanded `ir_assignment_gen.seen` from a single field-store helper into a broader assignment-lowering helper module that now owns receiver-pointer preparation, assignment field-type resolution, union stores, and shared bitfield writeback. `generateMemberAssignment()` is now a small dispatcher over focused helper phases instead of a mixed resolver/emitter blob.
- Extracted indexed-assignment bounds-check emission, primitive inline array stores, and generic boxed `Array_set(...)` stores into `ir_assignment_gen.seen`, then rewired `generateIndexAssignment()` to stay at the AST-dispatch layer instead of mixing expression generation with low-level array store IR plumbing.
- Split `generateReturnStatement()` into focused cleanup, errdefer preparation, async-return, empty-return, char-literal fast-path, return-value evaluation, and return-value normalization helpers. This turns the return path into a compact dispatcher over explicit phases instead of a single mixed control-flow block.
- Split `generateIfStatement()` into focused constant-fold, select-optimization, condition-type, branch-weight, and end-block helpers. This turns the main `if` emitter into a short orchestration layer while keeping the existing shared `ir_stmt_gen.seen` branching scaffold in place.
- Split declaration and array-literal lowering further by sharing declaration-type fallback between `collectStmtVars()` and `generateLetStatement()`, extracting the let-initializer / prealloc reuse path, extracting `if let` binding materialization, and breaking `generateArrayLiteral()` into focused element-type, allocation, nested-array, and push helpers. This keeps three more statement/expression entrypoints at the orchestration layer instead of leaving them as mixed resolver/emitter blobs.
- Split the conditional-expression cluster further by grouping conditional/special-form dispatch inside `generateExpression()`, extracting shared phi/nullish helpers, shrinking `generateIfExpression()`/`generateElvis()`/`generateSafeNavigation()`, and breaking `generateWhenExpression()` into focused pattern-match, guard, binding, and arm-result helpers. This makes the expression path read more like a dispatcher plus explicit lowering phases instead of one long sequence of unrelated special cases.
- Split the construction-expression cluster further by grouping struct/array/string construction dispatch inside `generateExpression()`, shrinking `generateStructLiteral()` behind focused layout/allocation/field helpers, routing struct-literal fallback field-type inference through `ir_struct_gen.seen`, shrinking `generateStringInterpolation()` behind shared string-normalization/concat helpers, and splitting `generateEnumConstructor()` into allocation plus field-store helpers. This keeps data-construction lowering on the same dispatcher-plus-phases pattern now used by the other expression families.
- Split the special-expression cluster further by routing `Await`/`Unary` through the grouped special dispatcher, shrinking cast/type-test/`?` propagation behind dedicated helpers, turning `generateAwaitExpression()` into a thin orchestrator over result-type, poll-loop, and promise-extraction helpers, and breaking `generateUnary()` into focused operator-overload, negation, logical-not, pointer, and bitwise-not helpers. This keeps the last mixed “misc expression” path aligned with the same dispatcher-plus-phases structure as the rest of the expression pipeline.
- Split the variable-resolution cluster further by extracting low-level variable load/store helpers into `ir_variable_gen.seen`, shrinking `generateAssignmentExpr()`/`generateAssignment()` behind shared global/implicit-`this`/local assignment helpers, shrinking `generateVariable()` behind focused moved/comptime/fallback loaders, and splitting `inferVariableExprTypeLocal()` into shared module-constant and implicit-`this` type helpers. This keeps bare-name reads and writes on the same dispatcher-plus-phases pattern as the rest of the statement/expression pipeline.
- Split the member/field-access cluster further by expanding `ir_member_access.seen` with shared swizzle/type-tail/field-load helpers, shrinking `generateMemberAccess()` behind receiver-kind and path-shape helpers, shrinking `generateFieldAccess()` / `generateFieldAccessPtr()` behind shared layout/load helpers, and shrinking `inferMemberAccessExprTypeLocal()` behind focused JSON/receiver-type helpers. This keeps member-access lowering and member-access type inference aligned on the same receiver-shape split instead of maintaining duplicate chained-path and field-lookup logic.
- Split the method-call receiver/type-inference cluster further by extracting shared literal receiver type resolution for module constants, enum literals, rebuilt local chains, and implicit-`this` field chains; shrinking `resolveChainedLiteralMethodReceiver()` into enum-plus-implicit-`this` fallback helpers; and breaking `inferMethodCallExprTypeLocal()` into focused receiver-type, static-call, builtin-receiver, registry, and bool-suffix helpers. This keeps runtime receiver preparation and method-call type inference aligned on the same receiver-shape split instead of duplicating chained literal path logic in two places.
- Split the method-call emission cluster further by collapsing receiver preparation behind `tryPrepareMethodCallReceiver()`, extracting builtin receiver dispatch for length/string-builder/conversion/numeric/string-like methods, and extracting normalized instance-call emission for traced unwrap handling, hot-reload interception, receiver ABI normalization, and specialized `unwrap()` handling. This turns `generateMethodCall()` into a short orchestration layer instead of leaving builtin and user-defined instance dispatch interleaved in one block.
- Split the module-entry orchestration further by extracting shared reset, string-collection, class-type emission, class emission, top-level function emission, defined-symbol collection, extra string-constant flush, closure emission, and optimization-stat helpers. This shrinks `generateMultiple()` and `generateSingle()` into explicit phase pipelines instead of leaving module sequencing, special top-level handling, and post-generation cleanup interleaved in two duplicated entrypoints.
- Split the block/statement pipeline further by extracting dead-store scan helpers, deferred-cleanup emission, assignment-like expression dispatch, unused-result warnings, loop-control emission, scoped/defer/unsafe/try-catch helpers, and grouped statement-family dispatchers. This turns `generateBlock()` and `generateStatement()` into short orchestration layers instead of leaving block cleanup, dead-store heuristics, and every statement-kind branch mixed together.
- Split the type/class-emission cluster further by shrinking `emitClassType()` behind focused dedup, header-layout reuse, decorator-metadata, special-case gpu/union emission, associated-type-alias registration, and default-field layout helpers; shrinking `generateClass()` behind dedicated trait/type-alias/decorator/hot-reload/inherited-thunk helpers; and deduplicating the StringBuilder runtime-backed method skip list shared by normal and large-class emission. This keeps the class/type pipeline on the same dispatcher-plus-phases shape now used across statements, expressions, and calls.
- Split the class-method emission cluster further by shrinking trait default-method codegen behind `emitTraitDefaultMethod()`, shrinking `generateClassMethodFromList()` behind focused state-reset, parameter-info collection, variable-collection prep, signature emission, receiver binding, parameter binding, constructor setup, constructor-return, and default-return helpers, and keeping the existing shared `ir_class_method_gen.seen` helpers responsible only for reusable ABI/signature/allocation pieces. This keeps class-method lowering on the same explicit phase-pipeline shape now used by top-level function lowering, type emission, and statement lowering.
- Split the loop-lowering cluster further by extracting shared loop-context save/restore, while-loop analysis/body emission, for-in variable/index allocation, range lowering, iterator-protocol lowering, array/string element lowering, and shared end-block finalization helpers. This turns `generateWhileStatement()` and `generateForInStatement()` into short orchestration layers instead of mixed control-flow, storage, and iteration emitters.
- Split the constructor/runtime call cluster further by extracting dedicated helpers for array constructors, collection constructors, repr-C constructors, heap-backed class allocation, default array-field initialization, positional constructor stores, option constructors, `super`, empty-callee evaluation, print/println, file-IO builtins, and panic lowering. This turns `tryGenerateConstructorLikeCall()` and `tryGenerateRuntimeBuiltinCall()` into short orchestration layers instead of mixed constructor/runtime dispatch blobs.
- Split the method-call fallback cluster further by extracting focused helpers for resolved option/smallvec/collection/array fast paths, static receiver-name resolution, specialized static factories, shared prepared static-call emission, unresolved receiver call fallback, standalone parser fallback, unresolved primitive conversion fallback, and unresolved receiver defaulting. This turns the top of `generateMethodCall()` into explicit phases instead of interleaving fast paths, static dispatch, and fallback behavior in three large helpers.
- Split the receiver-preparation cluster further by extracting explicit dyn-trait, option, hot-reload, and collection receiver helpers; simple-variable dyn/SIMD receiver helpers; module-constant receiver loading/type fallback; and implicit-`this` field receiver helpers. This shrinks both `tryPrepareExplicitMethodReceiver()` and `tryResolveSimpleLiteralMethodReceiver()` into short orchestration layers while keeping `generateMethodCall()` itself at the phase-dispatch level.
- Moved the shared receiver-type utilities that no longer need direct AST/state mutation into `ir_method_receiver.seen`, then rewired `llvm_ir_gen.seen` to delegate identifier quoting, TypeRegistry field inference, semantic field lookup, method-field path resolution, enum literal receiver resolution, explicit receiver-type normalization, and module-constant fallback typing through that helper module. This is the first receiver-preparation slice that now lives in its own file instead of only being split into class-local helpers.
- Expanded `ir_method_receiver.seen` further so it now also owns receiver-pointer normalization, prepared dyn-trait vtable dispatch, explicit option/hot-reload/collection receiver emission, and builtin receiver emission for length-like, StringBuilder, primitive-conversion, numeric, and string-like methods. `llvm_ir_gen.seen` now stays at the “prepare args, choose phase” layer for that part of `generateMethodCall()` instead of interleaving helper selection with low-level IR string emission.

### Validation Status

- Spot checks continue to use explicit RAM caps derived from current system memory.
- `./compiler_seen/target/seen check examples/hello_world/hello_english.seen` still passes under a `MemTotal / 4` cap after the latest method-call receiver-preparation split on top of the earlier method-call fallback split, constructor/runtime call split, loop-lowering split, class-method emission split, type/class-emission split, block/statement pipeline split, module-entry orchestration split, method-call emission split, method-call receiver/type-inference split, member/field-access split, variable-resolution split, special-expression split, construction-expression split, conditional-expression split, declaration/`let`/`if let`/array-literal split, `if`-statement split, return-statement split, assignment-lowering extraction, and indexed-assignment extraction.
- `./compiler_seen/target/seen check compiler_seen/test_array_struct.seen` also passes under the same cap, which now gives the current refactor stream a targeted sanity check across struct-literal construction plus array access.
- `./compiler_seen/target/seen check compiler_seen/test_result.seen` also passes under the same cap, which gives this batch direct coverage for enum-constructor lowering via `Ok(42)`.
- `./compiler_seen/target/seen check compiler_seen/test_conditional_exprs.seen` also passes under the same cap, which gives this batch a targeted sanity check across `if`-expression lowering, integer elvis lowering, and `when` expression arm dispatch.
- `./compiler_seen/target/seen check compiler_seen/test_special_exprs.seen` also passes under the same cap, which gives this batch direct coverage for `Result<T, E>?` propagation plus unary negation and logical-not lowering.
- `./compiler_seen/target/seen check compiler_seen/test_variable_resolution.seen` also passes under the same cap, which gives this batch direct coverage for implicit field reads/writes, local shadowing, and array-field initialization inside class methods.
- `./compiler_seen/target/seen check compiler_seen/test_member_access_paths.seen` also passes under the same cap, which gives this batch direct coverage for local member chains plus indexed member access on local array aliases.
- `./compiler_seen/target/seen check compiler_seen/test_method_receiver_resolution.seen` also passes under the same cap, which gives this batch direct coverage for chained method receivers on implicit-`this` field paths and rebuilt local chained paths.
- `./compiler_seen/target/seen check compiler_seen/test_method_receiver_sources.seen` also passes under the same cap, which gives this batch direct coverage for implicit-`this` simple field receivers plus module-constant method receivers through `trim()` and `length()`.
- `./compiler_seen/target/seen check compiler_seen/test_method_builtin_dispatch.seen` also passes under the same cap, which gives this batch direct coverage for StringBuilder method dispatch plus builtin string/int/float method lowering across `trim()`, `toInt()`, `toFloat()`, `toString()`, and `length()`.
- `./compiler_seen/target/seen check compiler_seen/test_import_io.seen` also passes under the same cap, which gives this batch a direct import-path sanity check across the shared `generateSingle()` module-entry sequencing.
- `./compiler_seen/target/seen check compiler_seen/test_statement_forms.seen` also passes under the same cap, which gives this batch direct coverage for the refactored statement dispatcher across `while`/`continue`/`break`, `try/catch`, `unsafe`, and inline block handling.
- `./compiler_seen/target/seen check compiler_seen/test_loop_iteration_forms.seen` also passes under the same cap, which gives this batch direct coverage for range `for-in`, array `for-in`, string `for-in`, and `while` loop `continue`/`break` behavior after the loop-helper split.
- `./compiler_seen/target/seen check compiler_seen/test_iterator_for_in.seen` also passes under the same cap, which gives this batch direct coverage for the extracted iterator-protocol `for-in` lowering path across `iter()`, `hasNext()`, and `next()` dispatch.
- `./compiler_seen/target/seen check compiler_seen/test_constructor_call_paths.seen` also passes under the same cap, which gives this batch direct coverage for array constructor forms, direct `HashMap<String, Int>()` construction, zero-arg heap class construction with array-field initialization, repr-C construction, and positional plain-class constructor stores.
- `./compiler_seen/target/seen check compiler_seen/test_runtime_builtin_calls.seen` also passes under the same cap, which gives this batch direct coverage for `Some`, `None`, `args`, `writeText`, and `readText` after the runtime-builtin split.
- `./compiler_seen/target/seen check compiler_seen/test_static_method_call_paths.seen` also passes under the same cap, which gives this batch direct coverage for `Array<T>.withLength(...)`, `HashMap<String, Int>.withCapacity(...)`, `StringBuilder.new()`, and `String.fromCharCode(...)` after the static-method split.
- `./compiler_seen/target/seen check compiler_seen/test_type_emission_forms.seen` also passes under the same cap, which gives this batch direct coverage for `type`/`distinct`, `union`, `@trait`, `@repr(C)`, `@gpu_buffer`, and decorator-generated class emission in the same module.
- `./compiler_seen/target/seen check compiler_seen/test_class_method_pipeline.seen` also passes under the same cap, which gives this batch direct coverage for explicit-receiver instance methods, inherited method thunks on `extends`, static constructor emission, and trait default-method emission in the same module.
- `./compiler_seen/target/seen check tests/codegen/test_game_engine_features.seen` also passes under the same cap, which gives this batch a broader integration sanity check across existing array `for-each` and range-loop lowering paths.
- `./compiler_seen/target/seen check tests/codegen/test_static_class_return_regression.seen` also passes under the same cap, which gives this batch direct coverage for static methods that construct and return heap-backed class instances.
- `./compiler_seen/target/seen check tests/codegen/test_implicit_zero_receiver_methodcall_regression.seen` also passes under the same cap, which gives this batch direct coverage for unresolved zero-receiver method calls lowering back through ordinary call dispatch.
- `./compiler_seen/target/seen check tests/misc_root_tests/test_method_name.seen` also passes under the same cap, which gives this batch direct coverage that user-defined methods like `toInt()` still beat builtin-fallback behavior when the receiver is resolved.
- `./compiler_seen/target/seen check tests/ffi/test_repr_c.seen` also passes under the same cap, which gives this batch a direct repr-C constructor regression over positional field lowering.
- `./compiler_seen/target/seen check tests/e2e_multilang/en/test_stdlib_collections_en.seen` also passes under the same cap, which gives this batch broader collection-constructor and collection-method coverage through array and hash-map usage.
- `./compiler_seen/target/seen check tests/test_gpu_buffer.seen` also passes under the same cap, which revalidates the extracted gpu-buffer type-emission path against the existing dedicated layout test.
- `./compiler_seen/target/seen check tests/p2/test_trait_monomorph.seen` also passes under the same cap, which revalidates the extracted trait-path helper against an existing trait codegen test.
- `./compiler_seen/target/seen check tests/p2/test_derive_debug.seen` also passes under the same cap, which revalidates the extracted class-decorator scan and generated-method dispatch path against an existing derive test.
- `./compiler_seen/target/seen check tests/oop/test_inheritance_simple.seen` also passes under the same cap, which gives this batch a broader inheritance-path sanity check across constructors, inherited methods, and overrides.
- `./compiler_seen/target/seen check tests/codegen/test_trait_vtable.seen` also passes under the same cap, which gives this batch a broader trait codegen sanity check across explicit `this:` receivers, impl blocks, and dyn-trait dispatch.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_call_fixups.seen` reaches the expected `missing main` diagnostic, which at least confirms the new helper module parses cleanly.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_method_finalize.seen` also reaches the expected `missing main` diagnostic.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_field_layout.seen` also reaches the expected `missing main` diagnostic.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_path_expr.seen` also reaches the expected `missing main` diagnostic.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_member_access.seen` also reaches the expected `missing main` diagnostic.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_binary_expr.seen` also reaches the expected `missing main` diagnostic.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_class_method_gen.seen` also reaches the expected `missing main` diagnostic.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_assignment_gen.seen` also reaches the expected `missing main` diagnostic.
- `./compiler_seen/target/seen check compiler_seen/src/codegen/ir_variable_gen.seen` also reaches the expected `missing main` diagnostic.
- `./bootstrap/stage1_frozen check compiler_seen/src/test_ir_method_receiver_import.seen` passed under a `MemTotal / 4` cap before that temporary harness was removed, which confirms `codegen.ir_method_receiver` resolves cleanly under the frozen bootstrap compiler.
- `./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen /tmp/seen_refactor_check_stage1_b --fast --no-cache --no-fork` also reached `Optimization stats (module 5)` for `compiler_seen/src/codegen/llvm_ir_gen.seen` under the same cap after the larger receiver-emission extraction.
- That full stage1 bootstrap path is now known to under-register newer helper modules carried only in the source tree, so it is not yet a complete validator for post-freeze helper extractions; in this run the failure surfaced as `quoteIdentIfNeededImpl()` lowering with a fallback `i64` return type inside `/tmp/seen_module_5.ll`.
- Bounded direct checks of `compiler_seen/src/main_compiler.seen` and `compiler_seen/src/codegen/llvm_ir_gen.seen` still did not finish within `45s` under the same cap after the latest method-call receiver-preparation split on top of the earlier method-call fallback split, constructor/runtime call split, loop-lowering split, class-method emission split, type/class-emission split, block/statement pipeline split, module-entry orchestration split, method-call emission split, method-call receiver/type-inference split, member/field-access split, variable-resolution split, special-expression split, construction-expression split, conditional-expression split, declaration/`let`/`if let`/array-literal split, `if`-statement split, return-statement split, indexed-assignment extraction, assignment-lowering extraction, shared binary-expression extraction, short-circuit helper reuse, class-method helper extraction, shared member-access extraction, shared path-expression extraction, shared field-layout extraction, `generateFunction()` split, while-loop split, shared control-flow dedup, `for-in` scaffold reuse, and shared `if` branching reuse.
- The previously observed late optimization failure (`/usr/bin/opt: unknown pass name 'polly-canonicalize'`) remains relevant for deeper rebuild paths that get past the earlier allocator issue.

### Phase Status

- Phase 1: partially complete; state sync and explicit function-lowering options are in place.
- Phase 2: core module-emission, module-entrypoint helper reuse, and call-argument dedup completed.
- Phase 3: in progress; declaration scan, async registry extraction, late user declare registry extraction, and trait registry extraction are started, but other registries still live in `llvm_ir_gen.seen`.
- Phase 4: well underway; function signature/default-return/coroutine helpers plus entry/setup, parameter pre-registration, `main` dispatch, and parameter alloca emission are split out, but body emission still largely lives in `llvm_ir_gen.seen`.
- Phase 5: well underway; final free-call emission, RealParser call fixups, final instance-method-call normalization, array mutator lowering, receiver-preparation helpers, method-call builtin/normalized-instance helpers, a full `generateCall()` phase split, and shared class-method lowering helpers are in place.
- Phase 6: in progress; `inferExpressionType()`, `inferVariableExprTypeLocal()`, `inferMethodCallExprTypeLocal()`, `inferMemberAccessExprTypeLocal()`, `generateExpression()`, `generateBlock()`, `generateStatement()`, `generateMethodCall()`, `generateAssignmentExpr()`, `generateAssignment()`, `generateVariable()`, `generateBinary()`, `generateWhileStatement()`, `generateForInStatement()`, `generateIfStatement()`, `generateIfLetStatement()`, `generateIfExpression()`, `generateLetStatement()`, `generateReturnStatement()`, `generateAwaitExpression()`, `generateUnary()`, `generateArrayLiteral()`, `generateStructLiteral()`, `generateStringInterpolation()`, `generateElvis()`, `generateSafeNavigation()`, `generateWhenExpression()`, `generateEnumConstructor()`, `generateMemberAccess()`, `generateFieldAccess()`, `generateFieldAccessPtr()`, `generateMemberAssignment()`, `generateIndexAssignment()`, `resolveChainedPathType()`, and the short-circuit boolean path now rely on focused helper phases or shared helper modules, and the loop/statement/expression pipeline reuses `ir_control_flow.seen`, `ir_stmt_gen.seen`, `ir_assignment_gen.seen`, `ir_variable_gen.seen`, `ir_field_layout.seen`, `ir_path_expr.seen`, `ir_member_access.seen`, `ir_binary_expr.seen`, `ir_class_method_gen.seen`, and `ir_struct_gen.seen`, while method-call receiver preparation, method-call emission, and statement dispatch now also rely on focused helper phases, but more statement/expression helpers still need to leave `llvm_ir_gen.seen`.
- Phase 7: not started yet.

## Baseline Snapshot

### 1. `llvm_ir_gen.seen` is still the monolith

- Plan baseline size: `16,086` lines.
- There are `196` module-level globals before the class even starts (`compiler_seen/src/codegen/llvm_ir_gen.seen:81-498`).
- The class still owns orchestration, type/registry lookups, declaration scanning, module emission, class emission, function lowering, statement lowering, expression lowering, type inference, and feature/decorator state.

Largest methods:

| Method | Lines |
|---|---:|
| `generateMethodCall` | 1501 |
| `generateCall` | 1153 |
| `generateFunction` | 1000 |
| `inferExpressionType` | 770 |
| `generateWhileStatement` | 543 |
| `generateSingle` | 495 |
| `generateMemberAccess` | 442 |
| `generateBinary` | 415 |
| `generateMultiple` | 387 |
| `generateClassMethodFromList` | 360 |
| `generateMemberAssignment` | 352 |
| `emitClassType` | 344 |
| `generateFieldAccess` | 334 |
| `generateClass` | 323 |

That is the main SRP failure: a single file is still acting as planner, registry, state store, and lowering engine.

### 2. The repo already contains the beginnings of the right architecture

There are already extracted helper modules such as:

- `type_registry.seen`
- `ir_call_dispatch.seen`
- `ir_struct_gen.seen`
- `ir_assignment_gen.seen`
- `ir_stmt_gen.seen`
- `ir_function_gen.seen`

Two especially important signs:

- `type_registry.seen` already defines `CodegenState` and explicitly calls it the foundation for moving expression lowering out of the monolith.
- `generateLiteral()` in `llvm_ir_gen.seen` is already a thin wrapper around `generateLiteralFree()`.

But the migration is incomplete:

- At the plan baseline, `LLVMIRGenerator.syncState()` was a no-op (`compiler_seen/src/codegen/llvm_ir_gen.seen:1112-1114`).
- `CodegenState` is mostly unused outside literal lowering.
- Several extracted modules still leave the monolith in charge of almost all orchestration and most branching logic.

So the right direction is not a redesign from scratch. It is to finish the architecture that has already started.

### 3. DRY problems are real, not just cosmetic

The file repeats the same patterns in multiple places:

- Module emission logic is duplicated between `generateMultiple()` and `generateSingle()`:
  - global variable emission (`3298-3369` and `3704-3774`)
  - class-first ordering explanation (`3388-3393` and `3869-3874`)
  - vtable emission (`3523-3568` and `3920-3970`)
  - `@llvm.global_ctors` emission (`3597-3613` and `4103-4119`)
- Call argument preparation is duplicated across free-function calls, implicit method calls, static method calls, and receiver method calls.
- Data-type boxing is duplicated in multiple call paths (`12330`, `12460`, `12559`, `13256`, `13338`, `13956`, `14094`).
- Int-to-Float promotion is duplicated in multiple call paths (`12469`, `12643`, `13965`, `14103`).
- Decorator parsing is scattered across `emitClassType()`, `generateClass()`, and `generateFunction()`.

`generateCall()` and `generateMethodCall()` are also heavily branch-driven:

- about `32` `if funcName == ...` checks
- about `35` `if methodName == ...` checks

That is a sign the code wants table-driven or domain-driven dispatch modules instead of ever-growing chains.

### 4. State boundaries are blurred

Right now several kinds of state are mixed together:

- process-wide configuration
- per-compilation registries
- per-module registries
- per-function lowering state
- per-block control-flow state
- temporary bootstrap workarounds

One especially fragile example is the `indent` field being used as a bit-packed transport for unrelated function metadata (`compiler_seen/src/codegen/llvm_ir_gen.seen:6295-6421`, `7343-7421`, `11924-11944`).

That is not just ugly. It makes extraction harder because helpers must know hidden encoding rules.

### 5. Bootstrap constraints are real and must shape the refactor

The refactor cannot assume a normal compiler implementation environment.

Important constraints I found:

- comments in `llvm_ir_gen.seen` mention bootstrap-sensitive globals, frozen-compiler layout issues, and `Map.get()` limitations
- `main_compiler.seen` has a curated bootstrap module list that must be updated whenever codegen modules move (`compiler_seen/src/main_compiler.seen:1819-1841`)
- `scripts/safe_rebuild.sh` documents a current split-brain situation where the frozen compiler handles `llvm_ir_gen.seen` well but satellite codegen modules poorly, while the production compiler shows the opposite tendency in some recovery scenarios (`scripts/safe_rebuild.sh:923-954`)

So the refactor plan must be incremental, additive, and bootstrap-safe.

### 6. The file-size problem is broader than one file

Files over 500 lines in `compiler_seen/src/codegen/` currently include:

- `llvm_ir_gen.seen` — 16086
- `glsl_gen.seen` — 1616
- `ir_decl_runtime.seen` — 1575
- `ir_type_info.seen` — 1427
- `ir_type_ops.seen` — 1360
- `generator.seen` — 1324
- `ir_type_tables.seen` — 1307
- `ir_type_mapping.seen` — 1200
- `wasm_gen.seen` — 1140
- `ir_optimization.seen` — 1048
- `type_registry.seen` — 1001
- `c_gen.seen` — 980
- `ir_decl_features.seen` — 959
- `ir_string_collect.seen` — 884
- `ir_call_dispatch.seen` — 827
- `ir_type_dispatch.seen` — 806
- `ir_decl_parser.seen` — 704
- `vectorization.seen` — 580
- `llvm_backend.seen` — 578
- `ir_method_gen.seen` — 521

So the right outcome is not only “shrink one file”. It is to put the LLVM codegen stack on a structure that can keep shrinking safely.

## Refactor Principles

### SRP rules

- One file should own one stage or one cohesive domain.
- One state object should represent one scope: compile, module, function, or block.
- Registry code should not also emit IR.
- Dispatch code should not also own storage details.
- Decorator parsing should produce explicit options, not hidden bit-packing.

### DRY rules

- One implementation for module-global emission, used by both `generateMultiple()` and `generateSingle()`.
- One implementation for call argument lowering, reused by function, static-method, and instance-method calls.
- One implementation for default return emission.
- One implementation for decorator parsing per scope.
- One implementation for cross-module declaration recording/emission.

### File-size rule

- Target: every non-generated source file under `500` lines.
- If a file must exceed `500`, document why and keep it on an allowlist.
- Prefer splitting by domain, not by arbitrary “part1/part2” naming.

## Recommended Target Architecture

Create a dedicated package namespace for the LLVM IR generator instead of keeping everything flat under `codegen/`.

Suggested layout:

```text
compiler_seen/src/codegen/llvm_ir_gen.seen                # thin facade only
compiler_seen/src/codegen/llvm_ir/
  mod.seen
  facade.seen                                             # orchestration entrypoints
  state/
    compile_state.seen
    module_state.seen
    function_state.seen
    feature_state.seen
  passes/
    declaration_pass.seen
    module_emit_pass.seen
    class_emit_pass.seen
    function_emit_pass.seen
  expr/
    dispatch.seen
    literals.seen
    calls.seen
    methods.seen
    access.seen
    ops.seen
    inference.seen
  stmt/
    dispatch.seen
    declarations.seen
    control_flow.seen
    loops.seen
    assignments.seen
  runtime/
    globals.seen
    strings.seen
    declarations.seen
    metadata.seen
  registry/
    type_registry.seen
    cross_module_registry.seen
    async_registry.seen
    trait_registry.seen
```

`llvm_ir_gen.seen` should become a compatibility facade that mainly:

- owns the public `LLVMIRGenerator` type
- forwards to the new package modules
- preserves bootstrap-facing entry points during the migration

Target size for the facade: under `300` lines.

## Concrete Architectural Changes

### 1. Finish the `CodegenState` migration

This is the highest-leverage structural change.

Do this first:

- make `syncState()` real
- define explicit writeback points for value fields
- pass `CodegenState` through extracted lowering helpers instead of raw globals

What should move behind state objects:

- output buffer
- register/block counters
- variable tables
- current function/class context
- loop labels
- pending array literal type
- generated string constants
- per-function feature flags

What should not remain as loose globals unless unavoidable:

- async lowering state
- profile state
- bounds-check labels
- cross-module declare accumulation
- decorator-derived options

### 2. Replace hidden encoded state with explicit structs

The `indent` field currently carries:

- unroll count
- vectorization disable flag
- alignment hint
- region allocation size
- async flag
- transient tail-call state

Replace that with a dedicated `FunctionLoweringOptions` object:

- `unrollCount`
- `vectorizeEnabled`
- `alignTo`
- `regionSizeBytes`
- `prefetchMode`
- `isAsync`
- `emitTailCall`

This will make helper extraction much easier and remove a major source of accidental coupling.

### 3. Split orchestration from emission

Right now `generateMultiple()` and `generateSingle()` both orchestrate and emit.

Refactor to:

- one shared module-planning layer
- one shared module-emission layer
- thin wrappers for single-module vs multi-module entrypoints

Shared helpers should cover:

- class-type scan
- global variable emission
- string constant emission
- defined-symbol collection
- cross-module declare emission
- vtable emission
- `@llvm.global_ctors` emission
- optimization stats emission

### 4. Separate declaration scanning from IR generation

`registerDeclarations()` is doing too much:

- class pre-registration
- layout registration
- method registration
- async registration
- float-param metadata
- enum registration
- cross-module declare generation
- cross-module constant registration

That should become a real declaration pass with focused submodules:

- type declaration scan
- callable declaration scan
- enum declaration scan
- cross-module symbol scan

This also makes it easier to test Pass 1 independently.

### 5. Unify call lowering

`generateCall()` and `generateMethodCall()` should stop doing their own argument adaptation.

Introduce shared helpers for:

- argument evaluation
- data-type boxing
- default-parameter filling
- Int-to-Float promotion
- static vs instance call target resolution
- late declare recording
- receiver coercion

Then split dispatch by domain:

- builtins/intrinsics
- collection constructors
- user free functions
- static methods
- instance methods
- async/coroutine helpers

This is the single biggest DRY opportunity in the file.

### 6. Mirror emitter structure in type inference

`inferExpressionType()` is still `770` lines, which means inference has drifted away from emitter structure.

Mirror the emitter split:

- `inferLiteralType`
- `inferCallType`
- `inferMethodCallType`
- `inferAccessType`
- `inferBinaryType`
- `inferUnaryType`
- `inferControlFlowExprType`

This keeps lowering and inference aligned and reduces bugs where one path learns a special case and the other does not.

### 7. Move special registries out of the main file

The following should be extracted from loose globals into focused modules:

- async registry and coroutine state
- trait/vtable registry
- cross-module declare registry
- module-constant registry
- optimization counters
- ML logging/replay state
- enum registry

Rule of thumb: if the state has its own lifecycle and reset semantics, it wants its own module.

## Recommended Migration Order

### Phase 0: Guardrails

- add a simple file-size check script with an allowlist for unavoidable exceptions
- capture current `safe_rebuild.sh` behavior and a small codegen smoke suite
- record current line counts and major-method sizes

### Phase 1: State extraction

- make `CodegenState` real for function/expression lowering
- introduce explicit module/function option structs
- move `indent` bit-packing to named fields

### Phase 2: Orchestration dedup

- extract shared helpers used by `generateMultiple()` and `generateSingle()`
- centralize global emission, vtable emission, and ctor emission
- keep the current public entrypoints as thin wrappers

### Phase 3: Declarations and registries

- split `registerDeclarations()` into pass-specific helpers
- move cross-module registries into dedicated modules
- move async/trait/enum registries out of `llvm_ir_gen.seen`

### Phase 4: Function pipeline

- break `generateFunction()` into:
  - analysis/setup
  - signature/attribute emission
  - parameter lowering
  - decorator lowering
  - body emission
  - epilogue/default return
  - async finalization

### Phase 5: Expression pipeline

- split `generateCall()` and `generateMethodCall()` first
- then split access/binary/unary paths
- keep `generateExpression()` as a dispatcher only

### Phase 6: Statement pipeline

- move loops, conditionals, returns, lets/vars, and assignments into dedicated modules
- ensure statement lowering uses the same shared state helpers as expression lowering

### Phase 7: Adjacent oversized modules

After `llvm_ir_gen.seen` is under control, apply the same pattern to:

- `ir_decl_runtime.seen`
- `type_registry.seen`
- `ir_type_info.seen`
- `ir_type_ops.seen`
- `ir_type_tables.seen`
- `ir_call_dispatch.seen`
- `llvm_backend.seen`

## Practical File Split Map

If I were starting the refactor, I would move the current largest methods into these first destinations:

| Current method | First destination |
|---|---|
| `registerDeclarations` | `llvm_ir/passes/declaration_pass.seen` |
| `generateMultiple` | `llvm_ir/passes/module_emit_pass.seen` |
| `generateSingle` | `llvm_ir/passes/module_emit_pass.seen` |
| `emitClassType` | `llvm_ir/passes/class_emit_pass.seen` |
| `generateClass` | `llvm_ir/passes/class_emit_pass.seen` |
| `generateClassMethodFromList` | `llvm_ir/passes/class_emit_pass.seen` |
| `generateFunction` | `llvm_ir/passes/function_emit_pass.seen` |
| `generateWhileStatement` | `llvm_ir/stmt/loops.seen` |
| `generateMemberAssignment` | `llvm_ir/stmt/assignments.seen` |
| `generateMemberAccess` | `llvm_ir/expr/access.seen` |
| `generateFieldAccess` | `llvm_ir/expr/access.seen` |
| `generateBinary` | `llvm_ir/expr/ops.seen` |
| `generateCall` | `llvm_ir/expr/calls.seen` |
| `generateMethodCall` | `llvm_ir/expr/methods.seen` |
| `inferExpressionType` | `llvm_ir/expr/inference.seen` |

Each of those destination files will still need sub-splitting if they approach 500 lines.

## Risks and How To Avoid Them

### Bootstrap breakage

Mitigation:

- keep `LLVMIRGenerator` as the stable facade until the end
- prefer additive helper modules over field-layout churn
- update the bootstrap module list in `main_compiler.seen` as each new module is added
- validate with `scripts/safe_rebuild.sh` at each phase, not just at the end

### Split without real decoupling

Mitigation:

- do not create files that still reach into dozens of globals directly
- each extraction should reduce hidden coupling, not just move text around

### “Utility module” dumping ground

Mitigation:

- group by compiler stage/domain
- avoid vague files like `helpers.seen` or `misc_codegen.seen`

### Divergence between inference and emission

Mitigation:

- mirror the emitter split in inference modules
- add tests for both generated IR and inferred type behavior

## Success Criteria

- `compiler_seen/src/codegen/llvm_ir_gen.seen` is a thin facade under `300` lines
- no LLVM codegen file exceeds `500` lines without an explicit allowlist exception
- no method exceeds `200` lines unless there is a strong documented reason
- `generateSingle()` and `generateMultiple()` share the same emission helpers
- call lowering has one argument-adaptation path
- decorator parsing produces explicit option objects
- `CodegenState` is the real shared lowering substrate, not a partial experiment
- `scripts/safe_rebuild.sh` and the existing codegen test suite stay green throughout

## Recommended First Refactor PR

This was the safest first implementation sequence, and it has now been completed:

1. Introduce real state structs for function/module options and wire `syncState()`.
2. Extract shared module emission helpers from `generateMultiple()` and `generateSingle()`.
3. Extract shared call-argument preparation from `generateCall()` and `generateMethodCall()`.

That sequence gives the best SRP/DRY payoff with the lowest bootstrap risk.
