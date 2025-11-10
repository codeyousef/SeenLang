# Seen Language — Unified **MVP** Plan (Multi‑Platform Updated)

This replaces previous MVP notes. It merges **Pre‑Bootstrap (PB)**, **Pre‑Self‑Host (PSH)**, and essential **Post‑Self‑Host (POST‑for‑MVP)** items so that the **engine + game** compile and run on **Linux, Windows, macOS, Android, iOS, and Web (JS/WASM)**.

---

## 1) Current Progress Snapshot
- Lexer/Parser ✅
- Type system (HM inference, traits, monomorphization, sealed classes) ✅
- Memory model (regions, RAII, generational refs, deterministic drop) ✅
- FFI/ABI (`extern "C"`, `repr(C)`, unions, align/pack, stable symbols) ✅
- Codegen (LLVM + deterministic IR emission) ✅
- LSP (hover, goto‑def, diagnostics, format, refs) ✅
- Tooling/CLI (`build/test/bench/fmt`, target triples, `--deterministic`) ✅
- Self‑hosting (Stage0→Stage1→Stage2 deterministic) ✅

> **Delta from earlier plans:** This MVP now **includes multi‑platform bring‑up** for minimal runnable samples on all targets.

---

## 2) Phase PB — Pre‑Bootstrap (In Progress)
Pre‑bootstrap should make the Rust toolchain a stable foundation before we attempt Stage‑1. These items were previously marked complete but are still missing. Break them down and check them off as we implement them:

- [x] **Unicode NFC + visibility policy**
  - Normalize identifiers/literals to NFC during lexing.
  - Support `Seen.toml` switches for `caps`/`explicit` visibility and error when source disagrees.
- [x] **Result/Abort error model**
  - Wire a consistent `Result<T, E>` type across compiler crates.
  - Add an `abort` intrinsic for unrecoverable failures and ensure diagnostics surface it.
- [x] **Operator precedence & formatter lock**
  - Freeze word/operator precedence tables in the parser.
  - Extend formatter/pretty-printer so it enforces the frozen precedence (no drift across runs).
- [x] **RAII `defer` + generational refs runtime**
  - ✅ Interpreter defer stack + scope unwinding complete (`seen_interpreter` + tests).
  - ✅ Task runtime now uses generational handles with stale-handle tests in `seen_concurrency`.
  - ✅ Channel handles: generational IDs + validation in runtime/interpreter.
  - ✅ Actor handles: generational IDs and stale-handle detection integrated with actor system.
  - ✅ LLVM backend/runtime parity so compiled stages enforce the same invariants.
- [x] **Deterministic IR emission**
  - IR generator/optimizer now emit sorted structures; regression tests hash IR display output deterministically.
- [x] **Runtime split**
  - Shared compiler surface exposed via new `seen_core` crate; CLI consumes `seen_core` instead of wiring per-crate
    deps.
- [x] **CLI determinism profile**
  - `--profile deterministic` pins timestamps/temp roots via env, documented in quickstart + MVP plan.
- [x] **Performance baselines & tooling**
  - ✅ `perf_baseline` harness (`tools/perf_baseline`) and default suite (`scripts/perf_baseline.toml`) collect runtime, peak RSS, binary sizes, and compile timings; `docs/performance-baseline.md` documents usage and `scripts/perf_baseline_report.json` seeds the baseline dataset.
  - ✅ CI now runs the baseline suite on every push/PR via the `Performance Baseline` workflow job, caching cargo artifacts, invoking the harness with `--baseline`, and uploading `target/perf/latest.json`; failures occur when mean runtimes regress past configured thresholds.
  - ✅ Initial Rust/C++ parity dashboard (`docs/performance-dashboard.md`) renders the seeded baseline report into a table so future C++ targets can be compared side-by-side.

## 3) Phase PSH — Pre‑Self‑Host (WIP)

Goal: Complete all components needed to compile the compiler with itself and produce identical Stage2/Stage3 binaries.

### PSH-1. Typestates & Phantom Types

*Status:* ✅ Completed — Phantom generics flow through the parser/typechecker, and sealed traits reject external
extensions.

* **Inputs:** type system and trait engine.
* **Outputs:** phantom parameters for state modeling and sealed traits to prevent invalid transitions (used later in Vulkan wrappers).
* **Acceptance:** Creating illegal transitions between states is rejected at compile time.

### PSH‑2. Async & Structured Concurrency

*Status:* ✅ Completed — `scope { ... }` now governs spawned tasks, non-detached spawns outside a scope produce compile
errors, and the interpreter/runtime automatically joins scoped work. A `cancel taskHandle` primitive delegates to the
async runtime.

* **Highlights:**
  - Parser/AST recognize `scope` blocks, `spawn detached`, and `cancel` expressions.
  - Type checker tracks scope depth, enforces that tasks are either scoped or detached, and wires `await`/`cancel`
    typing.
  - Interpreter/runtime push/pop scoped task frames, wait on completion, and expose cancellation by forwarding to the
    async runtime.
  - New unit coverage validates type errors and runtime defer-order behaviour.

* **Acceptance:** Compiler emits diagnostics for illegal non-scoped spawns; scoped tasks join deterministically at
  runtime (additional borrow/suspend analysis can build on this foundation).

### PSH‑3. Minimal Channels & Job System

*Status:* ⏳ In validation — channel futures now run on the shared async runtime with fair, waker-driven `select`
outcomes, and interpreter coverage exercises scoped jobs plus multi-stage pipelines. CLI/LLVM wiring remains the final
gap before we can call the phase entirely closed.

