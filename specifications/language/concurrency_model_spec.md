# Seen Concurrency Model Specification

**Version:** 0.1 (Initial Draft)

## 1. Introduction and Goals

This document specifies the concurrency model for the Seen programming language. The primary goal is to provide safe, efficient, and easy-to-use mechanisms for concurrent and parallel programming, aligning with Seen's overall design philosophy of safety, performance, and developer experience.

Seen will adopt a model based on **structured concurrency** principles, primarily leveraging **async/await syntax** built on top of an efficient **task-based runtime** and **message-passing (channels)** for communication between tasks.

**Core Goals:**

*   **Safety:** Prevent common concurrency hazards like data races and deadlocks by design where possible. Leverage Seen's memory safety features.
*   **Ergonomics:** Provide high-level, intuitive syntax (async/await) for writing concurrent code.
*   **Performance:** Enable efficient utilization of multi-core processors through lightweight tasks and non-blocking I/O.
*   **Structured Concurrency:** Ensure that the lifetime of concurrent tasks is well-defined and tied to specific scopes, simplifying reasoning about program behavior and resource management.
*   **Integration:** Seamlessly integrate with Seen's error handling, memory model, and bilingual features.

**Non-Goals (Initially):**

*   **Low-level threading primitives as primary API:** While the runtime will use threads, direct manipulation of threads and low-level synchronization primitives (like raw mutexes and condition variables exposed directly to the user for general use) will be discouraged in favor of higher-level abstractions. They might exist in `std::sync` for specific, advanced use cases or FFI.
*   **Global mutable state shared across tasks without synchronization:** Seen's memory model and concurrency primitives will aim to prevent or manage this.

## 2. Core Concurrency Primitives

### 2.1. Tasks (Lightweight Concurrency Units)

*   The fundamental unit of concurrent execution in Seen will be a **Task** (similar to goroutines in Go or tasks in modern async frameworks).
*   Tasks are lightweight, cooperatively scheduled units of work managed by the Seen runtime.
*   The runtime will manage a pool of OS threads and efficiently multiplex tasks onto these threads.

### 2.2. `async` Functions and `await` Operator

*   **`async func` / `دالة_غير_متزامنة`:** Functions that perform potentially long-running or I/O-bound operations without blocking the calling task's thread can be declared `async`.
    *   `async` functions return a `Future<T>` (or a similar named type, e.g., `Promise<T>`), representing a value that will be available in the future.
    ```seen
    async func fetch_data(url: String) -> Result<String, NetworkError> {
        // ... non-blocking network I/O ...
        // Simulating work
        runtime::sleep(1.second).await; // Example of awaiting another future
        return Ok("data from " + url);
    }
    ```
*   **`await` / `انتظر` Operator:** Used within an `async` function to pause its execution until a `Future<T>` it is waiting on completes. While paused, the underlying OS thread is freed to run other tasks.
    ```seen
    async func process_urls() {
        val data1 = fetch_data("url1.com").await?;
        val data2 = fetch_data("url2.com").await?;
        print(data1 + data2);
    }
    ```

### 2.3. Spawning Tasks

A mechanism will be provided to spawn new top-level tasks or tasks within a structured concurrency scope.

*   **`runtime::spawn` / `نظام_التشغيل::انشئ` (Illustrative):**
    ```seen
    func main() {
        // Spawn a new task that runs concurrently
        val task_handle = runtime::spawn(async {
            print("Hello from a new task!");
            val result = fetch_data("example.com").await;
            print("Task finished with: " + result.to_string());
        });

        // Main task can continue or wait for the spawned task
        // task_handle.join().await; // Example: wait for completion
        print("Hello from main task!");
        // Implicitly, main might await essential spawned tasks before exiting,
        // or the program exits when the main task and all non-detached tasks complete.
    }
    ```

### 2.4. Channels (Message Passing)

