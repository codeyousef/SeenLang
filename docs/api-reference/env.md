# Environment

Module: `env/env`

Environment helpers read process environment variables and expose small runtime
configuration utilities.

```seen
import env::env
```

Use this module when application behavior needs to inspect the host environment
without directly calling runtime C helpers.

| Function | Signature | Description |
|----------|-----------|-------------|
| `args` | `() r: Array<String>` | Create an owned command-line argument snapshot |
| `releaseArgs` | `(arguments: Array<String>) r: Void` | Consume a snapshot and its strings |
| `tryGet` | `(name: String) r: Option<String>` | Read an environment variable when present |
| `getOrDefault` | `(name: String, defaultValue: String) r: String` | Read an environment variable with a fallback |
| `get` | `(name: String) r: String` | Read an environment variable or return an empty string |
| `has` | `(name: String) r: Bool` | Check whether an environment variable exists |
| `set` | `(name: String, value: String) r: Bool` | Set or replace an environment variable |
| `removeEnv` | `(name: String) r: Bool` | Remove an environment variable |
| `remove` | `(name: String) r: Bool` | Alias for `removeEnv` |

`args()` owns both its array and each copied argument string. Call
`releaseArgs(arguments)` exactly once after all borrowed argument views are
finished. Keep the snapshot unmodified: do not add borrowed strings or duplicate
its owned entries. `Array.free()` releases only array storage.

Present values returned by `tryGet`, `get` and `getOrDefault` are copied strings,
not borrowed environment views. A missing `getOrDefault` returns its caller's
fallback unchanged, so that fallback does not become an owned environment copy.
Native owners releasing a present-value copy must release both its C allocation
and its tracked `length + 1` bytes; do not free borrowed fallbacks or pass them to
`releaseArgs`. The deterministic context's borrowed-view contract below is
separate from these copying helpers.

## Deterministic process inputs

`determinism/context` exposes
`deterministicContextFromProcess(request: DeterministicContextRequest,
operationContext: OperationContext) r: Result<DeterministicExecutionContext, SeenError>`.
This process constructor is an explicitly nondeterministic boundary: it requires
deterministic mode to have captured the bounded launch inputs before user code.
Changing ambient arguments or environment variables afterward does not change
the captured values or enable a previously disabled snapshot.
The process constructor requires runtime compatibility `runtime-v15`.

Only policy-granted application arguments and environment entries enter the
context. Application argument index `0` selects the first argument after the
executable name. A missing granted argument returns `core.004e.invalid`; a missing
environment entry remains distinct from a present empty value.

Argument bytes and environment values borrow the immutable process-lifetime
launch snapshot. The constructor does not allocate an owned copy of every
argument, and failure does not retain copies of unrelated arguments. Do not free
these borrowed strings or pass them to `releaseArgs`. Policy strings remain
borrowed from the request; keep their storage alive while using the context.
This does not change the owned-snapshot contract of `env/env.args()`.
