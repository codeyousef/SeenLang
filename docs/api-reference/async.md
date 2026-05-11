# Async

Modules: `async/future`, `async/runtime`

Seen's async stdlib surface provides the basic future/task types used by
compiler and application code that lowers `async`, `await`, and coroutine-style
work.

| Type | Module | Purpose |
|------|--------|---------|
| `Future` | `async/future` | Future contract and polling state |
| `Context` | `async/future` | Polling context passed to futures |
| `Waker` | `async/future` | Wake handle for suspended work |
| `ThenTask` | `async/future` | Future continuation helper |
| `MapTask` | `async/future` | Future mapping helper |
| `CoroutineTask` | `async/runtime` | Runtime task wrapper |
| `AsyncRuntime` | `async/runtime` | Simple async runtime coordinator |
| `AsyncScope` | `async/runtime` | Scope for grouped async work |

Import modules directly:

```seen
import async::future
import async::runtime
```
