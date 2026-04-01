# Native Target Rollout Plan

## Execution Status

### Implemented in the first patch set

- Added `Android_ARM64` to the native target model in `compiler_seen/src/codegen/interfaces.seen`.
- Added Android target parsing aliases for:
   - `android-arm64`
   - `aarch64-linux-android`
   - `aarch64-linux-android24`
- Switched the first concrete Windows target triple path to GNU-style Windows in `compiler_seen/src/codegen/interfaces.seen`.
- Exposed Android and native target names in `compiler_seen/src/main.seen`.
- Updated default output naming in `compiler_seen/src/main.seen` for:
   - Windows `.exe`
   - Android `lib*.so`
- Replaced the hardcoded failure implementations of:
   - `Linker.link()`
   - `Linker.linkStatic()`
   - `Linker.linkDynamic()`
- Added normalized target handling in `compiler_seen/src/main_compiler.seen` so one resolved target now drives:
   - LLVM triple override
   - target-aware runtime compilation
   - target-aware region runtime compilation
   - target-aware link command selection
- Added explicit stage-compiler handling for:
   - Windows x86_64
   - Android ARM64

### Implemented in the second patch set

- Added explicit Android NDK root and sysroot resolution in `compiler_seen/src/main_compiler.seen` using:
   - `ANDROID_NDK_HOME`
   - `ANDROID_NDK_ROOT`
- Changed Android cross-target compilation to fail fast when no valid NDK sysroot is available instead of proceeding with incomplete target flags.
- Added target-specific runtime compile flags in `compiler_seen/src/main_compiler.seen`, including:
   - `-pthread` for non-Windows runtime builds
   - Android API define propagation
   - Windows compatibility define propagation
- Changed cross-target runtime compilation to fail fast if `seen_runtime.c` cannot be compiled for the requested target, preventing accidental host-runtime reuse.
- Added explicit target capability gating for `seen_region.c` in `compiler_seen/src/main_compiler.seen` so region runtime compilation now only runs for currently supported targets:
   - `linux-x86_64`
   - `linux-arm64`
   - `android-arm64`
- Added target-aware GPU runtime compilation in `compiler_seen/src/main_compiler.seen` for cross-target builds instead of always omitting GPU runtime objects when not compiling for the host.
- Centralized target-specific link library selection in `compiler_seen/src/main_compiler.seen` so runtime-linked system libraries are now chosen by effective target instead of a single host-biased default.

### Implemented in the third patch set

- Disabled the host-native merged `llc` release path for cross-target builds in `compiler_seen/src/main_compiler.seen` so release cross-compilation no longer falls back to `-mcpu=native` object generation.
- Extended target runtime compile flags to cover target-specific PIC requirements, with Android cross-target runtime objects now compiled with `-fPIC`.
- Reused the same target runtime compile flags for `seen_region.c` and `seen_gpu.c` in `compiler_seen/src/main_compiler.seen` so auxiliary runtime objects stay aligned with Android API defines and other target-specific C flags.
- Removed the unconditional Windows `-lws2_32` default from `compiler_seen/src/main_compiler.seen` after runtime inspection confirmed the current cross-target runtime path depends on core Win32 APIs rather than Winsock.

### Implemented in the fourth patch set

- Added `scripts/native_target_smoke.sh` as the first native-target smoke harness using the real compiler CLI path, with automatic `build` versus `compile` selection so the harness works against both newer and older stage binaries.
- The smoke harness currently targets:
   - `linux-x86_64`
   - `linux-arm64`
   - `windows-x86_64`
   - `macos-x86_64`
   - `macos-arm64`
   - `ios-arm64`
   - `android-arm64`
- The harness records per-target `success`, `failure`, or `unavailable` status in a TSV summary and keeps per-target build logs under `artifacts/native-target-smoke/`.
- Apple and Android smoke tests now fail closed on missing host prerequisites by reporting `unavailable` instead of silently pretending the target was tested.
- Wired `scripts/platform_matrix.sh` to consume the native smoke harness for Windows, macOS, iOS, and Android instead of emitting placeholder `pending` results, while Linux reports now include both compile-smoke and runtime example status.
- Made Linux example execution opt-in in `scripts/platform_matrix.sh` so the default matrix remains deterministic and always emits smoke reports even when runtime examples are flaky or long-running.
- Verified the smoke-backed matrix on the current Linux host: `linux-x86_64` emits a real successful smoke artifact report, and `macos-arm64` now reports `unavailable` because `xcrun` is not present on this host.
- Hardened `scripts/native_target_smoke.sh` so a cross-target build only reports `success` when the emitted artifact format matches the requested target; host-ELF outputs for Windows or Linux ARM64 now fail instead of being counted as successful smoke results.
- Re-ran the corrected smoke harness on the current Linux host and verified the current checked-in stage binary still emits host-format x86_64 Linux ELF artifacts for both `windows-x86_64` and `linux-arm64`; those targets are now correctly reported as `failure` until the compiler binary is rebuilt and revalidated.
- Fixed a stage-compiler CLI mismatch where `compile` accepted `--target=<value>` but not `--target <value>`, and updated the smoke harness to use the compatible `--target=<value>` form for compile-mode invocations against older stage binaries.

