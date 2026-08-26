# Concurrency

Seen 0.14.0 provides LLVM-coroutine async functions, a cooperative runtime,
capture-free parallel loops, and low-level synchronization primitives. These
features are usable, but their current boundaries matter.

## Async and await

Declare a coroutine with `@async` and await another coroutine from its body:

```seen
@async
fun compute() r: Int {
    async_yield()
    return 42
}

fun main() {
    let rt = new_async_runtime()
    let value = runtime_block_on_int(rt, compute())
    println(value)
}
```

An `@async` call returns an opaque coroutine handle. `await` and the blocking
helpers currently resume and poll that handle synchronously. Concurrency between
several coroutines requires registering each handle with `runtime_spawn` and
driving the cooperative runtime:

```seen
let rt = new_async_runtime()
runtime_spawn(rt, first(), "first")
runtime_spawn(rt, second(), "second")
runtime_run_until_complete(rt)
```

The runtime is single-threaded and cooperative. `async_yield()` yields from the
current coroutine; it is not an operating-system thread scheduler.

Useful runtime functions include `runtime_tick`, `runtime_is_completed`,
`runtime_pending_count`, `runtime_block_on_void`, `runtime_block_on_int`,
`runtime_block_on_float`, and `runtime_stop`.

### Async scopes

Scopes track tasks and join them before the caller continues:

```seen
let rt = new_async_runtime()
let scope = new_async_scope(rt)
scope_spawn(scope, rt, first(), "first")
scope_spawn(scope, rt, second(), "second")
scope_join(scope, rt)
```

`scope_cancel(scope)` currently records best-effort cancellation state; it does
not forcibly interrupt coroutine frames.

## Parallel for

The current `parallel_for` lowering uses pthread workers with statically divided
index ranges:

```seen
parallel_for i in 0..1000 {
    independent_work(i)
}
```

In the shipped 0.14.0 compiler, a parallel body has no capture environment.
Keep it capture-free: do not read or mutate enclosing locals from the body.
Explicit value/reference/move captures and compiler-proven disjoint mutation
are planned language work, not part of this release. Worker count and scheduling
are runtime implementation details and code must not depend on iteration order.

## Synchronization primitives

Import the relevant module under `sync/` or `thread/` before using these types.

### Mutex and RwLock

```seen
let mutex = Mutex.new()
mutex.lock()
// protected work
mutex.unlock()

let rwlock = RwLock.new()
rwlock.readLock()
// read
rwlock.readUnlock()
rwlock.writeLock()
// write
rwlock.writeUnlock()
rwlock.destroy()
```

`Mutex.tryLock()` returns whether the lock was acquired.

### AtomicInt and AtomicBool

```seen
let counter = AtomicInt.new(0)
counter.store_release(42)
let value = counter.load_acquire()
let changed = counter.compare_exchange(42, 43)
```

`AtomicInt` also provides `load`, `load_relaxed`, `store`, `add`, and `sub`.
The compare-and-exchange method is named `compare_exchange`.

### Barrier, Channel, and ThreadLocal

`Barrier` waits until its configured number of participants arrive. `Channel`
is currently an `Int`-only blocking channel backed by the runtime, rather than a
generic typed channel. `ThreadLocal` stores one `Int` value per thread.

### Work-stealing pool

```seen
let pool = WorkStealingPool.new(4)
pool.submit(fn_ptr, arg)
pool.shutdown()
```

`submit` is a low-level API: both the function pointer and its argument are
passed as `Int` values. It is not yet a closure- or typed-task API.

## Thread-safety markers

The parser recognizes `@send` and `@sync` type markers. They express thread
safety intent, but 0.14.0 does not use them to make outer-local captures in
`parallel_for` safe; such captures are not lowered at all.

Actor declaration syntax and a general actor runtime are not shipped in the
active compiler path.

## Related

- [Memory Model](memory-model.md) -- ownership and regions
- [API Reference: Sync](api-reference/sync.md) -- synchronization APIs
- [SIMD and GPU](simd-and-gpu.md) -- data-parallel computation
