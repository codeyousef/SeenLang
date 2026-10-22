# GPU

The typed GPU frontend validates actual parsed `@compute` bodies and explicit
resource access. Kernel lowering and the experimental Vulkan runtime are separate
capabilities. Frontend validation does not certify GPU execution.

Source reflection and artifact emission use an exclusive, bounded Seen frontend
owner. Returned GPU IR and diagnostics retain their own data after temporary
parser storage is released. See [frontend allocation lifetime](../architecture/frontend-allocation-lifetime.md)
for limits, allocator provenance and the serial-session requirement.

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
return `E_GPU_SPIRV_CAPABILITY` or `E_GPU_SPIRV_UNSUPPORTED`. No fallback setting
substitutes another backend or precision. Native `seen compile` GPU execution
remains unavailable. Device limits must be queried before dispatch; offline
emission and official validation do not establish hardware compatibility.

Semantic rejection creates no new artifact. Each output is replaced atomically
after successful emission and reflection. If an optional reflection file fails
to publish after the binary, the command reports `E_GPU_OUTPUT` and retains the
published binary. Existing destinations survive errors before replacement.

The Vulkan emitter also translates Int32/UInt32 atomics, workgroup/subgroup
execution barriers, memory barriers, subgroup elect/vote/shuffle/broadcast/add,
and integer specialization constants. Memory scopes and success/failure orders
remain explicit. Vulkan sequentially consistent shader ordering is rejected with
`E_GPU_SPIRV_MEMORY`; it is never weakened. A broadcast source must be
subgroup-uniform. Shuffle and broadcast sources must name an active lane in
range; invalid sources yield zero and record a checked address fault.

Specialization IDs map to one module-global `SpecId`, even when helpers inline
into several kernels. Defaults remain native specialization constants. The
runtime must use the reflected domains to reject invalid overrides before
creating a pipeline.

All 13 explicit packed-word formats have scalar conversion lowering. FP16,
BF16, FP8 normal values and MXFP4 use integer significands and nearest-even
rounding; MXFP4 midpoint comparisons also use integer Float32 bit ordering.
This preserves the smallest UE8M0 scales without relying on device denormal
arithmetic. NaN insertion uses positive canonical payloads: FP32 `0x7fc00000`,
FP16 `0x7e00`, BF16 `0x7fc0`, FP8 E4M3FN `0x7f`, and E2M1 zero. Infinity
insertion is preserved where the format represents infinity and saturates in
finite-only FP8/E2M1. Signed zero is preserved. Invalid UE8M0 values outside
0..254 produce conversion fault bit four; invalid lanes produce address fault
bit one. Failed insertion preserves the original word.

Native `SPV_KHR_cooperative_matrix` lowering supports FP32, FP16 and signed or
unsigned INT8 inputs with matching FP32/INT32/UINT32 accumulators. Packed input
offsets and strides count UInt32 words; accumulator output offsets and strides
count 32-bit elements. Row/column geometry is checked without overflowing
address arithmetic, and stores require disjoint rows/columns. Invalid tile
loads produce zero fragments, invalid stores skip writes, and both record a
checked address fault. Native BF16, INT4, INT2, FP8 and MXFP4 matrix capabilities
remain explicitly unsupported; packed-word conversion does not imply native
matrix support. No precision is substituted. Dispatch must validate queried
native shape/precision tuples, complete subgroups and device limits before
execution. Offline emission establishes none of those device properties.

`tests/misc_root_tests/seen_gpu_spirv_b.sh` adds four-profile official validation,
independent descriptor/workgroup reflection checks, and a bounded Seen scalar
SPIR-V witness. The witness interprets actual emitted conversion instructions
against fixed IEEE/format goldens, including subnormals, midpoint ties, overflow,
NaNs and signed zero. It verifies this scalar subset in software; it does not
execute workgroups, atomics, subgroup operations or cooperative matrices on a
device. The harness also checks partial-path cleanup, unsupported diagnostics,
explicit fallback rejection, and 60-second malformed-IR capability fuzzing.

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