### Implemented in the fifth patch set

- Fixed the MinGW runtime compatibility header in `seen_runtime/seen_compat_win32.h` so `ETIMEDOUT` is visible before first use during Windows target runtime compilation.
- Added Linux ARM64 sysroot discovery in `compiler_seen/src/main_compiler.seen` via `SEEN_LINUX_ARM64_SYSROOT` plus common aarch64 sysroot paths, and changed non-ARM64 hosts to fail early with an explicit prerequisite error instead of diving into host glibc header mismatches.
- Added a matching Linux ARM64 smoke preflight in `scripts/native_target_smoke.sh` so hosts without an ARM64 sysroot now report `unavailable` instead of a misleading compiler `failure`.
- Re-ran the unresolved smoke targets on the current Linux host after the compile flag fix and narrowed the remaining results to:
   - `windows-x86_64`: real runtime compile failure in the Win32 compatibility layer, now fixed in source
   - `linux-arm64`: missing ARM64 sysroot on this host, now treated as an explicit prerequisite gap
   - `android-arm64`: still `unavailable` without NDK

### Implemented in the sixth patch set

- Traced the Windows cross-target path in `compiler_seen/src/main_compiler.seen` and removed a stray control-flow `else` that let runtime compilation run but skipped the final link step entirely.
- Re-ran the manual Windows compile trace after the control-flow fix and confirmed the compiler now reaches the final `clang` link invocation instead of returning success early with no artifact.
- Captured the next concrete Windows blocker from the traced link step: the optional GPU runtime pulled in `-lvulkan-1` for a hello-world smoke build, and MinGW link failed because that Vulkan import library was not available on this host.
- Disabled GPU runtime support for `windows-x86_64` in `compiler_seen/src/main_compiler.seen` so default Windows smoke builds no longer force Vulkan linkage when the program does not need GPU support.
- Rebuilt the compiler again after the Windows GPU-runtime change; the refreshed usable result is still the recovered Stage2 compiler, while S2->S3 bootstrap verification continues to segfault with `exit=139` on this host.
- Re-ran `scripts/native_target_smoke.sh` for `windows-x86_64` with the refreshed compiler and confirmed the current status is still `failure` with `build command succeeded but artifact was not produced`, so the remaining Windows issue is now narrowed past the runtime-header and Vulkan-link blockers.

### Implemented in the seventh patch set

- Re-ran the native smoke harness against the current checked-in compiler and established that the earlier Windows blocker was stale: `windows-x86_64` already emitted a real PE artifact, while the new active regression was `linux-x86_64` failing during link.
- Traced the Linux host failure to target-insensitive cache reuse in `compiler_seen/src/main_compiler.seen`: `.seen_cache` and `/tmp/seen_ir_cache` keys did not include target or compile-mode identity, so a Windows-target object could be reused in a Linux-target link.
- Added `buildCacheSignature(...)` in `compiler_seen/src/main_compiler.seen` and widened the cache namespace to include the effective target plus key compile-mode inputs for:
   - registry hash invalidation
   - source-level cache object/hash filenames
   - IR cache key derivation and cache promotion
- Rebuilt the compiler with `scripts/safe_rebuild.sh`; Stage2 recovery linked successfully and was installed as the production compiler, while S2->S3 bootstrap verification still segfaults with `exit=139` on this host.
- Re-ran `scripts/native_target_smoke.sh` with the rebuilt compiler and confirmed both:
   - `linux-x86_64`: `success` with a real ELF artifact
   - `windows-x86_64`: `success` with a real PE artifact
- Recorded the validating smoke outputs under:
   - `artifacts/native-target-smoke/20260327T211904Z/`
   - `artifacts/native-target-smoke/20260327T211915Z/`

### Implemented in the eighth patch set

