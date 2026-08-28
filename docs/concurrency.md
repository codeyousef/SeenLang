# Concurrency

Seen provides LLVM-coroutine async functions, a cooperative runtime, typed
parallel-loop captures, and bounded synchronization primitives. These features
are usable, but their ownership and scheduling boundaries matter.

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

`parallel_for` uses bounded worker ranges and a compiler-generated typed capture
environment:

```seen
parallel_for i in 0..1000 {
    independent_work(i)
}
```

`parallel_for` captures are immutable, parent-owned borrows and must satisfy
`Share`. The generated environment remains alive until every worker has joined;
workers neither move nor drop its fields. Mutable outer-local captures are
rejected unless a future compiler contract proves synchronized or disjoint
mutation. Values moved into independently owned task state must satisfy `Send`,
but `parallel_for` does not expose a move-capture form. Worker errors are
selected by the lowest failing iteration index and sibling work is joined.
Worker count and scheduling remain implementation details; code must not depend
on iteration order.

## Synchronization primitives

Import `sync.mod` or the specific module. Public resource types are move-only
and fallible:

- `Arc<T: Share>` provides checked explicit shared ownership.
- `AtomicInt`, `AtomicBool`, and `Atomic<T: Share>` use typed memory orders.
- `BoundedChannel<T: Send>` requires capacity and an `OperationContext` for
  blocking sends and receives.
- `Mutex<T: Share>` and `RwLock<T: Share>` return scoped guards, implement
  explicit poisoning/recovery, and require lock ranks.
- `LockOrderTracker` detects bounded rank inversions and dependency cycles.

Synchronization state machines and policy are Seen. Only atomic, mutex,
condition-variable, reader/writer-lock, and monotonic-clock normalization cross
the ledgered native ABI. The raw mutex/atomic/channel and C-owned collection
surfaces have no compatibility fallback.

### Work-stealing pool

```seen
let pool = WorkStealingPool.new(4)
pool.submit(fn_ptr, arg)
pool.shutdown()
```

`submit` is a low-level API: both the function pointer and its argument are
passed as `Int` values. It is not yet a closure- or typed-task API.

## Thread-safety markers

The compiler recognizes `@send` and `@share`. It proves their fields and generic
bounds structurally across the resolved import graph and enforces those proofs
at task transfer, shared ownership, channel, atomic, and parallel-capture
boundaries. The former `@sync` annotation is rejected.

Actor declaration syntax and a general actor runtime are not shipped in the
active compiler path.

## Related

- [Memory Model](memory-model.md) -- ownership and regions
- [API Reference: Sync](api-reference/sync.md) -- synchronization APIs
- [SIMD and GPU](simd-and-gpu.md) -- data-parallel computation
