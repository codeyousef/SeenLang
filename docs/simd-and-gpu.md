# SIMD and GPU

Seen 0.10.1 has native LLVM vector values for CPU SIMD and an experimental
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

## Experimental GPU compute

Compute declarations use the current decorator and built-in spellings:

```seen
@compute(workgroup: 64)
fun vector_add(a: Buffer<Float>, b: Buffer<Float>, out: Buffer<Float>) {
    let index = globalInvocationId.x
    out[index] = a[index] + b[index]
}
```

`--emit-glsl` writes shader artifacts below `<output>.shaders/`, including
`.comp.glsl` and reflection JSON. When `glslc` is available, the compiler also
produces SPIR-V `.comp.spv` output.

```bash
seen compile app.seen app --emit-glsl
```

This is not yet an automatic source-to-running-GPU pipeline. Arbitrary Seen
shader bodies are not all translated faithfully, and the generated host
dispatch wrapper does not construct a usable Vulkan pipeline for the caller.
Treat generated GLSL/reflection as inspectable build artifacts. Production
dispatch must explicitly initialize the runtime, load SPIR-V, create buffers
and a pipeline, dispatch with that pipeline handle, synchronize, and release
resources.

The C runtime API in `seen_gpu.h` uses these names:

- `seen_gpu_init`, `seen_gpu_shutdown`, and `seen_gpu_is_available`
- `seen_gpu_buffer_create`, `seen_gpu_buffer_write`,
  `seen_gpu_buffer_read`, and `seen_gpu_buffer_destroy`
- `seen_gpu_shader_load`, `seen_gpu_pipeline_create`, and
  `seen_gpu_pipeline_destroy`
- `seen_gpu_dispatch` or `seen_gpu_dispatch_handles`
- fence helpers and `seen_gpu_device_wait_idle`

GPU compilation needs `glslc`. GPU execution additionally needs Vulkan headers,
the Vulkan loader, and a suitable device/driver. Programs should check
`seen_gpu_init()` and `seen_gpu_is_available()` and provide a CPU fallback.

## Related

- [CLI Reference](cli-reference.md) -- compiler flags
- [API Reference: SIMD](api-reference/simd.md)
- [API Reference: GPU](api-reference/gpu.md)