- Traced the remaining Linux `S2->S3` bootstrap segfault to the class pre-registration path in `compiler_seen/src/codegen/type_registry.seen`: `findStruct()` cached pre-registered class entries into module-level cache arrays, and the first cache write during Pass 1 class layout registration crashed inside `seen_arr_push_str`.
- Removed the fragile struct-index cache writes in `compiler_seen/src/codegen/type_registry.seen`, which keeps `findStruct()` on the linear-scan path and avoids the imported-module cache corruption during bootstrap.
- Re-ran minimal compiler probes with the rebuilt production compiler and validated that all of the previously crashing class-only cases now compile successfully:
   - class with field only
   - class with method only
- Preserved the earlier compiler-root import-resolution fix in `compiler_seen/src/main_compiler.seen`, so nested compiler-module imports continue to resolve under `compiler_seen/src/` instead of being mis-rooted under subdirectories like `bootstrap/`.
- Traced the next self-host blocker to the compile-time circular-dependency check in `compiler_seen/src/main_compiler.seen`: the DFS stack logic could enqueue the same white module repeatedly on cyclic graphs because nodes were not marked before push.
- Fixed that DFS by marking modules as `queued` before pushing them during cycle detection, preventing duplicate stack growth on the compiler's cyclic import graph.
- Rebuilt again with `scripts/safe_rebuild.sh`; Stage2 recovery linked successfully and the refreshed production compiler now moves the Linux `S2->S3` verification substantially further:
   - previous blocker: segfault in Pass 1 class declaration registration
   - intermediate blocker: timeout before Pass 1 while doing pre-pass cycle analysis
   - current blocker: timeout during serial Pass 2 code generation for `compiler_seen/src/main_compiler.seen`, stalling after `[irgen module 0] functions start`

### Implemented in the ninth patch set

- Fixed parser chaining in `compiler_seen/src/parser/real_parser.seen` so complex receivers like `call().field.next` keep the nested member-access tree instead of being flattened into invalid textual receivers.
- Removed the stale `when` bootstrap workaround from `compiler_seen/src/parser/real_parser.seen`; current token mappings already cover `KeywordWhen`, so bootstrap now uses the normal parser path.
- Added chained-path reconstruction in `compiler_seen/src/codegen/llvm_ir_gen.seen` for textual zero-argument method segments like `unwrap()` so type inference and member-access lowering can recover older textual receiver encodings when they appear.
- Rebuilt with `scripts/safe_rebuild.sh` after the parser and codegen fixes and verified the Linux self-host path now completes end to end:
   - Stage2 build succeeded
   - Stage3 build succeeded
   - the safe rebuild now installs Stage3 as the production compiler on this host

### Implemented in the tenth patch set

- Traced a post-bootstrap runtime regression in the freshly installed compiler to lexer keyword lookup: trivial user compiles crashed in `SeenLexer_handleIdentifierOrKeyword(...)` immediately after scanning `fun`.
- Reworked `compiler_seen/src/lexer/keyword_manager.seen` so keyword resolution no longer depends on bootstrap-fragile generic `Map.get()` or direct generic-map field access for `SeenTokenType` values.
   - keyword TOML entries are now mirrored into bootstrap-stable `KW:<keyword>=<TokenName>` source entries
   - runtime keyword resolution now scans those stored entries and maps token names with a direct resolver
- Updated `compiler_seen/src/lexer/lexer.seen` to use the direct keyword lookup path instead of unwrapping the old generic `Option<SeenTokenType>` result.
- Made two bootstrap-compatibility source fixes in `compiler_seen/src/main_compiler.seen` uncovered while rebuilding the compiler with older saved stages:
   - `hashString(...)` now explicitly types `code` as `Int` so old bootstrap stages do not mis-lower `step + code` as string concatenation
   - `translateCommand()` now uses `printRaw(...)` for stdout output, avoiding a missing `print` declaration in module-0 bootstrap IR while preserving no-newline behavior
- Rebuilt a fresh candidate compiler from the saved `stage2_head` artifact and validated that it no longer crashes on the original hello-world compile regression.
- Re-ran the native smoke harness with that rebuilt compiler and recorded refreshed results under:
   - `artifacts/native-target-smoke/20260329T153620Z/`
   - `linux-x86_64`: `success`
   - `windows-x86_64`: `success`

### Implemented in the eleventh patch set

- Investigated the next compiler-correctness regression surfaced during native-target validation using a focused program that combined:
   - constructor inference via `Counter.new()`
   - instance `Void` methods
   - static `Void` methods
   - `main() r: Int`
- Hardened float-argument promotion metadata in:
   - `compiler_seen/src/codegen/llvm_ir_gen.seen`
   - `compiler_seen/src/codegen/ir_call_dispatch.seen`
  so later-stage codegen no longer crashes when class-method float-parameter metadata is missing or metadata arrays drift out of sync.
