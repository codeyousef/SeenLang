# I/O

## File I/O

### Production filesystem API

New code should import `fs.mod`. `OsString` and `Path` preserve the
length-delimited operating-system path bytes and reject empty, oversized, or
NUL-containing values before an ABI call. `File` is move-only and accepts an
`OperationContext`, `Path`, `OpenOptions`, and explicit `DirectIoPolicy`.

The production surface provides checked 64-bit positional reads and writes,
metadata, durability, preallocation, truncation, sparse-hole punching, space
queries, advisory locks, directory iteration, symbolic links, exclusive
temporary files, crash-safe atomic replacement, confined child paths,
non-following recursive deletion, and cross-filesystem tree moves. All
operations return stable `fs.*` errors; cleanup is explicit and idempotent.

`io.direct` adds bounded positional gather reads and writes. Gather functions
accept the opaque `File.handle` value without transferring the move-only
`File`; the caller remains responsible for synchronization and close. `Off` never asks
the operating system for direct I/O. `Preferred` may fall back only during
open and exposes `fallbackCount`; `Required` fails if direct I/O cannot be
established. Active direct I/O rejects misaligned offsets, lengths, and
buffers. Partial completion is reported rather than silently retried.

The older `io.file.FsFile` and scalar helpers below remain for bootstrap and
source compatibility. They are not the production filesystem ownership API.

| Production function/type | Contract |
|--------------------------|----------|
| `Path.fromString` | Preserve path bytes and reject empty, oversized, or embedded-NUL values |
| `File.open` | Open a move-only handle with explicit options and direct-I/O policy |
| `File.readAt` / `writeAt` | Checked positional I/O with exact partial progress |
| `Directory.open` / `next` | Owned bounded directory iteration with explicit close |
| `createDirectoryPath` | Context-aware directory creation for a checked `Path` |
| `confinedChild` | Reject absolute, empty, dot, and parent traversal components |
| `atomicReplace` | Adjacent exclusive temporary file, data sync, rename, and optional directory sync |
| `moveAcrossFilesystems` | Rename first, then bounded copy/sync/remove only on `EXDEV` |
| `readGather` / `writeGather` | At most 64 validated segments with explicit partial completion |

### High-Level Functions

```seen
import io.file
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `readText` | `(path: String) r: String` | Read entire file to string |
| `writeText` | `(path: String, content: String) r: Bool` | Write string to file |
| `appendText` | `(path: String, content: String) r: Bool` | Append string to file |
| `exists` | `(path: String) r: Bool` | Check if file exists |
| `deleteFile` | `(path: String) r: Bool` | Delete a file |
| `createDirectory` | `(path: String) r: Bool` | Create a directory |
| `ensureParentDirectories` | `(path: String) r: Bool` | Create parent directories needed for a file path |
| `writeLines` | `(path: String, lines: Array<String>) r: Bool` | Write lines with newline separators |
| `size` | `(path: String) r: Int` | Return file size, or `-1` on failure |
| `hash` | `(path: String) r: String` | Return a stable file-content hash string |

### Example

```seen
let content = readText("config.toml")
println("Config: {content}")

