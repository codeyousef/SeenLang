# GPU

Seen's GPU support is an experimental Vulkan compute path. Shader declaration,
artifact generation, and runtime dispatch are separate steps in 0.12.0.

## Shader declarations

```seen
@compute(workgroup: 64)
fun vector_add(a: Buffer<Float>, b: Buffer<Float>, out: Buffer<Float>) {
    let index = globalInvocationId.x
    out[index] = a[index] + b[index]
}
```

The parser also accepts comma-separated workgroup dimensions, for example
`@compute(workgroup: 16, 16)`. Tested compute built-ins use camelCase:
`globalInvocationId`, `localInvocationId`, and `workgroupId`.

`Buffer<T>`, `Uniform<T>`, and `Image<T>` describe shader resources. They are
opaque handles in host-side lowering; their declarations do not allocate or bind
Vulkan resources automatically.

## Artifacts

```bash
seen compile app.seen app --emit-glsl
```

Artifacts are written below `app.shaders/`: generated `.comp.glsl`, reflection
JSON, and—when `glslc` is available—`.comp.spv` files. Arbitrary Seen function
bodies are not yet guaranteed to translate faithfully to GLSL, so inspect and
test generated artifacts.

The compiler can emit a `<name>_gpu_dispatch` host wrapper, but the 0.12.0
wrapper does not construct the caller's Vulkan pipeline. A production caller
must load the emitted SPIR-V and pass a real pipeline handle through the runtime
API.

## Runtime C API

The authoritative declarations are in `seen_runtime/seen_gpu.h`:

| Function family | Purpose |
|-----------------|---------|
| `seen_gpu_init`, `seen_gpu_shutdown`, `seen_gpu_is_available` | context lifecycle and availability |
| `seen_gpu_buffer_create`, `seen_gpu_buffer_write`, `seen_gpu_buffer_read`, `seen_gpu_buffer_destroy` | buffer lifecycle and transfer |
| `seen_gpu_shader_load` | load a SPIR-V module from a path |
| `seen_gpu_pipeline_create`, `seen_gpu_pipeline_destroy` | compute-pipeline lifecycle |
| `seen_gpu_dispatch`, `seen_gpu_dispatch_handles`, `seen_gpu_dispatch_indirect` | dispatch workgroups |
| `seen_gpu_fence_create`, `seen_gpu_fence_wait`, `seen_gpu_fence_destroy` | fence lifecycle |
| `seen_gpu_device_wait_idle` | wait for outstanding device work |

`seen_gpu_buffer_create(size, usage)` uses usage `0` for storage, `1` for
uniform, and `2` for indirect buffers. Read/write calls receive a buffer handle,
a CPU pointer, and a byte count. `seen_gpu_dispatch` receives a pipeline handle,
three workgroup counts, a pointer to buffer handles, and a buffer count.

The runtime also provides fixed-arity dispatch and Seen-array transfer helpers
for source paths that cannot safely form a raw handle pointer.

## Availability and fallback

Shader compilation needs `glslc`. Runtime execution needs a Vulkan loader,
device, and driver. Always check initialization/availability and retain a CPU
fallback; GPU execution is not supported uniformly across every compiler target.

`@vertex` and `@fragment` are recognized by shader extraction, but the current
runtime documentation and tests focus on compute dispatch. WebGPU bindings under
`platform/web/webgpu` are a separate stdlib API and do not imply a shipped Wasm
compiler target.

See [SIMD and GPU](../simd-and-gpu.md) for compiler controls and limitations.
