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
- [ ] **Performance baselines & tooling**
  - ✅ `perf_baseline` harness (`tools/perf_baseline`) and default suite (`scripts/perf_baseline.toml`) collect runtime, peak RSS, binary sizes, and compile timings; `docs/performance-baseline.md` documents usage and `scripts/perf_baseline_report.json` seeds the baseline dataset.
  - ⏳ Wire compiler build-time sampling into CI (p95 incremental and release builds) and block regressions once thresholds regress beyond 5%. The harness now supports `--baseline` comparisons to fail on regressions (threshold configurable per task).
  - ⏳ Publish an initial Rust/C++ parity dashboard powered by the new baseline reports.

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

*Status:* ⏳ In progress — the Rayon-backed job pool remains in place, channel futures now flow through the Rust
interpreter/runtime, and buffered sends can be awaited safely. Interpreter-side `select` now races live channels fairly
instead of short-circuiting, while LLVM/backend work still needs full wake + future wiring.

* **Progress this iteration:**
  - Introduced a Rayon worker pool with `parallel_for` integration tests; parser/interpreter handle the construct
    without mis-parsing trailing lambdas.
  - Channel send/receive now surface async futures backed by the cooperative runtime; `await channel.send(...)`
    resolves once capacity frees and unit tests cover buffered back-pressure plus wakeups. Interpreter `select`
    now blocks fairly across channels and only returns once a handler matches, with dedicated regression coverage.
  - IR/LLVM lowering handles both `send` and `receive/select` expressions (preventing earlier crashes) but continues to
    rely on placeholder helpers until real wake semantics land in the backend.
  - Added `jobs.scope { ... }` syntax with parser/typechecker/interpreter coverage; scoped spawns now work under the
    jobs namespace.
  - Channel manager now exposes a waker-driven `select` future; interpreter `select` uses canonical wakers instead of
    busy waits and the async runtime surfaces the helper for stage integration.
  - Authored a concurrency patterns guide and new interpreter regression covering multi-channel select fairness; this
    seeds the documentation and regression matrix requested in the MVP plan.
  - CLI regression (`seen_cli/tests/channel_select.rs`) exercises `seen run` end-to-end with concurrent channel
    handlers to ensure the command surface routes through the waker-aware runtime.

* **Remaining tasks:**
  1. Extend channel runtime/async infrastructure into the CLI and Stage builds (drive channel futures through the
     command runner and seen_cli entrypoints, plus add end-to-end regression coverage).
  2. Wire the LLVM backend to call the real channel send/receive/select helpers (no placeholders) and ensure Stage
     builds/IR determinism checks continue to pass.
  3. ✅ Documentation and interpreter regressions landed; expand coverage to Stage runtime once the FFI layer is
     implemented.

* **Acceptance:** Channel send/receive semantics verified; jobs in a scope join before exit.

### PSH‑3a. Optimization & Auto‑Tuning Pipeline

*Status:* ⏳ Pending — research-backed optimizer workstreams need foundations before self-hosting can lock performance.

* **Outstanding tasks:**
  1. Integrate an equality-saturation pass (e-graph rewrite set inspired by `egg` / DialEgg) over Seen IR so algebraic simplifications, strength reductions, and fusion emerge deterministically (docs/research/13 - Language Performance.md).
  2. Prototype ML-driven heuristics (MLGO/Iterative BC-Max) for inlining and register allocation decisions, fed by compiler profiling data captured in PB-Perf (docs/research/13 - Language Performance.md).
  3. Wrap the hot-loop pipeline with a LENS-style superoptimizer to synthesize short instruction sequences that beat LLVM -O3 while honouring Seen's determinism guarantees (docs/research/13 - Language Performance.md).
  4. Establish an ML-guided PGO loop that exports training corpora, feeds reward signals, and replays decisions during deterministic builds.

* **Acceptance:** Optimizer reports show e-graph rewrites firing on targeted fixtures, ML heuristics reduce binary size ≥3% on benchmark suite without destabilizing determinism hashes, and superoptimized traces land in CI-perf dashboards.

### PSH‑3b. Memory & Data Layout Efficiency

*Status:* ⏳ Pending — the hybrid generational model needs instrumentation to hit the zero-overhead targets called out in the memory/performance research.

