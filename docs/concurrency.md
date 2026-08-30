# Concurrency

Seen provides bounded structured async, compiler-proven task transfer,
parallel loops, and synchronized shared ownership. Concurrency APIs require
explicit limits, ownership, cancellation, deadlines, and shutdown.

## Structured async

Import `async.mod`. Create finite limits, a deterministic executor, and a
scope-owned cancellation root. The scope owns its executor after construction:

```seen
let limits = AsyncLimits.bounded(16, 16, 8, 8, 1024)
let executor = DeterministicExecutor<Int>.create(limits).unwrap()
let scope = Scope<Int>.create(executor, CancellationSource.root(1))
let handle = scope.spawn(42, 0).unwrap()
scope.runUntilIdle()
let answer = scope.join(handle, operationContext()).unwrap()
scope.close()
```

The ready queue is wake-driven and bounded. It coalesces duplicate wakes and
does not scan dormant tasks. Cancellation propagates once through its bounded
tree. Joins are typed and exactly once; stale or consumed handles fail with a
stable `async.*` error. `selectTaskId` and `raceTaskId` choose the lowest ready
task ID, and fan-out refuses work beyond its declared bound.

`@async` and `await` remain compiler syntax for coroutine lowering, but the
removed poll-all coordinator is not a public scheduling fallback. Production
coordination uses `async.mod` and explicit wake events.

## Parallel for

`parallel_for` uses bounded worker ranges and a compiler-generated typed capture
environment:

```seen
parallel_for i in 0..1000 {
    independent_work(i)
}
```

Captures are immutable, parent-owned `Share` borrows and remain alive until
every worker joins. Mutable outer-local captures are rejected unless the
compiler proves synchronized or disjoint mutation. Values moved into owned
task state must satisfy `Send`. Worker errors are selected by the lowest
failing iteration and sibling work is joined.

## Synchronization

Import `sync.mod` for checked `Arc<T>`, atomics, bounded channels, scoped mutex
and reader/writer guards, poisoning, and lock-order diagnostics. Native Seen
owns capacity, cancellation, deadlines, ownership, and scheduling policy; only
the ledgered OS synchronization adapter crosses the native boundary.

See [Structured async](api-reference/async.md), [Synchronization](api-reference/sync.md),
and [Memory model](memory-model.md).