* **Progress this iteration:**
  - Refactored `ChannelManager` to sit atop the generational channel handles and expose a `channel_select_future`
    wrapper so every consumer (interpreter, CLI, Stage builds) shares the same async machinery.
  - Replaced the interpreter's polling `select` with the new future, binding patterns deterministically and bubbling
    send/receive outcomes through structured results.
  - Added regression coverage for scoped jobs waiting on channel-driven tasks and for multi-stage pipelines that route
    values through multiple futures before completion.
  - Promoted a first-class `Channel()` constructor that returns `Sender`/`Receiver` endpoints, including optional
    capacity support, plus type-checker and documentation updates so user code can adopt the new surface immediately.
  - Documented structured concurrency patterns (`docs/concurrency-patterns.md`) so onboarding material reflects the
    runtime contract, and linked it from the quickstart guide.

* **Remaining tasks:**
  1. ✅ **IR support for channel constructs** — `seen_ir` now models `scope`, `jobs_scope`, `spawn`, and `select` with
     dedicated instruction variants so deterministic backends can consume channel-heavy programs.
  2. ✅ **LLVM channel runtime surface (stubs)** — the backend injects placeholder `seen_channel_*`/`seen_spawn`/scope
     helpers so Stage builds link while real runtime shims are developed.
  3. ✅ **LLVM lowering for channel intrinsics** — translate the new IR instructions into calls that coordinate with the
     runtime surface (channel creation, send/receive/select, scope joins, task handles). This work must preserve the
     interpreter’s semantics, especially for pattern binding within `select` arms.
      * ✅ Scope and spawn IR now lower to runtime-friendly call sequences (`seen_ir/src/generator.rs`,
        `seen_ir/src/llvm_backend.rs`) so LLVM no longer errors on these constructs.
      * 🔄 *New breakdown for select support*:
          - [x] Reshape `Instruction::ChannelSelect` so it returns the selected case index, payload value, and status
            fields instead of an opaque result.
          - [x] Teach the IR generator to expand `select` expressions into explicit control-flow blocks that bind
            patterns, run handlers, and funnel results through a dedicated SSA register.
          - [x] Extend the LLVM backend to recognize the new instruction form and emit the corresponding runtime calls (
            initially backed by the existing stubs for Linux until the shared runtime lands).
  4. ✅ **Runtime implementation & linking** — replace the stubs with actual channel/task support by sharing or
     reimplementing `seen_concurrency` pieces, ensure the compiled artifact links the runtime on every platform, and
     propagate handles/results back into Seen values.
      * ⬜ Subtasks to stage this work:
          - [x] Extract a minimal `seen_runtime` crate that exposes the channel/task ABI over `@[no_mangle] extern "C"`
            shims.
          - [x] Link Linux LLVM builds against the new runtime so channel send/recv/select no longer rely on stubs (
            `seen_cli/src/main.rs`, `seen_ir/src/llvm_backend.rs`).

      - [x] Add host/Android/wasm build scripts so `seen_cli` can bundle the runtime archive per target triple
          (`scripts/build_seen_runtime.sh` invokes `cargo build -p seen_runtime` for the requested triples and stages
          the resulting `libseen_runtime.a` under `target/seen-runtime/<triple>/`).
        - [x] Implement value boxing helpers so LLVM lowering converts primitive payloads into heap-backed runtime
          values before calling `seen_channel_send`, plus exported runtime helpers (`seen_box_*`, `seen_unbox_*`) for
          future unboxing work (`seen_ir/src/llvm_backend.rs`, `seen_runtime/src/lib.rs`).
        - [x] Surface real scope/spawn/await entry points in `seen_runtime` (`__scope_push`, `__spawn_task`,
          `__task_handle_new`, `__await`) and teach the LLVM backend to call them directly so handle allocation +
          scope bookkeeping lives in the runtime instead of emit-time stubs (`seen_runtime/src/lib.rs`,
          `seen_ir/src/llvm_backend.rs`).
  5. ✅ **CLI + Stage wiring & regression coverage** — once lowering/runtime integration is complete, update `seen_cli`
     and the self-host pipeline to run channel-driven programs via the LLVM backend, add CLI/Stage tests, and record
     determinism hashes plus stdout/stderr validation.
      * [x] Add LLVM smoke tests for `seen_cli run tests/fixtures/channel_select.seen` on Linux to guard regressions
        (`seen_cli/tests/channel_select.rs` now runs the fixture via `seen run --backend llvm`).
     * [x] Extend Stage1 deterministic suites so channel traffic is exercised during bootstrap (hash + stdout checks) —
       `scripts/self_host_llvm.sh` now runs the `channel_select` fixture through Stage1/Stage2 with `--backend llvm`
       and diffs stdout to ensure parity.

* **Acceptance:** Channel send/receive semantics verified; jobs in a scope join before exit; CLI/Stage binaries observe
channel traffic with the same guarantees as the interpreter.

### PSH‑3a. Optimization & Auto‑Tuning Pipeline

*Status:* ⏳ Pending — research-backed optimizer workstreams need foundations before self-hosting can lock performance.

