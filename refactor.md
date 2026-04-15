# LLVM IR Generator Refactor Plan

## Goal

Refactor `compiler_seen/src/codegen/llvm_ir_gen.seen` and the surrounding LLVM codegen area so responsibilities are separated cleanly, duplication is reduced, and no source file is longer than 500 lines unless there is a strong, explicit reason.

This is an investigation and proposed plan, not an implementation.

## What I Found

### 1. `llvm_ir_gen.seen` is still the monolith

- Current size: `16,086` lines.
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

- `LLVMIRGenerator.syncState()` is currently a no-op (`compiler_seen/src/codegen/llvm_ir_gen.seen:1112-1114`).
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

If we want the safest first implementation step, I would start with this PR sequence:

1. Introduce real state structs for function/module options and wire `syncState()`.
2. Extract shared module emission helpers from `generateMultiple()` and `generateSingle()`.
3. Extract shared call-argument preparation from `generateCall()` and `generateMethodCall()`.

That sequence gives the best SRP/DRY payoff with the lowest bootstrap risk.