Precision intrinsics belong to `@compute` and `@device` bodies. Ordinary CPU
functions require definitions or imports even in a module containing GPU entries.
`seen check` applies the shared GPU validation rules to annotated bodies.

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
UE8M0 scales outside 0..254 record a fault and use the same zero/unchanged-word rule.
Backends must implement these semantics before admitting the corresponding
capability. The IR scalar witness currently executes integer conversions only; the SPIR-V
scalar witness also covers packed floating conversions.

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
the typed compute frontend rejects these stages with `E_GPU_SIGNATURE`. They do
not establish typed compute backend or Wasm target support.

See [SIMD and GPU](../simd-and-gpu.md) for related compiler controls.

## SPIR-V artifact integrity and official certification

`buildGpuSpirvArtifact` borrows a verified IR module and options and returns one
owned `GpuSpirvArtifact`, or owned validation diagnostics. The artifact owns its
binary, reflection, and two SHA-256 strings. Its creator calls `release()` once;
borrowers must not mutate or release these fields. Construction releases every
completed phase if a later phase fails. The shared Seen serializer bounds output
to 32 MiB and writes little-endian words through the existing byte-storage ABI.

`gpuSpirvArtifactManifest` borrows the artifact, checks its target/header and
instruction framing, recomputes both digests, and returns owned JSON. A changed
snapshot returns `E_GPU_SPIRV_ARTIFACT`. This integrity check does not validate
SPIR-V semantics. `seen gpu emit ... --artifact-info` prints the manifest defined
by `docs/schemas/seen-gpu-artifact-manifest-v1.schema.json` after successful output
publication. Manifest construction completes before any file write; an output
failure returns `E_GPU_OUTPUT`. Binary and reflection publication are individually
atomic; a failed reflection write can leave the completed binary on disk.

The compiler manifest always reports `official_validation: "not-run"`,
`classification: "compile-only"`, and both hardware/execution flags false.
Official certification is separate: `seen_gpu_spirv_c.sh` compares actual files
and compiler hashes, source relocation, reflection layout, and an official
assembler/disassembler round trip. Only the assembler's generator header word
may differ. The harness exercises malformed headers, truncated instructions,
invalid opcodes and 60 seconds of seeded malformed-byte fuzz. Validator crashes,
timeouts, or resource failures fail certification instead of counting as a
successful rejection. Four compiled profiles exercise construction, mutation
rejection and repeated allocation baselines.

The Linux x86-64 SDK package version and executable SHA-256 identities live in
`docs/architecture/spirv-toolchain.json`. Provisioning installs that exact Ubuntu
package and verifies all three executables. The shared test adapter invokes
absolute paths, checks file identity and hashes, clears inherited tool-loading
environment, bounds diagnostics, and enforces RAM-derived memory, CPU and wall
limits inside the parent CI containment. Each invocation is synchronous; there
is no detached work or hidden retry. Deadline failure kills and reaps the child.
The resulting `certification.json` records tool identities and artifact hashes.
Other hosts are explicitly unsupported by this certification profile. Offline
validation remains compile-only and proves no device capability, dispatch,
performance, or hardware correctness.

## Experimental offline CUDA PTX layout

`seen gpu emit source.seen --gpu-backend cuda --gpu-target sm_80 --output kernel.ptx`
produces PTX text through the shared GPU IR artifact pipeline. Supported base
profiles are `sm_70`, `sm_75`, `sm_80`, `sm_86`, `sm_89`, `sm_90` and `sm_100`.
The last uses PTX 8.7; the others use PTX 8.0. This command opens no CUDA driver
or device. Host executable compilation with a selected GPU backend still rejects
execution through its explicit backend-unavailable diagnostic.

The base emitter lowers fixed-width integer and Float32 values, vectors,
resources, uniforms, checked faults, shared storage, structured loops and
inlined device helpers. It also lowers scoped Int32/UInt32 atomics, execution
and memory barriers, fixed-width subgroups, all 13 packed-word formats, bounded
integer specialization and the native cooperative subset below. Capability
admission follows each selected entry's helper closure. An unrelated unselected
entry cannot impose its unsupported capability. Unavailable instructions produce
`E_GPU_PTX_CAPABILITY` rather than a substitute kernel. Wrong backend/target selection produces `E_GPU_PTX_TARGET` or the shared
strict-option diagnostic. Layout and emission ceilings use `E_GPU_PTX_LAYOUT`
and `E_GPU_PTX_LIMIT`.

