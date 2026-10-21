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

Modules contain at most 256 compute kernels and device helpers combined. Bindings are scoped and bounded to
4096; signatures have at most 64 parameters.
Traversal stops at 65536 nodes or 128 levels, and each kernel stops at 64 diagnostic records.
Type/workgroup parsing uses bounded byte ranges without temporary string
allocations. The checking context owns its arrays and diagnostic records;
source names and paths remain borrowed from the parsed module. A caller that
retains diagnostics must keep that module/path alive or copy them.

## Structured control and memory semantics

`seen check` lowers the whole parsed GPU module into shared IR and verifies it.
Concrete top-level `@device` helpers can accept fixed-width values or borrowed
resources and return a value or Void. Calls cannot recurse, capture host state,
return resources, or invoke ordinary host functions. Helpers are resolved within
the same module; imported device-helper linkage is currently unsupported.

Half-open integer ranges (`for i in 0..4`), `while`, `if`, `break`, `continue`,
and returns retain their actual bodies. Boolean `and`/`or` use lazy result
regions. Integer arithmetic uses 32-bit wrapping add/subtract/multiply; invalid
integer division, shifts and float-to-integer conversion report checked faults.
Each loop has a one-million-iteration ceiling. A checked buffer fault yields
zero for a read and skips a write while recording the fault; lanes continue
through subsequent collectives. A backend must provide the hidden checked-fault
sink and report it after completion, retaining partial writes and progress.

Shared storage is declared in the kernel prologue with a static extent:

```seen
@device
fun increment(value: Int32) r: Int32 { return value + 1 }

@compute(64)
fun count(output: WriteBuffer<Int32>) {
    let scratch: SharedBuffer<Int32> = sharedBuffer(64)
    for i in 0..4 {
        let previous = atomicAdd(scratch, i, 1, Relaxed, WorkgroupScope)
    }
    barrier(WorkgroupScope, AcquireRelease, WorkgroupMemory)
    output[0] = increment(atomicLoad(scratch, 0, Acquire, WorkgroupScope))
}
```

Shared storage is collectively zero-initialized before source operations and
cannot be replaced or aliased by locals. Atomic load/store, exchange,
add/subtract, min/max, bitwise operations and compare/exchange use scalar
Int32/UInt32 elements. Atomic calls take resource, index, optional value operands,
ordering and scope. Compare/exchange takes expected and replacement values,
success ordering, failure ordering and scope, and returns the prior value.
Scope identities are `InvocationScope`, `SubgroupScope`, `WorkgroupScope` and
`DeviceScope`. Ordering identities are `Relaxed`, `Acquire`, `Release`,
`AcquireRelease` and `SequentiallyConsistent`. Unsupported combinations,
including release loads and acquire stores, are rejected.

`barrier(scope, order, spaces)` is collective execution synchronization;
`memoryBarrier(scope, order, spaces)` is a memory fence. Spaces are
`StorageMemory`, `WorkgroupMemory` or `StorageAndWorkgroupMemory`. Execution
barriers support subgroup/workgroup scopes. Relaxed fences and device-wide
execution barriers are unsupported. Subgroup elect/all/any/shuffle/broadcast/add
have explicit capability requirements and convergence checks. Memory enum
identities must be unshadowed compile-time identifiers.

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
and verify the module immediately before consuming it. These checks do not
enable device execution.

`serializeGpuIr` revalidates the module and returns a deterministic owned JSON
string, bounded to 32 MiB. Release that string with `releaseJsonText`. Keys and
arrays have a fixed order and payloads are escaped. Callers supply a stable
module identity to obtain stable artifacts across workspaces. The
[IR schema](../schemas/seen-gpu-ir-v1.schema.json) describes the data format;
consumers must also enforce byte ceilings and semantic validation. Schema
validation alone cannot prove resource access, dominance or convergence.

The IR regression harness includes an independent, bounded scalar witness for
one invocation with integer/bool values. It executes structured control, helper
calls, mutable locals, checked buffer operations and serial atomics, and checks
known output values plus allocation baselines. Barriers are recorded as events
in this one-lane model. Other workgroup geometries, float/vector arithmetic and
subgroups are explicitly unsupported by the witness. These results establish
software semantics for the tested subset and never certify GPU execution or
inter-invocation memory ordering.

`seen check app.seen` checks compute bodies without creating a GPU device or
entering LLVM/backend generation. Native host compilation of a shader module
returns `E_GPU_BACKEND_UNAVAILABLE` before creating
kernel artifacts or native dispatch stubs. `--emit-glsl` returns the same explicit
unsupported diagnostic: legacy source scanning, known-name body substitution
and empty replacement bodies have been removed. A successful declaration check
never stands in for kernel execution.

