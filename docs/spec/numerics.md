# Seen Language — Numerics & SIMD

## 1. Numeric Primitives
- Signed integers: `i8 i16 i32 i64 i128 isize`.
- Unsigned integers: `u8 u16 u32 u64 u128 usize`.
- Floating point: `f16` (when target supports), `f32`, `f64`.
- Fixed-point proposal: `q<m>.<n>` syntax (desugars to `Fixed<m, n>`); implemented in standard library.
- Boolean: `bool`.

Literals support `_` separators and suffixes (e.g., `1_024u32`, `3.14f32`).

## 2. Float Environment
- Attributes configure FP environment at function granularity:
  ```seen
  #[float_env(ftz, daz, round = "nearest")]
  fun dot(a: vec4, b: vec4) -> f32 { /* ... */ }
  ```
- Supported modifiers:
  - Flush-to-zero (`ftz`)
  - Denormals-are-zero (`daz`)
  - Rounding mode: `nearest`, `zero`, `down`, `up`
  - Exceptions mask: `invalid`, `divbyzero`, `overflow`, `underflow`, `inexact`
- Deterministic profile enforces a portable subset (no FTZ/DAZ) unless the target triple opts in explicitly.

## 3. Composite Math Types
- `vec2/3/4<T>` for vector math (`T` = `f32` or `f64`).
- `mat3/4<T>` row-major matrices with column helpers.
- `quat<T>` quaternion type with normalized storage.
- Implementations guarantee 16-byte alignment and contiguous layout for FFI interop.

## 4. Saturation & Checked Math
- `add_saturating`, `sub_saturating`, `mul_saturating` provided for integer types.
- `checked_*` methods return `Option<T>`, aligning with `Result`-based error handling.
- Deterministic builds reject reliance on platform-specific overflow behavior; UB is disallowed.

## 5. Randomness
- `rand::*` module supplies deterministic PRNG families seeded explicitly.
- CLI `--deterministic` overrides ambient seeds to a fixed constant; entropy APIs require explicit opt-out.

## 6. SIMD Appendix
### 6.1 Portable SIMD Types
- `simd::u32x4`, `simd::f32x8`, etc., map to LLVM vector types or scalar fallback when target lacks support.
- Construction APIs:
  ```seen
  let lanes = simd::f32x4::splat(0.5);
  let sum = simd::reduce_add(lanes);
  ```
- Auto-vectorisation uses optimizer hints; `#[simd(flatten)]` ensures lane-friendly loops.

### 6.2 CLI Controls
- `--simd=off|auto|max` selects policy:
  - `off` emits scalar code (used in deterministic mode).
  - `auto` respects target CPU features.
  - `max` enables aggressive feature flags (requires explicit `--target-cpu`).
- `--simd-report` dumps per-function summaries to `target/simd/report.json`.

### 6.3 Target Feature Mapping
- x86: SSE2 baseline, AVX/AVX2/AVX-512 toggled via `--target-cpu`.
- ARM: NEON baseline, SVE widths negotiated at runtime; `simd::svf32xN` describes scalable vectors.
- Web: relies on WebAssembly SIMD proposal; falls back when unavailable.

### 6.4 Testing
- Regression tests compare scalar vs SIMD results for math kernels.
- Performance baselines record throughput deltas; failure thresholds tracked in `scripts/perf_baseline.toml`.
- Deterministic hashes ensure report stability; new SIMD features require updating fixtures.
