# Seen Language — **Alpha** Plan (Multi‑Platform + SIMD)

Alpha focuses on stabilization and ecosystem readiness **across Linux, Windows, macOS, Android, iOS, and Web** and **expands the SIMD program** beyond the MVP baseline.

---

## 1) Objectives

* Freeze syntax/semantics; polish diagnostics and IDE.
* Ship a runnable **mini‑engine** on all targets.
* Launch package registry + plugin ABI with signed/notarized artifacts.
* **Elevate SIMD**: multi‑versioned codegen and vectorization reporting.

---

## 2) Tracks

### A) Language Stability & Ergonomics

* Macro hygiene (attribute + item/proc); better error spans.
* Region/borrow visualizations; `seen doctor`.
* `seen fmt` project config; CI `--check`.

### B) Engine Integration (All Targets)

* `seen-engine-min` repo using window/input, audio, VFS, jobs.
* Backends: Vulkan/Metal/WebGPU; shader cross‑compile in `seen build shaders`.
* DoD: **zero validation errors** on canonical scenes.

### C) Ecosystem Infrastructure

* Package registry (login/publish/search), lockfile checksums, vendor mode.
* Plugin ABI versioning, capability flags; signed AAB/IPA/mac notarization/Win signing.

### D) Tooling & CI Matrix

* Incremental & cached builds; `seen trace` → `seen replay` CLI.
* CI matrix covering 6 targets; headers/templates (COOP/COEP; Android/iOS manifests).

---

## 3) SIMD Track (Alpha)

### C1. Compiler & Backend

* **Multi‑versioned codegen + runtime CPU feature dispatch**: emit scalar/SSE/AVX2/AVX‑512 (or AVX10 path), NEON/SVE, and select best at process start.
* **Vectorization reports v1**: `--simd-report` shows loops vectorized/not and short reasons (dependence/alignment/cost).

### C2. Language Surface & Stdlib

* **Portable vector types** expanded (wider sets), lane‑wise ops, masks, blends.
* **SIMD numerics utils**: reductions, prefix sums, min/max, common transforms.

### C3. Memory & Layout

* **SoA/AoS transforms (opt‑in)**: an optimizer pass guided by access patterns; source remains AoS if desired.
* **Prefetch/alignment hints** via attributes (pure hints): `#[prefetch(read)]`, `#[align_to(32)]`.

### C4. Tooling & Flags

* `--target-cpu=` families documented; `--simd=` policy integrated with `--deterministic`.
* WASM target ensures `-sSIMD=1 -sPTHREADS=1` where supported.

### C5. Acceptance

* Engine math paths and JSON/scan pipelines show SIMD codegen (verified by IR/obj inspection); scalar equivalence confirmed.

---

## 4) Alpha DoD (Updated)

| Area      | Requirement                                                            |
| --------- | ---------------------------------------------------------------------- |
| Language  | Syntax frozen; macro hygiene stable; SIMD types/intrinsics available.  |
| Engine    | Mini‑engine runs on all targets; zero validation errors; replay works. |
| Ecosystem | Registry online; plugin ABI stable; signed/notarized packages.         |
| Tooling   | CI matrix green; trace/replay; `--simd-report` available.              |
