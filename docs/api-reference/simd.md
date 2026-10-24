# SIMD

Modules: `simd/simd_math`, `simd/simd_types`

SIMD modules define vector types and math helpers used by the compiler/runtime
SIMD lowering paths.

The native LLVM vector types exercised by the compiler are `f32x4`, `f32x8`,
`f64x2`, `f64x4`, `i32x4`, and `i32x8`. Arithmetic returns another native
vector, and horizontal reductions are receiver calls such as
`value.reduce_add()`, `value.reduce_min()`, and `value.reduce_max()`.

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

Low-level `simd_load_*` and `simd_store_*` builtins consume raw pointer
addresses. They are not documented as `(Array, offset)` helpers; use reviewed
raw-memory code or the stdlib array functions below.

Runtime array reductions for `Array<Float>` use the double-array storage used
by Seen arrays, with AVX2 or NEON min/max/sum/dot/prefix paths where available
and scalar fallbacks on unsupported targets. SIMD temporary storage in the
compatibility wrappers is allocated through the same budget-aware aligned
allocator used by the rest of the runtime.

See [SIMD and GPU](../simd-and-gpu.md) for usage and CLI controls.

## Portable IR contract

`simd.ir_types` and `simd.dispatch_plan` define `seen-portable-simd-ir-v1`
using Seen. This contract describes geometry and dispatch decisions; it does
not probe hardware or certify execution on an ISA.

`PortableVectorShape.create(lane, widthBits, packedBits)` returns a typed
`Result`. Storage width is exactly 128, 256 or 512 bits. Lanes may be signed
or unsigned 8/16/32/64-bit integers, Float16, BFloat16, Float32 or Float64.
Packed 2/3/4/8-bit values require Int8/UInt8 storage; mask bits address storage
lanes. Packed operation execution belongs to the packed-kernel APIs.
`PortableSimdInstruction.create` checks mask cardinality, tail bounds and
power-of-two alignment, with a maximum of 64 bytes and 64 mask lanes.
`simdActiveLaneBits` handles zero and full tails without shifting by 64.
Prepare masks and plans before entering a hot loop: their Result containers
allocate; native LLVM vector arithmetic and select do not allocate.

The source value spellings additionally include `f32x16`, `f64x8`, `i32x16`
and `mask2/4/8/16/32/64`. Constructors require one numeric value per lane;
mask constructors additionally accept Bool. Invalid constructors report
`E_SIMD_ARITY` or `E_SIMD_LANE`. Comparisons return lane masks, including
unordered floating-point `!=`, and `simd_select(mask, yes, no)` checks equal
value types and mask cardinality (`E_SIMD_MASK`). Existing numeric vector
masks still select by nonzero lanes. Mismatched or incomplete constructors
are rejected rather than emitting undefined LLVM lanes.

`planPortableSimd(shape, capabilities, policy, allowScalarFallback,
deterministic)` accepts explicit caller-supplied capability snapshots for
x86_64 or aarch64. Policies are scalar, auto, sse2, avx2, avx512, neon and sve.
A capability snapshot must include OS vector-state permission. `avx512`
denotes AVX-512F; this plan grants no BW, DQ, VL, VNNI or BF16 permission.
SVE vector length must be a multiple of 128, at most 2048 bits. Impossible
architecture/feature combinations fail with `simd.invalid_capability`.
Native plans currently admit Float32/Float64/Int32; representation of other
lanes does not grant permission to execute an unimplemented kernel.
Deterministic mode requires explicit scalar permission. An unavailable
variant returns `simd.unsupported_capability`, unless the caller permits
scalar fallback, which is recorded in `fallbackUsed` and `reason`.
Plans do not change the existing runtime dispatcher; ISA discovery and
executable x86/ARM dispatch require their separate implementation and tests.

