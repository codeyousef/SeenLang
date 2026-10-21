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
keeps that context alive until `reserve` returns. A successful reservation owns
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