## Offline SPIR-V Vulkan artifacts

```sh
seen gpu emit kernel.seen --gpu-backend spirv --gpu-target vulkan1.2 \
  --output kernel.spv --reflection kernel.json
spirv-val --target-env vulkan1.2 kernel.spv
```

`gpu emit` uses the same bounded source reader, strict manifest/options parser,
entry selection and reflection pipeline as `gpu reflect`. The backend must be
explicit. Vulkan 1.2 emits SPIR-V 1.5; Vulkan 1.3 emits SPIR-V 1.6. An omitted
target defaults to Vulkan 1.2. Repeated `--gpu-entry` selects kernels in module
order. Emission opens no device and does not provide a host dispatch stub.

`emitGpuSpirv(module, options)` borrows verified structured IR and options and
returns owned unsigned 32-bit words stored in `Array<Int>`, or independently
owned validation diagnostics. The caller frees the word array or releases the
validation. Every partial writer, function frame, resource record and cache is
released before returning an error. Output bytes are little endian. Artifacts
are bounded to 32 MiB, identities to 1048576, constants and shared array types
to 4096 each, and pointer types to 512. Existing IR traversal, call expansion,
workgroup and shared-memory ceilings also apply.

The baseline supports Int32/UInt32/Float32 scalars and vectors, scalar Booleans,
uniform/storage/workgroup layouts, checked accesses and arithmetic, local
variables, structured branches and loops, lazy Boolean values, and acyclic
device helpers. Helpers inline into fresh frames; resource arguments retain
their entry resource identity. Each loop records a checked fault and exits on
its one-million-iteration limit. Invalid memory indices never form a data
pointer. Float-to-integer casts require a finite value in the destination's
half-open numeric range; invalid lanes convert from zero and record a fault.

Bindings use descriptor set zero and parameter order, matching reflection.
Storage arrays use 4/8/16-byte element strides; three-lane vectors have a
16-byte stride. Uniform members start at offset zero; Boolean uniforms use a
UInt32 representation with nonzero meaning true. Shared arrays are cleared by
disjoint invocation strides followed by a workgroup memory barrier. The hidden
fault binding follows user parameters and accumulates one bit per fault kind
with a device-scope atomic OR. Its presence matches reflection even when a
particular cast cannot fault.

Unsupported targets return `E_GPU_SPIRV_TARGET`; unavailable capability groups
return `E_GPU_SPIRV_CAPABILITY` or `E_GPU_SPIRV_UNSUPPORTED`. Source atomics,
explicit synchronization, subgroups, specialization and packed/cooperative
precision require their dedicated lowering contracts. No fallback setting
substitutes another backend or precision. Native `seen compile` GPU execution
remains unavailable. Device limits must be queried before dispatch; offline
emission and official validation do not establish hardware compatibility.

Semantic rejection creates no new artifact. Each output is replaced atomically
after successful emission and reflection. If an optional reflection file fails
to publish after the binary, the command reports `E_GPU_OUTPUT` and retains the
published binary. Existing destinations survive errors before replacement.
The capped `tests/misc_root_tests/seen_gpu_spirv_a.sh` harness validates actual
Seen-emitted artifacts with `spirv-val`, compares four compile profiles and CLI
reflection, checks repeated cleanup, and runs malformed-IR fuzzing for 60 seconds.

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

## Packed precision and cooperative IR

Precision intrinsics operate on explicit `UInt32` storage words. `unpackFp32`,
`unpackFp16`, `unpackBf16`, `unpackFp8E4M3FN` and `unpackMxfp4` return `Float32`;
`unpackInt8/Int4/Int2/Int32` return `Int32`, and the corresponding `UInt` forms
return `UInt32`. Each receives `(word, lane)` with a `UInt32` lane. MXFP4 also
requires a separate `UInt32` UE8M0 scale. Its logical block holds 32 E2M1 values
and one scale byte, totaling 17 bytes. FP8 specifically means E4M3FN.

The matching `insert*` intrinsics receive `(word, lane, value[, scale])`,
return a new `UInt32` word and preserve neighboring fields. Lane zero selects
the least significant bits. Signed integer unpacking sign extends; insertion
uses the low field bits. An invalid lane records a checked fault and yields zero
on unpack or the unchanged word on insert. Floating insertion uses round to
nearest, ties to even. Finite overflow saturates to the largest finite value;
NaNs use the format's canonical NaN where available and zero for E2M1. Invalid
UE8M0 scale 255 records a fault and uses the same zero/unchanged-word rule.
Backends must implement these semantics before admitting the corresponding
capability. The CPU scalar witness currently executes integer conversions only.

