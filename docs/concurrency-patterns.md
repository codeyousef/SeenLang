# Seen Concurrency Patterns

This note records the current state of Seen's channel and scoped job APIs so
that early users can exercise the new async runtime features without spelunking
through unit tests.

## Scoped Jobs

- Wrap fan-out work in `jobs.scope { ... }`.
- Each `jobs.spawn` returns immediately; `jobs.scope` drains the outstanding
  work before control leaves the block.
- Scoped jobs pair naturally with region-based memory rules. Apply `defer` to
  guarantee clean-up even if a job raises an error.

## Channel Futures

- `let (tx, rx) = Channel<ValueType>()` yields generational sender and receiver
  handles. Handles become stale after `close`.
- `await tx.send(value)` resolves when capacity frees (bounded channels) or the
  runtime has scheduled a copy (unbounded).
- `await rx.receive()` resolves with `ValueType`. Closed channels propagate an
  error.
- `select { rx <- value => handle(value) }` now blocks on a real future. The
  runtime parks the current task and wakes it via channel wakers once any arm
  becomes ready.
- Timeout arms can be written as `select { timeout(1.second) => on_timeout() }`.

## CLI & Stage Status

- `seen run` executes channel futures through the interpreter's waker bridge.
- LLVM output still links against the placeholder runtime; the CLI's Stage flow
  will replace the placeholder helpers with the Rust runtime once the FFI layer
  is stabilized.