Creators own shape, instruction, plan and error objects. Instructions borrow
shapes; plans borrow architecture and lane strings from their inputs. All
input strings remain borrowed, including strings retained in errors. Keep
inputs alive and immutable through their borrowers, free each object once,
and call `Result.releaseContainer()` after extracting or destroying its
payload. LLVM type strings are newly allocated and caller-owned. These
objects own no buffers or native handles and must not be mutated concurrently.
Planning is synchronous and bounded; cancellation, deadlines and retry
policy remain with the caller before or after a call. There are no detached
operations or hidden retries.

`--simd-report` adds a compact JSON record with the contract version,
representation widths, mask limit and `hardware_verified: false`. The
representation record does not certify execution. With reports enabled,
the compiler also emits frontend bounds decisions and observed LLVM module
vectorization remarks, as described below; benchmarks remain separate evidence.
Linux x86_64, Linux arm64, Windows x86_64 and macOS
x86_64/arm64 require their own execution results before being classified as
verified. No platform or performance certification is implied here.

Run the focused positive, adverse and fast/release lowering checks with:

```sh
bash tests/misc_root_tests/seen_inf_simd_a.sh
```

The harness uses the repository's attested compiler and verified aggregate
memory scope, serial compilation and ignored artifact root. Geometry/tail
checks cover every supported lane format, every tail and legal alignment;
they do not measure real memory access, throughput or device capability.
Hardware parity, sanitizer/leak checks and the required benchmark samples
must be recorded separately before certification.


`PortableVectorShape.llvmType()` and `maskType()` return `Result<String,
SimdIrError>`. Invalid mutated geometry returns `simd.invalid_shape`. Successful
spellings own their string storage and use `seen_string_release_owned`; they do
not borrow a temporary formatter buffer. Type spelling construction releases
its intermediate strings. Portable geometry and repeated dispatch fixtures
assert stable live-memory accounting after all returned owners are released.

### Checked x86 dot dispatch

`simd.x86_kernels.executeX86Dot(left, leftOffset, right, rightOffset, count,
operation, policy, allowScalarFallback, deterministic, context, progress)`
borrows two `OwnedBuffer<UInt8>` inputs and an exclusively owned
`DotProgress`. It returns `Result<Bool, SimdIrError>` without retaining any
borrow. `DotOperation` selects Float32, UnsignedSignedByte (u8 by i8) or
BFloat16. Progress contains `floating` or `integer`, `completed` elements and
`variant` (0 scalar, 1 AVX2, 2 AVX-512, 3 VNNI, 4 BF16). Both buffers may alias
for read-only inputs; callers must exclude concurrent mutation and keep their
storage/metadata valid for the call. The progress object must not alias either
input's storage. Creators close buffers and free progress after all calls.

Admission checks at most 4,194,304 elements, byte ranges, lane alignment and
address overflow before changing prior progress or reading data. Work runs in
chunks of at most 4096 elements without loop allocations. Cancellation and
monotonic deadlines are checked before the first chunk and after each chunk.
A deadline reached after computation retains the completed result/progress in
the caller's object and returns `simd.timeout`; no work detaches. Other stable
errors include `simd.cancelled`, `simd.limit`, `simd.invalid_shape`,
`simd.invalid_policy`, `simd.invalid_operation`, `simd.invalid_capability`,
`simd.scalar_required`, `simd.unsupported_capability` and `simd.unavailable`.
Diagnostics use bounded public literals without source input or trace contents.
There are no implicit retries.

`X86SimdCapabilities.host()` discovers OS-normalized instruction availability.
AVX requires hardware XSAVE/AVX, OSXSAVE and XCR0 XMM/YMM state. AVX-512 also
requires opmask/ZMM state. AVX2 Float32 additionally requires FMA; the AVX-512
variant requires F/BW/DQ/VL plus FMA. Packed variants require their separate
VNNI/BF16 bits. CPUID/XGETBV normalization is the existing narrow runtime ABI;
selection, bounds, scheduling of chunks, fallback and arithmetic live in Seen.
The x86 feature-name queries are additive: an older runtime returns false for
unknown features. They add no native symbol; the ARM section describes the
separate runtime-v9 SVE query.