`specializeInt32/UInt32(id, default, minimum, maximum)` declare a bounded integer
specialization. IDs are literal integers in 0..65535, unique across the module;
at most 64 are permitted. Default and bounds must be same-type literals with
`minimum <= default <= maximum`. `collectGpuSpecializations` returns owned
records in module order, released by `releaseGpuSpecializations`.
`resolveGpuSpecializations(module, ids, values)` returns an owned array in that
same order. Omitted overrides use defaults. Unknown IDs, duplicate IDs,
mismatched arrays and out-of-domain values produce
`E_GPU_SPECIALIZATION_OVERRIDE`. Resolution does not mutate the module.

Cooperative fragments are immutable, opaque SSA values. A/B input precision and
M/N/K shape must match; accumulators are FP32, INT32 or UINT32 according to input
precision. Supported shape identities are `Tile16x16x16`, `Tile16x8x16`,
`Tile8x8x8` and `Tile16x16x32`. Layout is explicitly `RowMajor` or `ColumnMajor`.
The source surface is:

```seen
let a = coopLoadA(input, offset, stride, FP16, Tile16x16x16, RowMajor)
let b = coopLoadB(input, offset, stride, FP16, Tile16x16x16, ColumnMajor)
let c = coopFillFp32(0.0, Tile16x16x16)
let result = coopMma(a, b, c)
coopStore(result, output, offset, stride, RowMajor)
```

Load storage is `ReadBuffer<UInt32>`, `ReadWriteBuffer<UInt32>` or shared UInt32
storage. Offset and stride are UInt32 element counts in the respective storage
resource. `coopLoadMxfp4A/B(input, offset, stride, scales, scaleOffset, shape,
layout)` additionally requires a readable UInt32 scale resource. Stores require
writable accumulator scalar storage. Bounds and geometry faults must be checked
before access. Execution control, load addresses, fill values and store
addresses must be proved subgroup-uniform. Fragment arithmetic may vary per
lane. Explicit fragment parameter/return spellings and mutable fragment locals
are unsupported by source lowering.

`GpuTargetCapabilities.create` owns a cloned target name and bounded matrix
tuple array. The creator calls `release`; validation borrows both target and
module. Backend discovery must supply the actual feature mask, subgroup width,
workgroup/shared-memory ceilings and supported shape/precision tuples.
`validateGpuTargetCapabilities` revalidates IR and profile, checks every required
feature and matrix tuple, and requires complete subgroups for cooperative
entrypoints. Missing support returns `E_GPU_CAPABILITY`; malformed profiles use
`E_GPU_CAPABILITY_PROFILE`. No precision conversion or alternative backend is
selected implicitly. Synthetic profiles in tests prove only admission logic.

The unreleased `seen-gpu-ir-v1` vocabulary includes opcodes 41..47 for unpack,
insert, specialization, matrix load/MMA/store/fill. Capability bits 10..25
distinguish packed widths, FP8/MXFP4, specialization and native matrix precision;
opaque fragment type codes encode shape, precision and A/B/accumulator role.
Consumers must reject unknown opcodes, types and capabilities. These are IR
contracts; typed GPU compilation still reports `E_GPU_BACKEND_UNAVAILABLE` until
a backend implements them. `tests/misc_root_tests/seen_gpu_ir_c.sh` runs the
precision contracts under four profiles and seeded metadata fuzz for 60 seconds.
Its evidence explicitly leaves GPU execution and hardware verification false.

## Reflection, manifest and strict selection

`seen gpu reflect source.seen --output reflection.json` runs the parsed-source
lowerer and IR verifier without LLVM, backend initialization or device discovery.
It emits every compute entry in source order. Repeat `--gpu-entry <name>` to
select a subset; selection order does not change output order. Unknown entries,
helper names and duplicate selections are errors. Helpers must currently be
defined in the same module. Source input is bounded to 1 MiB and output to 4 MiB.
The JSON output is written atomically only after all validation succeeds.

The command discovers the nearest `seen.toml`, or accepts `--manifest <path>`
(`--gpu-manifest` is the same option). `[gpu]` supports these fields:

```toml
[gpu]
backend = "cuda"
target = "sm_80"
fallback = "error"
entries = ["first", "second"]
```

