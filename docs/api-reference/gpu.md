# GPU

The typed GPU frontend validates actual parsed `@compute` bodies and explicit
resource access. Kernel lowering and the experimental Vulkan runtime are separate
capabilities. Frontend validation does not certify GPU execution.

## Typed compute declarations

```seen
@compute(workgroup: [8, 8, 1])
fun vector_add(a: ReadBuffer<Float32>, b: ReadBuffer<Float32>, out: WriteBuffer<Float32>) {
    let index = globalInvocationId.x
    if index < a.length and index < b.length and index < out.length {
        out[index] = a[index] + b[index]
    }
}
```

Workgroups accept one to three positional dimensions, `workgroup: 64`,
`workgroup=64`, or a named vector. Defaults are `[64, 1, 1]`. Dimensions must be
positive integers: X/Y are at most 1024, Z at most 64, and the checked product
is at most 1024. Unknown keys, strings, expressions, overflow and malformed
vectors are rejected. No invalid dimension silently becomes a default.

| Resource | Address space | Element access |
| --- | --- | --- |
| `ReadBuffer<T>` | storage | read only |
| `WriteBuffer<T>` | storage | write only |
| `Buffer<T>` | storage | read and write |
| `Uniform<T>` | uniform | read only |

Supported values are `Int32`/`i32`, `UInt32`/`u32`, `Float32`/`f32` and their
2/3/4-lane vectors (`i32x2`, `u32x3`, `f32x4`, etc.). `Bool` is supported for
local values and uniforms. Storage booleans, host-sized `Int`/`Float`, 64-bit
values, pointers, dynamic objects, nested resources and aggregate layouts are
explicitly unsupported in this frontend revision. Resource declarations do not
allocate or bind device memory. Parameters borrow resource identities; kernels
cannot replace or alias those identities as local values.

Buffer indexing requires Int32 or UInt32. Uniform parameters evaluate to their
contained value. Resource `.length` is UInt32; it does not read storage elements.
Builtins `globalInvocationId`, `localInvocationId`, `workgroupId`,
`numWorkgroups` and `workgroupSize` expose UInt32 vector components and cannot
be shadowed. Arithmetic, explicit numeric casts, scalar comparisons, vector
constructors, typed `min`/`max`/`clamp`/`abs`/`sqrt`, mutable locals, `if`, `while`
and Void returns are checked. Unknown calls, host effects, unsafe operations,
resource escapes and unsupported constructs fail with a stable `E_GPU_*` code.

Modules contain at most 256 compute kernels. Bindings are scoped and bounded to
4096; signatures have at most 64 parameters.
Traversal stops at 65536 nodes or 128 levels, and each kernel stops at 64 diagnostic records.
Type/workgroup parsing uses bounded byte ranges without temporary string
allocations. The checking context owns its arrays and diagnostic records;
source names and paths remain borrowed from the parsed module. A caller that
retains diagnostics must keep that module/path alive or copy them.

## Backend boundary and verification

The compiler's internal structured GPU IR owns copied function identities,
literal spellings, instructions and regions. It borrows no parser nodes. The
creator transfers each instruction to one block and each block to one function;
`GpuIrModule.release()` releases the entire tree exactly once. Validation borrows
the module and returns an independently owned `GpuIrValidation`, released with
`release()` after its diagnostics are consumed.

`verifyGpuIr` rejects malformed structure, duplicate or unavailable registers,
escaping local slots, incompatible resource access, invalid memory semantics,
missing returns, recursive helpers and unproved collective convergence. Limits
are 256 functions, 65536 total instructions, 262144 operands, 4096 blocks per
function, 65536 registers per function, 4096 local slots, 128 region levels,
16 helper call levels and 64 KiB shared memory per entrypoint. Shared allocations
must occur in the kernel prologue. Vec3 elements have a 16-byte stride.

Atomics retain their integer width, access permission, memory space, scope and
success/failure ordering. Control barriers and memory fences are distinct.
Short-circuit values use two explicitly owned yielding regions. The convergence
analysis conservatively treats storage reads, mutable locals and scalar helper
parameters as varying; uncertain collective control is an explicit rejection.
Module capability, shared-byte and checked-fault requirements must exactly match
the instructions. Backend implementations must provide the declared fault sink
and verify the module immediately before consuming it. This internal verifier
does not yet expose additional source constructs or enable device execution.

`seen check app.seen` checks compute bodies without creating a GPU device or
entering LLVM/backend generation. Until typed backend lowering is available,
compiling a shader module returns `E_GPU_BACKEND_UNAVAILABLE` before creating
kernel artifacts or native dispatch stubs. `--emit-glsl` returns the same explicit
unsupported diagnostic: legacy source scanning, known-name body substitution
and empty replacement bodies have been removed. A successful declaration check
never stands in for kernel execution.

CPU-only runtime selection examines actual parsed GPU/extern declarations.
Comments, string literals, file names and ordinary Seen functions mentioning
`@compute`, `Buffer` or `seen_gpu_` do not request Vulkan linkage.

`tests/misc_root_tests/seen_gpu_ir_a.sh` wires fast/release/UBSan/ASan contracts,
positive/adverse compiler checks, backend rejection, CPU linkage trace checks and
60 seconds of seeded metadata fuzz. Tests exercise parsed bodies (including a
known legacy kernel name with an invalid body), exact validator allocation
baselines, workgroup geometry, unsigned/minimum integer limits and finite float
literals. `seen-gpu-frontend-v1` reports classify this as frontend verification
with `kernel_execution_verified: false`. Platform/device execution requires
separate backend and hardware evidence.

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

## Availability and caller policy

The experimental runtime C ABI requires a Vulkan loader, device and driver.
Check initialization/availability and choose fallback explicitly in caller
policy. The frontend performs no device discovery and selects no fallback.
`@vertex` and `@fragment` extraction and WebGPU bindings are separate APIs;
they do not establish typed compute backend or Wasm target support.

See [SIMD and GPU](../simd-and-gpu.md) for related compiler controls.