* **Highlights:**
    1. ✅ Integrate an equality-saturation pass (e-graph rewrite set inspired by `egg` / DialEgg) over Seen IR so
       algebraic simplifications, strength reductions, and fusion emerge deterministically (docs/research/13 - Language
       Performance.md).
        * Added a lightweight `egg`-powered rewrite pass (`seen_ir/src/optimizer/egraph.rs`) that canonicalizes
          arithmetic (`+0`, `*1`, commutativity, etc.) and plugs into the `seen_ir` optimizer when `-O2`/`-O3` is
          requested; CLI determinism docs now note the pass.
  2. ✅ ML-driven heuristics now gate inlining and register-allocation pressure: `seen_ir/src/optimizer/ml.rs` ingests
     PB-Perf features, supports JSON weight files via `SEEN_ML_HEURISTICS`, and emits inline hints plus per-function
     register budgets. The LLVM backend honors `InlineHint` by setting `alwaysinline`/`noinline`, while high-pressure
     functions are rewritten through a register-reuse pass with new coverage in `optimizer::tests`.
  3. ✅ Hot-loop LENS superoptimizer rewrites linear instruction chains in loop blocks (add/sub, mul, shl).
     `IROptimizer::superoptimize_loop_chains` collapses temporary registers, and new tests (`lens_superoptimizer_*`)
     ensure the fused IR matches expectations while preserving determinism.
  4. ✅ ML-guided PGO loop preserves training corpora: setting `SEEN_ML_DECISION_LOG` + `SEEN_ML_REWARD` records every
     heuristic decision (features + reward) as NDJSON, and `SEEN_ML_DECISION_REPLAY` replays curated hints during
     deterministic builds. The logger/replay plumbing lives in `seen_ir/src/optimizer/ml.rs` and is exercised by fresh
     unit tests.

* **Acceptance:** Optimizer reports show e-graph rewrites firing on targeted fixtures, ML heuristics reduce binary size ≥3% on benchmark suite without destabilizing determinism hashes, and superoptimized traces land in CI-perf dashboards.

### PSH‑3b. Memory & Data Layout Efficiency

*Status:* 🔄 In progress — hybrid generational handles now ship in `seen_memory_manager`, strategy hints are wired through syntax/analysis, and remaining work focuses on arena flattening plus safety-check gating.

* **Outstanding tasks:**
  1. ✅ Extend the region/arena runtime with Vale-style hybrid generational handles plus validation benches proving no additional runtime checks are emitted on hot paths (`seen_memory_manager/src/handles.rs`, `seen_memory_manager/benches/hybrid_handles.rs`).
  2. ✅ Surface region strategy hints (`bump`, `stack`, `cxl_near`) in Seen syntax and teach the compiler to auto-select O(1) release strategies when lifetime analysis allows it (`seen_parser/src/ast.rs`, `seen_parser/src/parser.rs`, `seen_memory_manager/src/regions.rs`, `docs/spec/regions.md`).
  3. ✅ Continue flattening compiler data structures (AST arenas, IR graphs) to use 32-bit indices and cache-oblivious
     layouts—`seen_ir/src/arena.rs` now provides a shared 32-bit arena that powers IR programs, modules, call graphs,
     and CFG blocks (`ArenaIndex` replaces `usize` handles throughout `seen_ir/src/lib.rs`, `module.rs`, `function.rs`,
     and `instruction.rs`). Modules, globals, and blocks are packed contiguously to improve cache warmth, while their
     lookups are backed by compact `ArenaIndex` maps. Remaining HashMaps are limited to metadata/export symbol tables
     where string keys are required; rationale captured in docs/research/13 - Language Performance.md.
  4. Audit runtime safety checks, gating them behind debug profiles when static proofs exist, so production binaries keep the "zero memory safety overhead" promise.
      * ✅ Duplicate-allocation detection for region handles now runs only in debug/profile builds; release binaries
        elide the scan while still preserving analysis correctness (`seen_memory_manager/src/regions.rs`).

* **Acceptance:** Memory-intensive benchmarks report ≥1.5× throughput improvement, region drops are O(1) in profiler traces, and cache miss rates fall in line with the Cornell flattening targets.

### PSH‑3c. Backend Diversification & MLIR Bridge

*Status:* ⏳ Pending — LLVM remains the sole backend; research recommends MLIR/differentiated pipelines to unlock performance headroom.

* **Outstanding tasks:**
    1. ✅ Prototype an MLIR emission path (core dialect + Transform + DialEgg integration) and validate parity with the
       existing deterministic IR dumps (docs/research/13 - Language Performance.md). `seen_mlir/src/lib.rs` now wraps
       every emission in `module attributes { dialects = #mlir.dialect_array<...> }` and appends
       `transform.module @seen_pipeline` with a default `builtin.pipeline(canonicalize,cse)` so DialEgg/Transform passes
       can consume the output directly. `seen_cli --backend mlir` writes the new structure and the determinism command
       hashes it to keep Stage workflows honest.
    2. ✅ Bring up alternative codegen backends (Cranelift with ISLE patterns, Tilde sea-of-nodes) behind `--backend`
       switches for fast-compile and experimentation lanes. A new `seen_cranelift` crate converts IR into deterministic
       textual CLIF (`seen_cranelift/src/lib.rs`) and `seen_cli --backend clif` exposes it for fast iteration.
  3. ✅ Ensure backend selection is deterministic (same hash outputs) and CI exercises Stage0→Stage2 via at least one
     non-LLVM backend each night. `seen determinism` now supports `--backend mlir` and `--backend clif`, the CLI tests
     assert both paths, and `scripts/nightly_backends.sh` runs those hash comparisons so nightly automation can gate on
     non-LLVM regressions.

