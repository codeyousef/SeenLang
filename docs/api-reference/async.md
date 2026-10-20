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
runtime compatibility: `runtime-v7`.

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

## CPU and blocking pools (`seen-typed-worker-pools-v1`)

`TypedWorkerPool<T: Send + ExecutorJob>` uses the existing persistent executor
for either the `cpu` or `blocking` kind. `ExecutionPools<T>.create` creates an
independent pool of each kind and checks their combined worker budget before
side effects. Each pool permits at most 16 workers; the combined explicit
budget is at most 32. Task/queue/stack/help-depth limits are the executor's
limits. Submission does not create an OS thread.

`PoolPlacement.inherited()` requests inherited CPU and memory policy. An
explicit placement contains either an empty CPU list or one entry per worker
budget, plus NUMA node -1 (inherit) or 0..63. CPU IDs are 0..1023. Creation
copies the list, checks process cpuset/node permission through the OS ABI, and
rejects unavailable placement. No automatic placement or fallback is implied.
Accepted submissions cycle through the configured CPU slots in submission
order. Placement is applied to an owned worker before its job runs; default
inherited placement adds no placement syscall. Explicit placement may migrate
a worker between configured slots across tasks. The slots are a task-placement
policy, not a promise of one immutable OS-thread-to-core pairing.

NUMA binding controls subsequent allocations by that worker. It does not move
caller-owned buffers or the process-wide allocator's existing storage. Use
separately checked mapped/buffer placement when physical residency matters.
If an OS call fails, user code does not run and the task returns the exact
bounded failure class through `ExecutorTaskContext.fail`. This includes
unsupported-platform and unavailable/permission errors. A partial placement
failure can affect the owned worker's OS policy; later explicitly placed jobs
apply their policy again. Workers belong to one pool for their entire lifetime.

`submit(task, ctx)` moves the task only on success and returns
`PoolJoinHandle<T>`. A successful join transfers the original typed task and
consumes its placement wrapper. Failed/cancelled tasks are released once by
the executor. A timed-out join retains ownership for retry. `cancel` propagates
to the task. Final handle release rejects an active task until the executor
has closed; close the pool while retaining its core, dispose of join handles,
then release the pool. Pool release consumes its placement snapshot and worker
owner. Combined release preserves both owners for retry until both executor
close operations succeed.

Pool submission and destruction are exclusive owner operations; the worker
jobs run concurrently. Keep the pool alive through every handle and do not
bypass it with raw internal-core submission when relying on placement policy.
Use separate CPU and blocking budgets so waiting system calls cannot occupy
all CPU workers. Cancellation remains cooperative; the pool does not detach
or forcibly terminate a blocking OS operation.

The native boundary only validates/normalizes Linux x86-64 affinity and NUMA
calls. All placement selection, budgets, wrappers, cancellation and cleanup
are Seen. Other platforms explicitly reject non-inherited placement; executor
platform support still determines whether an inherited pool can be created.
Mocked native tests exercise permission, malformed and unsupported branches;
they do not certify real CPU/NUMA hardware placement. Run software fixtures:

```sh
bash tests/misc_root_tests/seen_cpu_blocking_pools_contract.sh
```

### Bounded reactor integration (ASYNC-002D)

`async.event_loop.AsyncEventLoop<T: Send + AsyncIoJob>` borrows a
`MultithreadExecutor<ReadyIoJob<T>>`. The coordinator exclusively owns the loop
and registration operations. Workers receive only transferred callback jobs;
registration never creates threads. The executor refuses destruction while a
loop borrows it. Close the loop before releasing the executor and reap every
returned executor join handle.

`register(task, descriptor, interest, timerDeadline, ctx)` transfers ownership
only on success. Use `timerDeadline = -1` for descriptor readiness, or an invalid
unowned `OwnedHandle` and a nonnegative monotonic deadline for a timer. All
registrations share the explicit capacity (1..1,048,576). A fixed indexed heap
orders timers and operation deadlines; an intrusive FIFO holds ready work.
Normal poll, cancellation and dispatch do not scan dormant registrations.
Generation checks reject stale handles; exhausted 32-bit generations retire
slots instead of wrapping.

