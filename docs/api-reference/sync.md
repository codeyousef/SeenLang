# Synchronization

## Mutex

Mutual exclusion lock using pthread mutex.

```seen
import sync.mutex
```

### Constructor

```seen
let mutex = Mutex.new()  // creates real pthread mutex
```

**Important:** `Mutex.new()` must be used (not `Mutex()` which zero-inits the handle).

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `lock()` | `Void` | Acquire lock (blocking) |
| `unlock()` | `Void` | Release lock |
| `tryLock()` | `Bool` | Non-blocking lock attempt |

### Example

```seen
let mutex = Mutex.new()
mutex.lock()
// critical section
sharedCounter = sharedCounter + 1
mutex.unlock()
```

## RwLock

Read-write lock allowing multiple concurrent readers or one exclusive writer.

```seen
import sync.rwlock
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `readLock()` | `Void` | Acquire read lock |
| `readUnlock()` | `Void` | Release read lock |
| `writeLock()` | `Void` | Acquire write lock |
| `writeUnlock()` | `Void` | Release write lock |
| `destroy()` | `Void` | Free resources |

### Example

```seen
let rw = RwLock.new()

// Multiple readers
rw.readLock()
let value = sharedData
rw.readUnlock()

// Exclusive writer
rw.writeLock()
sharedData = newValue
rw.writeUnlock()
```

## Barrier

Thread barrier -- blocks until N threads arrive.

```seen
import sync.barrier
```

### Constructor

```seen
let barrier = Barrier.new(4)  // 4 threads
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `wait()` | `Int` | Wait at barrier (returns 0 for serial thread) |
| `destroy()` | `Void` | Free resources |

## AtomicInt

Lock-free atomic integer operations.

```seen
import sync.atomic
```

### Constructor

```seen
let counter = AtomicInt.new(0)
```

Uses heap-allocated storage via `__AtomicAlloc(initial)`.

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `load()` | `Int` | Load value |
| `load_relaxed()` | `Int` | Load with relaxed ordering |
| `load_acquire()` | `Int` | Load with acquire ordering |
| `store(value: Int)` | `Void` | Store value |
| `store_release(value: Int)` | `Void` | Store with release ordering |
| `compareExchange(expected: Int, desired: Int)` | `Bool` | CAS operation |

### Example

```seen
let counter = AtomicInt.new(0)

// In thread 1:
counter.store_release(42)

// In thread 2:
let val = counter.load_acquire()
```

## Channel

Message passing between threads using Unix pipes (MPSC).

```seen
import sync.channel
```

### Constructor

```seen
let ch = Channel.new()  // creates pipe pair
```

Internally uses `__ChannelCreate` which packs `read_fd << 32 | write_fd` into an `i64` handle.

### Methods

| Method | Description |
|--------|-------------|
| `send(value)` | Send a value |
| `receive()` | Receive a value (blocking) |

## ThreadLocal

Per-thread storage.

```seen
import sync.thread_local
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `set(value: Int)` | `Void` | Set thread-local value |
| `get()` | `Int` | Get thread-local value |
| `destroy()` | `Void` | Free TLS key |

## MPSCQueue

Multi-producer single-consumer queue (mutex-protected).

```seen
import sync.mpsc_queue
```

| Function | Description |
|----------|-------------|
| `new_mpsc_queue()` | Create queue |
| `mpsc_push(queue, value)` | Push value |
| `mpsc_drain(queue)` | Drain all values |
| `mpsc_len(queue)` | Get length |
| `mpsc_destroy(queue)` | Free resources |

## SPSCQueue\<T\>

Single-producer single-consumer lock-free queue.

```seen
import sync.spsc_queue
```

## AtomicQueue / AtomicStack

Lock-free concurrent data structures.

```seen
import sync.atomic_queue
import sync.atomic_stack
```

### AtomicQueue

| Function | Description |
|----------|-------------|
| `seen_atomic_queue_new()` | Create queue |
| `seen_atomic_queue_push(q, val)` | Enqueue |
| `seen_atomic_queue_pop(q)` | Dequeue |
| `seen_atomic_queue_destroy(q)` | Free |

### AtomicStack

| Function | Description |
|----------|-------------|
| `seen_atomic_stack_new()` | Create stack |
| `seen_atomic_stack_push(s, val)` | Push |
| `seen_atomic_stack_pop(s)` | Pop |
| `seen_atomic_stack_destroy(s)` | Free |

## Work-Stealing Thread Pool

```seen
import thread.pool
```

| Function | Description |
|----------|-------------|
| `seen_ws_pool_new(nworkers)` | Create pool with N workers |
| `seen_ws_pool_submit(pool, fn_ptr, arg)` | Submit task |
| `seen_ws_pool_shutdown(pool)` | Shutdown pool |

## Memory Ordering

The `ordering` module provides constants:

```seen
import sync.ordering
```