- Added codegen-side free-function return recovery in `compiler_seen/src/codegen/llvm_ir_gen.seen` so top-level functions now fall back to body-return inference when the AST reaches LLVM codegen with `returnType = Unknown`.
- Kept constructor and static-call lowering aligned with the recovered return-type path by reusing resolved method and function return types during declaration registration and generic static-call lowering.
- Added `tests/codegen/test_void_method_calls.seen` as a focused regression covering:
   - constructor inference for `Counter.new()`
   - instance `Void` method calls
   - static `Void` method calls
   - integer return lowering for `main()`
- Rebuilt Stage2 from `bootstrap/stage1_frozen` and verified the focused regression now both:
   - compiles successfully
   - runs successfully with exit status `0`

### Implemented in the twelfth patch set

- Replaced split nested `TypeNode` field mutation in `compiler_seen/src/parser/real_parser.seen` with whole-node assignment for parser paths that populate:
   - top-level function return types
   - method return types
   - return-label parameters
   - regular and `comptime` parameter types
- Added parser-local helpers in `compiler_seen/src/parser/real_parser.seen` so the current scratch type state is materialized as a full `TypeNode` value and ownership/comptime/default-value type-name encoding is rebuilt before assignment instead of mutating nested inline fields.
- Rebuilt the compiler again with `scripts/safe_rebuild.sh`; the frozen bootstrap still produces a fresh Stage2 compiler successfully, but the current `S2->S3` verification path is back to failing with `exit=139`, so the refreshed Stage2 compiler remains the installed production compiler on this host.
- Re-validated `tests/codegen/test_void_method_calls.seen` end to end with the refreshed compiler and confirmed it still:
   - compiles successfully
   - emits a consistent integer-returning `main`
   - runs successfully with exit status `0`
- Added a second parser-discriminating validation using `tests/codegen/test_move_call.seen`: the top-level `Void` function `consume(...)` now lowers as `define void @consume(...)`, which the LLVM-side free-function fallback could not infer from the empty return path.
- Re-validated supported parameter parsing with `tests/test_comptime_params.seen`, which still compiles and runs successfully after the parser-side type handoff fix.

### Implemented in the thirteenth patch set

- Fixed a second parser self-host regression in `compiler_seen/src/parser/real_parser.seen` where direct expression-position uses of contextual identifier `data` could be misrouted into keyword-expression parsing and swallow later top-level items, producing false `missing main function` failures.
- Hardened identifier parsing in `parsePrimary()` so contextual `data` is forced through plain variable parsing before `if` / `when` / `match` expression branches, and switched the primary identifier read path to capture `peek().getValue()` before `advance()` to avoid another bootstrap-fragile token-copy path.
- Kept variable-expression name recovery aligned across parser and LLVM lowering by reusing helper-based lookup in:
   - `compiler_seen/src/parser/real_parser.seen`
   - `compiler_seen/src/codegen/llvm_ir_gen.seen`
  so assignment lowering, member-call lowering, and expression type inference continue to resolve variable names when older bootstrap paths leave one of the string fields empty.
- Removed parser/codegen debug instrumentation that had started emitting invalid LLVM during bootstrap iteration, including the expression-statement debug append in `compiler_seen/src/codegen/llvm_ir_gen.seen`.
- Added `tests/codegen/test_parser_function_body_regression.seen` as a focused regression covering:
   - direct `data: String` parameter use in loop conditions
   - `byteAt`, `substring`, and `length()` access on that parameter
   - preservation of later top-level functions including `main()`
- Rebuilt with `scripts/safe_rebuild.sh` after the parser fix; Stage2 rebuilt successfully and was installed as the production compiler, while Stage3 verification still does not complete cleanly on this host and the script falls back to the recovered production install path.
- Re-validated the parser-side regression with the rebuilt production compiler and confirmed that:
   - reduced `data` repros now pass `check`
   - `tests/codegen/test_parser_function_body_regression.seen` now passes both `check` and `compile`
- Separated the remaining nonzero runtime result from the parser bug by running minimal string probes with the rebuilt production compiler: small programs using `String.length()` and `== ""` still compile but return the wrong result even when the parameter is named `value`, so the next blocker is now string runtime/codegen correctness rather than parser corruption.

### Implemented in the fourteenth patch set

- Expanded the default native platform matrix in `scripts/platform_matrix.sh` so the compile-smoke sweep now includes:
   - `linux-x86_64`
   - `linux-arm64`
   - `windows-x86_64`
   - `macos-x86_64`
   - `macos-arm64`
   - `ios-arm64`
   - `android-arm64`
