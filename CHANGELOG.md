# Changelog

All notable changes to the Seen compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.3] - 2026-05-14

### Added

- FEL-261: Added conservative frontend dead-code warnings for unreachable statements, unused locals, unused parameters, unused import symbols, unused private top-level functions, and unused whole-module imports.
- FEL-261: Added LSP/editor diagnostic conversion coverage for Seen warning codes so dead-code warnings surface as warnings instead of errors.
- FEL-270: Added module namespace import aliases such as `import codegen.ir_call_driver as calls`, including alias-qualified resolution for functions, types, enums, and static-style references without injecting unqualified names.
- FEL-270: Added VS Code import-block folding for contiguous top-level `import`, `use`, and `pub import` lines.
- FEL-261/FEL-270: Added focused parser, compiler, and extension coverage for dead-code warnings, module alias imports, and import folding.
- FEL-262/FEL-271: Added focused stdlib coverage for `VecDeque`, `IntVecDeque`, and `BitSet` behavior around wraparound, growth, clearing, and word-boundary bit operations.

### Changed

- FEL-262/FEL-271: Optimized source-only `VecDeque`, `IntVecDeque`, and `BitSet` containers while preserving their public APIs.
- Bumped the shipped Seen compiler version to `0.8.3`.

## [0.8.2] - 2026-05-12

### Added

- Added focused regression coverage for prebuilt artifact `String` helper ABI preservation, `[build].modules` cross-module struct-argument calls, concurrent package prebuild temp-file isolation, and class values returned through Array-backed helper methods.

### Changed

- Documented the refactored LLVM codegen facade, shared state, lowering-driver boundaries, and prebuilt artifact declaration/object split for future compiler work.
- Clarified project configuration docs so `modules` under `[build]` is documented alongside `[project].modules`.

### Fixed

- Fixed prebuilt artifact dependency declaration seeding so linked dependency helpers that return `String` keep their `%SeenString` ABI even when the consumer calls them without an explicit source import.
- Fixed manifest module discovery so `modules` listed under `[build]` are included in the compilation graph, preserving cross-module struct layouts for method calls that pass struct literals.
- Fixed compiler package-build temp paths so concurrent `seen pkg prebuild` invocations no longer overwrite each other's module IR, optimization status, logs, or object files.
- Fixed method dispatch for user-defined `get` methods on class receivers so they no longer get mistaken for failed collection lookups, preserving class handles returned from Array-backed helper methods.

## [0.8.1] - 2026-05-12

### Fixed

#### Release packaging and CPU portability
- Fixed the Linux release rebuild path so portable `linux-x64` packages can be produced and verified against an explicit `x86-64` CPU baseline instead of inheriting AVX-512 instructions from the build host.
- Made the Windows cross-build helper use a serial, no-cache, portable IR generation path by default and lower transformed Windows IR through Clang's COFF object backend so release packages can be rebuilt from the current compiler on AVX-512 hosts.
- Fixed the Windows ABI transformer so struct parameters rewritten to `byval` pointers are materialized back into local aggregate values before the function body uses them.
- Fixed Windows COFF weak-symbol handling for runtime helpers and compiler package constants so cross-built compiler binaries link cleanly under MinGW.
- Pruned transient standard-library `*.tmp.*` files from Linux and Windows release staging so generated packages contain only intentional sources and metadata.
- Extended release artifact verification to require the packaged compiler to expose `seen pkg prebuild` and successfully emit both `objects.tsv` and `interface.index.tsv`, catching stale package-command surfaces before upload.
- Hardened low-memory rebuild recovery by repairing stale builder IR call-shape, void-parameter, and aggregate-return mismatches before LLVM verification.
- Fixed prebuilt package interface import resolution so artifact sources can resolve their own package-qualified sibling imports, preserving dependency class layouts for downstream `String` field access.
- Fixed prebuilt package object-manifest declaration discovery so `String`-returning helpers built into an artifact but omitted from the public interface module list keep their ABI when consumed downstream.
- Made prebuilt object manifests portable by recording project-relative source paths instead of build-machine absolute paths.
- Added shipped compiler support for `seen --version` / `seen -v` and `seen --help` / `seen -h`, and made legacy source-wrapper commands fail with explicit unsupported-command diagnostics.
- Fixed recovered compiler source reads in `seen check` by resolving real input paths more defensively and falling back to a shell-quoted read when the bootstrap runtime read path returns empty content.
- Updated compiler database output, CLI docs, known-limitations docs, and VS Code command labels so editor tooling follows the shipped `seen compile` command surface.
- Revalidated the package/hot-module workflow with the fixed compiler: package dependencies remain declaration-only during consumption, prebuilt objects link correctly, and shared module exports keep the expected ABI symbols.

