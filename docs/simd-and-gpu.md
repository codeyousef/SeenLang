# SIMD and GPU

## SIMD Vector Types

Seen provides built-in SIMD types for data-parallel computation:

| Type | Description | Elements |
|------|-------------|----------|
| `f32x4` | 4 x 32-bit float | 4 |
| `f64x2` | 2 x 64-bit float | 2 |
| `i32x4` | 4 x 32-bit integer | 4 |
| `i64x2` | 2 x 64-bit integer | 2 |
| `i16x8` | 8 x 16-bit integer | 8 |
| `i8x16` | 16 x 8-bit integer | 16 |

### Construction

```seen
let v = f32x4(1.0, 2.0, 3.0, 4.0)
let zeros = f32x4(0.0, 0.0, 0.0, 0.0)
let ints = i32x4(1, 2, 3, 4)
```

### Arithmetic

Standard operators work on SIMD types:

```seen
let a = f32x4(1.0, 2.0, 3.0, 4.0)
let b = f32x4(5.0, 6.0, 7.0, 8.0)
let sum = a + b    // f32x4(6.0, 8.0, 10.0, 12.0)
let prod = a * b   // f32x4(5.0, 12.0, 21.0, 32.0)
let diff = a - b
let quot = a / b
```

### Horizontal Reductions

```seen
let v = f32x4(1.0, 2.0, 3.0, 4.0)
let total = reduce_add(v)   // 10.0
let minimum = reduce_min(v) // 1.0
let maximum = reduce_max(v) // 4.0
```

### Load and Store

```seen
let vec = simd_load_f32x4(array, offset)
simd_store_f32x4(vec, array, offset)
```

### Example: Dot Product

```seen
fun dot_product(a: Array<Float>, b: Array<Float>, n: Int) r: Float {
    var sum = f32x4(0.0, 0.0, 0.0, 0.0)
    var i = 0
    while i + 4 <= n {
        let va = simd_load_f32x4(a, i)
        let vb = simd_load_f32x4(b, i)
        sum = sum + va * vb
        i = i + 4
    }
    return reduce_add(sum)
}
```

## SIMD Policy Flags

Control SIMD code generation:

```bash
seen build app.seen --simd=auto     # Auto-detect (default)
seen build app.seen --simd=none     # Disable SIMD
seen build app.seen --simd=sse4.2   # Force SSE 4.2
seen build app.seen --simd=avx2     # Force AVX2
seen build app.seen --simd=avx512   # Force AVX-512
```

Vectorization report:

```bash
seen build app.seen --simd-report       # Summary
seen build app.seen --simd-report=full  # Per-loop detail
```

## Runtime SIMD Functions

Auto-dispatching functions that select the best SIMD width at runtime:

```seen
// Sum array elements with SIMD
let total = seen_simd_reduce_sum(array_data, length)

// Dot product
let dot = seen_simd_dot_product(a_data, b_data, length)

// Min/max
let min_val = seen_simd_reduce_min(array_data, length)
let max_val = seen_simd_reduce_max(array_data, length)

// Prefix sum
seen_simd_prefix_sum(array_data, length)
```

## CPU Feature Detection

```seen
seen_cpu_detect()                        // detect features at startup
let has_avx2 = seen_cpu_has_feature("avx2")
let tier = seen_cpu_simd_tier()          // 0=scalar, 1=SSE, 2=AVX2, 3=AVX512, 4=NEON
```

---

## GPU Compute

Seen supports GPU compute shaders via Vulkan with GLSL code generation.

### Pipeline

```
Seen AST → GLSL #version 450 → glslc → SPIR-V (.spv) → Vulkan runtime
```

### Writing GPU Compute Shaders

```seen
@compute(workgroup_size = 64)
fun vector_add(a: Buffer<Float>, b: Buffer<Float>, out: Buffer<Float>) {
    let idx = global_invocation_id.x
    out[idx] = a[idx] + b[idx]
}
```

The `@compute` decorator:
- Marks the function as a GPU compute shader
- `workgroup_size` sets the local workgroup dimensions
- The function body is compiled to GLSL, then to SPIR-V

### GPU Types

| Type | Description |
|------|-------------|
| `Buffer<T>` | Storage buffer (read/write) |
| `Uniform<T>` | Uniform buffer (read-only, shared across invocations) |
| `Image<T>` | Texture/image (read/write) |

All GPU types are opaque handles (`i64`) in the host code.

### Dispatching GPU Compute

```seen
fun main() {
    let device = gpu_init()
    let a = gpu_create_buffer(device, data_a, size)
    let b = gpu_create_buffer(device, data_b, size)
    let out = gpu_create_buffer(device, null, size)

    // Dispatch compute shader
    vector_add_gpu_dispatch(groups_x, groups_y, groups_z, buffers, num_buffers)

    let result = gpu_read_buffer(out, size)
    gpu_destroy(device)
}
```

### Inspecting Generated Shaders

```bash
seen build app.seen --emit-glsl
```

This saves the generated GLSL alongside the binary.

### Example: Matrix Multiply on GPU

```seen
@compute(workgroup_size = 16)
fun matmul(a: Buffer<Float>, b: Buffer<Float>, c: Buffer<Float>) {
    let row = global_invocation_id.y
    let col = global_invocation_id.x
    var sum = 0.0
    var k = 0
    while k < N {
        sum = sum + a[row * N + k] * b[k * N + col]
        k = k + 1
    }
    c[row * N + col] = sum
}
```

### GPU Runtime Functions

The Vulkan runtime (`seen_gpu.c`) provides:

| Function | Description |
|----------|-------------|
| `seen_gpu_init()` | Initialize Vulkan instance and device |
| `seen_gpu_create_buffer()` | Create GPU buffer |
| `seen_gpu_write_buffer()` | Write data to GPU buffer |
| `seen_gpu_read_buffer()` | Read data from GPU buffer |
| `seen_gpu_create_pipeline()` | Create compute pipeline from SPIR-V |
| `seen_gpu_dispatch()` | Dispatch compute workgroups |
| `seen_gpu_barrier()` | Memory barrier |
| `seen_gpu_destroy()` | Cleanup Vulkan resources |

### Prerequisites

GPU compute requires:
- Vulkan SDK and drivers
- `glslc` (from the Vulkan SDK) for SPIR-V compilation
- Link with `-lvulkan`

## Related

- [CLI Reference](cli-reference.md) -- SIMD and GPU flags
- [API Reference: GPU](api-reference/gpu.md) -- GPU type reference
