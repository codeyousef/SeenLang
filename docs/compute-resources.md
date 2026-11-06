# Typed GPU resource pools

The unreleased 0.23.0 compute API provides explicit, creator-confined allocators
and move-only `GpuBuffer<T>` owners. CPU defaults do not change. Device discovery
and context ownership are described in [compute-discovery.md](compute-discovery.md).

`GpuAllocator.create(context, kind, blockBytes, maxBlocks, operation)` asks the
already-selected device for its resource provider. `GpuAllocator.open` accepts an
explicit provider handle instead and consumes it on every outcome. Both paths
validate backend, provider and native context identity and share admission and
cleanup logic. Unsupported providers return a typed error without selecting
another backend.

`GpuBuffer<T>.allocate(allocator, count, elementBytes, alignment, operation)` checks
element multiplication, unsigned byte ceilings, power-of-two alignment, metadata
budget, resource quota and cancellation before native allocation. Element stride
is explicit, as in `OwnedBuffer<T>`; kernel binding validation must compare it
with reflected layout before launch. The type parameter alone does not establish
a shader layout. `byteOffset` returns a checked offset while the buffer is open.

The allocator rounds reserved ranges to the greater of the requested alignment
and the provider's binding alignment. Cached blocks supply disjoint ranges.
Closing a buffer returns its range and coalesces adjacent free storage.
Free-range records are admitted before publishing each allocation; returning
space needs no new range records. The existing Result protocol still uses
bounded return-value storage. Reused owner slots advance their generation; exhausted generations retire. No native
allocation or destruction is needed for a buffer lifecycle that fits a cached
block. Seen metadata owners still have their normal allocation lifecycle.

`trim` explicitly destroys unused blocks. `close` rejects live buffers with
`gpu.busy`, drains cached blocks and then releases the provider and context lease.
`release` also consumes the allocator wrapper. Context shutdown remains blocked
while allocator, block or buffer owners retain leases. Their shared device-wide
quota comes from `GpuLimits.maxResources`; block count is independently bounded
by `maxBlocks`. Allocators on the same device share its commitment budget,
including native allocation padding. A provider must check the remaining budget
against native requirements before committing memory.

Closing an inactive buffer is idempotent and does not dereference its former
allocator after destruction. Moving an owner invalidates its previous binding;
these mutable owners do not implicitly satisfy cross-worker `Send` admission.

## Accounting and failures

`stats` returns an independent owning `GpuStats` snapshot:

| Field | Meaning |
| --- | --- |
| `requestedBytes` | Live element payload bytes |
| `allocatedBytes` | Live aligned range reservations |
| `usableBytes` | Native buffer capacity available to the pool |
| `committedBytes` | Native memory allocation sizes, including padding |
| `freeBytes` | Usable capacity minus live reservations; it may be fragmented |
| `peakReservedBytes` | Highest commitment reservation, including conservative quarantine charges |
| `residentBytes` | Optional measured residency; Vulkan currently reports unknown |
| `liveBuffers`, `blocks` | Current owned buffer ranges and native blocks |

Commitment is not evidence of physical residency. The Vulkan core queries used
here cannot measure residency, so they report `None`. Cached free capacity stays
committed until trim or shutdown.

Failed destruction retains the block and its lease. Partial cleanup errors carry
the number of committed bytes released by that call. A late native creation
result is destroyed before returning timeout or cancellation; failed cleanup
remains owned by the allocator and is attached as a cause. Inconsistent provider
metadata is quarantined with a conservative budget reservation. Allocations and
exact statistics then fail until cleanup resolves the unknown commitment.
There is no automatic retry or memory-kind fallback.

Cancellation and deadlines are observed before admission and after synchronous
native creation. Vendor calls cannot be preempted. Cleanup drains owned resources
independently of the cancelled operation context.

## Vulkan memory and verification

The Vulkan implementation directly calls buffer creation, memory requirements,
allocation, binding, buffer destruction and memory free. Seen owns type selection,
quota checks, suballocation, cached storage and accounting. Discovery and resource
factories share one bounded memory-property decoder.

`DeviceLocal` requires device-local memory. `HostVisible` requires host visibility;
`HostCoherent` also requires coherence. `Unified` requires host-visible device-local
memory; it does not promise coherence, managed migration or automatic transfers.
Mapping and asynchronous transfer operations are separate APIs.

The compiling example in
[vulkan_resource_contract.seen](../tests/production_gpu_runtime/vulkan_resource_contract.seen)
opens an explicitly permitted software reference device, allocates typed ranges,
checks SDK commitment and closes all owners. Run the complete software acceptance
entry point with `bash tests/misc_root_tests/seen_gpu_runtime_b.sh`. It covers
fast, release/ThinLTO, ASan, UBSan, source coverage, arithmetic fuzzing, independent
byte-bitmap range checks and injected native construction failures. The test-only
vendor interposer tracks native buffer and memory handles and rejects a process
that exits with either still owned.

These are Linux x86-64 software reference checks. They do not certify GPU hardware,
performance, other platforms or native CUDA/HIP/Metal resource providers.
