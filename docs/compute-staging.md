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

## Native asynchronous byte copies

`compute.vulkan_transfer.GpuVulkanCopyQueue.open(allocator, slots, operation)`
borrows the explicitly selected Vulkan context and preallocates a command pool,
up to 256 command buffers (also bounded by the context limit), and one private
timeline semaphore. The selected queue family comes from device discovery,
never a guessed index. Timeline support must have been enabled during context
creation. There is no backend or memory-kind fallback.

`gpuVkCopy(queue, source, sourceOffset, destination, destinationOffset, bytes,
operation)` copies bytes independently of the two buffers' element types. It
checks logical bounds, context identity, native overlap, aliases, metadata
headroom and queue capacity before submission. Source read and destination
exclusive-write leases are retained by the queue. Buffer/context destruction
and conflicting host mappings remain busy until native completion is observed.
The queue records range-specific transfer/host visibility barriers, not a
per-copy fence or a device/queue-wide idle wait.

Successful submission returns the next private timeline value. OOM submission
failure releases the unaccepted leases and leaves that signal reusable. Other
submission failures retain potentially accepted work, poison further admission
and report its signal in `GpuError.progress`; callers must still drain it.
Cancellation/deadline expiry before submission releases unsubmitted leases.
Expiry during or after accepted submission cannot revoke native ownership.

`queue.poll(signal)` uses a zero-timeout timeline query: `false` retains pending
owners; observed completion retires leases for this and earlier signals. Cleanup
does not consult the cancelled operation. A device-lost observation retires the
corresponding native resource use but returns an error, never correct output;
subsequent admission remains poisoned. Other observation failures preserve work
for retry. Shutdown is deterministic and idempotent, and rejects pending work.
Queue owners are creator-confined, not a cross-thread submission API.

Host-visible/coherent buffers provide explicit Vulkan staging storage; supported
`Unified` buffers require actual device-local plus host-visible memory flags.
This does not promise fault-driven managed migration, universal zero-copy,
OS page-locking APIs or performance. `GpuStagingRing` below remains independent
metadata: a transfer engine must compose its tickets with its own private native
timeline and backing buffer before claiming an integrated pinned ring.

Run `bash tests/misc_root_tests/seen_gpu_vulkan_transfer.sh` through the attested
capped path. Its software ICD checks byte parity, command reuse, cold cleanup,
fault-prefix cleanup, OOM retry, uncertain submission, completion failure,
late deadlines and source coverage. Native-call traces reject per-copy command,
semaphore or memory creation, fences and global idle waits. Such traces describe
SDK calls made by Seen, not every internal driver allocation. Physical-device,
benchmark and other-platform certification are separate evidence.

## Owned Vulkan staging

`compute.vulkan_staging.GpuVulkanStagingRing.open(context, blockBytes,
slotBytes, count, operation)` composes fixed host-coherent backing, bounded
metadata and a private preallocated copy queue. Slots can share a native arena
block but own disjoint byte ranges. Counts, bytes, context quotas and metadata
budgets are checked before admission. Partial construction releases its acquired
prefix. This creator-confined, move-only API does not silently change backend or
memory kind, and host coherence is not a claim of OS page locking.

`ring.reserve(bytes, operation)` returns a move-only ticket no larger than its
fixed slot. `ticket.write(Span<UInt8>, offset, operation)` synchronously copies a
checked, nonempty host span into reserved backing. Only a contiguous initialized
prefix can be extended, and upload requires the entire payload initialized.
The caller keeps the source alive for that call; its pointer is never retained
for asynchronous device work.

`gpuVkStagingUpload(ticket, destination, offset, operation)` and
`gpuVkStagingDownload(ticket, source, offset, operation)` submit checked bytes
without numeric conversion. Rejected recording or guaranteed OOM leaves the
ticket reserved and reusable. Potentially accepted submissions are recorded in
the private timeline even when the native call returns an error; backing remains
owned until observed retirement. A poisoned queue rejects new reservations.

`ticket.poll()` observes only that private timeline, regardless of cancellation.
Completion does not recycle a ticket. `ticket.read(MutableSpan<UInt8>, offset,
operation)` synchronously copies valid completed download bytes into exclusively
borrowed caller storage. Device loss retires native use but never permits output
reads. Closing the consumer ticket acknowledges its bytes and allows reuse;
closing submitted work or a ring with outstanding tickets returns `gpu.busy`.
Close is idempotent; `release()` closes and destroys its wrapper once.

Run `bash tests/misc_root_tests/seen_gpu_vulkan_staging.sh` through the attested
capped entry point. It exercises bulk high-bit bytes, retained unread output,
bounded reuse, cancellation, invalid spans and geometry, partial construction,
mapping, submit/poll faults and exact cold live-allocation cleanup. Native traces
check fixed pool/semaphore/command/memory counts and reject per-copy fences or
global idle waits. Fast, ThinLTO, ASan and UBSan software-reference execution is
distinct from physical hardware, pinning, throughput and other-platform evidence.

## Independent staging metadata

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
own native timeline. Completion retires native work but retains each completed
range until its consumer closes the ticket. This acknowledgement returns the
range to the free set; observing completion alone must not allow a later upload
to overwrite unread download bytes. Consumers may close completed tickets out of
order: each close releases only that ticket's range and metadata lease. The ring
rejects shutdown while any tickets remain. Cleanup does not depend on the
cancelled operation context.

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