- Removed out-of-scope `web-wasm32` from the default platform-matrix run while keeping the explicit web path available for manual use.
- Fixed Linux report aggregation in `scripts/platform_matrix.sh` so top-level platform status now reflects the compile-smoke outcome instead of being masked by the default runtime-example `skipped` status.
- Made `scripts/platform_matrix.sh` exit nonzero on real target `failure` states while preserving `unavailable` as nonfatal, so the matrix can now act as a CI gate instead of only emitting JSON.
- Added a dedicated `native-target-matrix` job to `.github/workflows/ci.yml` that:
   - downloads the rebuilt compiler artifact from the main Ubuntu build job
   - installs MinGW and Linux ARM64 cross prerequisites on the runner
   - runs `scripts/platform_matrix.sh`
   - uploads the generated native-matrix reports as workflow artifacts
- Re-ran the focused string regression `tests/codegen/test_string_param_literal_regression.seen` with the current production compiler and confirmed it now compiles and runs successfully with exit status `0`, so the earlier isolated string-runtime note no longer reproduces via that regression.
- Locally validated the updated matrix script on the current Linux host with:
   - `linux-x86_64`: real smoke run completed successfully
   - `linux-arm64`: honest `unavailable` status without a local ARM64 sysroot

### Implemented in the fifteenth patch set

- Split `.github/workflows/ci.yml` native-target coverage into two host-appropriate jobs instead of running the full native sweep from a single Ubuntu environment.
- Narrowed the Ubuntu native matrix job to the targets that should be exercised from Linux CI:
   - `linux-x86_64`
   - `linux-arm64`
   - `windows-x86_64`
   - `android-arm64`
- Added explicit Android NDK provisioning to the Ubuntu native matrix job by downloading and exporting a real NDK under:
   - `ANDROID_NDK_HOME`
   - `ANDROID_NDK_ROOT`
  so Android smoke no longer depends on a manually preconfigured runner.
- Added a dedicated Apple native matrix job on `macos-14` that:
   - installs Homebrew LLVM
   - rebuilds the compiler natively with `./scripts/safe_rebuild.sh`
   - runs `scripts/platform_matrix.sh` for `macos-x86_64`, `macos-arm64`, and `ios-arm64`
   - uploads separate Apple matrix reports as workflow artifacts
- Kept the Linux/Cross and Apple report artifacts separate in CI so failures can be attributed to the correct host/toolchain lane.
- Validated the updated workflow file locally for YAML/tooling errors; actual hosted CI execution still needs to be observed on GitHub runners.

### Implemented in the sixteenth patch set

- Kept the GitHub Actions workflow definitions disabled on this branch and updated the rollout notes so hosted CI follow-up stays explicitly paused until manual native-target verification is complete.
- Moved float-parameter promotion metadata registration in `compiler_seen/src/codegen/llvm_ir_gen.seen` fully into Pass 1 declaration scanning, including normalized impl-method names, so later codegen no longer mutates that global registry while class bodies are being emitted.
- Removed the redundant Pass 2 float-registry mutations from extern and function codegen paths; the stale Linux self-host crash was hitting `promoteFloatArgsImpl` after those late mutations during `seen_std/src/json/parser.seen` class generation.
- Re-ran `./scripts/safe_rebuild.sh` on the current Linux host and confirmed:
   - `S1->S2` still completes successfully from the frozen bootstrap
   - `S2->S3` now compiles successfully without the historical `exit=139`
   - the rebuild promotes Stage3 as the installed production compiler on this host

### Remaining in-progress work

- Harden Windows link flags and runtime library selection beyond the initial GNU path.
- Validate the reduced Windows default link-library baseline on a real MinGW/LLVM toolchain and add back only the Windows system libraries that are proven necessary.
- Validate Android NDK-backed runtime, GPU runtime, and final link behavior on a real NDK installation.
- Validate cross-target release-mode builds after the merged-`llc` path bypass so ThinLTO-backed per-module linking remains correct for the native target matrix.
- Keep the CI workflow definitions disabled on this branch until manual native-target verification is complete, then observe the first hosted Apple and Android CI runs and harden the workflow if GitHub runner differences expose bootstrap, SDK, or provisioning issues.
- Keep validating cache isolation across target/profile combinations so cross-target requests do not reuse incompatible cached objects.
- Re-establish or retire the earlier string runtime/codegen bug with a fresh failing repro; the focused `String.length()` / empty-string equality regression now passes with the current production compiler.