`pollOnce(maxEvents, ctx)` waits for OS readiness, a timer or a bounded native
wakeup, then queues one-shot callbacks. `dispatchNext()` admits one ready callback
to the borrowed executor and returns its typed join handle. Backpressure retains
the callback and descriptor for retry. Reap the join and call `ReadyIoJob.release`
to release callback storage before closing its descriptor. `cancel(registration)`
deregisters and releases pending work; cancellation after dispatch uses the
executor join handle. `close()` releases pending registrations and is idempotent.
Already dispatched work belongs to the executor, not the loop. Immutable context
snapshots own their string storage. A timed-out pending registration is cleaned
before `pollOnce` returns `async.timeout`.

Linux x86-64 uses direct epoll calls with a Seen-owned event buffer and no native
registration lookup table. It supports sockets, device notification descriptors and
completion-queue notification descriptors. A readiness notification transfers
no device or kernel request buffer ownership: the callback must retain request
storage until its completion protocol finishes. Native overlapped/IOCP request
submission is not exposed by this readiness API. Direct-file completion requests
use `AsyncFileCompletionQueue`, described below. Other platforms return
`async.unsupported_platform`; `Interest.Completion` is rejected rather than
misrepresented as readiness. `wakeHandle()` creates a bounded move-only producer
lease. Move each handle to its producer, call `wakeup()` there, then release it
after its final wake. The loop refuses to close while any producer lease remains.
The loop object itself stays on the coordinator.

`async.sockets.AsyncTcpSocket` owns a nonblocking, close-on-exec TCP descriptor.
`SocketAddress.fromBytes` validates packed IPv4/IPv6 bytes and a port; it performs
no name resolution. `bind`, `accept`, `beginConnect`, `read` and `write` accept a
context. `beginConnect` returns `Pending` for an in-progress connection; wait for
writability before `finishConnect`. Each transfer performs one nonblocking OS
call, bounded by the buffer and explicit maximum (1..16 MiB). Success reports
actual partial bytes and read EOF; would-block returns retryable
`async.unavailable` with zero progress. Use readiness before retrying. Transfer
socket ownership with `intoHandle()` when registering it. Retain buffer ownership
until the callback completes. Linux x86-64 is implemented; other platforms return
`async.unsupported_platform`.


`async.completion_io.AsyncFileCompletionQueue` owns a fixed-capacity Linux kernel
AIO context and a semaphore eventfd. `submit` accepts an explicit direct-I/O file
descriptor, an owned aligned buffer, byte/offset bounds, operation and context.
Success transfers descriptor and buffer ownership and allocates one 64-byte
kernel request from Seen storage. Rejection retains both caller owners. Byte
counts and addresses require at least 512-byte alignment; filesystems can require
stricter alignment, so use their reported direct-I/O requirement (4096-byte
`OwnedBuffer` alignment is supported). Buffered descriptors are explicitly
unsupported because Linux AIO submission can otherwise perform buffered work
synchronously.

Use `notificationLease()` to borrow the completion eventfd. Register an unowned
handle for that fd with the readiness loop and retain the lease in the callback
job until the registration is removed. Completion reaping belongs to the queue's
exclusive coordinator. Each `reap()` is nonblocking and returns `None` or a
`CompletedFileIo` owner. Its `outcome()` returns actual partial byte progress or
a typed error; the owner retains the buffer even on an error. Release completed
owners after consuming their buffers. The semaphore notification preserves
readiness when a caller reaps only part of a completion batch.

`cancel(registration)` marks the operation cancelled and requests kernel
cancellation. `None` means the kernel still borrows the request and buffer;
continue waiting and reaping. A returned completion owner confirms those borrows
have ended. Never release in-flight storage merely because a deadline expired.
Deadline/cancellation outcomes preserve completed byte progress. Queue close
returns `async.unavailable` while requests or notification leases remain, and
preserves the owner for retry. Drain owned requests, release leases, then close.
Generation tokens never wrap. Other platforms return
`async.unsupported_platform`. Native software probes cover OS struct marshalling,
partial results, deferred cancellation, notification retention and failed-close
retry; these probes do not certify a physical storage device.


Linux descriptor close errors consume ownership even when delayed I/O errors are
reported. The shared `OwnedHandle` marks such descriptors invalid before returning
the error, so a retry cannot close a descriptor number reused by another owner.
The typed owner remains available to finish metadata cleanup.