For communication and data sharing between tasks, Seen will provide **channels** (similar to Go channels or Rust's `std::sync::mpsc`).

*   **Types:**
    *   **Asynchronous Channels:** Buffered channels where send operations do not block unless the buffer is full.
    *   **Synchronous Channels:** Unbuffered channels where a send operation blocks until a receiver is ready, and vice-versa.
*   **Operations:** `send`, `receive`.
    *   `send` and `receive` operations on channels will be `async` by default when used in `async` contexts, integrating with the `await` mechanism.
*   **Ownership Semantics:** Values sent over channels will adhere to Seen's ownership and memory safety rules (e.g., ownership transfer).

```seen
async func producer(channel: Sender<String>) {
    channel.send("message 1").await?;
    channel.send("message 2").await?;
}

async func consumer(channel: Receiver<String>) {
    val msg1 = channel.receive().await?;
    print(msg1);
    val msg2 = channel.receive().await?;
    print(msg2);
}

func main_channels() {
    val (sender, receiver) = channel::create<String>(buffer_size: 10);

    runtime::spawn(producer(sender));
    runtime::spawn(consumer(receiver));
    
    // Need a way for main to wait or for the program to run until tasks are done
    // For example, runtime::block_on or similar to run the main async event loop.
}
```

## 3. Structured Concurrency

Seen will emphasize structured concurrency to manage task lifetimes and ensure proper resource cleanup.

*   **Task Scopes:** Introduce constructs (e.g., `runtime::scope` or similar) that define a lexical scope for a group of tasks.
    *   A scope will not exit until all tasks spawned within it have completed.
    *   If any task within the scope fails (panics or returns an error that isn't handled within the scope), all other tasks in that scope are signaled for cancellation.
    *   This helps prevent orphaned tasks and makes error propagation more predictable.

```seen
// Hypothetical structured concurrency scope
async func perform_parallel_work() -> Result<(), ErrorType> {
    try runtime::scope(|s| { // s is a Scope object
        s.spawn(async { /* work item 1 */ fetch_data("url1").await?; });
        s.spawn(async { /* work item 2 */ fetch_data("url2").await?; });
        // The scope 's' automatically awaits all spawned tasks here.
        // If any task returns Err or panics, others are cancelled, and scope returns Err.
    }); // The 'try' keyword here would handle errors propagated from the scope
    return Ok(());
}
```

## 4. Synchronization Primitives (Limited Exposure)

While channels and structured concurrency are preferred, a minimal set of traditional synchronization primitives might be available in `std::sync` for specific low-level scenarios or interoperation with FFI.

*   **Mutex (`std::sync::Mutex<T>`):** Provides mutual exclusion for shared data. Accessing data within a Mutex will likely involve an `async` lock operation in `async` contexts.
*   **Condition Variables (`std::sync::Condvar`):** For more complex synchronization patterns with Mutexes.
*   **Atomic Types (`std::sync::atomic`):** For low-level atomic operations on primitive types.

Use of these will be carefully documented with warnings about potential misuse (deadlocks, etc.).

## 5. Cancellation and Timeouts

*   **Cancellation:** Futures and tasks should be cancellable.
    *   Structured concurrency scopes will propagate cancellation automatically.
    *   An explicit cancellation mechanism (e.g., cancellation tokens or futures that resolve on cancellation request) will be provided.
*   **Timeouts:** A standard way to apply timeouts to `await` operations or entire task scopes.
    ```seen
    // Hypothetical timeout
    try runtime::timeout(5.seconds, async {
        val data = fetch_data("very_slow_service.com").await?;
        // ... process data ...
    }).await except TimeoutError {
        print("Operation timed out!");
    }
    ```

## 6. Error Handling in Concurrent Code

*   Errors propagated from `async` functions (`Result<T, E>`) will be handled using `?` or explicit matching, just like synchronous code.
*   Panics in a task will typically propagate and be caught by its parent task or structured concurrency scope.
    *   A panic in one task within a structured scope may lead to the cancellation of sibling tasks and the scope itself reporting an error.
*   Uncaught panics in a top-level task will typically terminate the program after attempting cleanup.

## 7. Interaction with Memory Model

*   Seen's automated memory model must ensure safety when data is shared or sent between tasks.
*   **Send + Sync Traits (Implicit or Explicit):** Similar to Rust, types will implicitly or explicitly need to be `Send` (safe to send to another task) or `Sync` (safe to share via `Ref` between tasks) for use across task boundaries.
*   The compiler will enforce these rules to prevent data races.

## 8. Bilingualism

*   Keywords related to concurrency (`async`, `await`) will have Arabic equivalents (`غير_متزامن`, `انتظر`).
*   Standard library modules and functions for concurrency (e.g., `runtime::spawn`, `channel::create`) will adhere to Seen's bilingual API naming conventions.

## 9. Runtime Implementation Notes (High-Level)

*   A work-stealing scheduler is a likely candidate for the task runtime for efficient CPU utilization.
*   Non-blocking I/O will be integrated via an event loop (e.g., using epoll on Linux, kqueue on macOS, IOCP on Windows).
*   The runtime will be part of `std` or a core library linked by default.

## 10. Open Questions

*   Exact naming of `Future` type and related traits.
*   Specific API for `runtime::spawn`, `runtime::scope`, and channel creation.
*   Granularity of cancellation (cooperative cancellation points).
*   Detailed API for `std::sync` primitives if exposed widely.
*   Executor model: single global executor vs. multiple configurable executors.
*   Integration with FFI: How `async` operations interact across FFI boundaries.

This specification outlines a modern, safe, and ergonomic concurrency model for Seen. Further refinements will occur during implementation and testing phases.