## [0.8.0] - 2026-05-10

### Added

#### Language, imports, and package workflows
- Added standalone `/// ... ///` block comments as first-class Seen syntax, including compiler raw-scanner masking so fake imports, declarations, braces, and diagnostics inside comment bodies do not affect parsing, import discovery, package visibility checks, runtime-memory discovery, or interface indexing.
- Added array and literal-set membership lowering with `in`, including string and integer membership checks.
- Added `seen pkg prebuild` artifact support with PIC objects, `objects.tsv`, and `interface.index.tsv` so package dependencies can be consumed as declaration/interface artifacts while linking their prebuilt objects and skipping duplicate dependency codegen.
- Added package-root imports from manifest modules without requiring `src/mod.seen`, while keeping explicit package source imports available.
- Added package-private visibility diagnostics for outside-package imports of non-public declarations.
- Added shared-module and hot-reload workflows around `@export`, object manifests, PIC object emission, and inline string casts.
- Added cyclic import diagnostics that report clear cycle paths before compilation proceeds.

#### Bootstrap and validation guardrails
- Added strict prebuild gates through `scripts/seen_prebuild_gates.sh`, including compiler module seeding/import checks, import-cycle scans, codegen ABI-boundary checks, LLVM call-shape verification, aggregate ABI risk checks, and self-hosted ABI smoke coverage.
- Added focused Stage2 IR recovery pattern tests for undefined local register operands, aggregate/scalar mismatches, stale generator layout indices, malformed pointer/scalar stores, raw diagnostic text leaks, and float/double zero literal call operands.
- Added safer Stage2/Stage3 rebuild progress reporting, fail-fast behavior, explicit first-failure logs, recovery source directory control, and guarded recovery paths that avoid relying on stale live `/tmp` IR snapshots.

### Changed

#### LLVM codegen architecture
- Completed the LLVM generator facade split: `compiler_seen/src/codegen/llvm_ir_gen.seen` is now a thin public facade, with lowering moved into focused state-based drivers for modules, functions, statements, expressions, method calls, regular calls, low-level calls, binary expressions, member/index access, loops, `when`, unary, variables, struct literals, and package/interface support.
- Kept `LLVMIRGenerator` and `overrideTargetTriple` as the stable public API while replacing recursive helper coupling with lowering-context/state bridge modules and avoiding helper imports of `codegen.llvm_ir_gen`.
- Split remaining touched LLVM helper files below the 500-line cap by moving binary operands, for-in statement orchestration, and for-in element emission into dedicated modules.
- Broadened bootstrap module seeding in `main_compiler.seen` for the extracted codegen drivers and package artifact helpers.

#### Tooling, docs, and editor support
- Updated packaging docs for prebuilt artifacts, object manifests, interface indexes, artifact dependency linking, and package-root imports.
- Updated VS Code syntax/snippet support for `///` block comments, `@export`, and shared-module compile workflows.
- Made rebuild output less quiet during long bootstrap stages by showing stage, module, object, and latest-artifact progress.

### Fixed

