# Bitfield

Packed bit manipulation types with network byte order support.

```seen
import core.bitfield
```

## Types

Four bitfield sizes are available:

| Type | Bits | Range |
|------|------|-------|
| `Bitfield8` | 8 | 0-255 |
| `Bitfield16` | 16 | 0-65535 |
| `Bitfield32` | 32 | 0-4294967295 |
| `Bitfield64` | 64 | 0-2^64-1 |

## Construction

```seen
let bf = Bitfield8.new(0xFF)
let bf = Bitfield8.zero()
let bf = Bitfield8.fromBits(1, 0, 1, 1, 0, 0, 1, 0)
```

Convenience constructors:

```seen
let bf = bitfield8(42)
let bf = bitfield16(1024)
let bf = bitfield32(0xDEADBEEF)
let bf = bitfield64(0)
```

## Bit Operations

All types share the same methods:

| Method | Return | Description |
|--------|--------|-------------|
| `getBit(index: Int)` | `Int` | Get bit at position (0 or 1) |
| `setBit(index: Int, value: Int)` | `Bitfield*` | Set bit, return new |
| `getField(start: Int, length: Int)` | `Int` | Extract bit field |
| `setField(start: Int, length: Int, value: Int)` | `Bitfield*` | Set bit field, return new |

## Bitwise Operations

| Method | Return | Description |
|--------|--------|-------------|
| `bitwiseAnd(other: Bitfield*)` | `Bitfield*` | AND |
| `bitwiseOr(other: Bitfield*)` | `Bitfield*` | OR |
| `bitwiseXor(other: Bitfield*)` | `Bitfield*` | XOR |
| `bitwiseNot()` | `Bitfield*` | NOT |
| `shiftLeft(amount: Int)` | `Bitfield*` | Left shift |
| `shiftRight(amount: Int)` | `Bitfield*` | Right shift |

## Bit Counting

| Method | Return | Description |
|--------|--------|-------------|
| `countOnes()` | `Int` | Population count |
| `countZeros()` | `Int` | Zero count |
| `leadingZeros()` | `Int` | Leading zero count |
| `trailingZeros()` | `Int` | Trailing zero count |

## Conversion

| Method | Return | Description |
|--------|--------|-------------|
| `toInt()` | `Int` | Convert to integer |
| `toUnsigned()` | `Int` | Convert to unsigned integer |
| `toBinaryString()` | `String` | Binary representation |
| `toHexString()` | `String` | Hexadecimal representation |
| `toString()` | `String` | String representation |

## Endianness Conversion

| Method | Return | Description |
|--------|--------|-------------|
| `toBigEndian()` | `Bitfield*` | Convert to big endian |
| `toLittleEndian()` | `Bitfield*` | Convert to little endian |

### Network Byte Order Functions

| Function | Description |
|----------|-------------|
| `htons(value)` | Host to network short (16-bit) |
| `htonl(value)` | Host to network long (32-bit) |
| `htonll(value)` | Host to network long long (64-bit) |
| `ntohs(value)` | Network to host short |
| `ntohl(value)` | Network to host long |
| `ntohll(value)` | Network to host long long |

## Utility

```seen
fun pow2(n: Int) r: Int  // 2^n
```

## Example

```seen
let flags = Bitfield8.zero()
let flags = flags.setBit(0, 1)  // set bit 0
let flags = flags.setBit(3, 1)  // set bit 3
println(flags.toBinaryString()) // "00001001"
println("Bit 0: {flags.getBit(0)}")  // 1
println("Ones: {flags.countOnes()}")  // 2

// Network packet header
let header = bitfield32(0)
let header = header.setField(0, 4, 6)   // version = 6
let header = header.setField(4, 8, 128) // traffic class
let network = header.toBigEndian()
```
