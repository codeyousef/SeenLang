# Memory Model

Seen uses region-based memory management with no garbage collector. Memory is managed through regions, arenas, ownership semantics, and compile-time lifetime tracking.

## Stack vs Heap

Primitive types (`Int`, `Float`, `Bool`, `Char`) live on the stack. Classes are heap-allocated and accessed via handles (pointers stored as `i64`). Arrays use heap-allocated backing storage.

```seen
let x = 42          // stack
let name = "hello"  // string data on heap, handle on stack
let arr = [1, 2, 3] // array data on heap, handle on stack
```

## Ownership

### `own` -- exclusive ownership

```seen
fun processData(own data: Array<Int>) {
    // data is exclusively owned here
    // caller can no longer use it
}
```

### `move` -- transfer ownership

```seen
let a = Array<Int>()
let b = move a  // ownership transferred to b
// a is no longer valid
```

### `borrow` -- temporary read access

```seen
fun printData(borrow data: Array<Int>) {
    // read-only access, caller retains ownership
    for item in data {
        println("{item}")
    }
}
```

### `ref` -- reference (pointer)

```seen
fun increment(ref counter: Int) {
    counter = counter + 1
}
```

### `inout` -- mutable reference

```seen
fun swap(inout a: Int, inout b: Int) {
    let temp = a
    a = b
    b = temp
}
```

## Regions

Regions define memory lifetimes for groups of allocations:

```seen
region gameFrame {
    let particles = Array<Particle>()
    // all allocations freed when region exits
}
```

## Arena Allocators

Arenas provide bulk allocation with a single free:

```seen
arena {
    let nodes = Array<TreeNode>()
    for i in 0..1000 {
        nodes.push(TreeNode.new(i))
    }
    // all nodes freed at once when arena exits
}
```

### Runtime Arena API

```seen
let a = seen_arena_new(1048576)     // 1MB arena
let idx = seen_arena_alloc(a, 64)   // allocate 64 bytes
let ptr = seen_arena_get(a, idx)    // convert index to pointer
seen_arena_reset(a)                  // bulk free
seen_arena_free(a)                   // release arena
```

## Stack Regions

LIFO allocation for temporary data:

```seen
let region = seen_stack_region_new(4096)
let ptr = seen_stack_region_alloc(region, 32)
seen_stack_region_pop(region, 32)
seen_stack_region_destroy(region)
```

## Pool Regions

Fixed-size block allocation for uniform objects:

```seen
let pool = seen_pool_region_new(64, 100)  // 64-byte blocks, 100 of them
let block = seen_pool_region_alloc(pool)
seen_pool_region_free(pool, block)
seen_pool_region_destroy(pool)
```

## Memory-Mapped Regions

For file-backed memory:

```seen
let mapped = seen_mapped_new(path, size, flags)
let data = seen_mapped_data(mapped)
seen_mapped_sync(mapped)
seen_mapped_free(mapped)
```

## Defer

Execute cleanup code when leaving a scope, regardless of how the scope exits:

```seen
fun processFile(path: String) r: String {
    let file = File.open(path)
    defer { file.close() }

    return file.readContent()
    // file.close() runs here automatically
}
```

### errdefer

Only runs when scope exits via error:

```seen
fun allocateResources() r: Result<Handle, String> {
    let handle = acquire()
    errdefer { release(handle) }

    configure(handle)?  // if this fails, handle is released
    return Ok(handle)
}
```

## Layout Control

### `@packed`

Remove padding between struct fields:

```seen
@packed
class NetworkHeader {
    var version: Int
    var flags: Int
    var length: Int
}
```

### `@cache_line`

Align to cache line boundary (typically 64 bytes):

```seen
@cache_line
class HotData {
    var counter: Int
}
```

### `@trivially_copyable`

Mark a type as safe to memcpy:

```seen
@trivially_copyable
class Vec2 {
    var x: Float
    var y: Float
}
```

## Safety Annotations

### `@move`

Enforce move semantics on a type:

```seen
@move
class UniqueHandle {
    var fd: Int
}
```

Once moved, the original binding becomes invalid.

## Related

- [Concurrency](concurrency.md) -- thread safety with `@send`/`@sync`
- [API Reference: Sync](api-reference/sync.md) -- Mutex, AtomicInt