#### Codegen and bootstrap correctness
- Fixed while-condition lowering so loop predicates generate a real `i1` comparison and branch instead of reusing stale or integer-shaped condition values.
- Fixed Stage2/Stage3 malformed IR recovery for undefined register operands, aggregate loads/stores, `store ptr <integer>` cases, `%SeenString` pointer/handle coercions, raw diagnostic line leaks, and `float 0` / `double 0` call operands.
- Fixed object-manifest flows so `--emit-llvm` still emits objects when requested and package prebuild artifacts preserve package and `@export` symbols.
- Fixed PascalCase static `new` locals so class handles use the expected storage shape.
- Fixed package artifact interface codegen so artifact modules are declaration-only during consumption and their prebuilt objects are linked exactly once.
- Fixed `effect(Token)` capability syntax and imported capability propagation so wrong or missing capability tokens are diagnosed consistently.
- Fixed package/source import resolution for project-root runtime memory imports, manifest companion modules, whole-module package imports, and legacy whole-module imports.
- Fixed missing imported modules and cyclic imports so they fail early with actionable diagnostics instead of reaching later bootstrap/codegen failures.
- Fixed installed compiler language-data discovery with executable-relative search paths and an English keyword fallback, so packaged compilers can build package artifacts and large external projects outside the source checkout.
- Fixed synthetic parser-expression construction in loop lowering and added a guardrail so codegen-created AST nodes use initialized defaults instead of partially initialized aggregate literals.

## [0.7.2] - 2026-04-29

### Fixed

#### Release packaging
- Fixed Linux x64 release packaging so the default artifact is built and verified against a portable `x86-64` CPU baseline instead of inheriting native build-host ISA features.
- Added an explicit `linux-x64-v3` release artifact tier for optimized AVX2-class machines.
- Added release-package verification for CPU baseline metadata, AVX-512 instruction evidence in default x64 binaries, and packaged compiler check/compile smoke tests.
- Fixed tarball and Unix installer replacement of `bin/seen` so existing symlink destinations are not overwritten unexpectedly.

#### Bootstrap and codegen
- Fixed S2→S3 self-hosted bootstrap verification by resolving LSP class fields such as `LspError.data` with explicit known field indices and Seen types instead of falling back to the first struct field.
- Hardened known-struct field layout fallback so it only uses a known layout when the requested field has a valid resolved index, preventing invalid LLVM IR in optional field method calls.

## [0.7.1] - 2026-04-28

### Fixed

#### Codegen and runtime regressions
- Fixed `Float32` pointer dereference casts to `Int` so codegen emits a direct floating-point-to-integer conversion instead of invalid LLVM IR.
- Fixed `&&` and `||` lexing, type inference, and short-circuit planning, and fixed Bool-aware `&` / `|` lowering to emit type-correct `i1` operations with integer predicate coercion.
- Fixed no-value class method metadata so calls lower with the `void` ABI instead of as integer-returning calls.
- Fixed declaration scanning for self-hosted builds by avoiding method-body walks while collecting class method ABI metadata.
- Fixed fresh self-hosted compiler crashes by avoiding unsafe global memo-cache mutations in codegen type lookup, method-return inference, and chained-path inference helpers.
- Fixed implicit-`this` dotted receiver lookup for nested property method calls, preserving object receiver pointers through chained member access.
- Made the runtime timestamp-period fallback weak so native implementations can override it at link time.
- Fixed low-memory rebuild recovery so intermediate Polly IR side files are not counted as raw module IR, and serialized legacy frozen bootstrap child processes under the memory guard.
- Added focused regression coverage for the codegen/runtime fixes and the no-value method-call IR shape.

## [0.7.0] - 2026-04-28

### Added

#### LLVM IR generator SRP refactor
- Extracted ~90 new single-responsibility helper modules from the monolithic `llvm_ir_gen.seen`, covering all major codegen concerns: function entry/exit, method dispatch, member access, binary expressions, loop control-flow, async/await, string interpolation, type layout, struct/class/enum emission, and statement dispatch.
- Added `ir_function_lifecycle.seen`, `ir_function_prebody.seen`, `ir_function_state.seen`, `ir_function_param_meta.seen`, `ir_method_receiver.seen`, `ir_method_receiver_lookup_plan.seen`, `ir_method_finalize.seen`, `ir_binary_fold_plan.seen`, `ir_loop_context.seen`, `ir_stmt_scope.seen`, `ir_decl_items.seen`, `ir_decl_registry.seen`, `ir_decl_scan.seen`, `ir_type_header.seen`, `ir_struct_layout_build.seen`, `ir_class_type_layout.seen`, `ir_class_type_decorators.seen`, `ir_module_constants.seen`, `ir_module_emit.seen`, `ir_string_emit.seen`, `ir_when_expr_plan.seen`, `ir_when_pattern_gen.seen`, `ir_when_arm_gen.seen`, `ir_constructor_field_emit.seen`, `ir_call_runtime_plan.seen`, `ir_call_builtin_plan.seen`, `ir_call_constructors.seen`, `ir_async_await.seen`, `ir_async_registry.seen`, and many more.
- Added `scripts/memory_guard.sh` for memory-aware process management during bootstrap builds.