writeText("output.txt", "Hello, World!")
```

### FsFile Class

For more control over file operations:

```seen
let file = FsFile.open("data.txt")
let content = file.readContent()
file.closeFile()
```

| Method | Return | Description |
|--------|--------|-------------|
| `open(path: String)` | `FsFile` | Open for reading |
| `create(path: String)` | `FsFile` | Open for writing (create/truncate) |
| `readContent()` | `String` | Read all content |
| `read_bytes(size: Int)` | `Array<Int>` | Read N bytes |
| `writeContent(content: String)` | `Void` | Write string |
| `write_bytes(data: Array<Int>)` | `Void` | Write bytes |
| `closeFile()` | `Void` | Close file |
| `size()` | `Int` | Get file size |

`FsFileResult` represents file-open/read outcomes that need to carry success
state alongside file data or errors.

### Runtime File Functions

| Function | Description |
|----------|-------------|
| `__OpenFile(path, mode)` | Open file, return fd (-1 on error) |
| `__ReadFile(fd)` | Read entire content |
| `__ReadFileBytes(fd, size)` | Read N bytes |
| `__WriteFile(fd, content)` | Write string |
| `__WriteFileBytes(fd, bytes)` | Write byte array |
| `__CloseFile(fd)` | Close file descriptor |
| `__FileSize(fd)` | Get file size |
| `__FileError(fd)` | Get error message |
| `__FileExists(path)` | Check existence |
| `__DeleteFile(path)` | Delete file |
| `__CreateDirectory(path)` | Create directory |

## Asynchronous byte I/O

Import `io.bytes` for `AsyncReader`, `AsyncWriter`, `AsyncSeeker`,
`AsyncBufRead`, and `AsyncFlush`. Their operations accept an
`OperationContext` and checked `ByteSlice` or `MutableByteSlice` views.
`AsyncByteReader.fromArray` and `tryAsyncByteWriterWithLimit` provide move-only,
bounded in-memory owners. They are direct-ready coroutine operations, not
kernel asynchronous file I/O, background workers, or reactor readiness waits.

Use `await` with the concrete methods or the free entrypoints `asyncRead`,
`asyncWrite`, `asyncSeek`, `asyncFillBuf`, `asyncConsume`, and `asyncFlush`.
Read/write return `Result<Int, IoError>` with the exact transferred count;
short progress and zero-byte EOF are valid results, not implicit retries.
Cancelled contexts fail before transfer, and closed/stale views are rejected
through the existing `byte.*` error contract. Writer limits are explicit.

Keep the backing owner alive while a view is borrowed. Close a buffered view
before consuming its bytes, close acquired views before owner cleanup, and
call the stream owner's `close()` explicitly. A repeated successful close
returns `Ok(false)`. The [compiling example](../../seen_std/examples/async_byte_streams.seen)
shows checked view acquisition, an awaited read, and ordered cleanup.

## Bounded completion I/O

Import `io.completion_types` and `io.completion_queue`. `IoQueue` is a
coordinator-owned state machine for platform adapters. It introduces no native
submission mechanism and never silently selects a worker or blocking fallback.
The existing Linux `async.completion_io` API remains available separately.
The new surface is versioned in standard library 0.11.0; it adds no runtime ABI.

`IoQueueLimits` requires depth 1–65,536, transfers 1 byte–16 MiB, an aggregate
buffer budget at least as large as one transfer and at most 512 MiB, and a
metadata string budget 1 byte–64 MiB. Arrays are allocated once for the bounded
depth. Every live request retains its full buffer length and copied trace and
resource identity lengths against those budgets until `poll` transfers ownership.
Descriptor count is bounded by depth. Admission never allocates an unbounded
copy before validating the context and checking all budgets.

`IoRequest.create` consumes one independently owned `OwnedHandle` and one
`OwnedBuffer<UInt8>`. Its context is borrowed until admission, so the caller
keeps that context alive until admission succeeds or the rejected request is
released/rebound before retry. A successful reservation owns
a bounded context snapshot. Cancellation is then requested through the queue;
mutating the original context does not mutate the snapshot. Negative or
overflowing file ranges, invalid owners, overlapping full buffer ranges, and
duplicate close ownership of one native descriptor are rejected before borrow.
Other queues and external borrowers must respect the same exclusive ownership
contract; the coordinator cannot discover raw aliases created outside its API.

`reserve` always consumes the request. On rejection its typed
`IoRejectedSubmission` returns the request and an `AsyncError`; the caller may
recover those owners or call the rejection's `release`. No rejected request is
detached or freed implicitly. Tokens combine a slot and a 32-bit generation.
An exhausted generation retires the slot rather than permitting stale-token
reuse. Saturation returns `async.limit`.

| Transition | Adapter responsibility |
|------------|------------------------|
| `reserve` → queued | Queue owns all resources; no adapter borrow exists |
| `submission(token)` → preparing | One borrowed `IoSubmissionView`; a second view is rejected |
| `submitted(token)` → submitted | Mark before publishing storage to the OS; queue retains owners |
| `rejectSubmission` → terminal | Allowed only while queued/preparing and after proving no OS borrow exists |
| `complete` → terminal | Submitted requests only; terminal OS acknowledgement ends the borrow |
| `poll` → empty | FIFO terminal delivery moves the request into one `IoCompletion` |

The adapter frees each view wrapper after use. Its raw descriptor and pointer
remain valid only while preparing/submitted, and it must stop using them before
terminal acknowledgement. A failed OS submission after `submitted` must use
`complete` once the adapter proves that no borrow remains. Invalid progress,
nonterminal status 8, duplicate completion, and stale tokens leave owners and
accounting unchanged. Successful short I/O is an explicit partial result;
`IoCompletion.partial` and `outcome` expose exact progress without retrying.

`cancel` completes queued requests immediately. Preparing or submitted requests
retain all owners until the adapter rejects or acknowledges them. The first
observed cancellation/deadline reason is preserved, including partial progress.
`nextCancellation` delivers each adapter action once; retrying that native
cancellation action is the adapter/caller's explicit policy. It also observes
monotonic deadlines. Late successful acknowledgements report expiration instead
of erasing it. A failed clock reports `async.unavailable`.

`shutdown` stops admission and requests cancellation. Drain terminal results
through `poll`, acknowledge every outstanding adapter borrow, then call `close`.
A busy close returns `async.limit` and retains the queue; successful close is
idempotent. Completion/rejection `release` closes the descriptor before freeing
the buffer and context snapshot. A close failure retains the owner for an
explicit retry. Queue `release` succeeds only after drain and frees its arrays.

All mutation belongs to one serialized coordinator. Worker/native callbacks
must deliver completion records back to that coordinator instead of mutating
these arrays concurrently. Do not mutate the public implementation fields.
`async.invalid`, `async.limit`, `async.unavailable`, `async.unsupported_platform`,
`async.timeout`, and `async.cancelled` retain bounded operation locations and
progress. Native result numbers remain in `request.resources.nativeResult`;
the coordinator never infers retry policy from a foreign platform's errno.
Backend availability and alignment requirements must be checked by each adapter
before it publishes a request. Absence of an adapter is unsupported, with no
implicit fallback.

`tests/misc_root_tests/seen_async_io_a.sh` runs the contract in fast, release,
UBSan and ASan profiles plus a 60-second transition fuzz. It checks exactly-once
FIFO delivery, two simultaneously submitted requests, late acknowledgements,
invalid transitions, aliases, checked geometry, bounded budgets, generation
retirement and a repeated live-allocation baseline. Its machine-readable record
uses `seen-io-contract-v1`, `backend: simulated`, and
`native_overlap_verified: false`. This proves coordinator behavior only.
Linux fixtures use owned pipe handles; Linux arm64, Windows and macOS adapter
execution and actual file-I/O overlap require their own platform evidence.
Source/frontend checks alone do not establish executable platform verification.

## Standard I/O

### Output

| Function | Description |
|----------|-------------|
| `println(s: String)` | Print string with newline |
| `__Print(s: String)` | Print without newline |
| `__PrintInt(n: Int)` | Print integer |
| `__PrintFloat(f: Float)` | Print float |
| `__PrintRaw(s: String)` | Print without newline (for LSP headers) |
| `__FlushStdout()` | Flush stdout buffer |

### Input

| Function | Description |
|----------|-------------|
| `__ReadStdinLine()` | Read one line from stdin (blocking) |
| `__ReadStdinBytes(count: Int)` | Read exactly N bytes from stdin |

### StdinReader Class

```seen
import io.stdio
```

| Method | Return | Description |
|--------|--------|-------------|
| `nextLine()` | `String` | Read next line |
| `isEof()` | `Bool` | Check end of input |

### ContentLengthReader

For LSP/JSON-RPC message framing:

```seen
let reader = ContentLengthReader.new()
let message = reader.readMessage()
let err = reader.getLastError()
```

## Buffered I/O

```seen
import io.buffered
```

- `BufferedWriter` -- buffered output
- `BufferedReader` -- buffered input

## Path Operations

```seen
import fs.path
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `isAbsolute` | `(path: String) r: Bool` | Check absolute path |
| `normalize` | `(path: String) r: String` | Normalize path |
| `joinPath` | `(parts: Array<String>) r: String` | Join path components |
| `basename` | `(path: String) r: String` | Get filename |
| `dirname` | `(path: String) r: String` | Get directory |
| `pathExtension` | `(path: String) r: String` | Get file extension |
| `withoutExtension` | `(path: String) r: String` | Remove extension |
| `splitComponents` | `(path: String) r: Array<String>` | Split into components |