* **Acceptance:** Stage1 builds succeed with MLIR and Cranelift prototypes, deterministic hashes stay stable across backends, and compile-time telemetry matches the “10× faster than LLVM” research targets for fast lanes.

### PSH‑3d. Runtime Scheduling & Concurrency Efficiency

*Status:* ✅ Completed — scope-bound coroutine frames now live on structured stacks, the scheduler exposes
fairness/backoff telemetry, and PB-Perf can alert on wake-latency regressions.

* **Highlights:**
    1. The async runtime gained `TaskSpawnOptions` + `CoroutineFrameHints`, allowing escape analysis to request
       stack-bound frames. Scoped arenas recycle those frames without heap churn, and new unit tests cover stack scopes,
       reuse, and deterministic heap fallbacks.
    2. `TaskScheduler` now records per-priority dispatch counts, queue promotions, idle polls, and cooperative
       backoff/yield events. High/normal/low tasks meter fairness, and repeated idle polls trigger deterministic
       `yield_now()` to avoid hot spinning.
    3. Every dispatch records wake latency and flags starvation (`>5 ms` by default). The aggregated counters (
       `runtime.metrics_snapshot().scheduler.*`) feed PB-Perf dashboards so nightly perf baselines can detect queue
       contention spikes or latent starvation.

* **Acceptance:** `cargo test -p seen_concurrency` exercises the stack allocator, scheduler backoff, and starvation
  detections; `runtime.metrics_snapshot()` surfaces the frame + scheduler snapshots consumed by `tools/perf_baseline`,
  and PB-Perf alerts stay green when starvation events remain at 0 on the baseline workloads.

### PSH‑3e. Hardware-Aware Codegen & Memory Topology

*Status:* ✅ Completed — hardware metadata now influences every backend plus the CLI/runtime guards that keep
deterministic runs portable.

* **Highlights:**
    1. ✅ `TargetOptions` continue to record structured overrides (`IntelApx`, `Avx10(width)`, `ArmSve(vector)`) while
       the LLVM backend now stamps each function with vector width, register budget, scheduler, and translated
       `target-features` attributes so LLVM's own register allocator and scheduler react to the requested ISA mix.
  2. ✅ Memory topology hints flow through the CLI via `--memory-topology {cxl-near,cxl-far}`, configure the
     memory manager/region analyzer, and generate PB-Perf summaries that distinguish host vs. near vs. far CXL
     regions. Small/stack-friendly regions stay near compute while bump-heavy regions spill to far CXL, and
     deterministic
     builds refuse topology overrides in both `seen build` and `seen run`.
    3. ✅ The CLI accepts repeatable `--cpu-feature ...` flags (APX, AVX10-256/512, SVE 128/256/512), threads them into
       the IR hardware profile, and downstream MLIR/Cranelift emitters now tag every function with per-function vector,
       scheduler, and register hints while the Cranelift textual output reorders blocks based on those scheduling
       classes. LLVM builds pick up the same hints via native attributes and the CLI regression suite verifies the
       emitted MLIR/CLIF artifacts.

* **Acceptance:** Hardware-feature smoke tests validate emitted binaries on APX-capable x86 and SVE ARM targets,
  vector reports enumerate chosen widths, per-function scheduler hints appear in LLVM/MLIR/CLIF output, and CXL
  placement improves memory-bound fixture throughput.

### PSH‑4. Embedding & Packaging

*Status:* ✅ Completed — embed attributes flow through the compiler in deterministic mode and the CLI ships shared/static
packaging with documentation coverage.

* **Outstanding tasks:**
    1. ✅ Ensure embedded assets survive Stage1→Stage2 compiles without breaking determinism (new tests + fixtures).
        * Added deterministic LLVM artifact coverage in `seen_cli/tests/embed_determinism.rs` to ensure repeated builds
          embed identical payloads under `--profile deterministic`.
    2. ✅ Update documentation/quickstart with packaging instructions and add integration tests for `.so`, `.dll`,
       `.dylib`,
     `.a` generation.
        * Quickstart now lists shared/static build commands plus cross-target (`.dll`, `.dylib`) guidance, and
          Linux-focused CLI tests verify `.so`/`.a` outputs build successfully under LLVM.

* **Acceptance:** Assets embed correctly and appear in deterministic object sections; all library types build successfully.

### PSH‑5. Multi‑Platform Target Bring‑Up

*Status:* 🔄 In progress — CLI now accepts `--target <triple>` and wires LLVM toolchains/clangers; Linux/Web flows are
live while macOS/Windows remain deferred until host machines are available.