* **Outstanding tasks:**
  1. Extend the region/arena runtime with Vale-style hybrid generational handles plus validation benches proving no additional runtime checks are emitted on hot paths (docs/research/13 - Language Performance.md; docs/research/3 - GC-Free Memory Management Model for Automated Safety and Intuitive Control.md).
  2. Surface region strategy hints (`bump`, `stack`, `cxl_near`) in Seen syntax and teach the compiler to auto-select O(1) release strategies when lifetime analysis allows it.
  3. Flatten compiler data structures (AST arenas, IR graphs) to use 32-bit indices and cache-oblivious layouts, measuring the cited 2.4× speedups on cache-bound fixtures (docs/research/13 - Language Performance.md).
  4. Audit runtime safety checks, gating them behind debug profiles when static proofs exist, so production binaries keep the "zero memory safety overhead" promise.

* **Acceptance:** Memory-intensive benchmarks report ≥1.5× throughput improvement, region drops are O(1) in profiler traces, and cache miss rates fall in line with the Cornell flattening targets.

### PSH‑3c. Backend Diversification & MLIR Bridge

*Status:* ⏳ Pending — LLVM remains the sole backend; research recommends MLIR/differentiated pipelines to unlock performance headroom.

* **Outstanding tasks:**
  1. Prototype an MLIR emission path (core dialect + Transform + DialEgg integration) and validate parity with the existing deterministic IR dumps (docs/research/13 - Language Performance.md).
  2. Bring up alternative codegen backends (Cranelift with ISLE patterns, Tilde sea-of-nodes) behind `--backend` switches for fast-compile and experimentation lanes.
  3. Ensure backend selection is deterministic (same hash outputs) and CI exercises Stage0→Stage2 via at least one non-LLVM backend each night.

* **Acceptance:** Stage1 builds succeed with MLIR and Cranelift prototypes, deterministic hashes stay stable across backends, and compile-time telemetry matches the “10× faster than LLVM” research targets for fast lanes.

### PSH‑3d. Runtime Scheduling & Concurrency Efficiency

*Status:* ⏳ Pending — concurrency research highlights deeper optimizations beyond current scoped jobs.

* **Outstanding tasks:**
  1. Teach the async runtime to stack-allocate coroutine frames when escape analysis proves bounded lifetimes, mirroring the coroutine optimization path in docs/research/4 - A Concurrency Model for the Seen Programming Language.md.
  2. Profile and tune the work-stealing scheduler (queue contention, fairness, back-off) so Rayon replacement remains optional, not mandatory.
  3. Add instrumentation for task wake latency and starvation, exporting metrics to the PB-Perf dashboard for regression tracking.

* **Acceptance:** Scheduler benchmarks demonstrate lower tail latency and fewer context switches, coroutine-heavy programs drop heap allocations measurably, and starvation alerts remain green in CI.

### PSH‑3e. Hardware-Aware Codegen & Memory Topology

*Status:* ⏳ Pending — the MVP must recognize upcoming ISA and memory innovations to meet the forward-looking performance goals.

* **Outstanding tasks:**
  1. Model Intel APX register allocation, AVX10 vector widths, and ARM SVE scalable vectors inside the codegen pipeline so SIMD work in PSH‑7 can target per-core capabilities (docs/research/13 - Language Performance.md).
  2. Add CXL-aware placement hints to the allocator/runtime, keeping hot regions near compute while spilling bulk data to expandable memory (docs/research/13 - Language Performance.md).
  3. Expose CLI flags (`--cpu-feature`, `--memory-topology`) and ensure deterministic mode locks features to portable baselines.

* **Acceptance:** Hardware-feature smoke tests validate emitted binaries on APX-capable x86 and SVE ARM targets, vector reports enumerate chosen widths, and CXL placement improves memory-bound fixture throughput.

### PSH‑4. Embedding & Packaging

*Status:* ⏳ Pending — parser/CLI lack `#[embed]`, and build driver has no shared/static output support.

* **Outstanding tasks:**
  1. Ensure embedded assets survive Stage1→Stage2 compiles without breaking determinism (new tests + fixtures).
  2. Update documentation/quickstart with packaging instructions and add integration tests for `.so`, `.dll`, `.dylib`,
     `.a` generation.

* **Acceptance:** Assets embed correctly and appear in deterministic object sections; all library types build successfully.

### PSH‑5. Multi‑Platform Target Bring‑Up

*Status:* ⏳ Pending — no cross-compilation flags, signing, or per-platform build recipes exist in the CLI today.