### Example

```seen
let full = joinPath(["home", "user", "docs", "file.txt"])
let dir = dirname(full)       // "home/user/docs"
let file = basename(full)     // "file.txt"
let ext = pathExtension(full) // "txt"
```

## Linux io_uring adapter

`io.linux_uring.LinuxUringQueue.create(limits, maxMappedBytes, maxWorkers)`
adds an explicit Linux adapter to the completion contracts. The standard
library version is 0.12.0 with runtime ABI `runtime-v10`. It accepts queue depth
1–32,768, mapping budget 1 byte–8 MiB, and 1–64 workers in each kernel io-wq
worker category. The core transfer, descriptor, buffer and metadata ceilings
still apply. The kernel's returned queue sizes and every head/tail, mask, entry,
array and completion offset are validated before ring access. Mapping budgets
include the actual base page rounding, with supported page sizes 4–64 KiB.
Overlapping structures, invalid ring distances, changed masks, dropped SQEs,
CQ overflow, multishot flags and invalid acknowledgements fail closed.

The constructor requires `IORING_FEAT_NODROP`, an ordinary 64-byte SQE / 16-byte
CQE layout, and successful kernel worker-limit registration. Eventfd registration
provides reactor readiness notification through `notificationLease`; deregister
that borrower and release its lease before close. File descriptors and buffers
remain ordinary independently owned request resources. They are not pinned in
a permanent registered-file/buffer table, because their ownership returns at
each terminal delivery. Only eventfd is registered for the queue lifetime.

