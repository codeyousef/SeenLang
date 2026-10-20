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
representation widths, mask limit and `hardware_verified: false`. The record
is representation evidence; LLVM remarks and benchmark results remain
separate evidence. Linux x86_64, Linux arm64, Windows x86_64 and macOS
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

`executeX86DotWithClock<C: DotClock>` borrows an explicit Seen clock provider
for deterministic execution. The default API uses the native monotonic clock.
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
execution and platform certification remain deferred until actually run.

The shared five-warmup/thirty-sample paired benchmark observes native/scalar
performance and live allocations. `SEEN_REQUIRE_ARM_PERF=1` enables the 1.05
ceiling on a designated ARM machine and requires an available native variant.
The default CI run does not invent a designated machine or RSS measurement.
