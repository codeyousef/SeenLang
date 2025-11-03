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
- [ ] **Deterministic IR emission**
  - Ensure IR generator/optimizer emit sorted structures (no HashMap iteration).
  - Add regression test that hashes IR for the same input twice and matches.
- [ ] **Runtime split**
  - Carve shared code into `seen_core` (compiler) vs `seen_std` (future runtime) crates.
  - Update CLI to depend only on `seen_core`.
- [ ] **CLI determinism profile**
  - Introduce `--profile deterministic` (or similar) that locks randomness, timestamps, temp paths.
  - Document usage in quickstart/plan.

## 3) Phase PSH — Pre‑Self‑Host (WIP)

Goal: Complete all components needed to compile the compiler with itself and produce identical Stage2/Stage3 binaries.

### PSH‑1. Typestates & Phantom Types

* **Inputs:** type system and trait engine.
* **Outputs:** phantom parameters for state modeling and sealed traits to prevent invalid transitions (used later in Vulkan wrappers).
* **Acceptance:** Creating illegal transitions between states is rejected at compile time.

### PSH‑2. Async & Structured Concurrency

* **Inputs:** coroutine design and region model.
* **Outputs:** `spawn`, `scope`, and `cancel` primitives; suspension prohibited across region borrows; `move` semantics for cross‑task data.
* **Acceptance:** Compiler emits diagnostics for illegal suspend patterns; unjoined non‑detached tasks cause a compile‑time error.

### PSH‑3. Minimal Channels & Job System

* **Inputs:** concurrency module.
* **Outputs:** bounded MPMC channel API and a scoped job pool implementing parallel_for.
* **Acceptance:** Channel send/receive semantics verified; jobs in a scope join before exit.

### PSH‑4. Embedding & Packaging

* **Inputs:** build driver and runtime.
* **Outputs:** `#[embed(path)]` attribute for byte inclusion, `seen build --shared` and `--static` to produce `.so`, `.dll`, `.dylib`, and `.a`.
* **Acceptance:** Assets embed correctly and appear in deterministic object sections; all library types build successfully.

### PSH‑5. Multi‑Platform Target Bring‑Up

* **Inputs:** platform toolchains (Xcode, MSVC, NDK, Emscripten).
* **Outputs:** cross‑compilation profiles and signing configurations.

  * Linux: ELF executables and shared libs.
  * Windows: PE/COFF executables and DLLs.
  * macOS: Universal2 dylibs and app bundles (codesigned).
  * Android: `.so` + `.aab` via NDK.
  * iOS: `.framework` + `.ipa`.
  * Web: `.wasm` + JS loader (COOP/COEP headers).
* **Acceptance:** Each target produces a build artifact; execution of simple samples (textured quad) succeeds with no validation errors.

### PSH‑6. Graphics Backends & Shader Flow

* **Inputs:** SPIR‑V/WGSL toolchains.
* **Outputs:** integrated `seen build shaders` converting SPIR‑V→MSL (Metal) and SPIR‑V→WGSL (WebGPU).
* **Acceptance:** Invalid shaders produce clear diagnostics with file and stage references; valid shaders compile on all backends.

### PSH‑7. SIMD Baseline

* **Inputs:** compiler backend, optimizer, and math library.
* **Outputs:** auto‑vectorization in optimizer, portable vector types (`f32x4`, `i8x16`, etc.), numerics intrinsics (FMA, rsqrt, min/max), SIMD policy flags.

  * Flags: `--simd=off|auto|max`, `--target-cpu=native`, `--simd-report`.
  * Determinism mode may force scalar.
  * Default alignment: 16 bytes.
  * SoA/AoS helpers documented.
* **Acceptance:** Build logs include per‑function SIMD decisions; deterministic mode disables SIMD; scalar and vector results match.

### PSH‑8. Deterministic Self‑Host

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