`io.uring_file.uringFileHandle` borrows a production `fs.file.File` and returns
an independently owned, close-on-exec duplicate. Buffered files and explicitly
direct files are supported; the adapter never changes the file's direct-I/O
policy. Kernel errors such as direct-I/O alignment failures are reported with
no buffered retry. The source `File` can close after duplication.

A successful `reserve` transfers ownership to `core`; `submit(token)` encodes
one SQE and marks the request submitted before release-publishing the tail.
`flush` invokes `io_uring_enter` without waiting for completions. Its returned
count may be partial. A failed or interrupted enter retains every published
owner, and callers explicitly choose whether to flush again, cancel or drain.
There is no automatic resubmission. `reap(maximum)` retires at most twice the
core depth, using acquire/release ring atomics and exact checked progress.
`poll` then delivers the core's terminal FIFO.

`cancel` records the request; `publishCancellation` publishes at most one native
action, also observing deadlines through the core. On SQ or cancellation-record
saturation, the pending action remains owned for a subsequent explicit call.
Cancellation acknowledgements use reserved user token zero and never release
the target request's resources. Only the target's own CQE ends that borrow.
`lastCancellationResult` preserves the latest raw Linux cancellation result,
including completion races such as `-ENOENT`. It does not trigger a retry.
Original generation tokens always remain nonzero, including at wraparound.

The adapter is single-coordinator. Native/kernel producers only update their
UAPI ring fields. All ring encoding, geometry, queue policy, cancellation,
deadlines, worker ceilings and ownership live in Seen. The native boundary
contains direct syscalls, mapping, descriptor duplication, eventfd, base page
size and 32-bit acquire/release operations. It introduces no liburing dependency.
`io.uring_native` is internal ABI; its results use negative **Linux** errno on
every platform. `lastNativeResult` and stable `io.uring.*` error operations
provide bounded diagnostic evidence. A disabled/denied kernel, unsupported
architecture, missing no-drop feature or unavailable worker-limit registration
is an explicit unsupported capability. There is no worker/AIO fallback.

Creation returns `UringCreateFailure` on error. Its optional `partial` queue
owns every successfully created resource, including mappings after partial
construction. Call `release` on that failure or recover the partial owner;
a failed cleanup retains it for retry. Shutdown stops admission and records
cancellation but does not wait or detach. Continue publishing/flush/reap/poll,
release notification leases, then close. Close also waits for cancellation CQEs
and published SQEs. It closes OS handles before unmapping, and retains each
unreleased mapping on error. Successful close is idempotent; release destroys
the empty core and lease counter. Malformed kernel evidence poisons the adapter
and preserves outstanding owners; it never fabricates acknowledgements or
frees memory that might still be borrowed.