Backend defaults to `none`, fallback defaults to `error`, and an empty entry list
selects all kernels for reflection. Manifest selection values must be quoted
ASCII literals without escapes. Entries use a single-line array of identifiers.
Whitespace and trailing comments are accepted. Unknown keys, duplicate keys or
tables, nested GPU tables, duplicate entries and mismatched targets fail with
`E_GPU_OPTIONS`. Limits are 64 KiB manifest bytes, 2048 lines, 4096 bytes per
line, 128 bytes per selection value and 256 entries. Other project tables remain
owned by their existing parsers.

`--gpu-backend`, `--gpu-target`, `--gpu-fallback` and repeated `--gpu-entry`
override the corresponding manifest fields. The manifest must itself be valid;
CLI overrides are validated again. CLI entry selection replaces the manifest
entry list. Backend labels are `none`, `spirv`, `cuda`, `hip` and `metal`.
Recognized target labels are Vulkan 1.2/1.3 (`vulkan1.2`, `vulkan1.3`), CUDA
`sm_70/75/80/86/89/90/100`, HIP `gfx90a`, `gfx942`, `gfx1100/1101/1200`, and Metal
`macos13/14/15`. A target requires its matching explicit backend. Fallback is
`error` or explicitly requested `cpu`. Selecting a policy never implements a
fallback or supplies capability evidence.

The same strict options apply to `seen compile`. Until typed backend execution
is implemented, selecting a GPU backend returns `E_GPU_BACKEND_UNAVAILABLE`
before creating a binary. CPU-only compilation with the default `none` policy
continues through the existing compiler path. `--emit-glsl` remains recognized
for one release and emits `W_GPU_LEGACY_GLSL`, followed by the existing explicit
backend-unavailable error; it does not restore legacy shader substitution.

[Reflection v1](../schemas/seen-gpu-reflection-v1.schema.json) includes each entry's
workgroup geometry, ordered resource bindings, scalar/element layout, required
capabilities, shared bytes and checked-fault binding. User bindings use group 0
in parameter order. Storage element strides are 4/8/16 bytes; vec3 has stride 16.
A scalar/vector uniform occupies a separately bound, 16-byte padded block.
Storage minimum size is zero because runtime lengths remain checked. A required
fault sink uses the next binding after the user parameters; otherwise its binding
is -1. These descriptors define backend obligations rather than device support.
Per-entry requirements include reachable helpers and exclude unrelated kernels.
Bounded specialization records retain their IDs, defaults and admitted domains.

`ir_sha256` hashes the exact canonical `seen-gpu-ir-v1` UTF-8 serialization.
`abi_sha256` hashes the compact JSON object containing, in order, `workgroup`,
`bindings`, `required_capabilities`, `shared_bytes`, `requires_fault_buffer`,
`fault_binding`, and the module's `specializations` array. Hashes use Seen's
shared SHA-256 implementation. `--module-id <stable-id>` controls canonical source
identity (default `module`); use a stable project-relative identity for exported
artifacts. Source locations are part of canonical IR, so formatting can change
the IR hash. ABI hashes describe the emitted layout/domain contract.

`reflectGpuModule` borrows the immutable IR and options and returns owned JSON or
owned validation diagnostics. Release JSON with `releaseJsonText` and validation
with `release()`. `GpuOptions` owns cloned entry names; its creator calls
`release()`. Reflection always records `hardware_verified: false` and
`gpu_execution_verified: false`. The regression harness independently recomputes
IR/ABI hashes, compares four execution profiles, checks deterministic CLI output,
exercises adverse selection and includes 60 seconds of seeded metadata fuzz.

## Unsupported source and rejection goldens

Single-function checks, module checks and reflection share the same parsed
lowering and IR verifier. Parsed stage annotations have one classifier. Graphics
stages mixed with compute entries and GPU methods on host classes are rejected;
they cannot disappear from a partial reflection result. Unknown statements or
expressions return `E_GPU_UNSUPPORTED` with the source location. Typed return
mismatches are reported before IR emission.

The unused legacy GLSL generator and its unsupported-node fallthroughs have
been removed. Kernel names never supply a body. The rejection contract executes
two different parsed bodies under ten names, including former conventional
kernel names, and checks their distinct results. A CPU function mentioning stage
names in a string remains CPU-only. The regression harness compares exact
diagnostic messages, codes, cardinality and locations with checked-in goldens
under four profiles, checks that rejection creates no backend artifact, and runs
60 seconds of seeded AST mutation fuzz with allocation baselines. These checks
exercise software semantics and do not establish hardware execution.

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
