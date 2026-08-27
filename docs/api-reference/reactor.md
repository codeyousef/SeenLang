# Reactor

Modules: `reactor.mod` and `reactor.reactor`.

Seen's reactor is a bounded, backend-neutral event source. Linux uses epoll,
macOS uses kqueue readiness, and Windows uses IOCP completion. Backend
capabilities are explicit: asking for a backend that is not active returns
`reactor.unsupported_platform`; the runtime never substitutes polling or a
different completion model silently.

## Core contract

`Reactor.withCapacity` fixes the maximum registration, timer, and event-batch
size before native resources are created. `register` consumes an
`OwnedHandle` and returns a generation-bearing `Registration`. `deregister`
invalidates that generation before closing an owned native handle, so late
readiness, I/O, or device completions cannot be delivered to a reused token.

`poll(OperationContext, maxEvents)` returns a finite `Array<Event>`. It checks
cancellation and the monotonic deadline before blocking, limits the native
batch, alternates due timers with native readiness under sustained load, and
reports exact completion progress where a completion backend provides it.
Wakeups are coalesced and safe to signal from another thread.

Errors use stable ASCII codes: `reactor.invalid`, `reactor.limit`,
`reactor.cancelled`, `reactor.timeout`, `reactor.unavailable`, and
`reactor.unsupported_platform`. Only unavailable or interrupted operations are
retryable.

## Platform semantics

- epoll supports bounded readable/writable readiness and coexists with later
  io_uring completion sources through the backend-neutral event shape.
- kqueue reports readiness only. Bounded positional I/O and `F_NOCACHE` work
  use the fixed blocking pool; they are not mislabeled as native completion.
- IOCP reports completion progress and preserves generation filtering for
  late overlapped results. The owner of an overlapped operation retains its
  storage until cancellation or late-result cleanup completes.

`BlockingPool.create(workers, capacity)` creates a fixed number of workers and
a bounded queue. Submission rejects overflow, cancellation removes only work
that has not started, and `close` drains accepted work and joins every worker.
It does not create nested workers.

## Example

```seen
import reactor.mod.{Reactor}

let opened = Reactor.withCapacity(256)
if opened.isErr() {
    abort(opened.unwrapErr().code)
}
let reactor = opened.unwrap()
// Transfer an OwnedHandle with reactor.register(...), then poll with a
// caller-owned OperationContext and a finite event budget.
let closed = reactor.close()
```

The native ledger entry `reactor-os-adapter` lists the complete ABI. Native
Seen owns validation, scheduling, timer policy, fairness, generations,
fallback decisions, and diagnostics; the C runtime only normalizes operating
system queues, wakeups, handle closure, and fixed worker execution.