`seen-ptx-kernel-v1` expands each storage parameter into a 64-bit address and a
32-bit element count. A uniform passes one 16-byte value block aligned to 16
bytes, including scalar and Boolean uniforms. The checked-fault address follows
user parameters when required. Reflection records physical parameter order,
logical binding, size, alignment and packed-buffer offsets; this plan participates
in each entry's ABI digest. The alternative CUDA array-of-argument-pointers API
must supply values with the same physical types. Parameters remain borrowed for
the eventual launch lifetime; emission does not allocate device resources.

The portable parameter ceiling is 4 KiB and the static shared-storage ceiling is
48 KiB per entry. Resources keep their logical element stride, including 16-byte
storage stride for three-lane vectors. Source loop iteration and checked-fault
semantics remain those of the shared GPU IR. Shared storage is initialized by
workgroup lanes before use. Partial construction releases text fragments,
emission frames, reflection and digest containers.

[The pinned assembler policy](../architecture/ptx-toolchain.json) selects only
NVIDIA `ptxas` 12.8.93 from a SHA-256 verified package and executable. Install it
with `python3 scripts/install_gpu_ptx_toolchain.py /absolute/existing/parent/ptxas`
and set `SEEN_PTXAS` to that absolute path. The installer preserves TLS and proxy
configuration and extracts only the exact pinned member. Offline invocations
have memory, CPU, output and wall-time limits; tool identity is checked before
and after each invocation.

`tests/misc_root_tests/seen_gpu_ptx_a.sh` exercises seven targets in fast,
ThinLTO, UBSan and ASan profiles, independent launch-ABI and hash checks, official
assembly, nested helper returns and bounded malformed-IR rejection. Artifacts
remain **compile-only** with hardware/device verification false. A successful
assembler check establishes accepted PTX syntax and layout, not numerical GPU
execution, launch safety or production certification.

### PTX capabilities, specialization and cooperative ownership

CUDA subgroups have a fixed width of 32. Their synchronization masks come from
launch geometry, including a partial final warp, rather than a scheduler's
instantaneous active mask. Shuffle/broadcast sources outside the participating
lanes yield zero and set bounds fault bit 1. Ordered reductions visit participating
lanes in index order and seed from the first value, preserving floating signed
zero. Invocation and subgroup memory ordering strengthen to CTA scope without
introducing a rendezvous. Device ordering retains GPU scope; sequentially
consistent operations use SC fences. Compare-exchange may strengthen failure
ordering to its admitted success ordering. Invalid atomic indices execute no data
atomic and retain the shared checked-fault behavior.

Packed precision uses one bounded scalar conversion plan shared by PTX and
SPIR-V. Each backend renders those operations in its own instruction set. The
plan owns its arrays, borrows native input identities, and is consumed exactly
once by a renderer; returned identities belong to the containing emitter. Dynamic
lanes, finite scale admission, canonical NaNs and untouched neighboring fields
retain the shared packed-word contract. Independent emitted-instruction witnesses
check both backends against the same fixed payload and whole-word goldens.

PTX emits specialization defaults as ordinary constants. The Seen API
`materializeGpuSpecializations(module, ids, values)` validates explicit choices
and returns a separately owned, verified module with those choices frozen into
its canonical IR and hashes. Unknown or duplicate IDs, mismatched arrays, more
than 64 overrides and values outside declared domains return
`E_GPU_SPECIALIZATION_OVERRIDE`. The input module and override arrays remain
borrowed and unchanged. An empty override list freezes defaults. The caller
releases the returned module or validation exactly once, and may release the
original independently. Materialized reflection has no remaining specialization
domains; retain the original module when later choices are required. The CLI
currently emits defaults and does not accept an override flag.