### Current implementation posture

The compiler is no longer relying on target names alone for Android. The native path now has real target-model, target-aware runtime compilation, capability-gated auxiliary runtime compilation, and target-aware link-library selection for Android and Windows. The remaining work is now primarily validation and Windows-specific hardening rather than missing compiler routing.

The latest native-target and bootstrap validation closed the parser-side `data` regression that was swallowing top-level items in reduced self-host repros, and the later float-promotion registry fix now lets `scripts/safe_rebuild.sh` complete `S2->S3` on the current Linux host without the earlier `promoteFloatArgsImpl` segfault. On this machine, the rebuilt production compiler still emits the correct artifact formats for both `linux-x86_64` and `windows-x86_64`; Linux ARM64 remains an honest sysroot prerequisite gap, and the repository now carries dedicated Ubuntu Android-backed and macOS Apple-host native matrix definitions that are intentionally disabled until manual verification is complete. Native-target validation now has the constructor-plus-void-method regression, the top-level `Void` free-function lowering check, the parser function-body regression, the focused string literal regression, and a successful Linux `S2->S3` rebuild all validated locally; the most important unresolved risks are the first real hosted CI executions after re-enable plus Windows and Android toolchain hardening.

## Goal

Get Seen to reliable native compilation on:

1. Linux
2. Windows
3. macOS
4. iOS
5. Android

This plan is intentionally ordered by compiler risk, toolchain complexity, and current codebase reality.

## Current State

### Already partially real

- The main compiler pipeline is LLVM-based and already supports target triple overrides in `compiler_seen/src/main_compiler.seen`.
- The codebase already contains explicit target handling for:
  - `linux-x86_64`
  - `linux-arm64`
  - `windows-x86_64`
  - `macos-x86_64`
  - `macos-arm64`
  - `ios-arm64`
- The backend target enum in `compiler_seen/src/codegen/interfaces.seen` already includes most native targets.
- The iOS path already has dedicated runtime compilation and link logic.
- Android is already assumed by scripts and examples, especially `scripts/bundle_android.sh`, and is now wired through the compiler target model plus the stage compiler's runtime and link setup.

### Still blocking native completeness

- Windows still needs more ABI-specific hardening, but the current rebuilt stage compiler now produces a real `.exe` for the smoke target instead of silently succeeding without an artifact.
- Windows link defaults are now less Unix-biased and no longer force a Winsock dependency, but the exact MinGW system-library set still needs toolchain validation.
- Android now depends on a real NDK/sysroot instead of implicit host fallback, and still needs end-to-end validation on an installed toolchain.
- Linux ARM64 cross-compilation now requires an explicit ARM64 sysroot on non-ARM64 hosts instead of falling through host glibc headers; the compiler probes `SEEN_LINUX_ARM64_SYSROOT` and common system paths before attempting the runtime build.
- Auxiliary runtimes are now target-aware, but `seen_region.c` is intentionally capability-gated because it is not yet portable across the full native target matrix.
- Cross-target release builds now avoid the host-native merged `llc` path, but still need validation under real Windows and Android toolchains.
- A first compile-only smoke harness now exists for the native target matrix, and disabled workflow definitions for Ubuntu and macOS CI lanes are checked in for later re-enable once manual verification is complete.
- The existing platform matrix now surfaces real smoke status for the native targets instead of placeholder JSON, and the repository has preserved Apple and Android CI workflow definitions ready to restore later; remaining risk is in real hosted-run stability once those workflows are intentionally turned back on.
- Cross-target GPU runtime compilation is now attempted per target, but still needs validation on real Android and Windows toolchains.
- Cache reuse is now namespaced by effective target and compile mode in the stage compiler, but that isolation still needs broader validation across release, sanitizer, and profile combinations.
- The current parser-side return-type handoff fix plus the later float-promotion registry fix now let `scripts/safe_rebuild.sh` complete a full Linux `S2->S3` rebuild and promote Stage3 on this host, though broader host coverage and continued bootstrap validation are still warranted.
- WASM still has scaffold placeholders, but that is out of scope for this native-first plan.

## Non-Goals For This Phase

- Native UI frameworks
- SwiftUI / Compose / AppKit / UIKit code generation
- Full cross-platform packaging UX
- Fixing every placeholder in unrelated ML, parser shim, or experimental code
- Replacing the minimal core typechecker

The first milestone is native code generation and linking, not productized app framework support.

## Strategy

Sequence targets by shortest path to stable compiler support:

