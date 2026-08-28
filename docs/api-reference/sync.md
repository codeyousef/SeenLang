# Synchronization

Seen's public synchronization surface is native Seen policy over a small,
ledgered operating-system wait/wakeup ABI. Resource-owning values are
move-only, blocking operations take an `OperationContext`, and all failures use
stable `sync.*` codes.

```seen
import sync.mod.{Arc, AtomicInt, BoundedChannel, LockOrderTracker,
    MemoryOrder, Mutex, RwLock}
```

## Send and Share

`Send` means a value may be transferred to another task. `Share` means a value
may be referenced concurrently without permitting an undeclared mutable alias.
The compiler proves both properties structurally; declaring an outer type does
not make an unsafe field legal.

```seen
@share
class SharedId {
    let value: Int
}

@share
class SharedPair<T: Share> {
    let first: T
    let second: T
}
```

Mutable arrays and other unprotected mutable aggregates are not `Share`.
Generic `T: Send` and `T: Share` bounds are checked at every use, including
imports and parallel-loop captures. The former `@sync` spelling is rejected.

## Errors and operation context

Fallible methods return `Result<T, SyncError>`. `SyncError` carries `code`,
`operation`, `message`, `retryable`, and item `progress`. Public error codes
are:

| Code | Meaning |
|---|---|
| `sync.invalid` | Invalid state, handle, order, ownership, or lock use |
| `sync.limit` | A checked capacity or diagnostic bound was exceeded |
| `sync.cancelled` | The supplied operation context was cancelled |
| `sync.timeout` | The monotonic deadline expired |
| `sync.unavailable` | A transient operating-system resource was unavailable |
| `sync.unsupported_platform` | No supported adapter exists on this platform |

Cancellation and expired deadlines are rejected before a blocking side effect.
A caller supplies an immutable `OperationContext`; deadlines are absolute
monotonic nanoseconds. Only `sync.unavailable` can be retryable.

## Arc<T>

`Arc<T: Share>` is bounded explicit shared ownership. Cloning increments a
checked strong count; `close()` releases one owner and is idempotent for that
owner. Access through a released owner fails.

```seen
let created = Arc<Int>.create(41)
let first = created.unwrap()
let second = first.cloneShared().unwrap()

let lease = second.borrow().unwrap()
let value = lease.get().unwrap()
let owners = first.strongCount().unwrap()

lease.close()
first.close()
second.close()
```

`borrow()` returns a move-only lease that owns an additional strong reference;
the shared value cannot outlive that lease's owner. The lease must be closed
on every exit. The last owner close releases the native atomic count. There is
no implicit global owner, garbage collector, or fallback implementation.

## Atomics and memory order

`AtomicInt`, `AtomicBool`, and `Atomic<T: Share>` expose typed memory ordering:

```seen
enum MemoryOrder { Relaxed; Acquire; Release; AcqRel; SeqCst }
```

```seen
let counter = AtomicInt.create(0 as Int64).unwrap()
counter.store(42 as Int64, MemoryOrder.Release)
let observed = counter.load(MemoryOrder.Acquire).unwrap()
let update = counter.compareExchange(42 as Int64, 43 as Int64,
    MemoryOrder.AcqRel, MemoryOrder.Acquire).unwrap()
counter.close()
```

Release/AcqRel loads and Acquire/AcqRel stores fail with `sync.invalid`.
Compare-exchange validates its failure order separately and returns both the
observed value and whether the exchange occurred. Integer operations also
include `exchange`, `fetchAdd`, and `fetchSub`. Generic atomics use the same
typed API with a narrow mutex adapter; they do not silently claim lock-free
operation.

## BoundedChannel<T>

Every channel has a positive capacity no greater than 1,048,576. Sending and
receiving are typed, FIFO, deadline-aware, cancellable, and never create an
unbounded queue.

```seen
let channel = BoundedChannel<Int>.create(16).unwrap()
channel.send(context, 42)?
let value = channel.receive(context)?

channel.close()       // wakes blocked senders and receivers
channel.destroy()     // only after close, drain, and waiter shutdown
```

`close()` is idempotent and prevents new sends. Buffered values remain
receivable. `destroy()` fails until the channel is closed, drained, and has no
registered waiters, then releases both condition variables and the mutex.

## Mutex<T> and RwLock<T>

Locks own the protected `Share` value. A positive rank is required and a
`LockOrderTracker` accompanies acquisition. The returned guard is move-only;
all access and mutation occur through that guard, and `release()` unlocks and
removes the rank from the tracker.

```seen
let tracker = LockOrderTracker.create(16, 64, true).unwrap()
let cell = Mutex<Int>.create(0, 10).unwrap()
let guard = cell.lock(context, tracker).unwrap()
guard.set(1)?
guard.release()?
cell.close()?
```

`RwLock<T>` provides `read(context, tracker)` and
`write(context, tracker)`. Read guards expose `get`; write guards expose
`get`, `set`, and `poison`.

If a protected operation cannot preserve its invariant, mark its guard
poisoned before releasing it. Subsequent acquisition fails until the owner
calls `recover()` explicitly. Closing a held resource fails; cleanup never
guesses that a live guard is safe to discard.

## Lock-order diagnostics

`LockOrderTracker.create(maxDepth, maxEdges, diagnostics)` bounds both the held
stack and dependency graph. In diagnostic mode ranks must strictly increase on
acquisition and release in reverse order. A repeated edge is ignored; an edge
that creates a cycle fails with `sync.invalid`; a full graph fails with
`sync.limit`.

Diagnostics are deterministic and native Seen. They do not start a monitor
thread or retain an unbounded global graph.

## Platform mapping

Linux and macOS use generation-checked slots over pthread atomics, mutexes,
condition variables, and reader/writer locks. Windows uses Interlocked,
SRWLOCK, CONDITION_VARIABLE, and critical-section primitives. The adapter owns
no queue, ownership, poisoning, lock-order, retry, or shutdown policy.

Stale, reused, mistyped, and closing handles are rejected. The adapter has a
finite resource registry, numerically tracked live-resource counts, explicit
deadline results, and idempotent destruction through zeroed handles.

The former raw `__Mutex*`, `__Atomic*`, pipe channel, runtime RwLock/barrier/TLS,
atomic queue, and atomic stack surfaces were removed as a clean pre-1.0
replacement. There is no compatibility bridge.