Native WMMA supports `Tile16x16x16`, row-major A, column-major B and FP32
accumulators. FP16 multiplicands work on every listed target. BF16 requires
`sm_80` or later; it is never converted to FP16. Stores support either layout.
All workgroups using these collectives must contain complete warps. Other
precisions, tile shapes and input layouts return `E_GPU_PTX_CAPABILITY`. FP32
multiplicands are not silently narrowed to TF32. Native fragment register counts are separate from ordinary vector lanes;
fragments remain immutable SSA values.

Cooperative resource offsets and input strides count UInt32 words; accumulator
stores count Float32 elements. The emitter converts input strides to native
16-bit element strides. Every access checks its full 16-row footprint using
subtraction and division before address formation, with no overflowing
`offset + stride * rows` expression. Stores require nonoverlapping leading
dimensions. Binding base addresses, offsets and strides must align to the native
fragment size: 32 bytes for FP16 inputs and FP32 stores, 16 bytes for BF16 inputs.
An invalid footprint or alignment skips the collective access, yields a zero
load fragment and sets bounds fault bit 1. Shared resources remain subject to
these alignment checks. Source convergence validation requires all participating
lanes to reach a collective with uniform resource geometry.

`tests/misc_root_tests/seen_gpu_ptx_b.sh` checks numeric goldens, independent
specialization ownership, native/unsupported capability boundaries, deterministic
reflection and artifacts across four build profiles. The pinned assembler checks
FP16 on all seven targets and BF16 on all five `sm_80+` targets. This is offline
software evidence; numerical execution and device certification remain separate.

`gpuArtifactManifest` returns `E_GPU_PTX_ARTIFACT` for corrupt CUDA headers,
targets, bytes or digest snapshots; SPIR-V retains `E_GPU_SPIRV_ARTIFACT`.
Artifact bytes, reflection and digest fields remain owned by their creator and
borrowed during manifest construction. Rejection does not consume or repair the
artifact; restore an intentionally mutated field before releasing its owner.

`tests/misc_root_tests/seen_gpu_ptx_c.sh` checks integrity rejection/cleanup in
four build profiles and byte-identical PTX, reflection and hashes after source
relocation on every pinned target. The official assembler must produce a CUDA
ELF object for each accepted file and reject malformed PTX, including a timed
60-second corpus. `certification.json` records tool identity, target hashes,
profile/relocation agreement and rejection progress, with hardware and GPU
execution verification false. Certification input snapshots have explicit byte
ceilings, reject symlinks/nonregular files, close descriptors on every path and
reject metadata changes during a read. This software certificate establishes
neither numerical device execution nor production readiness.

### HIP/AMDGPU offline emission

`seen gpu emit source.seen --gpu-backend=hip --gpu-target=gfx90a --output=kernel.ll`
emits LLVM IR for the HSA v5 ABI. Targets are `gfx90a` (the default), `gfx942`,
`gfx1100`, `gfx1101` and `gfx1200`. The first two use wave64 and the remaining
targets use wave32. The emitter supports storage, padded uniforms, workgroup
storage, scalar and vector arithmetic, checked conversions, helper inlining,
structured control, scoped atomic operations, subgroup operations, packed
precision and bounded specialization. Unsupported native capabilities reject
with `E_GPU_AMDGPU_CAPABILITY`; target and layout errors remain explicit.

The Seen emitter owns bounded text fragments and private value identities.
Shared helpers pack CUDA/HSA explicit arguments: each storage binding supplies
a 64-bit address and a 32-bit logical element count; uniforms occupy 16 bytes.
Reflection includes ordered physical offsets and alignments, explicit byte
count, `seen-amdgpu-kernel-v1`, the 16-byte kernarg alignment, and the full HSA
argument byte count. HSA's 256-byte implicit block starts at the next 8-byte
boundary after explicit arguments. Callers own the complete argument buffer.
Every AMDGPU entry requires a fault buffer for its launch guard.

The uniform launch guard runs before source instructions or barriers. It requires
nonzero workgroup counts, complete workgroups matching the declared shape,
32-bit axis products, zero global offsets and a dimension count from 1 to 3.
Invalid geometry sets fault bit 16 and exits without source memory operations.
Checked source bounds, integer operations, loop budgets and casts retain fault
bits 1, 2, 4 and 8. Invalid loads return zero and invalid stores leave memory
unchanged. Three-lane vectors retain 16-byte array stride and leave padding
unchanged; scalar constructors broadcast the same payload bits to every lane.

