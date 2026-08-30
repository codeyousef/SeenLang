# Structured async

Module: `async.mod`

Seen 0.19.2 provides a bounded, native-Seen structured-concurrency surface.
The executor consumes explicit wakeups in FIFO order and never polls every
dormant task. The former unbounded coroutine coordinator and unrelated future
class are not compatibility surfaces.

## Core types

| Type | Purpose |
|---|---|
| `AsyncLimits` | Explicit task, ready-queue, tree, fan-out, and poll budgets |
| `AsyncOperationContext` | Cancellation, monotonic deadline, trace and resource identity, and explicit fallback policy |
| `CancellationSource` | One-shot tree cancellation with exactly-once delivery |
| `ReadyQueue` | Bounded, duplicate-suppressing FIFO wake queue |
| `DeterministicExecutor<T>` | Typed wake-driven task state and deterministic scheduling |
| `Scope<T>` | Move-only owner that spawns, joins, cancels, and closes child work |
| `JoinHandle<T>` | Move-only typed task identity with exactly-once join state |
| `AsyncWaitGroup`, `AsyncBarrier` | Bounded coordination helpers |

Fallible operations return `Result<T, AsyncError>`. Stable error codes are
`async.invalid`, `async.limit`, `async.cancelled`, `async.timeout`,
`async.unavailable`, and `async.unsupported_platform`. Only unavailable work is
retryable. A caller must supply finite limits before any task is queued.

## Scheduling and shutdown

`spawn` queues one task wake. `tick` removes exactly one ready task, while
`runUntilIdle` drains only the ready queue. Repeated wakes for an already queued
task are coalesced. Task IDs increase monotonically, and simultaneous ready
results are selected by the lowest task ID. `Scope.close` and
`DeterministicExecutor.shutdown` are idempotent and cancel all remaining child
work before discarding the queue.

`selectTaskId`, `raceTaskId`, `timeoutJoin`, and `boundedFanOut` never introduce
an implicit fallback. Cancellation and timeout errors carry trace identity and
completed-item progress from the supplied context.

The scheduling, validation, cancellation, and cleanup state machines are Seen.
Only the already-ledgered monotonic-clock ABI is used to normalize OS time.

See [the compiling structured-async example](../../seen_std/examples/structured_async.seen).
