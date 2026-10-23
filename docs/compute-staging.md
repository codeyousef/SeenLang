# Staging metadata and buffer byte borrows

The unreleased 0.23.0 compute API provides bounded staging metadata and explicit
buffer borrows for transfer engines. `GpuStagingRing` owns metadata for a fixed
byte span. Backing memory, native submission and completion observation belong
to the engine using that span. This API does not allocate pinned storage or
submit copies.

## Native host mappings

`compute.vulkan_mapping.gpuVkMapBuffer(buffer, offset, bytes, write, operation)`
maps an explicitly selected host-visible Vulkan buffer. It returns a move-only
`GpuVulkanMappedRegion` with checked `readByte`/`writeByte` access. Bytes remain
bytes; no floating-point conversion is involved. The buffer, allocator and native
context remain borrowed until the mapped view closes. Read-only views reject
writes, and all offsets are checked against the view rather than pool capacity.

Each native block is mapped only once while any views exist. Coherent views may
share that mapping under the existing buffer reader/exclusive-writer rules.
Noncoherent mapping is exclusive across the entire block, including other byte
region leases, because whole-allocation cache visibility operations cover the
allocation's atom-aligned extent. Invalidate precedes host reads; flush precedes
unmapping after host writes. A failed flush keeps the view and its leases live
for explicit retry. Late cancellation before publishing a view undoes the native
map without flushing unpublished writes. Closing an already closed view is safe
even after its former buffer has been released.

`HostVisible`, `HostCoherent` and supported `Unified` allocations use actual
selected memory-type flags, not assumptions based on requested kind. Unsupported
host visibility is a typed error. Mapping does not submit work or wait for a
queue; callers must retain transfer leases until actual device completion.

Run `bash tests/misc_root_tests/seen_gpu_vulkan_mapping.sh` through the attested
capped test path. Its explicit software ICD checks native map/unmap counts,
partial-view byte parity, cache visibility, malformed native results, failed
flush retry, rejected aliases and cold cleanup in fast, ThinLTO, ASan and UBSan.
This is software-reference evidence, not physical-device certification.

`GpuStagingRing.create(capacity, slots)` bounds bytes, ticket count and metadata
before admission. Free-range records are prepared up front. `reserve(bytes,
alignment, operation)` checks cancellation, deadline, alignment, capacity and
available tickets. Reservations cannot overlap; closing a reserved slice returns
its range. Slot generations advance on reuse and retire before wrapping.

A submitter obtains `slice.nextSignal()` before issuing native work, then calls
`slice.submitted(signal)` only after the native queue accepts that work. Signals
advance one private timeline exactly once. The submitter must record acceptance
even if cancellation or a deadline arrives during the vendor call. Closing a
submitted slice returns `gpu.busy` and retains its storage.

`ring.complete(observed)` accepts only a monotonically increasing value within
that ring's submitted timeline. The engine must supply a value observed from its
own native timeline. Completion returns ranges to the free set. Completed ticket
wrappers still borrow ring metadata until closed. The ring rejects shutdown while
any tickets remain. Cleanup does not depend on the cancelled operation context.

`GpuBuffer<T>.region(offset, bytes, write, operation)` creates a move-only byte
borrow with checked payload bounds and a context lease. Shared readers can
coexist. A writer requires exclusive access to the entire buffer, including when
another requested range would be disjoint. Conflicts and buffer close attempts
return `gpu.busy`. Every region consumes the same device-wide resource quota as
other context leases. Closing the region releases its borrow exactly once.
Inactive region wrappers can close after their former buffer and allocator have
been destroyed.

Vulkan discovery queries the core 1.2 timeline-semaphore feature chain, validates
its identities and Boolean, reports supported timeline events and enables the
feature during logical-device creation. Unsupported features remain disabled.
No timeline is emulated and no backend is selected implicitly. Native mapping,
submission and transfer APIs are separate from these foundations.

Run `bash tests/misc_root_tests/seen_gpu_staging_foundations.sh` for the software
regressions. They cover staging reuse, cancellation, saturation, stale tickets,
exhausted counters, byte-borrow aliasing, native feature-chain faults, a
60-second metadata fuzzer, ownership rejections and source coverage. The native
fixtures explicitly use Mesa's software ICD. These Linux x86-64 results do not
certify hardware, performance or other platforms.
