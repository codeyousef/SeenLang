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
