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

## CUDA completion primitive

The existing opt-in `accelerator.cuda.mod` resource family includes
`CudaEvent.poll()`. It returns `Ok(false)` while the recorded event is pending,
`Ok(true)` after native completion, or the existing typed `CudaError` without
granting completion. The owner must retain the event and submitted resources
while pending or after a query error. Polling neither records nor destroys an
event, allocates staging policy, synchronizes a stream/device/event, nor changes
fallback policy. A caller must bound repeated queries by its deadline or fuel.

The additive `seen_cuda_event_query(event, int32_t *ready)` C ABI sets `ready`
to zero before checking the event and before any fallible native work. CUDA's
not-ready status is normal pending progress. Only a successful native query
sets it to one. A missing output is invalid. This is the same version-1 resource
ABI, not a second allocator/stream interface. CPU-only headers still need no
CUDA SDK; native use requires the existing explicit bundled dependency.

`bash tests/misc_root_tests/seen_cuda_event_query.sh` checks the CPU-only header
and Seen frontend. With `SEEN_CUDA_QUERY_HARDWARE=1`, it explicitly requires the
RTX 4090, runs 64 pinned H2D/D2H byte round trips with event reuse, injects one
pending or failed query, rejects synchronization calls, runs device memcheck,
and exercises fast/ThinLTO Seen consumers including exact live-usage cleanup.
This primitive is necessary for composing a pinned CUDA staging ring; it does
not by itself provide that ring or certify unrelated backends.

## Experimental owned CUDA staging

`compute.cuda_staging.GpuCudaStagingRing.prepare(slotBytes, count, maxBytes)`
creates the move-only cleanup owner and fixed metadata before acquiring native
resources. `maxBytes` bounds pinned plus device payload, not just one side;
count is at most 256. `ring.open(deviceOrdinal, operation)` explicitly selects
CUDA and preallocates a pinned host allocation, device allocation and event per
slot plus one private nonblocking stream. It composes existing version-1 CUDA
owners and resource ABI, not a parallel allocator or stream stack.

Open is a single native-initialization attempt. A cancelled preflight leaves the
owner fresh. Any native initialization failure leaves every acquired prefix
inside the caller's owner. The caller must close/release it even after a failed
open. Close failures preserve their handles and wrappers for retry; successful
close is idempotent. There is no fallible constructor that discards the only
handle when cleanup also fails.