`tests/misc_root_tests/seen_async_io_b.sh` runs mocked ring contracts in
fast/release/UBSan/ASan, a 60-second layout fuzz, and real Linux positional
writes/reads, cancellation races, eventfd leases and repeated cleanup when the
host permits io_uring. The `seen-io-uring-v1` JSON record distinguishes
`unsupported` from `verified` file-contract execution. Set
`SEEN_REQUIRE_IO_URING=1` to require native execution on a designated runner.
Two requests are published before the first enter/retirement; this is submission
overlap evidence. Physical storage execution overlap remains unmeasured and
`native_overlap_verified` stays false. No throughput claim is made.

Linux x86-64 is the executable fixture target. Linux arm64 has the UAPI/syscall
implementation but still requires platform execution evidence. Windows and
macOS explicitly reject this backend; their own adapters are separate APIs.
Frontend and C syntax checks are source validation and do not certify any
platform's execution or hardware behavior.

## Windows overlapped completion adapter

`io.windows_iocp.WindowsIocpQueue.create(limits, maxOverlappedBytes)` uses the
same bounded `IoQueue` transitions and owner-returning rejections. Standard
library 0.13.0 requires runtime ABI `runtime-v11`. Depth is 1–65,536; one
32-byte, 16-byte-aligned OVERLAPPED record per slot is preallocated inside an
explicit budget of at most 2 MiB. Core buffer, transfer and metadata limits
apply independently. The IOCP concurrency hint is one, and Seen mutations
belong to one serialized coordinator. The adapter spawns no worker threads.

`reserveFile(path, options, buffer, operation, offset, bytes, context)` consumes
the buffer on both success and failure. It validates shared admission geometry,
contexts, aliases, quotas and deadlines before opening the file, then obtains a
fresh overlapped file object for that request. Queued admission accepts existing
files only; creation, truncation, append and directory operations are rejected.
For explicit file preparation, `io.iocp_file.iocpOpenHandle` accepts the supported
`fs.types.OpenOptions` modes and returns an independently owned handle to close.
No direct-I/O or blocking fallback is selected. Paths are bounded to 4,095 UTF-8
bytes, reject embedded NUL and undergo strict Win32 UTF-16 normalization into
Seen-owned bounded temporary storage. Sharing allows read/write/delete; no-follow
options request `FILE_FLAG_OPEN_REPARSE_POINT` explicitly.

Each request owns a fresh `CreateFileW` file object. The adapter deliberately
does not admit arbitrary or duplicated raw handles: an IOCP association's key
is immutable and duplicate handles can share that association. A fresh object
can carry the full core generation token as its completion key. Both that key
and the exact slot's OVERLAPPED address must match before any acknowledgement.
Reusing a record address for a later generation therefore does not authorize a
stale packet. Returned `IoRejectedSubmission` objects own every acquired handle
and buffer, including failures after opening but before core admission.

`submit` encodes the checked 64-bit offset, leaves `hEvent` zero, associates the
fresh file with the port, and marks the core submitted before calling
`ReadFile`/`WriteFile`. Both immediate success and `ERROR_IO_PENDING` retain the
record, handle and buffer until the target completion packet. `ERROR_MORE_DATA`
also retains that packet ownership. Other immediate failures have no pending
borrow: a terminal result is queued and the submit call returns the typed error.
Immediate EOF produces an explicit zero-byte partial success. Even when submit
returns an error, drain the queued completion to reclaim its owner.

`reap(maximum)` consumes at most one core depth of records without blocking.
A wait timeout with no OVERLAPPED is empty readiness, rather than a fabricated
completion. Failed packets with an OVERLAPPED preserve their Win32 status and
partial bytes. Unknown generation keys, wrong pointers, duplicate packets,
pending/timeout pseudo-statuses or oversized progress poison the adapter and
retain any outstanding owners. `lastNativeResult` retains Win32 evidence;
`iocpStatus` translates only the Windows vocabulary into stable `async.*`
codes and `io.iocp.*` operations. Non-Windows targets return explicit
`ERROR_NOT_SUPPORTED`; an errno is never interpreted as a Win32 status.

`cancel` records intent. `publishCancellation` issues at most one `CancelIoEx`
action, including deadline actions. A cancellation success or `ERROR_NOT_FOUND`
completion race does not acknowledge the original request. Its own packet is
still required before record or buffer reuse. The latest raw cancellation
result is exposed, and failed actions remain owned for explicit caller retry.
There is no automatic retry. The first observed cancellation/deadline reason
and exact progress remain in the core completion even if a late packet succeeds.

