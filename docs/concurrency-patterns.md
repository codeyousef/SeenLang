# Concurrency Patterns in Seen

This guide documents the structured concurrency features that the MVP plan calls out in §PSH‑3. It explains how
`jobs.scope` integrates with the async runtime, how channel futures compose, and how to build `select`-based control
flow that remains deterministic across interpreter and LLVM backends.

## jobs.scope

`jobs.scope { ... }` mirrors `scope { ... }`, but is intended for job-system and channel heavy workloads. Every
non-detached `spawn` executed inside the scope is registered with the runtime. When the scope body finishes evaluating,
the runtime drains the outstanding tasks before unwinding the scope frame.

```seen
jobs.scope {
    spawn {
        let (tx, _) = Channel<Int>();
        await tx.Send(42);
    };
    1
}
```

Key points:

- Only `spawn` without `detached` participate in scope joins. Detached tasks are allowed but remain the caller's
  responsibility.
- Scope unwinding waits for the async runtime to report task completion. If any task fails, the failure bubbles out of
  the scope evaluation.
- `jobs.scope` nests: inner scopes join before outer scopes continue.

## Channel futures

Seen channels surface futures for both send and receive operations. A bounded channel back-pressures senders and
registers wakers so that `await tx.Send(value)` resumes as soon as capacity frees.

```seen
let (tx, rx) = Channel<Int>(capacity = 1);

jobs.scope {
    spawn { await tx.Send(7); };
    spawn { await tx.Send(9); }; // waits until the first send is received
    spawn {
        let first = await select {
            when rx receives value: { value }
        };
        let second = await select {
            when rx receives value: { value }
        };
    };
}
```

Implementation notes:

- Channel handles are generational. Dropping or closing a channel invalidates older handles, and wakers wake pending
  futures with an error when this happens.
- Send futures return `Boolean(true)` on success. Receive futures return the delivered `AsyncValue`.

## Select expressions

`select` races multiple channel operations fairly. Each `when` clause can receive into a pattern or execute a send.
Timeout arms are optional and run after the requested `Duration`.

```seen
let (input, output) = Channel<String>();
let (fallback_tx, fallback_rx) = Channel<String>();

jobs.scope {
    spawn { await output.Send("ok"); };

    let result = await select {
        when output receives value: { value }
        when fallback_rx receives value: { "fallback: " + value }
        when timeout(500.ms): { "timed out" }
    };

    // result == "ok" once the first send completes
}
```

Runtime behaviour:

- `select` pulls non-blocking attempts first. If no case is ready, it registers wakers with each participating channel
  and awaits a wake-up.
- When multiple cases become ready simultaneously the executor rotates the starting index so no arm starves.
- Timeout cases arm a timer and cancel the select if no other case fires before the deadline.

## Deterministic testing tips

- Use `jobs.scope` in tests to ensure spawned work finishes before the test asserts on side-effects.
- For pipeline-style channel tests, run the async runtime to completion (`runtime.run_until_complete()`) and only then
  inspect collected results.
- Keep channel capacities explicit in tests so that back-pressure behaviour remains reproducible across interpreter and
  LLVM backends.