`selectX86Variant` accepts synthetic capabilities for planning/tests only.
Executable calls always discover the executing host and never trust a synthetic
capability snapshot. Policies are `auto`, `scalar`, `avx2`, `avx512`, `vnni` and
`bf16`; an unavailable requested operation fails unless scalar fallback was
explicitly allowed. Deterministic mode selects scalar with explicit permission.
Native floating reductions may differ in rounding order; BF16 hardware follows
the ISA's fused arithmetic/subnormal rules. Choose deterministic scalar when
those differences are unacceptable. Integer byte products accumulate exactly
into Int64 under the checked count/chunk ceilings.

`executeX86DotWithClock` borrows a `DotClockHandle` for deterministic execution.
Create it with `DotClockHandle.create(provider)`; release the handle before
freeing the caller-owned concrete `DotClock` provider. The handle owns only its
compiler-created trait box. ARM uses the same clock handle.
`DotBackendHandle.create(provider)` similarly owns a dispatch box and borrows
its concrete `DotBackend` provider; release the handle before freeing that
provider. Shared parity and benchmark drivers borrow these handles.
Explicit vtable calls preserve each provider across module boundaries without
requiring free-function monomorphization. The default API uses the native monotonic clock.
Negative or backwards provider readings return `simd.unavailable` while
retaining already completed progress. Providers grant no additional authority.
The `@simd_target("scalar"|"avx2"|"avx512"|"vnni"|"bf16")` compiler annotation
is an explicit low-level ISA requirement, not an automatic runtime check.
Only a checked dispatcher may call such a body on a potentially older CPU.
The compiler rejects unknown/duplicate target strings (`E_SIMD_TARGET`) and
keeps each target function `noinline`, including under ThinLTO. Packed raw
builtins require exactly an accumulator and two packed i32x16 operands;
invalid arity/lane types report `E_SIMD_ARITY`/`E_SIMD_LANE`. BF16 input words
bitcast to LLVM's actual 32-lane bfloat operand type.

Run `bash tests/misc_root_tests/seen_inf_simd_b.sh` in the attested capped
scope. Fixtures cover capability matrices, all tails through 65 elements,
all lane-legal alignments within 64 bytes, native/scalar parity, injected
partial deadlines/backwards clocks, redacted diagnostics and stable live
allocations. Fast, release/ThinLTO, UBSan and ASan are included. Native OS-state
probes simulate unavailable XCR0 state without executing unsupported vector
instructions. Compiling an unavailable variant never counts as execution.
Linux x86-64 is the development target; other platform execution and hardware
certification remain unverified.

The release benchmark uses five warmups and thirty adjacent, alternating
native/scalar control samples with a paired median ratio and correctness/live
allocation checks. Default output is observation, not certification. Set
`SEEN_REQUIRE_X86_PERF=1` on the designated machine to enforce the 1.05 ratio
ceiling and require an available native variant. Keep hardware, compiler commit,
command, cgroup read-backs and the benchmark output together. Allocator peak
bytes are reported separately from RSS; no RSS or hardware result is invented.

### ARM NEON and SVE dot dispatch

`simd.arm_kernels.executeArmDot` and `executeArmDotWithClock` have the same
checked inputs and progress contract as the x86 entry points. Both use
`simd.dot_contract.{DotOperation, DotProgress, DotClock, DotBackend}` and one
shared driver for geometry, ownership, bounded chunks, cancellation, deadlines,
clock monotonicity and partial results. Scalar reference blocks and the
conformance/performance fixtures are shared as well. This alpha API advances
the standard library to 0.8.0; callers import the shared Dot types directly.
There are no alternate names retained for architecture-specific progress types.

