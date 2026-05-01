# Hot Reload

The `framework.hotreload` module wraps dynamic shared-module loading and reload bookkeeping.

```seen
import framework.hotreload.{HotModule, getHotReloadError}
```

## HotModule

| Method | Return | Description |
|--------|--------|-------------|
| `HotModule.load(path: String)` | `Option<HotModule>` | Load a shared library from disk |
| `HotModule.loadNamed(path: String, name: String)` | `Option<HotModule>` | Load a shared library with a lookup name |
| `HotModule.find(name: String)` | `Option<HotModule>` | Find an already loaded module |
| `reload()` | `Bool` | Reload the module from its current path |
| `reloadFrom(path: String)` | `Bool` | Reload from a new shared-library path |
| `unload()` | `Void` | Close the loaded shared library |
| `getFunction(name: String)` | `*Void` | Return a raw function pointer |
| `callInt(name: String)` | `Int` | Call an exported `fun name() r: Int` entrypoint |
| `callIntPtr(name: String, arg0: *Void)` | `Int` | Call an exported `fun name(arg0: *Void) r: Int` entrypoint |
| `getVersion()` | `Int` | Current reload count |
| `isActive()` | `Bool` | Whether the module remains loaded |

## Typed Calls

Use `callInt` and `callIntPtr` when the host program needs to call a known shared-module signature without manually casting a raw pointer.

```seen
let loaded = HotModule.load("./plugin.so")
if loaded.isSome() {
    let module = loaded.unwrap()
    let value = module.callInt("plugin_answer")
    println("answer: {value}")
}
```

If loading or symbol lookup fails, inspect `getHotReloadError()`.