Each shared declaration occupies a separate 16-byte-aligned region. Reflection's
`static_shared_byte_ceiling` includes that padding and must fit 64 KiB. LLVM may
eliminate unused storage; compiled metadata must not exceed the reflected
ceiling. Kernel arguments must fit 4096 bytes. Artifacts and validation results
have the same single-owner lifecycle as CUDA; corrupt AMDGPU artifact snapshots
return `E_GPU_AMDGPU_ARTIFACT`.

[The pinned LLVM policy](../architecture/amdgpu-toolchain.json) records the exact
Ubuntu LLVM package version, tool/library digests and compilation pipeline:
verify, `default<O2>`, verify, AMDGPU object generation, HSA linking and metadata
inspection. Validation checks physical arguments, wave size, workgroup size,
implicit bytes, shared limits and the private-memory ceiling. Required fixtures
validate LLVM IR, objects, HSA code objects and inspection records across four
build profiles; failed runs retain available diagnostics and artifacts.
Adverse metadata tests reject changed identities, argument offsets, sizes,
storage classes, launch geometry, storage ceilings and duplicate fields.
`tests/misc_root_tests/seen_gpu_amdgpu_a.sh` also executes fixed emitted-LLVM
fixtures after explicit normalization for CPU execution, comparing raw numeric
bits, checked faults, padding and invalid geometry against independent goldens.
These results remain **compile-only** with hardware and GPU execution verification
false; CPU normalization is not GPU execution.

`tests/misc_root_tests/seen_gpu_amdgpu_c.sh` checks owning integrity snapshots
in four profiles and emits the same source from original and relocated files.
Pinned LLVM/HSA tools validate both copies, physical reflection and identical
code-object hashes on all five targets. Guaranteed malformed LLVM and a
60-second seeded corpus must receive actual parser/type rejection; tool identity,
resource and timeout failures stop certification. Shared snapshot checks reject
oversized, symlink, directory and FIFO inputs without descriptor leaks.
`certification.json` binds the tool identities, target hashes, profile identity
and relocation/fuzz evidence. This remains offline, compile-only certification.


Atomics retain their declared memory order and workgroup/device scope. Checked
invalid addresses produce zero for returned values and skip writes. Subgroups
use native wave permutations and wave barriers. Participation follows the
logical workgroup, including a partial final wave; invalid requested lanes
produce zero and fault bit 1. Floating reductions visit participating lanes in
order and seed from the first value, preserving a lone negative zero.
Specialization uses the shared bounded clone/materialization contract and leaves
the source IR and independently owned clones unchanged.