`ArmSimdCapabilities.current()` discovers NEON and SVE availability for the
executing host and queries SVE length for the calling thread on every admission.
Linux NEON requires the kernel's ASIMD capability; SVE also requires a successful
`PR_SVE_GET_VL` query. Legal lengths are 16..256 bytes in multiples of 16.
Interrupted/unavailable probes return `simd.unavailable` without retry. An
unsupported SVE thread state never grants SVE execution. Caller-supplied
snapshots and `fromProbe` are validation/planning inputs; executable APIs always
rediscover actual state. Metadata snapshots must not be mutated concurrently.

Policies are scalar, auto, neon and sve. Auto selects SVE, then NEON; an
unavailable explicitly requested variant fails unless scalar fallback is
permitted. Deterministic mode requires explicit scalar permission. ARM progress
variants are 0 scalar, 1 NEON and 2 SVE. Errors and returned diagnostics use the
shared stable `simd.*` codes and public literal text. Capability errors do not
retain untrusted policies or clock/provider storage. Context cancellation and
deadlines are observed before discovery, after discovery and after each chunk.

NEON lowers fixed-width Float32, u8-by-i8 and expanded-BF16 products/reductions
without requiring optional dot-product or native BF16 extensions. SVE uses
scalable vector predicates, masked loads, widening products and reductions.
Inactive tail lanes cannot read outside their checked range. Each SVE block
uses the thread's actual `vscale`; no vector length or native vector value is
retained between calls. Float32/BF16 native reductions may use a different
rounding order; deterministic scalar preserves the reference order within the
fixed chunk schedule. Caller code keeps the floating-point environment stable.

Low-level `simd_unsafe_arm_*` builtins accept explicit pointer/count words and
require already validated storage, host state and a chunk of at most 4096
elements. They are not checked buffer APIs. Incorrect arity/types fail with
`E_SIMD_ARITY`/`E_SIMD_LANE`. SVE helpers trap on non-AArch64 targets if called
through this unchecked interface; the public dispatcher returns typed
unsupported errors or explicitly permitted scalar fallback before reaching it.
`@simd_target("portable128")` selects the architecture's baseline 128-bit
features; `scalar` also maps to the current architecture. Explicit neon/sve
requirements on other architectures fail with `E_SIMD_TARGET`.

The only added foreign ABI is
`seen_arm_sve_vector_bytes(UInt64*) -> Int32`, bound by runtime-v9. It copies a
single OS-reported length into caller-owned storage, clears it on failure,
normalizes errno and allocates nothing. It owns no policy, fallback, arithmetic,
thread pool or scheduling. Non-Linux platforms return unsupported; macOS ARM
uses its existing NEON capability and does not claim SVE support. The export
exists on other platforms so unsupported behavior is explicit. Runtime cache
and program-build identities include both adapter source/header files.

Run `bash tests/misc_root_tests/seen_inf_simd_c.sh` in the existing capped
scope. It covers all vector-length boundaries, policies, all tails through 65
elements and lane-legal alignments, scalar/native oracles, partial deadlines,
clock failures, allocation stability, OS-state mocks and metadata fuzzing for
60 seconds with seed 1101. Fast/release/UBSan/ASan are included. Cross-target
IR and objects cover Linux arm64, macOS arm64, Windows x86-64 and macOS x86-64;
each is recorded as compile-only and is never substituted for execution.
Fixture JSON separates an executing host's verified software paths from
unsupported paths and always sets hardware_certified:false. Physical ARM
execution and platform certification need separate machine-bound evidence.

The shared five-warmup/thirty-sample paired benchmark observes native/scalar
performance and live allocations. `SEEN_REQUIRE_ARM_PERF=1` enables the 1.05
ceiling on a designated ARM machine and requires an available native variant.
The default CI run does not invent a designated machine or RSS measurement.
Scalar dot controls use `@no_vectorize` so LLVM cannot turn the reference loop
into another SIMD candidate. The benchmark captures each candidate's variant
before its paired scalar call, rejects mixed measured variants, and preserves
all thirty candidate/control durations outside the timed interval. The
test-only `seen_simd_benchmark_metadata.sh` probe checks that ordering; its mock
provider is never hardware or performance evidence.