`shutdown` stops admission and records cancellation. Continue cancellation,
reap and poll until all owners are delivered, then close. A busy close returns
`async.limit`; the OVERLAPPED pool stays live. Close the port before freeing
that pool, retain it on a close error, and use `release` after successful drain.
Successful close is idempotent. Creation failures contain a typed error and raw
Win32 result; all construction prefixes are released before returning them.
The `completion_bytes` helpers and borrow-only `ioQueuePreflight` are shared by
the Linux/Windows adapters to keep byte encoding and admission in one place.

`tests/misc_root_tests/seen_async_io_c.sh` runs Linux-hosted mocked contracts in
fast/release/UBSan/ASan, a 60-second completion fuzz, and an explicit unsupported
native probe on other operating systems. `iocp_windows.seen` additionally tests
real overlapped writes/reads, EOF, cancellation races and repeated cleanup on a
Windows runner. Its `seen-io-iocp-v1` JSON distinguishes unsupported from verified
file-contract execution. `SEEN_REQUIRE_IOCP=1` requires native verification.
Windows runs should invoke the native fixture with a fresh path in their ignored
artifact directory. The Linux mock uses pipe-owned handles and does not certify
Windows execution. Two outstanding submissions are recorded separately from
unmeasured physical-storage overlap; `native_overlap_verified` remains false.
Linux arm64/macOS/Windows platform execution still requires its own evidence;
syntax and frontend checks alone are compile-only source validation.

## macOS positional workers and F_NOCACHE

`io.macos_positional.MacPositionalQueue.create(limits, workers, stackBytes,
context)` explicitly selects blocking positional I/O on bounded workers.
Standard library 0.14.0 requires runtime ABI `runtime-v12`. This is a worker
backend, not io_uring or kernel asynchronous file I/O. Non-macOS hosts return
`async.unsupported_platform` before creating workers. The existing pthread
adapter supports inherited placement on macOS x86-64/arm64; explicit Linux
CPU/NUMA placement remains unsupported there.

The shared `io.worker_queue.WorkerCompletionQueue` uses the existing typed
blocking pool. Depth is capped at 65,536, workers at 16 and at queue depth,
individual stacks at 80 KiB–8 MiB, and helper depth at one. Core transfer, buffer
and request metadata ceilings still apply. One fixed joins/outcomes array per
depth bounds coordinator storage. A separate owned drain context preserves at
most 4,096 identity bytes and is used only to collect already terminal jobs.
It ignores a request's expired wait context while retaining that request's
cancellation/deadline result. The executor also owns one context snapshot per
scheduled job; total request identity storage is therefore bounded by twice
`maxMetadataBytes`, plus the 4,096-byte drain snapshot. Constructor prefixes release their owners on
failure; no extra unbounded pool or background fallback is created.

`reserveFile` consumes a buffer, opens an existing regular file under the shared
borrow-only admission checks, and returns the same owner-carrying rejection
contract as other adapters. Creation/truncation are separate explicit
`io.macos_file.macosOpenHandle` operations. Paths retain their POSIX bytes,
reject embedded NUL, and are bounded to 4,095 bytes. Fresh opens use CLOEXEC and
NONBLOCK, respect no-follow options, and reject non-regular files. Each request
gets an independent open file description: F_NOCACHE cannot change a borrowed
source `File` or another request's cache policy.

Choose `MacCachePolicy.Cached` or `UncachedRequired`. The latter must successfully
set F_NOCACHE; failures retain the descriptor in the rejection owner and never
retry in cached mode. F_NOCACHE requests bypass of the filesystem data cache;
it does not establish O_DIRECT alignment guarantees or certify physical storage
behavior. Darwin flag geometry is asserted against the platform SDK. The
native boundary contains only open/fstat/fcntl/pread/pwrite and preserves
negative Darwin errno even on unsupported targets. All admission, cache policy,
worker scheduling, ownership, cancellation and result interpretation stay in Seen.

A `MacPositionalJob` borrows only one checked descriptor/buffer range and invokes
one positional syscall. Short results and EINTR are explicit results; neither
is retried automatically. Queued cancellation can prevent work. Once a worker
has entered a blocking syscall, cancellation/deadline observation is cooperative
and cannot preempt the kernel call. The queue retains the descriptor, buffer
and completion storage until that worker reaches terminal publication and is
joined. Shutdown/drain may therefore depend on filesystem/device responsiveness;
it never detaches the operation or claims a hard I/O latency bound.