1. Linux as the baseline reference path
2. macOS to validate Apple-targeted cross compilation without mobile constraints
3. Windows to close the non-Unix linker gap
4. iOS to harden the existing partial mobile path
5. Android to add the missing mobile target end to end

This order is deliberate:

- Linux is already the strongest path and should remain the correctness anchor.
- macOS and iOS share Apple toolchain assumptions.
- Windows requires explicit linker and runtime flag handling that the current code skips.
- Android requires new target modeling plus NDK-aware runtime and link steps.

## Phase 1: Normalize Native Target Modeling

### Objective

Make the compiler's declared native targets match the targets actually supported by the stage compiler.

### Work

1. Extend `Target` in `compiler_seen/src/codegen/interfaces.seen` with Android target entries.
2. Add CLI parsing aliases for:
   - `android-arm64`
   - `aarch64-linux-android`
3. Update user-facing target display strings in `compiler_seen/src/main.seen`.
4. Update default output naming rules:
   - executable targets: no extension on Unix-like systems by default
   - Windows executable target: `.exe`
   - Android native library target: `.so`
   - iOS build artifact stays target-driven and packaging-specific
5. Ensure the stage compiler accepts these targets through `--target=` in `compiler_seen/src/main_compiler.seen`.

### Deliverable

The compiler can parse and route all five native targets without falling back to host defaults.

## Phase 2: Make Runtime Compilation Target-Aware

### Objective

Compile `seen_runtime.c`, `seen_region.c`, and optional GPU runtime objects with the requested target, not the host machine.

### Work

1. Refactor runtime object compilation in `compiler_seen/src/main_compiler.seen` so target flags are derived once and reused.
2. Introduce target-specific compile argument builders for:
   - Linux x86_64
   - Linux ARM64
   - Windows x86_64
   - macOS x86_64
   - macOS ARM64
   - iOS ARM64
   - Android ARM64
3. For Apple targets:
   - continue using `xcrun --show-sdk-path`
   - separate macOS SDK and iPhoneOS SDK selection
4. For Android:
   - require `ANDROID_NDK_HOME`
   - derive the LLVM toolchain path from the NDK
   - compile runtime with `--target=aarch64-linux-android<api>` and `--sysroot`
5. For Windows:
   - choose one supported ABI explicitly
   - recommended first target: `x86_64-w64-windows-gnu`
   - only keep `x86_64-pc-windows-msvc` if an actual MSVC-hosted build path is implemented and testable

### Recommended decision

Switch the first Windows implementation target from MSVC triple naming to MinGW-compatible GNU Windows linking unless there is a proven MSVC environment already available in CI. The current code uses `x86_64-pc-windows-msvc` strings but does not implement the corresponding environment assumptions.

### Deliverable

All runtime objects are compiled with the same target ABI as the generated LLVM objects.

## Phase 3: Make Linking Target-Aware

### Objective

Stop using host-OS heuristics as the main driver for link behavior.

### Work

1. Replace the current Darwin-versus-Linux split in `compiler_seen/src/main_compiler.seen` with target-based branching.
2. Add explicit link command builders per target family:
   - Linux
   - Windows GNU or Windows MSVC
   - macOS
   - iOS
   - Android
3. For Linux:
   - preserve current LLD ThinLTO flow
4. For macOS:
   - preserve current ld64-oriented flow
5. For iOS:
   - keep SDK-based link path and then validate static or executable outputs separately
6. For Android:
   - use the NDK clang driver with target triple and sysroot
   - emit `.so` for JNI-ready output first
7. For Windows:
   - if GNU path: use `clang --target=x86_64-w64-windows-gnu`
   - if MSVC path: use `clang-cl` or `lld-link` with proper CRT and SDK discovery
8. Move target-specific libraries out of host-only assumptions such as unconditional `-lpthread`.

### Deliverable

Link commands become deterministic for the requested target and do not rely on the host OS matching the target OS.

## Phase 4: Remove Native Toolchain Surface Stubs

### Objective

Eliminate the stubs that directly block native compilation infrastructure.

### Must-remove stubs

1. `Linker.link()` in `compiler_seen/src/codegen/interfaces.seen`
2. `Linker.linkStatic()` in `compiler_seen/src/codegen/interfaces.seen`
3. `Linker.linkDynamic()` in `compiler_seen/src/codegen/interfaces.seen`

### Implementation approach

These do not need to become a full alternative compiler path. They just need to stop being hardcoded failures.

Implement them as thin wrappers over target-specific clang or linker invocations:

- `link()` for normal native executable linking
- `linkStatic()` for archive or mostly-static linking where supported
- `linkDynamic()` for `.so`, `.dylib`, or `.dll` style outputs

