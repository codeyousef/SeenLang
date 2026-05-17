# SIMD

Modules: `simd/simd_math`, `simd/simd_types`

SIMD modules define vector types and math helpers used by the compiler/runtime
SIMD lowering paths.

| Type | Purpose |
|------|---------|
| `SimdFloat4` | 4-lane floating-point vector wrapper |
| `SimdFloat8` | 8-lane floating-point vector wrapper |

Helper functions include `simd_f4_*`, `simd_f8_*`, `simd_reduce_sum`,
`simd_prefix_sum`, `simd_min`, `simd_max`, and `simd_dot_product`.
Native vector expressions such as `f32x4(...) + f32x4(...)` lower as LLVM
vector values and do not allocate temporary heap objects. The `SimdFloat4` and
`SimdFloat8` wrapper types remain as handle-based compatibility APIs; runtime
also exposes non-allocating `*_into` entrypoints for codegen or FFI paths that
can provide caller-owned storage.

Runtime array reductions for `Array<Float>` use the double-array storage used
by Seen arrays, with AVX2 or NEON min/max/sum/dot/prefix paths where available
and scalar fallbacks on unsupported targets. SIMD temporary storage in the
compatibility wrappers is allocated through the same budget-aware aligned
allocator used by the rest of the runtime.

See [SIMD and GPU](../simd-and-gpu.md) for usage and CLI controls.