Worker outcome storage belongs to the coordinator independently of the job
object. This preserves actual partial progress and raw native results even
when the executor releases a cancelled/panicked job. Only the worker writes it
while running; the coordinator reads it after the executor's terminal mutex
publication and successful join. An unknown token, oversized progress or invalid
status after a known synchronous join becomes a terminal unavailable result
with zero trusted progress, retaining raw evidence and returning an explicit
error. A successful join proves that the original borrow ended; it does not
invent a driver acknowledgement for an unrelated request.

`prepare`/`submit` are adapter support operations. A preparation view and outcome
borrow must be released before `rejectPrepared` acknowledges it. Cancellation
or shutdown retains preparation owners until that acknowledgement. A submitted
job destructor consumes only its own storage, never queue-owned outcomes or
borrowed request resources. Jobs must be synchronous and must not retain those
borrows after returning. These low-level operations are for adapter authors;
the macOS wrapper performs preparation/submission in one serialized call.

`reap` scans at most one queue depth and collects only terminal jobs. Executor
readiness briefly locks bookkeeping and never waits for user work to finish.
`poll` transfers terminal owners and releases their independent outcome records.
`shutdown` stops admission and records cancellation; continue cancellation,
reap and poll before closing. A busy close retains every owner. Close joins
workers before releasing their queue; successful close is idempotent. Release
then destroys the pool, core, arrays and drain snapshot. Queued deadline expiry
also becomes a terminal timeout without acquiring a worker/native borrow.

`tests/misc_root_tests/seen_async_io_d.sh` adds fast/release/UBSan/ASan contracts,
a 60-second worker fuzz, and a native macOS probe. Linux-hosted worker fixtures
use real fixed workers and atomics to prove two jobs are simultaneously running,
partial progress survives cancellation, preparation views survive shutdown,
invalid joined results fail explicitly and repeated lifecycle allocations return
to baseline. These jobs simulate file operations and do not certify Darwin I/O.
On a Mac, `macos_positional.seen` exercises cached/F_NOCACHE positional writes,
reads, cancellation and repeated cleanup. `seen-io-macos-v1` records distinguish
unsupported from verified file-contract execution. Set
`SEEN_REQUIRE_MACOS_POSITIONAL=1` on a designated runner to require the latter.
Physical device overlap remains unmeasured. Linux/Windows syntax and Seen
frontend checks are source validation, not a Darwin SDK or runtime certification.

Completion-boundary errors use the static `completion-io` trace tag when a
dependency error would otherwise borrow a context string. Those errors can
outlive released snapshots; request trace/resource identities remain in the
owned admitted context until completion release.

## Shared adverse completion and device removal

Standard library 0.15.0 adds `IO_DEVICE_REMOVED_STATUS` and the stable
`io.device_removed` error without changing runtime ABI `runtime-v12`. Linux and
Darwin ENODEV/ENXIO and Windows ERROR_DEVICE_NOT_CONNECTED/ERROR_DEV_NOT_EXIST/
ERROR_NO_SUCH_DEVICE normalize to that status in Seen. Generic I/O errors,
access denial and interrupted calls remain distinct unavailable/invalid results;
they do not prove removal. Native error numbers are interpreted only in their
own backend vocabulary, and no adapter chooses retry or fallback policy.

`deviceRemoved(nativeResult)` on a queue is an explicit caller/backend-session
notification. It permanently stops new admission and marks active requests.
Use it only when the whole queue's device/session is known to be lost; one
request's native device error does not automatically invalidate unrelated files
on other devices. Tokens belong to their original queue and cannot be moved to
a replacement queue. Repeated notifications are idempotent. A replacement
backend/device requires a fresh queue.

Queued requests terminate immediately because no adapter borrow exists.
Preparing/submitted requests retain descriptors, buffers, budgets and context
snapshots. The event is a request to stop, never an invented native
acknowledgement. Preparing adapters must release their view and reject or call
`submitted`; a pending stop makes `submitted` terminate without publication.
Native/kernel borrows still require their own completion record or synchronous
worker join. Close remains busy until acknowledgement and terminal delivery.
A lost device/driver that never acknowledges remains quarantined with its
owners; neither timeout nor shutdown frees memory that could still be borrowed.

