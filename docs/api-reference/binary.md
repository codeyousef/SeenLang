# Binary Serialization

## @derive(Serialize, Deserialize)

Auto-generate binary serialization for classes:

```seen
@derive(Serialize, Deserialize)
class GameState {
    var level: Int
    var score: Int
    var playerName: String
}
```

This generates `serialize()` and `deserialize()` methods that encode/decode the class fields to/from binary format.

## Binary Buffer

Low-level binary buffer API:

```seen
import core.binary
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `newBinaryBuffer` | `(capacity: Int) r: Int` | Create buffer (returns handle) |
| `binaryBufferLength` | `(handle: Int) r: Int` | Get buffer length |

### Runtime Functions

| Function | Description |
|----------|-------------|
| `__BinaryBufferNew(capacity)` | Create binary buffer |
| `__BinaryBufferData(handle)` | Get data pointer |
| `__BinaryBufferLength(handle)` | Get length |
| `__BinaryWriteI64(handle, val)` | Write 64-bit integer |
| `__BinaryReadI64(handle, offset)` | Read 64-bit integer |
| `__BinaryWriteF64(handle, val)` | Write 64-bit float |
| `__BinaryReadF64(handle, offset)` | Read 64-bit float |

### Example

```seen
let buf = newBinaryBuffer(1024)
__BinaryWriteI64(buf, 42)
__BinaryWriteF64(buf, 3.14)
let len = binaryBufferLength(buf)
println("Buffer size: {len}")
```

## RLE Compression

Run-length encoding for binary data:

```seen
import core.compress
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `compressRLE` | `(src: Int, srcLen: Int, dst: Int, dstCap: Int) r: Int` | Compress data |
| `decompressRLE` | `(src: Int, srcLen: Int, dst: Int, dstCap: Int) r: Int` | Decompress data |

Returns the output length, or -1 on error.

## Packets

Network packet utilities:

```seen
import core.packet
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `packetHeaderId` | `(header: Int) r: Int` | Extract packet ID from header |
| `packetHeaderLength` | `(header: Int) r: Int` | Extract payload length from header |

### @packet Decorator

For network protocol types:

```seen
@packet(id = 1)
class LoginRequest {
    var username: String
    var password: String
}
```

## @binary Decorator

Mark a class for binary layout:

```seen
@binary
class RawData {
    var header: Int
    var payload_size: Int
    var checksum: Int
}
```