* **Outstanding tasks:**
  1. ✅ Add `--target <triple>` support to the CLI and map to LLVM target machines/toolchains (clang, wasm-ld, NDK).
    * `seen build` now forwards triples to LLVM, emits target-specific objects, and selects appropriate
      linkers/archivers (with `clang`/`wasm-ld` fallbacks and `SEEN_LLVM_*` overrides).

    2. ✅ Create platform-specific linker pipelines for Linux (ELF exe/so), WebAssembly (wasm-ld), and Android NDK `.so`
     packaging; queue macOS/Windows code paths once non-Linux builders are provisioned.
    * Linux executables/shared libs now default to platform extensions; wasm targets drive `wasm-ld` with deterministic
      exports, optional JS/HTML loader generation, optional `--bundle` archives, and fail-fast diagnostics when
      `wasm-ld` is missing; Android triples
      resolve dedicated NDK toolchains via `ANDROID_NDK_HOME` / `ANDROID_API_LEVEL`. Android bundling emits production
      `.aab` layouts (manifest, assets, res, root, dex, optional resources.pb), injects a deterministic stub
      `classes.dex` when a project omits one, and offers keystore-driven signing via `jarsigner`.
    * The CLI automatically switches Android builds to shared-library mode when no explicit `--shared/--static` flag is
      provided and surfaces actionable errors if `ANDROID_NDK_HOME` is missing. The helper script (
      `scripts/bundle_android.sh`) mirrors the richer bundling flow (ABI overrides, stub dex, signing env vars) for
      automation hooks.

    3. ✅ Provide sample projects per Linux/Web/Android target (textured quad) and automated smoke tests that run in
     CI/device farms.
    * Added `examples/linux/hello_cli`, `examples/web/hello_wasm`, and `examples/android/hello_ndk` as starter
      fixtures, plus CLI regression tests covering Linux IR output, wasm emit/bundle flows, and Android env validation.
      The Android example now bundles manifest/assets/res/root/dex fixtures and unit tests assert the richer bundle
      contents, while new CLI smoke tests (`seen_cli/tests/linux_sample.rs`) run the Linux sample through the
      interpreter
      backend to mimic CI/device execution.

    4. ✅ Document Linux/Web/Android toolchain prerequisites (clang/LLD, wasm-ld, Android SDK/NDK) and integrate
     signing/provisioning scripts where applicable.

    * Quickstart now lists wasm/Android dependencies, the `--wasm-loader` flag, `--bundle` flows for wasm/android,
      required env (`ANDROID_NDK_HOME`) and optional signing knobs (`SEEN_ANDROID_*`), plus references to the updated
      Android bundle script.

  * Linux: ELF executables and shared libs (active work with CLI defaults).
  * Windows: PE/COFF executables and DLLs _(deferred until Windows hosts available)_.
  * macOS: Universal2 dylibs and app bundles (codesigned) _(deferred until macOS hosts available)_.
  * Android: `.so` + `.aab` via NDK.
  * iOS: `.framework` + `.ipa` _(deferred)_.
  * Web: `.wasm` + JS loader (COOP/COEP headers).
* **Acceptance:** Each active target (Linux/Web/Android) produces a build artifact; execution of simple samples (
  textured quad) succeeds with no validation errors. Deferred platforms are tracked separately once host infrastructure
  exists.

### PSH‑6. Graphics Backends & Shader Flow

*Status:* ✅ Completed — `seen_shaders` now anchors deterministic shader validation/transpilation flows and the CLI
exposes a dedicated entry point for asset pipelines.

* **Highlights:**
    1. ✅ New `seen_shaders` crate loads SPIR-V via `naga`, validates modules, records entry-point stages, and emits
       WGSL/MSL outputs alongside optional `.spv` copies with deterministic naming. Errors bubble up with file context
       so
       invalid payloads cite the failing shader and stage summary.
    2. ✅ `seen shaders ...` traverses individual files or directories (with `--recursive`), supports `--target`
       selections, and offers `--validate-only` for CI smoke tests. Entry-point stages are summarized per input so
       Vulkan
       (vertex/fragment/compute) coverage is easy to audit from logs.
    3. ✅ Added `examples/shaders/triangle.spv` as the canonical sample plus CLI regressions in
       `seen_cli/tests/shaders.rs` that cover single-file conversion, validation-only mode, and recursive directory
       handling.

* **Acceptance:** Invalid shaders now surface actionable CLI errors naming the `.spv` path and entry-point stage, while
  valid inputs deterministically emit WGSL and Metal outputs ready for downstream Vulkan/WebGPU tooling.

### PSH‑7. SIMD Baseline

*Status:* ✅ Completed — SIMD controls now span the CLI, optimizer, and regression reports with deterministic policy
guardrails.

* **Highlights:**
    1. ✅ `IRType::Vector` and `Instruction::SimdSplat`/`SimdReduceAdd` extend the core IR with portable vector
       semantics;
       LLVM now lowers vector types directly, MLIR/CLIF emit deterministic ops, and new unit coverage ensures splat +
       reduction paths stay stable.
  2. ✅ `seen_shaders` + `seen shaders ...` keep WGSL/MSL metadata aligned with shader entry points.
  3. ✅ CLI flags (`--simd=off|auto|max`, `--target-cpu`, `--simd-report`) thread policies through the hardware profile;
     deterministic builds coerce scalar mode, and `--simd-report` emits per-function JSON summaries (policy, mode,
     reason, ops, estimated speedup).
  4. ✅ `IROptimizer` now records SIMD metadata for every function, using hardware-aware heuristics (loop detection,
     arithmetic density, register pressure) so LLVM/MLIR/CLIF backends receive consistent annotations.
  5. ✅ Regression coverage spans optimizer unit tests (auto-vectorization vs. forced scalar) plus a CLI integration
     test that compares scalar vs. forced-vector runs via the public report.

* **Acceptance:** Build logs include per-function SIMD decisions; deterministic mode disables SIMD; scalar and vector
  results match; CLI reports capture the recorded policy/mode/reason for each optimized function.

### PSH-8. Deterministic Self-Host

*Status:* ✅ Linux complete — LLVM backend fixes and Stage‑1 tooling produce matching Stage2/Stage3 hashes on Linux (via
`llc` + `cc`); macOS/Windows CI verification queued.