### Portable memory and lane operations

The standard library 0.9.0 adds `simd.memory_operations` and
`simd.operation_types`. Runtime ABI remains v9; these operations add no native
symbols or foreign kernels. The compiler uses the same closed operation metadata
for arity, return types, argument shapes, and lowering.

The supported source values are `f32x4/8/16`, `f64x2/4/8`, and `i32x4/8/16`.
Raw address operations deliberately carry an `unsafe` name. Their callers must
keep the allocation alive and prove each accessed byte belongs to it. These
intrinsics allocate no storage and retain no pointer:

| Operation | Arguments and result | Contract |
| --- | --- | --- |
| `simd_unsafe_masked_load_TYPE` | address, matching mask, passthrough → TYPE | Alignment 1; false lanes retain passthrough and do not access memory. |
| `simd_unsafe_masked_store_TYPE` | TYPE, address, matching mask → Void | Alignment 1; false lanes do not write memory. |
| `simd_unsafe_widen_u8_i32xN` / `simd_unsafe_widen_s8_i32xN` | address → i32xN | Load N bytes with zero/sign extension; N is 4, 8, or 16. |
| `simd_unsafe_narrow_u8_i32xN` / `simd_unsafe_narrow_s8_i32xN` | i32xN, address, maskN → Void | Clamp signed input lanes to [0,255] or [-128,127], truncate to bytes, and write only active lanes. |
| `simd_fma_TYPE` | TYPE, TYPE, TYPE → TYPE | Float vectors only; one fused rounding, with no fast-math reassociation promise. |
| `simd_permute_TYPE` | TYPE, TYPE, matching i32 indices → TYPE | Indices [0,N) select the first input; [N,2N) select the second. Negative/out-of-range indices produce zero. All extraction indices are bounded before use. `f64x2` is unsupported because no i32x2 source index value is shipped. |
| `simd_unsafe_prefetch_read_0/1/2/3` | address → Void | Explicit LLVM locality; an advisory hint, with no residency or completion guarantee. |

Existing `simd_shuffle` now requires exactly one in-range decimal integer literal
per output lane and matching input types. Dynamic indices use `simd_permute_TYPE`.
Malformed names, arity, types, masks, and shuffle indices fail with
`E_SIMD_OPERATION`, `E_SIMD_ARITY`, `E_SIMD_LANE`, or `E_SIMD_INDEX` before unsafe
lowering. Existing horizontal add/min/max and the x86/ARM dot kernels remain the
reduction/dot operations; unsupported formats do not silently acquire a backend.

`copyMaskedSimd(source, sourceOffset, destination, destinationOffset, vector,
active, context, progress)` borrows two `OwnedBuffer<UInt8>` owners. Offsets may
be byte-unaligned. Only `active` contiguous lanes need storage; inactive lanes
are neither read nor written. A single integer implementation preserves every
lane's bits, including NaNs. Admission rejects unknown vectors, invalid counts,
closed owners, capacity/address overflow, and any overlap between nonempty
ranges. The function never takes or releases buffer ownership.

`transformPackedSimdBytes(source, sourceOffset, destination, destinationOffset,
count, bits, pack, context, progress)` packs or unpacks unsigned 2/3/4/8-bit
LSB-first values. It reuses `inference.span` packing semantics and preserves
padding bits and surrounding bytes. Count is bounded to 4096; both ranges and
non-aliasing are validated, and packing preflights all source values before
writing. Invalid values leave the destination unchanged. It does not implement
quantization policy or signed offset encoding.

`prefetchSimdBlock(buffer, offset, locality, context, progress)` validates one
byte in a live owner before issuing the chosen hint. Locality is 0–3. Byte-range
validation is shared with dot admission through
`memory.aligned_buffer.ownedBufferByteRangeValid`; work limits remain specific
to each operation.