### Changed

#### LLVM IR generator internal architecture
- Function lowering now uses dedicated state-wrapping helpers (`ir_function_state`, `ir_function_prebody`, `ir_function_lifecycle`) for cleaner access to lowering options, entry setup, and exit cleanup.
- Method receiver resolution split into planning (`ir_method_receiver_lookup_plan`) and code generation (`ir_method_receiver_lookup_gen`) phases, with predicates moved to `ir_method_receiver_predicates.seen`.
- Binary expression emission split into planning (`ir_binary_plan`, `ir_binary_fold_plan`) and emit (`ir_binary_emit`) drivers; short-circuit evaluation extracted to `ir_binary_short_circuit.seen`.
- Loop context and control-flow extracted to `ir_loop_context.seen` and `ir_loop_plan.seen`.
- Declaration registration and scanning extracted to `ir_decl_scan.seen`, `ir_decl_registry.seen`, and `ir_decl_items.seen`.
- Module-level constant and emit logic extracted to `ir_module_constants.seen` and `ir_module_emit.seen`.
- Type header and layout building extracted to `ir_type_header.seen`, `ir_type_layout.seen`, `ir_struct_layout_build.seen`, and `ir_field_layout.seen`.
- Class method generation extracted to `ir_class_method_gen.seen`; class constructor and type decoration split into `ir_class_constructor_gen.seen`, `ir_class_type_decorators.seen`, and `ir_class_type_special.seen`.
- Enum constructor emission extracted to `ir_enum_ctor_gen.seen`.
- String collection and emission split across `ir_string_collect.seen`, `ir_string_emit.seen`, `ir_string_interp_gen.seen`, and `ir_string_interp_infer.seen`.
- `when` expression handling split into `ir_when_expr_plan.seen`, `ir_when_pattern_gen.seen`, `ir_when_arm_gen.seen`, and `ir_when_result_gen.seen`.
- Statement dead-store analysis extracted to `ir_stmt_dead_code.seen` and `ir_stmt_scope.seen`.
- `import-c` frontend split into `c_import_ast.seen`, `c_import_emit.seen`, `c_import_layout.seen`, `c_import_type_builtin.seen`, `c_import_type_map.seen`, `c_import_util.seen`, and `c_import_state.seen`.

#### Bootstrap and build system
- `safe_rebuild.sh` now uses `memory_guard.sh` for memory-capped subprocess execution during low-memory bootstrap recovery, with serialized opt execution and cleaner temp-state resets.

### Fixed

#### Validation, codegen, and manifest regressions
- Fixed FEL-18 and FEL-22 validation regressions introduced during the IR refactor.
- Fixed post-manifest codegen regressions in package-aware builds.
- Fixed package source import resolution when using `src/` layout in `Seen.toml`.
- Fixed FEL-22 scanner memory usage during large file ingestion.
- Fixed FEL-22 manifest graph preflight not accounting for transitive package dependencies.
- Fixed legacy import visibility resolution for nested module paths.
- Fixed class method return type receiver inference (FEL-20 regression).
- Fixed high-arity parameter handling in codegen for `i64` alias and `uint32` global initialization.
- Fixed nested `continue` in high-arity loop contexts.
- Fixed bool-to-int coercion return paths in codegen.
- Fixed class method receiver resolution for static method calls on class instances.
- Fixed typed store return semantics in the IR backend.
- Fixed `import-c` output to not emit bogus helper bindings for real system headers.

