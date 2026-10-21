# Structured async

Module: `async.mod`

Seen 0.22.9 provides a bounded, native-Seen structured-concurrency surface.
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
| `DeterministicWaitGroup`, `DeterministicBarrier` | Single-thread coordination models |

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

## Fixed-worker execution

Module: `async.multithread`. Contract: `seen-multithread-executor-v1`,
runtime compatibility: `runtime-v5`.

`MultithreadExecutor<T: Send + ExecutorJob>` starts exactly the explicitly
configured worker count and reuses those threads until shutdown. `T` is the
concrete task object, including its typed input and output fields. Implement
`ExecutorJob.run(context): Bool` and `ExecutorJob.release(): Bool` for that
object. A successful join returns the completed `T`; there is no integer
function-pointer submission API or erased user result.

`ExecutorLimits` bounds workers (1–16), outstanding tasks, queued tasks,
helping depth (1–64), and each worker's stack (at most 8 MiB). Stack size
must provide at least 64 KiB plus 16 KiB per configured helping level.
Saturation returns `async.limit` before admission and retains the caller's
concrete task. Completed results count against the task bound until joined.
Admission copies context and limit metadata so the caller can reuse its
original context after submission.

Jobs borrow `ExecutorTaskContext` only for the duration of `run`. Use
`context.isCancelled()` at bounded work checkpoints. Nested jobs use
`spawnExecutorTask<T>(context, task)`. When a worker joins a task on its own
pool, it executes queued work while waiting, allowing a single-worker parent
to spawn and join a child. Helping has an explicit depth bound; it does not
create extra threads. Joining oneself is invalid. Jobs must not construct
cyclic dependencies, retain the borrowed task context, or panic.

Ownership follows these rules:

- Before successful spawn, the caller owns the task. After admission, the
  executor owns it until join or shutdown cleanup. The caller must not access
  or mutate an admitted task through an alias.
- Successful `join(context)` consumes the join operation and transfers the
  concrete task back to the caller. The caller then releases it once.
- Worker cancellation observation includes the immutable task deadline. A late
  successful return after expiry becomes `async.timeout` and releases the job
  exactly once; running jobs must cooperate by checking `isCancelled()`.
- A terminal job failure or task cancellation consumes the join operation
  and calls the task's `release` once. A cancelled/timed-out wait retains
  running work and its join handle for retry.
- `shutdown(false)` stops admission, drains work, and joins workers while
  preserving unjoined results. `shutdown(true)` requests cooperative task
  cancellation before joining. Neither detaches a running job.
- `close()` cancels/drains, joins, releases all unjoined tasks, and destroys
  synchronization and queue storage. It is idempotent. `release()` additionally
  consumes the executor object; no method may be called after it.
- The owner must exclude external spawn/join/close calls during shutdown or
  destruction. Jobs run outside the scheduler mutex. User destructors may
  allocate but must not reenter a closing executor.

Caller-owned join-handle wrappers and `Result` containers must also be freed
after use. Task destructors release their own fields and concrete object;
the executor releases only its task metadata and dispatch box.

Linux x86-64 is the implemented OS thread target. Other targets return
`async.unsupported_platform`; no scalar fallback is selected. Pool scheduling,
validation, lifetime and cancellation are Seen. The adapter owns only OS
thread lifetime and thread-local callback identity. The shared runtime
allocator protects allocation/budget transactions and reinstates its guard
around process fork.

The executable task examples and named lifecycle cases are in
[`async-002a.seen`](../../seen_std/tests/async/async-002a.seen). Certification
entrypoints are `tests/misc_root_tests/seen_multithread_executor_contract.sh`
and `tests/misc_root_tests/seen_executor_native_abi.sh`. Set
`SEEN_EXECUTOR_TSAN=1` on a host with the required TSan address-space policy
to include its allocator race run; an omitted TSan run is not certification.

## Worker synchronization (`seen-async-sync-v1`)

`async.task_group.TaskGroup<T: Send + ExecutorJob>` is an exclusive coordinator
owner for a bounded set of child jobs. `create(core, limit, context)` reserves
its handle table. `spawn` transfers a job only on success. `barrier(context)`
seals admission, joins in submission order, and transfers `Array<T>` exactly
once. On the first error it cancels remaining children, drains owned work with
an uncancelled cleanup context, and releases successful partial results.
The error records completed group results as progress. `close` cancels and
drains; `release` consumes the group and its context/table owners.
Creating a coordinator group inside a worker is rejected; workers use typed
nested spawn/join and the synchronization primitives below.

`AsyncWaitGroup.create(core, count, limit)` supplies checked `done`, `wait(ctx)`,
`cancel`, `close` and `release`. A wait timeout leaves its owner intact for a
later observation. Counts cannot underflow. Progress counts completed items,
not unused capacity.

`AsyncBarrier.create(core, parties)` creates a **single-use** phase barrier.
`arrive(ctx)` waits for every participant and returns phase 0. A failed wait
breaks the phase for all participants; another arrival after completion is
invalid. Create a fresh barrier for another phase. This explicit lifecycle
avoids a later phase blocking a participant whose outer helping frame has
not yet resumed.

`AsyncQueue<T: Send>.create(core, capacity)` preallocates its bounded FIFO.
`put(value, ctx)` transfers ownership on success and backpressures when full;
`take(ctx)` transfers one value. Closing rejects producers, wakes waiters and
allows existing values to be drained. Release rejects nonempty storage or
active waiters: payloads are never silently discarded. Drain and dispose of
owned values before final release.

`AsyncIntReduction.create(core, capacity)` reserves scalar result slots.
`submit(index, value, ctx)` fills each index once. `finish(ctx)` waits for all
slots and folds in index order with checked signed addition. Duplicates,
invalid indices and overflow fail with stable `async.invalid`/`async.limit`
diagnostics. The fold allocates no per-item state and observes stop requests
at bounded intervals. `close` wakes observers; release requires no waiters.

All primitives borrow the executor core, use its mutex/condition, and notify
state changes under that mutex. Workers help ready work outside the lock;
there is no dormant-task polling scan or thread creation per wait. Helping
uses the current TLS slot/depth and enforces stack/tree limits. A nested
helping frame with no ready work fails with `async.limit` rather than blocking
an outer producer or participant needed for progress. Callers should keep
producer steps bounded and size capacity/workers for their dependency graph.
Task cancellation, deadlines, closed state and saturation remain explicit.
Errors are typed `AsyncError` values; retries remain caller policy.

At most `core.limits.tasks` synchronization owners and native parked waiters
are admitted. The core refuses final close while synchronization borrowers
exist. Close/wake primitives, join every caller, drain payloads, release
primitives/groups, then release the executor. The owner excludes concurrent
calls during final destruction. Shared method calls are protected; copying a
borrowed handle does not extend its lifetime or authorize another destructor.
The compiler recognizes these exact canonical foundation types as synchronized
Send/Share boundaries; similarly named user classes receive no exemption.

The previous counter-only coordination models are now explicitly named
`DeterministicWaitGroup` and `DeterministicBarrier`. They remain single-thread
models for the deterministic executor and are not worker synchronization APIs.
Integer deterministic reductions now reject overflow as well.

Linux x86-64 uses the existing executor and synchronization OS adapters. No new
foreign symbol or foreign scheduling/queue/reduction algorithm is introduced.
Other platforms retain the executor's explicit unsupported classification;
no platform or hardware certification is implied. Run software regressions:

```sh
bash tests/misc_root_tests/seen_async_synchronization_contract.sh
```