The first **observed** stop reason wins. An already recorded cancellation or
deadline remains authoritative if removal follows. Removal observed first stays
`io.device_removed` if cancellation or late success follows. A previously
terminal result is immutable. Merely having an expired clock is not an observed
queue event until submission, cancellation polling or completion checks it.
Successful short reads and EOF are explicit partial successes, never retries.
Errors and late successes preserve exact trusted byte progress, including a
full transfer followed by an adverse result. Invalid progress or duplicate
acknowledgements leave ownership unchanged.

`IoRequest.nativeEvent: Option<Int64>` retains explicit removal evidence
separately from the terminal backend `resources.nativeResult`. `Some(0)` is a
recorded event with a zero native result and is distinct from `None`. Both
survive terminal delivery in the owned completion. Each request owns and frees
its event wrapper. Typed errors are nonretryable by default: callers must use
progress and their own idempotency/device policy before choosing another action.

`tests/misc_root_tests/seen_async_io_e.sh` runs the cross-backend error matrix
and adverse lifecycle in fast/release/UBSan/ASan plus 60 seconds of seeded mixed
queued/preparing/submitted removal fuzz. Queue invariants are shared with the
ordinary transition fuzz. Coverage includes first-reason ordering, partial/EOF,
zero-valued event evidence, blocked new admission, late/duplicate acknowledgements,
retained budgets and repeated live-allocation baselines. `seen-io-adverse-v1`
records model execution separately from physical device hot-unplug, which
remains unverified until designated hardware testing. No hardware fault or
platform certification is inferred from source/frontend validation.

## Completion diagnostics and overlap evidence

Standard library 0.16.0 adds coordinator-owned `IoQueue.diagnostics` without
changing runtime ABI `runtime-v12`. `IoQueueDiagnostics` has fixed-size scalar
counters for accepted/rejected requests, committed submissions, terminal results,
delivered owners, bytes and adverse outcomes. Peaks track active/submitted
requests, owned buffer bytes and metadata. `concurrentSubmissionCount` counts
submissions made while another adapter borrow remains outstanding; it measures
queue concurrency and cannot establish physical I/O overlap.

All counters saturate at UInt64 maximum and set `saturated`, rather than wrapping.
Saturated counters cannot certify exact lifecycle accounting. `snapshot()` returns
an independent owned record; the caller frees it, and it may outlive the queue.
Only the serialized coordinator reads or updates diagnostics. Worker jobs never
touch them. Diagnostics retain no tokens, strings or per-request history and
consume constant memory. Releasing the queue frees its diagnostic record.

`tests/misc_root_tests/seen_async_io_f.sh` tests snapshot lifetime, overflow and
exact cleanup in fast/release/UBSan/ASan, then compiles a Seen overlap witness.
Two fixed worker jobs enter actual blocking Linux pipe reads. A supervising
collector observes both read syscalls through `/proc`, acknowledges the
checkpoint, and lets the coordinator perform a PID-seeded CPU calculation.
The collector observes both reads still blocked after the calculation before
allowing the coordinator to write the payloads. Only joined workers' intervals
are read, and both intervals must bracket the calculation and kernel observations.
Payload checks and unique completion tokens verify both results exactly once.

Seven acknowledged checkpoints capture managed bytes, process RSS/high-water
RSS, descriptor counts and queue peaks. Ten thousand further two-request rounds
must return to the live-allocation baseline after every round. Final descriptor
and managed allocation counts return to their original baselines. RSS high-water
growth is limited to 64 MiB (512 MiB under ASan), further capped to one eighth of
the actual aggregate memory limit. The existing hard-scope wrapper verifies the
transient cgroup; the collector independently reads finite memory/pid limits,
zero swap, group OOM handling and identical child membership. Execution has a
120-second ceiling and bounded output. Missing `/proc` permissions or kernel
observations fail certification explicitly.

`seen-io-overlap-v1` reports retain source commit, source/executable/compiler
hashes, command, machine identity, containment readback, syscall observations and
resource samples. `scripts/io_overlap_evidence.py` validates their strict schema,
checksum, lifecycle, intervals and resource bounds. CI retains validated reports
for 14 days. This witness classifies Linux kernel-pipe I/O/compute overlap only;
`storage_overlap_verified` is always false. Native file/accelerator overlap and
other operating systems require their own execution evidence. These interval
checks are correctness witnesses, not throughput benchmarks or storage results.
