# Concurrency

Seen provides async/await, parallel for loops, and synchronization primitives for concurrent programming.

## Async / Await

Async functions are implemented using LLVM coroutines.

### Declaring async functions

```seen
@async
fun fetchData(url: String) r: String {
    let response = await httpGet(url)
    return response.body
}
```

The `@async` decorator transforms the function into a coroutine that returns a handle (`ptr`).

### Awaiting results

```seen
@async
fun processAll() r: Int {
    let a = await fetchData("https://api.example.com/a")
    let b = await fetchData("https://api.example.com/b")
    return a.length() + b.length()
}
```

### Async runtime

```seen
fun main() {
    let rt = new_async_runtime()
    runtime_spawn(rt, processAll(), "main_task")
    runtime_run_until_complete(rt)
}
```

Runtime methods:
- `new_async_runtime()` -- create a runtime
- `runtime_spawn(rt, task, name)` -- schedule a coroutine
- `runtime_tick(rt)` -- process one tick
- `runtime_run_until_complete(rt)` -- run until all tasks finish
- `runtime_block_on_int(rt, task)` -- block on a task returning Int
- `runtime_block_on_void(rt, task)` -- block on a void task
- `runtime_pending_count(rt)` -- number of pending tasks
- `runtime_stop(rt)` -- stop the runtime

### async_yield

Yield control back to the runtime from within an async function:

```seen
@async
fun longRunning() {
    var i = 0
    while i < 1000 {
        doWork(i)
        async_yield()  // allow other tasks to run
        i = i + 1
    }
}
```

### Async Scopes

Structured concurrency with scopes:

```seen
let scope = new_async_scope()
scope_spawn(scope, task1(), "t1")
scope_spawn(scope, task2(), "t2")
scope_join(scope)   // wait for all tasks
scope_cancel(scope) // or cancel all
```

## Parallel For

Fork-based parallel iteration:

```seen
var results = Array<Int>.withLength(1000)
parallel_for i in 0..1000 {
    results[i] = computeExpensive(i)
}
```

Each iteration may run in a separate forked process.

## Synchronization Primitives

### Mutex

```seen
let mutex = Mutex.new()
mutex.lock()
// critical section
mutex.unlock()
```

`Mutex.new()` creates a real pthread mutex. Methods:
- `lock()` -- acquire the lock (blocking)
- `unlock()` -- release the lock
- `tryLock()` -- non-blocking attempt, returns Bool

### RwLock

Read-write lock for concurrent reads:

```seen
let rwlock = RwLock.new()

// Multiple readers
rwlock.readLock()
let data = sharedData
rwlock.readUnlock()

// Exclusive writer
rwlock.writeLock()
sharedData = newValue
rwlock.writeUnlock()
```

### Barrier

Synchronize N threads at a rendezvous point:

```seen
let barrier = Barrier.new(4)  // 4 threads must arrive

// In each thread:
barrier.wait()  // blocks until all 4 arrive
```

### AtomicInt

Lock-free integer operations:

```seen
let counter = AtomicInt.new(0)
counter.store(42)
let val = counter.load()
counter.compareExchange(expected, desired)
```

Operations:
- `load()` / `load_relaxed()` / `load_acquire()` -- read value
- `store()` / `store_release()` -- write value
- `compareExchange(expected, desired)` -- CAS operation

### Channel

Message passing between threads (MPSC via Unix pipes):

```seen
let ch = Channel.new()
ch.send(42)
let value = ch.receive()
```

### ThreadLocal

Per-thread storage:

```seen
let tls = ThreadLocal.new()
tls.set(42)
let value = tls.get()
```

## Thread Safety Markers

### `@send`

Mark a type as safe to transfer between threads:

```seen
@send
class Message {
    var data: String
}
```

### `@sync`

Mark a type as safe to share between threads:

```seen
@sync
class SharedCounter {
    var mutex: Mutex
    var count: Int
}
```

## Work-Stealing Thread Pool

```seen
let pool = WorkStealingPool.new(4)  // 4 worker threads
pool.submit(taskFunction, arg)
pool.shutdown()
```

## Actor Model

```seen
actor CounterActor {
    var count: Int

    receive Increment {
        this.count = this.count + 1
    }

    receive GetCount {
        reply this.count
    }
}
```

## Related

- [Memory Model](memory-model.md) -- ownership and regions
- [API Reference: Sync](api-reference/sync.md) -- full sync primitive API
- [SIMD and GPU](simd-and-gpu.md) -- parallel computation