## [0.6.0] - 2026-04-14

### Added

#### Seen package registries and source packages
- Added `seen pkg fetch`, `seen pkg pack`, and `seen pkg publish` for installing, archiving, and publishing Seen source packages.
- Added package dependency support in `Seen.toml` via `[registries]` and `[dependencies]`, including exact-version registry dependencies and local path dependencies.
- Added static-registry support with package indexes under `index/<package>.toml`, source archives under `archives/<package>/<package>-<version>.seenpkg.tgz`, local package installs under `.seen/packages/`, and generated `Seen.lock` files for registry-backed projects.
- Added initial package-management docs covering project config, CLI usage, and static registry hosting, plus a local-registry smoke test for publish/fetch/compile flow.

### Changed

#### Project manifest and import resolution
- Build and check flows now prepare package dependencies automatically before compilation, and import resolution can now load modules from declared package dependencies.
- `seen init` now scaffolds package-aware manifests with a default Seen registry entry.
- Native linker dependencies are now documented and preferred under `[native.dependencies]`, while legacy `[dependencies]` entries with `system = true` remain supported for compatibility.

## [0.5.0] - 2026-04-14

### Added

#### `import-c` bindings and FFI layout support
- Added `seen import-c <header.h>` to generate Seen bindings directly from C headers.
- `import-c` now imports enum constants, typedef aliases, opaque handle aliases, named `struct` records as `@repr(C)` classes, named `union` records as `@union` classes, and function-pointer typedefs as typed Seen callback aliases.
- Added lowering for inline C arrays to Seen fixed-size array types like `T[N]`, synthesized carrier types for anonymous structs/unions, and ABI-correct bitfield storage with generated getter/setter helpers for validated 32-bit and 64-bit storage groups.
- Added focused regression coverage for `import-c` bootstrap availability, typedefs, unions, anonymous records, inline arrays, bitfields, real system headers (`string.h`, `stdio.h`, `vulkan.h`), `@llvm.global_ctors` fixup safety, and runtime Vulkan symbol gating.

### Changed

#### Compiler and bootstrap behavior
- Seen now parses fixed-size type syntax `T[N]` through the parser, type sizing, index inference, and LLVM lowering so imported FFI layouts preserve inline-array ABI end to end.
- Imported unions now lower as storage for their largest field type instead of an opaque byte array, improving generated layouts for C union bindings.
- `import-c` now deduplicates redeclaration-heavy real-header AST dumps, reuses imported aliases and record types in generated fields/signatures, and uses bootstrap-safe `class` carriers for importer result/value objects.
- `--no-fork` now serializes Pass 2b opt/object work as well as IR generation, reducing peak memory pressure during bootstrap rebuilds.
- Added project notes documenting the HeartOn no-project-C groundwork and the remaining import-c/bootstrap follow-up work, and removed the obsolete `seen-fixes.md` scratch note.

### Fixed

#### Parser, import-c, and rebuild reliability
- Fixed bootstrap parser construction so `RealParser.new()` no longer rebuilds the token array in a way that could cause bootstrap-built parsers to drop the entire token stream before parsing.
- Fixed parser method-state handling by mutating `lastFunction` directly before pushing class methods, avoiding stale copies when marking parsed methods static, async, or override.
- Fixed `import-c` handling of redeclaration-style real-header AST lines so imports no longer emit bogus helper bindings like `prev` / `referenced` when run on system headers.
- `fix_ir.py` now inserts synthesized declarations at a stable top-level boundary instead of splitting multiline globals like `@llvm.global_ctors`, keeping rewritten IR valid.
- `safe_rebuild.sh` now hardens low-memory rebuilds with bootstrap smoke fallback, preserved-compiler recovery, opt-wrapper memory caps, serialized low-memory opt execution, and cleaner retry temp-state resets.
- Default runtime builds no longer export raw weak Vulkan `vk*` stubs that can shadow the real Vulkan loader; those stubs are now opt-in for bootstrap-only builds.

## [0.4.0] - 2026-04-09

### Added