* **Inputs:** all previous subsystems.
* **Outputs:** successful Stage0→Stage1→Stage2→Stage3 build pipeline with identical Stage2/Stage3 hashes.
* **Acceptance:** Reproducible hashes verified on Linux and macOS; reproducibility confirmed on Windows CI.

---

### PSH-9. Production Self-Host Pipeline

*Status:* 🔄 In progress — Stage-1 bootstraps still rely on the Rust CLI via temp file shims instead of the Seen compiler
modules, preventing a true Seen-only pipeline.

* **Outstanding tasks:**
    1. ✅ Keep the temp-file bootstrap shim isolated (now writing under `compiler_seen/stage_cache` and exporting
       `SEEN_ENABLE_MANIFEST_MODULES` so the CLI sees `Seen.toml`), preserving shell-out logic until the Seen-native
       pipeline replaces it.
    2. ✅ Extend the CLI/bootstrap loader to bundle every `.seen` module declared in `Seen.toml` (compiler library +
       runtime) deterministically so Stage-1 compiles the full module graph instead of a single file.
    3. ✅ Gate manifest-module bundling behind `SEEN_ENABLE_MANIFEST_MODULES`, propagate the env flag through Stage-0
       scripts and the Stage-1 runner, and add regression coverage so bootstrap builds fail fast if the flag is
       missing. `scripts/self_host_llvm.sh` and `scripts/nightly_backends.sh` now export the variable before invoking
       `seen_cli`, the Stage-1 driver injects `SEEN_ENABLE_MANIFEST_MODULES=1` into its `seen build` subprocesses, and
       `seen_cli/tests/manifest_modules.rs` asserts that manifest entries are ignored by default but enforced whenever
       the env flag is set.
  4. ✅ Run the Seen-native frontend (lexer, parser, type checker) inside Stage-1 before delegating to backend codegen,
     surfacing diagnostics directly from the self-hosted sources. The new `bootstrap.frontend` module powers
     `run_frontend`, `compiler_seen/src/main.seen` now blocks builds when the frontend fails, and regression coverage
     lives in `compiler_seen/tests/frontend_smoke.seen` plus the CLI test `seen_cli/tests/bootstrap_frontend.rs`.
  5. ✅ Replace the temp-file shim that spawned the Rust CLI with the Seen-native compiler pipeline from
     `main_compiler.seen`, so Stage-1 now emits artifacts without ever calling `seen build`. The compile pipeline is
     exercised by `compiler_seen/tests/compile_smoke.seen`, and `seen_cli/tests/bootstrap_frontend.rs` runs both
     frontend + compile smoke tests to ensure the path stays green. A guard in `main_compiler::ExecuteCommand`
     outright refuses to run `seen`/`seen_cli` invocations (verified by
     `compiler_seen/tests/forbid_seen_shell.seen`) so regressions surface immediately.
  6. ✅ Extend the Seen-native parser (`compiler_seen/src/parser/real_parser.seen`) to accept the same import syntax
     as the Rust frontend (nested module paths plus per-symbol `as` aliases). The parser now captures structured
     import paths + alias metadata, `compiler_seen/tests/parser_import_aliases.seen` locks the behaviour, and
     Stage-1 bootstraps no longer choke on `typechecker.typechecker.{TypeChecker as RealTypeChecker}`-style imports.
     Next bootstrap runs should re-record hashes with the Seen parser fully in charge.

* **Acceptance:** Stage-1 builds run entirely in Seen, module bundling is deterministic, and the bootstrap script/tests
  fail if the Rust CLI is invoked as part of self-hosting.

---

## 4) Post‑Self‑Host — MVP Finalization (Pending)

Goal: Ship a self‑hosted compiler and minimal ecosystem capable of cross‑platform and SIMD builds.

### POST‑1. Documentation Completion

*Status:* ✅ Completed — `/docs/spec` now ships split chapters (`lexical.md`, `grammar.md`, `types.md`, `regions.md`,
`errors.md`, `ffi_abi.md`, `numerics.md`) plus `index.md` cross-links, all of which track NFC tables and SIMD appendices
alongside the compiler.

* **Inputs:** finalized grammar, region rules, FFI layout, numerics, and SIMD policies.
* **Outputs:** `/docs/spec` folder containing: lexical.md, grammar.md, types.md, regions.md, errors.md, ffi_abi.md, numerics.md (with SIMD appendix).
* **Acceptance:** Each file present with deterministic tables and cross‑references; index file lists all specs.

### POST‑2. Example & Validation Set

*Status:* ✅ Completed — `examples/seen-vulkan-min` bundles the deterministic triangle driver (manifest, README, shader
asset) while `examples/seen-ecs-min` provides the ECS micro-simulation. Both ship run/build instructions for
Linux/Web/Android and are exercised via new CLI interpreter tests (`seen_cli/tests/post_examples.rs`).

* **Inputs:** examples directory.
* **Outputs:** `seen-vulkan-min` (graphics), `seen-ecs-min` (systems) projects that run unchanged across all targets.
* **Acceptance:** Both projects build and run; Vulkan validation layers and platform equivalents report zero errors.

### POST‑3. Tooling & QA

* **Inputs:** CLI and CI configuration.
* **Outputs:** `seen fmt --check`, `seen trace`, `seen pkg` (local), CI jobs verifying deterministic builds and cross‑target packaging.
    * ✅ `seen fmt --check` now validates formatting without touching files; CLI tests cover both failure and success
      paths.
  * ✅ `seen trace` prints optimized IR/control-flow graphs through the CLI so developers can inspect lowering output
    (`seen trace <file> -O1`).
  * ✅ `seen pkg <dir> [--output foo.zip]` packages project trees into deterministic zip archives (excludes missing
    directories with actionable errors); regression coverage ensures archives contain the expected files.
