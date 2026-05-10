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

See [SIMD and GPU](../simd-and-gpu.md) for usage and CLI controls.