Native cooperative multiplication is available only on `gfx90a` and `gfx942`:
16×16×16, row-major A, column-major B, FP16 or BF16 multiplicands, and FP32
accumulators in complete wave64 workgroups. RDNA targets, other shapes,
precisions, operand layouts and partial waves reject explicitly. Fragments are
immutable SSA values. Each input fragment holds two packed words; each
accumulator holds four FP32 words. For lane `l`, A row and B column are `l % 16`,
and its four input elements start at K=`4*(l/16)`. Accumulator register `r`
corresponds to row `4*(l/16)+r`, column `l % 16`. These register facts follow the
[AMD matrix instruction reference](https://github.com/ROCm/amd_matrix_instruction_calculator/tree/2ef91896bcdc4d26624f952e5c905c787cd9bc9e).

Matrix buffers require natural 4-byte word alignment. Register gathers impose
no extra hardware matrix-load alignment. Whole-footprint checks precede pointer
formation; invalid loads zero the entire fragment, while invalid stores leave
the entire backing buffer unchanged and set fault bit 1. Output strides must
prevent aliasing between rows or columns. Each wave owns its writable matrix
footprint; concurrently executing waves require disjoint output ranges.

`tests/misc_root_tests/seen_gpu_amdgpu_b.sh` validates official code objects across
four compiler profiles and requires real native FP16/BF16 MFMA instructions.
Independent CPU fixtures cover atomic semantics, the shared fixed packed-number
corpus, wave membership and matrix address/fragment routing. Native permutation
and MFMA instructions use explicit fixed operand/result oracles in those CPU
fixtures; these do not certify GPU execution or matrix numerical accuracy.
Hardware execution and numerical certification remain pending.

### Experimental Metal MSL source emission

The offline command emits deterministic MSL source:

```bash
seen gpu emit input.seen --gpu-backend metal --gpu-target macos13 \
    --output module.metal --reflection module.json --artifact-info
```

Targets `macos13`, `macos14` and `macos15` select `metal3.0`,
`metal3.1` and `metal3.2`; the default is `macos13`. MSL 3.2 requires Apple silicon.
These mappings follow Apple's Metal Shading Language specification dated
2026-06-04. A deployment label establishes no device capability.

The `seen-metal-kernel-v1` ABI uses native buffer bindings:

| Binding | Host obligation |
| --- | --- |
| Source parameter `i` | Bind its storage buffer or 16-byte uniform payload at index `i`. |
| Fault buffer `n` | Bind an owning, initially zero 4-byte atomic fault word. |
| Launch buffer `n + 1` | Bind `max(1, n)` 16-byte `uint4` records. Storage parameter `i` reads its logical element count from record `i`'s first word. Uniform records are unused. |

At most 29 source parameters are admitted, reserving two of the 31 buffer slots.
Use 256-byte aligned buffer offsets and a 16-byte aligned launch buffer. Uniforms
occupy 16 bytes; boolean uniforms normalize any nonzero word to true. Storage
elements use four bytes per lane and pad three-lane vectors to a 16-byte stride.
The host must validate each logical count against the actual allocation using
that stride before binding. The launch buffer contains counts, never pointers.
Reflection includes these indices, offsets, units and minimum byte sizes in the
entry ABI digest. Native entry names prepend `seen_metal_entry_` to the source
name, so MSL keywords cannot collide with source entry names.

Dispatch complete threadgroups of the declared shape. A uniform entry guard
rejects partial groups, zero group counts and per-axis grid overflow with fault
bit 16 before executing source code or barriers. Bounds, arithmetic, loop-budget
and conversion failures use bits 1, 2, 4 and 8. Invalid loads yield zero, invalid
stores are skipped, and faulting arithmetic returns zero. Memory checks precede
pointer indexing. The host retains bindings until completion and reads the fault
word after command-buffer completion; it must not destroy or rebind borrowed
storage during execution.

MSL control flow uses structured regions rather than `goto`, including helper
returns inside nested loops. Each loop has an independent million-iteration
budget. Threadgroup storage is zero-initialized cooperatively, padded to 16-byte
boundaries and limited to 32 KiB per emitted entry. The shared IR permits shared
declarations only in kernel bodies; helpers may borrow those resources.
Unsupported precision, atomic, subgroup and cooperative operations return
`E_GPU_METAL_CAPABILITY`; layout, target and resource limits have explicit
`E_GPU_METAL_LAYOUT`, `E_GPU_METAL_TARGET` and `E_GPU_METAL_LIMIT` diagnostics.
No operation silently falls back to a CPU implementation.

Compile emitted source with `-fno-fast-math`; Metal's default fast-math mode
does not preserve the required NaN and signed-zero selections. The source also
disables floating contraction. Native floating arithmetic retains Metal's
documented device accuracy, rounding and possible subnormal flushing. CPU
witnesses do not establish bitwise floating parity on a Metal device; that must
be checked before certifying an execution profile. Packed precision lowering is
currently unsupported.

The source manifest records `official_validation: "not-run"` and both execution
flags as false. `.metal` source is not AIR or a metallib. The Linux diagnostic
harness checks deterministic bytes/reflection across compiler profiles, repeated
ownership cleanup, adverse metadata and malformed IR. Its CPU witnesses execute
the emitted scalar/control/bounds code using test-only address-space and builtin
stand-ins with UBSan; they do not model GPU synchronization or validate Apple's
ABI. The shared scalar corpus is also used by HIP. Apple SDK compilation,
metallib linking, hardware execution and performance remain separate requirements.
The host compiler identity and specification mapping are recorded in
`docs/architecture/metal-toolchain.json`.