The caller creates and ultimately frees `SimdMemoryProgress`, which records the
committed element count and the previous monotonic clock observation. Geometry/range admission
errors preserve the previous progress. An admitted operation clears progress;
cancellation/deadline checks occur before access, every 64 packing elements,
and after completion. Errors are `simd.cancelled`, `simd.timeout`,
`simd.unavailable`, `simd.limit`, `simd.invalid_shape`, `simd.invalid_alias`,
`simd.invalid_mask`, `simd.invalid_packing`, or `simd.invalid_policy`. A late
cancellation may report an error with all elements committed. Callers use
progress to decide how to resume; the library never retries or detaches work.
Only result/error containers allocate; lane loops and compiler intrinsics do
not allocate, retain storage, or perform hidden synchronization.

Run `bash tests/misc_root_tests/seen_inf_simd_d.sh` inside the attested capped
regression environment. It checks all byte alignments, tail masks, null-address
all-false masks, sentinel preservation, widening, saturation, a fused-rounding
counterexample, dynamic/literal permutes, an independent packed-bit oracle,
invalid aliases/ranges/policies, cancellation/deadlines, and stable live memory.
It runs fast/release/UBSan/ASan plus 60-second metadata fuzzing (seed 1101).
The benchmark uses five warmups and thirty paired, alternating scalar controls;
`SEEN_REQUIRE_SIMD_OPERATION_PERF=1` enables the designated-machine five-percent
gate. Host observations and allocator peak do not constitute RSS or hardware
certification. Hardware/platform execution and designated performance evidence
remain separately classified; compile-only results are never execution claims.


## Tensor bounds proofs and compiler reports

With `--bounds-check`, the compiler removes a check only for a direct numeric
`Array<T>` access `values[i]` in a canonical `for i in 0..values.length()`
loop. The body must contain only scalar numeric assignments and array element
reads/writes. Calls, aliases through members or pointers, declarations,
control flow, array rebinding, and induction-variable writes reject the proof.
The analysis caps nesting at 32, visited expressions at 1024, and body
statements at 128. Unsupported cases retain checks. Other subscripts, such as
`values[i - 1]`, retain their own guards even inside an otherwise proved loop.
Checked builds also retain guards for legacy bulk-loop transforms and refresh
bounds before each mutable array iteration fetch. Unchecked compilation
preserves the existing opt-out and reports `caller-disabled-checks`.

`--simd-report=brief` emits bounded JSON summaries; `full` additionally emits
source loop records and escaped LLVM diagnostics. Frontend records use
`seen-simd-loop-v1`, with source line/column, a module-local loop ID, the bounds
decision, reason, and count of checks actually removed. Full records carry the
module index in a `seen-simd-module-record-v1` wrapper. The separate
`llvm-module` phase records selected/rejected vectorization and observed
widths, or an explicit unavailable/no-observation reason. These are module
optimizer observations; they do not describe final LTO decisions or prove a
runtime speedup. Reports disable object-cache reuse to observe this invocation.
`seen-simd-policy-v1` records explicit policy rejection for `--simd=none`.

Each module input is capped at 256 KiB and 4096 records; records are at most
8192 bytes and full diagnostic details at most 2048 bytes. A summary exposes
truncation. No report certifies hardware. The compiler process owns proof
scopes and restores the previous scope after each lowered loop. Reports use
the existing JSON builder's escaping and ownership rules. There are no new
native symbols, retries, background work, or runtime owners in this feature.

Run `bash tests/misc_root_tests/seen_inf_simd_e.sh` inside the repository's
verified aggregate scope. The harness covers fast/release/UBSan/ASan execution,
60 seconds of seeded AST fuzzing, stable live allocations, raw-IR proof and
retained-guard checks, negative bounds traps, and observed JSON decisions.
Cross-platform execution and designated-machine performance remain separate
verification steps; source/IR inspection alone is compile-only evidence.


## Quantized block decode corpus

