# SIMD and GPU

Seen 0.22.9 has native LLVM vector values for CPU SIMD and an experimental
Vulkan compute path. The two paths are independent.

## Native SIMD vectors

The native vector types exercised by the shipped compiler are:

| Type | Lanes |
|------|-------|
| `f32x4`, `f32x8` | four or eight 32-bit floats |
| `f64x2`, `f64x4` | two or four 64-bit floats |
| `i32x4`, `i32x8` | four or eight 32-bit integers |

Constructors, arithmetic, and receiver-style horizontal reductions lower to
LLVM vector values:

```seen
fun vector_total() r: Float {
    let a = f32x4(1.0, 2.0, 3.0, 4.0)
    let b = f32x4(5.0, 6.0, 7.0, 8.0)
    let sum = a + b
    return sum.reduce_add()
}
```

The reduction methods are `reduce_add`, `reduce_min`, and `reduce_max`. Native
vector expressions do not allocate wrapper objects.

The low-level `simd_load_*` and `simd_store_*` builtins operate on raw pointer
addresses, not `(Array, offset)` pairs. Prefer the tested constructors and
stdlib array helpers unless code is already at a reviewed FFI/raw-memory
boundary.

### Stdlib SIMD helpers

`simd/simd_math` provides array operations such as `simd_reduce_sum`,
`simd_prefix_sum`, `simd_min`, `simd_max`, and `simd_dot_product`. These work on
Seen's double-backed `Array<Float>` storage. Runtime dispatch uses AVX2 or NEON
paths where available and scalar fallbacks otherwise. `SimdFloat4` and
`SimdFloat8` remain handle-based compatibility wrappers.

### Compiler controls

```bash
seen compile app.seen app --simd=auto
seen compile app.seen app --simd=none
seen compile app.seen app --simd=sse4.2
seen compile app.seen app --simd=avx2
seen compile app.seen app --simd=avx512
seen compile app.seen app --simd-report=full
```

`--simd=auto` is the default. Use `--target-cpu` to choose the target CPU
baseline; deterministic mode forces the compiler's deterministic SIMD policy.

## Typed GPU frontend and experimental runtime

```seen
@compute(workgroup: [8, 8, 1])
fun vector_add(a: ReadBuffer<Float32>, b: ReadBuffer<Float32>, out: WriteBuffer<Float32>) {
    let index = globalInvocationId.x
    if index < a.length and index < b.length and index < out.length {
        out[index] = a[index] + b[index]
    }
}
```

`seen check app.seen` validates the parsed kernel body, fixed-width resources,
access modes, scopes and checked workgroup geometry. It performs no GPU discovery
or execution. Unsupported expressions and host effects fail with `E_GPU_*`
diagnostics. Until a typed backend is available, compiling compute modules and
using `--emit-glsl` fail explicitly with `E_GPU_BACKEND_UNAVAILABLE`; the legacy
known-name/empty-body shader emitter has been removed.

The experimental Vulkan C API in `seen_gpu.h` still provides context, buffer,
shader/pipeline, dispatch and fence operations. Callers must check availability
and choose fallback explicitly. Those runtime bindings do not certify typed
kernel lowering. Ordinary CPU modules mentioning GPU markers in literals or
non-extern function names do not select Vulkan linkage.

See [GPU API](api-reference/gpu.md) for the supported frontend vocabulary,
resource ownership, diagnostics, verification harness and runtime declarations.

## Related

- [CLI Reference](cli-reference.md) -- compiler flags
- [API Reference: SIMD](api-reference/simd.md)
- [API Reference: GPU](api-reference/gpu.md)
