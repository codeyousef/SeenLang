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

*Status:* ⏳ Pending — channel primitives land in `seen_concurrency`, but there is no scoped job pool or parallel_for
yet.

* **Outstanding tasks:**
  1. Implement a job pool with cooperative work stealing and expose `parallel_for` / `jobs.scope` APIs in Seen code.
  2. Wire bounded-capacity channels to the async runtime (send/recv futures, select integration) and add stress tests.
  3. Surface send/receive back-pressure diagnostics (`WouldBlock`, `Closed`) in the interpreter and LLVM runtime.
  4. Document concurrency patterns and add regression tests that ensure scoped jobs drain before exit.

* **Acceptance:** Channel send/receive semantics verified; jobs in a scope join before exit.

### PSH‑4. Embedding & Packaging

*Status:* ⏳ Pending — parser/CLI lack `#[embed]`, and build driver has no shared/static output support.

* **Outstanding tasks:**
  1. Extend the parser + AST to accept `#[embed(path="...")]` on constants; bundle binary blobs into IR/LLVM output.
  2. Teach the CLI to process `--shared` / `--static` flags and invoke platform linkers appropriately.
  3. Ensure embedded assets survive Stage1→Stage2 compiles without breaking determinism (new tests + fixtures).
  4. Update documentation/quickstart with packaging instructions and add integration tests for `.so`, `.dll`, `.dylib`,
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