Even a pragmatic shell-command-based implementation is better than returning `false` unconditionally.

### Not required in this phase

- Replacing every placeholder comment elsewhere in the repo
- WASM placeholder removal
- Experimental ML placeholders

### Deliverable

No native-link helper used by the compiler surface should still be a hardcoded stub.

## Phase 5: Platform Validation Matrix

### Objective

Turn target support from nominal to verified.

### Minimum validation for each platform

1. Compile a hello-world Seen program
2. Compile a program using strings and arrays
3. Compile a program using the C runtime
4. Compile a multi-module program

### Platform-specific validation

#### Linux

- x86_64 host-native
- arm64 cross-compile on Linux host if toolchain exists

#### macOS

- x86_64 output
- arm64 output
- validate host and cross scenarios separately if possible

#### Windows

- produce a working `.exe`
- validate basic runtime I/O and string ABI

#### iOS

- produce a linkable ARM64 artifact
- confirm packaging path via existing `ipa` flow or document the remaining packaging step

#### Android

- produce a working `libapp.so`
- validate `scripts/bundle_android.sh`
- confirm example projects using `aarch64-linux-android` build successfully

### Deliverable

A CI-readable target matrix with pass or fail status per platform.

Current implementation: `scripts/native_target_smoke.sh` provides the compile-only native smoke layer, and `scripts/platform_matrix.sh` now consumes it for the native target matrix reports.

## Phase 6: CI And Release Integration

### Objective

Keep native support from regressing.

### Work

1. Add a compiler-target smoke-test matrix for:
   - Linux x86_64
   - Linux ARM64 cross
   - Windows x86_64 cross
   - macOS x86_64 cross
   - macOS ARM64 cross
   - iOS ARM64 cross
   - Android ARM64 cross
2. Separate compile-only target tests from run-on-host tests.
3. Add artifact inspection checks:
   - file format
   - target triple where applicable
   - extension or bundle type
4. Keep bootstrap verification separate from target-matrix verification.

### Deliverable

Native target support becomes part of the normal release gate instead of a one-off effort.

## Recommended Implementation Order

### Milestone A

Linux, macOS, iOS cleanup:

1. Refactor target parsing and naming
2. Refactor runtime compile flags per target
3. Refactor link command creation per target
4. Keep Linux and Apple green

### Milestone B

Windows:

1. Choose GNU vs MSVC as the first-class path
2. Implement real target triple, runtime compile, and link rules
3. Validate `.exe` generation

### Milestone C

Android:

1. Add Android target enum and parser aliases
2. Add NDK-aware runtime and link flow
3. Validate `.so` output
4. Validate `scripts/bundle_android.sh`

### Milestone D

Remove native helper stubs and add CI coverage.

## Concrete File-Level Worklist

### `compiler_seen/src/codegen/interfaces.seen`

- Add Android target enum entry or entries
- Add parser aliases for Android target strings
- Update target triple and data layout mapping
- Implement `Linker.link()`
- Implement `Linker.linkStatic()`
- Implement `Linker.linkDynamic()`

### `compiler_seen/src/main.seen`

- Add Android target text in CLI help
- Update target-to-string conversion
- Update default output naming for Windows and Android

### `compiler_seen/src/main_compiler.seen`

- Add Android cross-target triple override
- Add Windows cross-target triple override in the real path
- Refactor runtime compilation to be target-aware
- Refactor region and GPU runtime compilation to be target-aware
- Refactor link command construction to branch on target, not host
- Add Android linker flow using NDK clang

### `scripts/bundle_android.sh`

- Keep as-is initially, but align with the compiler’s actual accepted target string set
- Remove any target spelling drift after compiler changes

## Success Criteria

We are done with this phase when all of the following are true:

1. `seen build` can target Linux, Windows, macOS, iOS, and Android using the real LLVM path.
2. Runtime objects are compiled for the selected target, not the host.
3. The stage compiler no longer relies on Unix-only host heuristics for Windows or Android.
4. The generic native linker helpers are no longer hardcoded failure stubs.
5. Android examples and packaging scripts use a compiler-supported target string.
6. Each native target has at least one compile smoke test.

## First Patch Set To Land

The first patch set should be limited to this:

1. Add Android target parsing and naming
2. Make `main_compiler.seen` recognize Android and Windows explicitly in cross-target selection
3. Refactor runtime compile flags so they are target-derived
4. Refactor link command creation so it is target-derived
5. Implement non-failing `Linker` helper methods

That patch set gets the repo from nominal target declarations to actual native-target plumbing. Everything after that is hardening, validation, and packaging.