* **Acceptance:** Formatter detects violations deterministically; CI matrix completes without errors; reports archived per build.

### POST‑4. Final Definition of Done

* Compiler self‑hosts with deterministic equality verified.
* All platform artifacts build and run successfully.
* SIMD baseline operational with reporting.
* Documentation and examples published.
* CI matrix green on all targets.

---

**End of Clarified MVP Plan**

## 5) Phase PROD — Production Self-Hosting (Active)

Goal: turn the deterministic bootstrap into a releasable, self-hosted toolchain with repeatable artifacts, attestations,
and installers across every supported platform.

### PROD-1. Release Bootstrap Matrix & Signing

*Status:* 🔄 In progress — Matrix + manifest emission implemented; hardware signing workflow still pending.

* **Inputs:** bootstrap scripts, Stage1/2/3 outputs, release metadata.
* **Progress:** `releases/bootstrap_matrix.toml` seeds the host/backend/profile tuples, `scripts/release_bootstrap_matrix.sh`
  iterates the matrix to build Stage1→Stage3 per entry, and `tools/sign_bootstrap_artifact sign ...` now emits structured
  manifests (git commit, CLI version, per-stage SHA256/size, equality flag) plus optional Ed25519 signatures via
  `--signing-key`, producing `.sig` files beside every manifest. `sign_bootstrap_artifact verify ...` validates
  signatures against the public key, the matrix script can self-verify via `--public-key`, and it now also runs
  `abi_guard verify` / `abi_guard snapshot` so stdlib hashes are checked before release. `docs/release-playbook.md`
  documents the workflow end-to-end. Artifacts land under `artifacts/bootstrap/<entry>/` alongside `manifest.json`.
* **Outstanding:** integrate HSM/sigstore-backed signing (keys currently read from local files), wire CI/release tagging
  to require fresh manifests + signatures before publishing, and publish the public key + verification instructions as
  part of every release announcement.

### PROD-2. Self-Hosted Stdlib & ABI Freeze

*Status:* 🔄 In progress — the stdlib skeleton exists but ABI reporting and integration remain.

* **Progress:** Added a dedicated `seen_std/` package with `Seen.toml`, `Seen.lock`, and starter modules covering `core`
  (Option/Result/prelude), `collections`, `async`, and `ffi`. The package now has a README that explains the scope so
  Stage-1 can depend on `../seen_std`, and the Seen CLI entrypoint now imports the shared Option/Result helpers for
  argument parsing/validation instead of re-implementing ad-hoc logic. `tools/abi_guard` snapshots module hashes and
  updates/validates `Seen.lock`, documentation lives in `docs/release-playbook.md`, and `scripts/package_seen_std.sh`
  invokes `seen pkg` to generate deterministic `libseen_std-<version>.seenpkg` bundles (plus `.sha256` checksums) ready
  to publish alongside release artifacts.
* **Update:** Manifest-module bundling now walks dependency manifests (including `seen_std`) whenever
  `SEEN_ENABLE_MANIFEST_MODULES=1`, so Stage-1 compiles the stdlib modules directly instead of relying on ad-hoc helper
  copies. `seen pkg` enforces `Seen.lock` while emitting deterministic `libseen_std-<version>.seenpkg` bundles, giving
  CI an ABI guardrail for published stdlib artifacts.
* **Outstanding tasks:**
    1. Land `std.str` with UTF-8 helpers, builders, split/trim/search utilities, and CString adapters so CLI/runtime
       modules can drop bespoke string helpers.
    2. Deliver scalar `std.math` (constants, abs/trig/sqrt, checked/saturating/wrapping integer ops) plus follow-on
       SIMD/linalg modules aligned with the SIMD baseline.
    3. Implement allocator-backed collections (`Vec`, `String`, `HashMap`) that integrate with the region/RAII analysis
       and determinism profile.
    4. Stand up `std.io`, `std.fs`, and `std.net` (sync + async traits) so Stage-1/tooling stop shelling out to
       handwritten wrappers.
    5. Flesh out `std.concurrent` / `std.sync` (structured scopes, channels, mutex/condvar/atomics) wired into the
       runtime schedulers validated in PSH-3b/3d.
    6. Expand `std.ffi`, `std.time`, `std.env`, and `std.process` so bootstrap scripts and installers rely on shared
       primitives instead of ad-hoc glue.

### PROD-3. Installer & Updater

*Status:* 🔄 In progress — Linux installers + release automation wired; remaining platforms pending.

* **Progress:** Added `scripts/build_installers.sh` plus release-script wiring so tagged builds produce Linux DEB/RPM/AppImage installers
  (and placeholder notes for other targets) alongside the stdlib bundle; `--run-platform-matrix` ensures Linux smoke tests run post-build.
* **Outstanding:** implement Windows MSI/macOS pkg/Android AAB/iOS IPA builders and hook notarization/signing into the
  release pipeline.

### PROD-4. Observability & Crash Triage

*Status:* 🔄 In progress — crash playbook + doctor command landed; runtime instrumentation pending.

* **Progress:** Drafted `docs/crash-triage.md` describing the desired workflow and added `seen doctor [--dump-build-id]`
  so engineers can read the embedded git hash/timestamp and inspect binaries for `.note.seen.build_id` sections.
