# Seen Language — **Beta** Plan (Multi‑Platform + SIMD)

Beta emphasizes performance, parity, and robustness across platforms and fully **industrializes SIMD**.

---

## 1) Objectives

* Optimize compiler/backends (x86_64, RV64, Metal/Vulkan/WebGPU) and stdlib.
* Harden determinism and long‑run stability on desktop/mobile/web.
* Expand `seen-std`, official plugins, and engine scale.
* **SIMD**: full ISA coverage and expert controls.

---

## 2) Tracks

### A) Compiler & Runtime Optimization

* LTO/PGO; sanitizers; cross‑module inlining; improved dependency pruning.
* Memory/layout audits for mobile/web (allocator arenas, low‑footprint modes).

### B) Ecosystem Maturity

* `seen-std` grows: collections (stable iteration), math, io, serde, concurrency utils.
* Certified plugin catalog (graphics, physics, audio, networking).

### C) Platform & Engine Scaling

* Backend parity: DX12/DXC on Windows; MSL argument buffers; WebGPU stability.
* ECS parallelism; 10k+ jobs/frame target; mobile thermal/memory policies.

### D) Tooling & DX

* `seen trace` GUI; gdb/lldb pretty‑printers; advanced LSP (refactors/actions/semantic tokens).

---

## 3) SIMD Track (Beta)

### B1. Compiler & Backend

* **Portable lowering** to **x86 (AVX2/AVX‑512/AVX10)**, **ARM (NEON/SVE/SVE2)**, **RISC‑V (RVV 1.0)**, and **WASM SIMD**.
* **Auto‑vectorizer cost model** tuned for lane utilization & bandwidth.
* **Expert hints**: `#[vectorize]`, `#[no_vectorize]`, `#[lane_width(N)]` — hints only.
* **Inline intrinsics/asm** as an escape hatch with strict typing.

### B2. Stdlib & Data Paths

* Vectorized primitives for JSON/UTF‑8 scan, deflate/brotli blocks, image swizzles.
* Vectorized crypto/checksums (AES/SHA via AES‑NI/ARMv8 crypto/RVV equivalents; CRC/Adler) where legal.

### B3. Data Layout & Transform Passes

* **Automatic SoA/AoS transforms** (opt‑in) guided by profiling metadata.
* **Prefetch planning** pass; document cache‑line assumptions.

### B4. Tooling & WASM

* `--simd-report=full` with loop IDs, dependence reasons, lane fill %, chosen width.
* Web builds default to WASM SIMD + threads when headers permit.

### B5. QA & Determinism

* Equivalence tests (scalar vs SIMD) across x86/ARM/RVV/WASM; property‑based numerics tests where bit‑exactness matters.
* Determinism profiles validated per target; CI regresses on vectorization correctness (not performance).

---

## 4) Beta DoD (Updated)

| Area        | Requirement                                                                      |
| ----------- | -------------------------------------------------------------------------------- |
| Compiler    | Full ISA coverage; expert SIMD controls; portable lowering; determinism honored. |
| Runtime     | `seen-std` stable; vectorized core paths available with scalar fallbacks.        |
| Engine      | Backend parity + stable performance envelopes; mobile/web constraints respected. |
| Tooling     | `--simd-report=full`; trace GUI; IDE integration.                                |
| Determinism | Scalar/SIMD equivalence validated across targets.                                |