`ring.reserve(bytes, operation)` returns a fixed-slot ticket. `ticket.write()`
borrows a checked `Span<UInt8>` synchronously and tracks the contiguous initialized
prefix; upload requires all payload bytes initialized. `gpuCudaStagingUpload`
copies into that ticket's owned device backing. `gpuCudaStagingCopy(destination,
source, offset, operation)` and `gpuCudaStagingDownload(...)` require completed
device bytes and disjoint tickets from the same ring. Native transfers retain
source readers until actual completion; a source close remains busy meanwhile.
These APIs do not expose caller-owned allocations as untracked DMA pointers.

Tickets record potential native ownership before entering CUDA. A copy or
event-record error conservatively consumes the signal and poisons further
admission; CUDA may be reporting an earlier asynchronous error. Pending owners
must still be drained. `ticket.poll()` normally queries the preallocated event.
If recording or a later event query failed, subsequent polls use
`seen_cuda_stream_query` on the private stream: an old/unrecorded/unusable event
is never completion proof for new work. The first query error does not imply
completion. Query errors preserve pending owners and poison output; a later successful observation can
retire resource use but cannot validate poisoned bytes. `ticket.retired()` lets
the caller distinguish safe cleanup from valid output. Permanent native failures
retain ownership rather than inventing completion or silently resetting a device.

Completion does not recycle a consumer ticket. `ticket.read(MutableSpan<UInt8>,
offset, operation)` synchronously copies valid completed download bytes to the
caller; closing the ticket acknowledges consumption. Cleanup/polling ignores
operation cancellation, while writes, reservations, reads and submissions check
it. Late cancellation after accepted work carries the consumed signal in
`GpuError.progress` and never revokes ownership. No fallback, per-copy allocation
of native memory/events/streams, or event/stream/device synchronization is used.

The default capped `seen_gpu_cuda_staging.sh` gate checks frontend, rejected
move/sharing and fast/ThinLTO execution of a CPU-only unused staging import.
Its executable dependencies must not include accelerator libraries.
`SEEN_CUDA_STAGING_HARDWARE=1` explicitly requires the RTX 4090 and
executes byte parity, bounds, saturation, cancellation, partial initialization,
potentially accepted copy errors, failed event recording, failed queries and
retryable cleanup in fast, ThinLTO, UBSan and strict ASan, plus device memcheck.
It also checks native error classification, closed tickets, cross-ring rejection
and a deadline expiring after accepted work. A three-submission batch verifies
same-stream completion ordering and rejects an old closed ticket after its
physical slot is recycled. A bounded, seeded 4,096-cycle state corpus exercises
every payload width from 1 through 64 bytes, partial writes, source offsets,
cancellation, consumer retention, canaries and alternate cleanup order. Every
cycle must restore exact Seen live allocation and pending/borrower/dependency
counts. Native resources remain fixed at three pinned buffers, three device
buffers, one stream and three events for all 12,288 copies.
The test reports process RSS high-water after 256 cycles and at shutdown, plus
elapsed time from its first native copy through cleanup. The
ordinary-profile growth envelope is 16 MiB; instrumented ASan allows 320 MiB for
its unchanged default quarantine/shadow overhead. Neither changes the enclosing
hard physical cap or relaxes exact live-resource and leak checks. This short,
small-payload corpus is not a throughput benchmark or long-duration soak.
The separate `lifecycle-soak` case requires at least 60 seconds and 10,000 full
child-ring lifecycles, with a hard ceiling of 1,000,000. Each child reuses its
three fixed slots four times before closing. A retained parent upload survives
every teardown and must download its original sentinel afterward. Every child
restores exact live allocation; recyclable Seen reserve must stay flat after
256 warm-up lifecycles. The native witness independently checks elapsed time,
copy counts, acquired/released SDK resources and a peak of 20 observed objects
(parent plus one child). Its pinned-byte metric is requested payload capacity,
peaking at 384 bytes, not OS page-lock granularity or physical VRAM. This case
runs in every requested execution profile without increasing existing time,
memory or worker limits. Its measured JSON does not claim unobserved internal
driver resources or replace other-platform, sanitizer or release acceptance.
Coverage requires at least 90% of
owned source lines and 80% of branch directions, without excluding guard paths.
CUDA host ASan uses an explicit `--address-layout cuda`: NVIDIA documents that
CUDA requires the default shadow gap as application address space. LLVM maps
additional shadow for that range with `protect_shadow_gap=0`; heap poisoning,
redzones, quarantine, leak detection and error exits remain enabled. The default
runner layout remains protected and unchanged. The `seen_asan_address_layout.sh`
contract requires both layouts to detect heap/UAF/stack/global/leak errors, and
requires poison detection within the newly supported gap mapping itself.
See [NVIDIA's ASan build instructions](https://github.com/NVIDIA/nccl/blob/master/makefiles/common.mk)
and [LLVM's shadow setup](https://github.com/llvm/llvm-project/blob/release/21.x/compiler-rt/lib/asan/asan_shadow_setup.cpp).
This resolves the independently reproduced startup OOM, but the hardware gate
still fails on an independently reproduced native-library leak. No leak is
suppressed or counted as a pass. Passing other profiles or device memcheck does
not imply complete sanitizer/hardware acceptance.
If the ASan consumer fails initialization, an independent C-only CUDA control
records whether SDK startup also fails without Seen codegen/runtime or staging.
It uses the same strict sanitizer runner; its result never replaces the original
failure. Each executed case retains its own log and status, including failures.

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
