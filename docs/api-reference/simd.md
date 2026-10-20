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