#### Native targets and packaging
- Cross-target CLI/build plumbing now covers `windows-x86_64`, `macos-x86_64`, `macos-arm64`, `ios-arm64`, `ios-sim-arm64`, `linux-arm64`, and `android-arm64`, including target-aware output naming and nearest-`Seen.toml` language detection for project builds.
- Added native rollout tooling and packaging helpers, including `scripts/native_target_smoke.sh`, expanded `scripts/platform_matrix.sh`, `scripts/setup_linux_arm64_sysroot.sh`, `scripts/android_ndk_env.sh`, and `scripts/package_android_apk.sh`.

#### Safety and validation
- Added capability-token enforcement for `@using(...)` / `@effect(...)` checks and expanded validation coverage for `@send`, `@sync`, and `sealed` restrictions.
- Added regression coverage for multi-module compiler fixes, `Seen.toml` project resolution, recovery-opt failure handling, and current limitation fixtures.

### Changed

#### Release and tooling
- Corrected the release line back to the pre-1.0 series; this branch is versioned as `0.4.0`, replacing the erroneous `1.0.4-alpha` heading.
- Build caches are now isolated by effective target/profile/build signature, and the native-target documentation now covers Apple, Android, and local Linux ARM64 toolchains plus runtime smoke commands.
- GitHub-hosted CI/release workflows in this branch were moved to `.disabled` variants while the expanded native-target rollout work proceeded outside hosted workflows.

### Fixed

#### Bootstrap and build system
- Linux Stage2→Stage3 bootstrap, self-host rebuild, and recovery-opt handling now recover from staged failures, stale artifacts, and bootstrap crashes without the previous manual cleanup paths.
- Fixed Android bundle/release path handling, emulator APK validation, and widened Windows/Android native smoke coverage.
- `Seen.toml` project discovery now handles absolute-path project members, standalone non-members, build-entry seeding, and root-level scratch `main()` files correctly. This removes the HeartOn standalone IR-generation crash path while preserving nested project fallback behavior.
- `Seen.toml` system dependencies can now declare a local `path`, so project-local native shims link with resolved `-L` flags and native Linux/macOS runtime search paths without extra library-path wrappers.
- Fixed stripped bootstrap workspace lexing by teaching `KeywordManager` to recover `languages/` through the real compiler source checkout when the temporary workspace omits language TOMLs; Stage2→Stage3 self-hosting now reaches the full module graph and links successfully from that layout again.
- Removed unsupported `-align-loops=32` flags from the merged `llc` release path and ThinLTO link flags, so release builds no longer depend on LLVM options rejected by some toolchain installs.

#### Frontend, parser, and codegen
- Fixed keyword lookup, parser type handoff, parser data/function-body regressions, and frontend/class-detection issues that were blocking self-host and multi-module builds.
- Fixed default-parameter lowering so omitted arguments now inject their registered defaults correctly at call sites, including string and integer defaults.
- Added regression coverage for static class methods returning class instances and other receiver-free static call paths.
- Fixed receiver-free static method lowering for class methods like `Type.fromJson(...)`, so derive-generated JSON helpers no longer receive a bogus null receiver and deserialize back into populated objects.
- Fixed positional class-constructor initialization so `ClassName(arg1, arg2)` writes those arguments into the allocated object fields instead of returning a zero-initialized instance; `@derive(Json)` serialize round-trips now observe the constructed values.
- Fixed unicode string lowering, `Vec` dispatch, `HashMap` `Option` lowering, `StringHashMap` dispatch, module-constant type inference, void method calls, extern Float parameter registration/promotion, and `for`-loop SSA ordering regressions.
- Fixed documented multi-module and recovery regressions, including the HeartOn module-handling failures that previously crashed during IR generation and now progress to ordinary diagnostics or linker failures instead.
- Fixed interpolated-string parsing for empty leading, trailing, and adjacent literal segments, eliminating malformed LLVM in cases like `\"{expr}\"`, `\"{expr}!\"`, and `\"{a}{b}\"`; added focused regression coverage for interpolation edge cases.

