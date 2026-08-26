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