* **Outstanding tasks:**
  1. Add `--target <triple>` support to the CLI and map to LLVM target machines/toolchains (MSVC, clang, NDK, wasm-ld).
  2. Create platform-specific linker pipelines (Windows `.exe/.dll`, macOS Universal2 + codesign, Android `.so/.aab`,
     iOS `.framework/.ipa`, Web `.wasm` + loader).
  3. Provide sample projects per platform (textured quad) and automated smoke tests that run on CI/device farms.
  4. Document toolchain prerequisites (Xcode, MSVC, SDK/NDK, Emscripten) and integrate signing/provisioning scripts.

  * Linux: ELF executables and shared libs.
  * Windows: PE/COFF executables and DLLs.
  * macOS: Universal2 dylibs and app bundles (codesigned).
  * Android: `.so` + `.aab` via NDK.
  * iOS: `.framework` + `.ipa`.
  * Web: `.wasm` + JS loader (COOP/COEP headers).
* **Acceptance:** Each target produces a build artifact; execution of simple samples (textured quad) succeeds with no validation errors.

### PSH‑6. Graphics Backends & Shader Flow

*Status:* ⏳ Pending — CLI lacks shader tooling and no SPIR-V/MSL/WGSL flow is implemented yet.

* **Outstanding tasks:**
  1. Introduce `seen build shaders` (and library API) to ingest SPIR-V, validate, and emit Metal/WGSL variants.
  2. Integrate toolchains (`spirv-cross`, `dxc`, `naga` or in-tree converters) with deterministic outputs; cache
     artifacts.
  3. Add diagnostics that map back to source files/stages (vertex/fragment/compute) with line info.
  4. Ship sample shader pipelines and unit tests covering error/success paths across Vulkan, Metal, WebGPU.

* **Acceptance:** Invalid shaders produce clear diagnostics with file and stage references; valid shaders compile on all backends.

### PSH‑7. SIMD Baseline

*Status:* ⏳ Pending — optimizer and CLI do not expose SIMD policies or reports; runtime lacks vector intrinsics.

* **Outstanding tasks:**
  1. Implement portable vector types in the standard library plus lowering logic in IR/LLVM (mapping to native SIMD
     instructions or scalar fallback).
  2. Add CLI flags (`--simd=off|auto|max`, `--target-cpu`, `--simd-report`) and ensure deterministic mode forces scalar
     codegen.
  3. Extend optimizer passes to drive auto-vectorization decisions and emit per-function reports.
  4. Add regression tests comparing scalar vs SIMD outputs, plus integration benchmarks for LLVM backends.

* **Acceptance:** Build logs include per-function SIMD decisions; deterministic mode disables SIMD; scalar and vector
  results match.

### PSH-8. Deterministic Self-Host

*Status:* ✅ Linux complete — LLVM backend fixes and Stage‑1 tooling produce matching Stage2/Stage3 hashes on Linux (via
`llc` + `cc`); macOS/Windows CI verification queued.

* **Inputs:** all previous subsystems.
* **Outputs:** successful Stage0→Stage1→Stage2→Stage3 build pipeline with identical Stage2/Stage3 hashes.
* **Acceptance:** Reproducible hashes verified on Linux and macOS; reproducibility confirmed on Windows CI.

---

## 4) Post‑Self‑Host — MVP Finalization (Pending)

Goal: Ship a self‑hosted compiler and minimal ecosystem capable of cross‑platform and SIMD builds.

### POST‑1. Documentation Completion

* **Inputs:** finalized grammar, region rules, FFI layout, numerics, and SIMD policies.
* **Outputs:** `/docs/spec` folder containing: lexical.md, grammar.md, types.md, regions.md, errors.md, ffi_abi.md, numerics.md (with SIMD appendix).
* **Acceptance:** Each file present with deterministic tables and cross‑references; index file lists all specs.

### POST‑2. Example & Validation Set

* **Inputs:** examples directory.
* **Outputs:** `seen-vulkan-min` (graphics), `seen-ecs-min` (systems) projects that run unchanged across all targets.
* **Acceptance:** Both projects build and run; Vulkan validation layers and platform equivalents report zero errors.

### POST‑3. Tooling & QA

* **Inputs:** CLI and CI configuration.
* **Outputs:** `seen fmt --check`, `seen trace`, `seen pkg` (local), CI jobs verifying deterministic builds and cross‑target packaging.
* **Acceptance:** Formatter detects violations deterministically; CI matrix completes without errors; reports archived per build.

### POST‑4. Final Definition of Done

* Compiler self‑hosts with deterministic equality verified.
* All platform artifacts build and run successfully.
* SIMD baseline operational with reporting.
* Documentation and examples published.
* CI matrix green on all targets.

---

**End of Clarified MVP Plan**