#### Runtime, standard library, and validation
- Updated runtime and stdlib support used by the new native-target lanes, including string helpers, file I/O paths, `Option`, and hash collection behavior.
- Hardened TEE/enclave support so stub mode is no longer enabled implicitly when no hardware TEE is present, added import-safe availability helpers in `security.enclave`, and linked the TEE runtime object into host/cross-target builds.
- Fixed math wrapper/runtime declaration mismatches around `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, and `tan`, and added regression coverage for float special values like `INFINITY`, `NEG_INFINITY`, and `NAN`.
- Expanded regression fixtures and root-level smoke checks for compiler crash repros, native-target rollout, and safety-rule enforcement.

## [0.3.7] - 2026-03-16

### Fixed

#### Codegen
- `seen_arr_get_element_ptr` ABI mismatch for both Array<String> and Array<Int>: the function returns a pointer to the element, but the frozen compiler stored/used that pointer directly as the value. For strings, pointer was interpreted as SeenString (causing SIGSEGV). For integers, pointer was used as the size value (causing malloc with ~94TB). Fix inserts proper `load` from the element pointer for both `%SeenString` (16-byte) and `i64` (8-byte) element types
- `extractvalue %SeenString ..., 1` for emptiness checks corrected to field index `0` (length), not `1` (data pointer)
- `this` pre-registration in `generateClassMethod` no longer incorrectly adds `this` for `new` constructors (which are static)

#### Build System
- `fix_ir.py` signature-based detection now scans both `define` and `declare` lines, catching cross-module functions like `pipeGet(%SeenString, i64)` that were previously invisible
- `fix_ir.py` multi-store alloca guard relaxed — shared allocas (used for both array access and boolean/integer values on different control flow paths) are now correctly handled
- `fix_ir.py` alloca widening guard prevents %SeenString widening when the alloca also stores integer constants (avoids type conflicts in mixed-use allocas)
- `safe_rebuild.sh` Linux path now attempts S2→S3 bootstrap verification with 30-minute timeout instead of skipping entirely

## [0.3.6] - 2026-03-15

### Fixed

#### Codegen
- Int-to-Float implicit promotion now works at function/method call sites. Passing an `Int` argument to a `Float` parameter no longer produces garbage values from raw bit reinterpretation — `sitofp i64 → double` is emitted automatically. Covers free functions, implicit `this.method()`, static methods, and receiver method calls.

#### Build System
- `safe_rebuild.sh` opt script race condition fixed with recovery mechanism — parallel opt processes no longer clobber each other's output

## [0.3.5] - 2026-03-15

### Improved
- Rust-quality error reporting with span underlining (`^^^^`), `help:` suggestions, `note:` context, and error codes (`error[E001]`, `E003`, `E004`)
- Type mismatch errors now show conversion hints (e.g., `help: use .toString() to convert Int to String`)
- Missing `main` function error includes suggestion: `help: add 'fun main() r: Void { }' as entry point`
- LSP diagnostics now carry error codes, span the full token range, and include help/note text in hover tooltips
- LSP `textDocument/codeAction` handler returns QuickFix actions from diagnostic suggestions
- VSCode extension parses caret span width (`^^^^`) for accurate error underlining
- VSCode extension parses `note:` lines alongside `help:` lines from compiler output
- VSCode QuickFix actions use correct Seen method names (`.toString()`, `parseInt()`, `.toFloat()`, `.toInt()`)

### Changed
- VSCode extension version bumped to 1.4.0

## [0.3.4] - 2026-03-15

### Fixed

#### Codegen
- FMA peephole optimization (`a + b*c` → `fmuladd`) no longer produces type errors when Int operands are mixed with Float. `sitofp` conversion is now inserted for all non-Float operands before emitting `llvm.fmuladd.f64`
- `getFieldType` for LLVMIRGenerator now falls through to the dynamic struct registry when the hardcoded field table returns empty, enabling correct type resolution for new fields

#### Build System
- Frozen compiler's phantom `declare` statements (from function names in string constants) are now removed during optimization, fixing undefined symbol link errors
- Corrupt `declare` statements containing `\00` escapes from string constant parsing are stripped
- `fix_ir.py`: added `fix_empty_type` pass to handle empty-type `store`/`load` instructions from the frozen compiler

### Added
- `TypeRegistry` class in `type_registry.seen` — encapsulates struct/function registry operations (findStruct, ensureStructEntry, registerStructMethod, getFieldInfo, type sizing, header parsing) with owned parallel arrays. Free-function wrappers preserved for backward compatibility

## [0.3.3] - 2026-03-14

### Fixed

#### Build System / Path Resolution
- Compiler no longer requires CWD to be the project root. All internal paths (`languages/`, `seen_runtime/`, `compiler_seen/src/`, `seen_std/src/`) now resolve relative to the binary's installation location using `/proc/$PPID/exe` on Linux, with `args()[0]` fallback for macOS
- `resolveModulePath()` now prefixes stdlib and compiler module paths with the compiler root
- Runtime linking paths (`seen_runtime/`, `seen_region.c`, `seen_gpu.c`) use absolute paths derived from the compiler root
- `KeywordManager` TOML loading (`languages/`) resolves via `resolveLanguagesRoot()` — probes CWD first (fast path), falls back to binary-relative resolution
- Relative input file paths are resolved to absolute at the entry point of `compileCommandWithAllOptions()` and `jitRunCommand()`
- `ensureJitRuntime()` uses compiler-root-relative paths for the JIT runtime object

## [0.3.2] - 2026-03-14

### Fixed

#### Codegen / Type Errors
- Bool return values compared with `==` no longer produce i1/i64 type mismatches in LLVM IR
- Float comparisons (`!=`, `==`) with mixed types now emit `fcmp` instead of `icmp`
- Implicit Int-to-Float promotion in arithmetic expressions now inserts `sitofp` conversion
- Cross-class method calls preserve the declared return type instead of inferring i1

#### Loop Compilation
- Nested while loops no longer clobber local variables across function calls. Removed incorrect `nuw`/`nsw` flags from general integer arithmetic and `nosync` from function attributes that were enabling unsafe LLVM optimizations. Induction variable state is now properly saved/restored across nested loop boundaries.
- `for i in 0..N` loops in nested contexts no longer hang or produce wrong values
- Nested loops with `floorFloat` calls no longer miscompile at specific coordinates

#### Linker / Module System
- `Array.pop()` now dispatches to runtime functions (`seen_arr_pop_i64`/`f64`/`str`) instead of emitting undefined `@Array_pop`
- `extern fun` declarations are now registered in the cross-module symbol table, eliminating the need to duplicate FFI declarations in every module that uses them
- `__Tan` is now auto-declared in multi-module builds alongside `__Sin` and `__Cos`
- `extern fun` return values used in conditionals are no longer eliminated by LTO; extern declares now carry `memory(inaccessiblemem: readwrite)` to preserve side-effect semantics

#### Build System / Module Resolver
- The compiler no longer requires CWD to be the seenlang root directory. Stdlib imports now resolve relative to the compiler binary's installation path.
- Import resolver no longer unconditionally strips the first path segment when it matches the source directory name. The segment is only skipped if the resulting file actually exists on disk.
- Stale `.o` files from previous builds with different module counts are now cleaned before linking

#### Runtime / Performance
- Method name `get()` with a single parameter no longer incorrectly resolves to `Array.get()` when called on non-Array receivers
- Added `Array.clear()` method that resets length to zero without freeing the backing buffer, enabling array reuse in hot loops instead of repeated allocation

### Added
- `seen_arr_clear` runtime function for zero-cost array reuse
- `getCompilerRoot()` helper for resolving compiler-relative paths from the binary location
- `fix_ir.py` fix for `allocsize(0)` attribute incorrectly rewritten to `allocsize(i64)` during IR fixup

## [0.3.1] - 2026-03-13

### Fixed
- Cross-module `let` constants no longer resolve as 0

### Added
- VSCode Tasks with problem matcher for build/run/check commands
- LSP diagnostics for missing `main`, clickable terminal links

## [0.3.0] - 2026-03-12

### Added
- Windows x86-64 cross-compilation support
- Windows installer (NSIS-based)
