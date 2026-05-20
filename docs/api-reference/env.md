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
| `args` | `() r: Array<String>` | Read command-line arguments |
| `tryGet` | `(name: String) r: Option<String>` | Read an environment variable when present |
| `getOrDefault` | `(name: String, defaultValue: String) r: String` | Read an environment variable with a fallback |
| `get` | `(name: String) r: String` | Read an environment variable or return an empty string |
| `has` | `(name: String) r: Bool` | Check whether an environment variable exists |
| `set` | `(name: String, value: String) r: Bool` | Set or replace an environment variable |
| `removeEnv` | `(name: String) r: Bool` | Remove an environment variable |
| `remove` | `(name: String) r: Bool` | Alias for `removeEnv` |
