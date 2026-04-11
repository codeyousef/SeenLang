# Changelog

All notable changes to the Seen compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