* **Update:** Linux/Android LLVM builds now inject `.note.seen.build_id` sections at link time (via `llvm-objcopy`), so
  Stage3 artifacts and Android bundles report doctor-friendly hashes. CLI `seen trace --runtime/--replay` now captures
  interpreter events (program start/end, function/method entry, and failures) into JSON traces, plus a replay printer
  and
  regression tests so Alpha’s trace/replay gate has a working baseline. Runtime traces also thread effect breadcrumbs
  (registration, handle enter/exit, handler-vs-runtime dispatch, and per-operation success/error) and crash breadcrumbs
  for parse/type/interpret failures, giving triage a full breadcrumb trail. Remaining work: publish the full triage
  playbook alongside the release notes.

### PROD-4a. Parser Hardening for Stdlib & Tooling

*Status:* 🔄 In progress — parser now accepts class/struct generics, struct literals, and `<` disambiguation, and the new
statement parser (with newline terminators) restored trailing-lambda call sites plus regression tests.

* **Completed this sprint:**
    - Class/struct definitions and literals support generics end-to-end, including `<` expression disambiguation.
    - Statement blocks now parse real statements (let/var/return/loops) with newline terminators, so stdlib control-flow
      bodies stop hijacking `{ … }` as trailing lambdas.
    - Added parser fixtures covering while-blocks with nested if/else, trailing lambdas inside statement bodies, and
      let-initializers that pass lambdas through, so regressions surface immediately.
  - `when` expressions are wired through the parser and dedicated control-flow tests, and the lexer now has regression
    coverage proving multilingual keyword tables continue to source from the TOML manifests.
  - Typechecker now registers class/struct types, trailing-lambda statements, and builtin constructors/abort so
    `seen_std/src/collections/vec.seen` type-checks cleanly (CLI now trips in the interpreter instead of the parser).
      - Interpreter/runtime gained full class/value plumbing (shared Vec storage, instance fields, method dispatch) so
        manifest-loaded stdlib modules execute;
        `SEEN_ENABLE_MANIFEST_MODULES=1 seen_cli run seen_std/tests/vec_basic.seen`
        is green and wired into the manifest test.
      - Removed the duplicate/unreachable interpreter arms and scrubbed the obvious warning sources (unused params, dead
        struct fields) so parser + interpreter builds are quiet outside of the async/reactive crates.
      - Cleared the warning backlog across `seen_concurrency`, `seen_effects`, and `seen_reactive`, so the interpreter
        build
        is warning-free (aside from the workspace-level `panic` notice) and ready for `-D warnings`.
      - Effect `handle { ... } with IO { ... }` blocks now push/pop handler frames in the interpreter and the
        `IO.Read()`
        style member calls dispatch through those stacks, so the stdlib can route effect operations without crashing.
      - Effect definitions register with the advanced runtime’s effect system, so handler stacks resolve real `EffectId`
        s
        and direct `IO.Read()` invocations route through the effect runtime (raising a sensible error if no handler is
        installed); interpreter tests cover both the success and failure paths.
    - Actor runtime now tracks pending request promises (with timeouts) and the interpreter’s `request … from actor`
      expression produces real `Promise` values. New unit coverage in `seen_concurrency::actors` plus an interpreter
      regression ensure pending requests resolve/reject deterministically before we wire stdlib actors through the
      manifest gate.
    - Actor handler bodies now execute through the real interpreter runtime so assignments mutate per-actor state and
      scoped variables survive block boundaries. Executors install instance contexts for each actor, the interpreter
      updates `ActorInstance::state` via those contexts, and regression tests cover `Inc`/`Get` handlers retaining state
      across multiple requests.
    - Flow/observable factories no longer emit placeholders: `flow { emit(...) }` now runs through the interpreter (with
      real `emit`/`delay` semantics), captures arbitrary values, and produces a `Flow<Value>` inside the reactive
      runtime.
      Reactive property assignments synchronize through the runtime manager so `@Reactive var Foo` bindings update
      observers immediately.
    - `seen_cli/tests/manifest_modules.rs::manifest_std_vec_smoke_test` now runs
      `SEEN_ENABLE_MANIFEST_MODULES=1 seen_cli run seen_std/tests/vec_basic.seen`, and CLI `run` type-checks the user
      bundle whenever manifest modules are disabled, so the stdlib manifest scenario is covered by automation while
      manifest-hosted projects continue to receive diagnostics pre-execution.

### PROD-5. Production QA & Platform Certification

*Status:* 🔄 In progress — Linux harness landed; non-Linux targets pending.

* **Inputs:** `examples/`, `tests/`, `scripts/nightly_backends.sh`, mobile/Web build steps, installer artifacts.
* **Progress:** Added `scripts/platform_matrix.sh` which drives Stage3 smoke tests on Linux (build + run `seen-ecs-min`, build
  `seen-vulkan-min`) and emits JSON reports under `artifacts/platform-matrix/<timestamp>/`. Placeholder entries mark
  Windows/macOS/Android/iOS/Web as `pending` so CI can track coverage as harnesses are implemented.
* **Outstanding:** flesh out provisioning + execution for the remaining platforms, add perf/determinism gates, and block
  release tags on a fully green matrix.
* **Acceptance:** No release can be published unless every platform row in the matrix passes build/run/determinism checks,
  and the stored artifacts allow engineers to re-run any failing configuration locally.