Standard library 0.10.0 adds `inference.quantized_formats.QuantizedFormat` and
`simd.quantized_decode.decodeQuantizedBlock`. The explicit layouts are GGML
Q8_0 (32 values / 34 bytes), Q4_0 (32 / 18), Q4_K grouped Q4 (256 / 144),
Q2_K (256 / 84), IQ3_XXS (256 / 98), MXFP4 (32 / 17), and finite E4M3FN
(1 / 1). Q4 and MXFP4 payloads contain the low-half lanes before high-half
lanes. Q4_K carries packed six-bit group scales/minima; Q2_K carries four-bit
scales/minima. Headers are little endian and may be unaligned. MXFP4 uses an
E8M0 scale and signed E2M1 values. E4M3FN has a maximum finite magnitude of
448, including subnormals and signed zero. Nonfinite half scales, MX scale
255, E4M3FN NaNs, and decoded Float32 overflow return typed errors before any
output write. The immutable IQ3_XXS codebook and its MIT attribution are in
`seen_std/THIRD_PARTY_NOTICES.txt`, pinned to ggml commit
`da8e2a54b919557b816b1c2beb432ccb766f7ce0`. Custom IQ3/E8 layouts are not aliases
of IQ3_XXS and require a separate format contract.

`decodeQuantizedBlock(source, sourceOffset, destination, destinationOffset,
format, count, vectorBits, context, progress)` borrows both byte-buffer owners
and returns `Result<Bool, SimdIrError>`. The source must contain a complete
block, including when decoding a zero-length prefix. `count` is between zero
and the format's block element count. The output uses `count * 4` bytes of
Float32, at any byte alignment. `vectorBits` explicitly selects scalar (0)
or masked stores of 128/256/512 bits; scalar lane decoding is shared. This
API does not promise an architecture-specific arithmetic kernel or native
execution at a requested store width: LLVM may legalize wider stores for the
selected CPU baseline. Invalid widths never silently select another policy.
The existing checked owner-range and alias helpers reject closed owners,
address wrap, insufficient ranges and overlapping input/output. A block
produces at most 1024 bytes, with no kernel allocation or global mutable state.

Geometry admission errors preserve prior progress. After admission, progress
resets before cancellation/deadline and metadata checks. Cancellation is
observed before every scalar/vector store and after the final store. A late
error leaves at most one additional vector (16 lanes) committed, and
`progress.completed` records the prefix. Errors use `simd.invalid_quantization`,
`simd.invalid_shape`, `simd.invalid_alias`, `simd.invalid_policy`,
`simd.cancelled`, `simd.timeout`, or `simd.unavailable`. The caller owns each
Result container and any error; destroy/release them once. Inputs stay immutable
and output/progress require exclusive access during the synchronous call.
There are no retries or detached owners. A caller may explicitly restart the
block after handling a partial result.

`bash tests/misc_root_tests/seen_inf_simd_f.sh` runs the attested corpus:
all tails, 64 byte offsets, every store width, independently encoded golden
expectations, all finite half encodings, all E4M3FN bytes, every MXFP4 exponent
and E2M1 code, IQ3 codebook entries/sign parity, invalid geometry/aliasing,
metadata, cancellation and deadlines. Fast/release/UBSan/ASan executions also
check output sentinels and live allocations. A seeded 60-second metadata fuzz
covers rejection paths. The bounded executable time limit is 900 seconds per
case; compiler containment continues to use the aggregate project scope.

The paired benchmark records seven formats and all three vector store widths,
with five warmups and thirty alternating scalar/candidate measurements per
case. It reports median ratio and allocator peak and checks correctness/live
allocations. `SEEN_REQUIRE_QUANTIZED_SIMD_PERF=1` enforces a median ratio no
higher than 1.05 on a designated machine; ordinary CI reports observations.
Record the exact commit, toolchain, CPU/OS and aggregate-limit readback with
these logs. Software fixtures and legalizable vector IR do not certify an ISA,
platform, final LTO performance or accelerator hardware. Hardware testing and
performance certification remain deferred to the designated machine.
