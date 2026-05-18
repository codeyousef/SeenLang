# Framework

Modules: `framework/component`, `framework/executor`, `framework/hotreload`,
`framework/middleware`, `framework/routing`, `framework/store`,
`framework/vdom`, `framework/vdom_lite`

The framework modules are application-facing building blocks for component
registries, simple task execution, middleware/routing, state stores, virtual DOM
diffing, and hot reload.

| Area | Notable Types |
|------|---------------|
| Components | `ComponentInfo`, `ComponentRegistry` |
| Executor | `Task`, `TaskHandle`, `SimpleExecutor` |
| Hot reload | `HotModule`, `HotReloadWatcher`, `HotState`, `StateSnapshot` |
| Middleware | `MiddlewareContext`, `SimpleMiddlewareStack`, `LoggingMiddleware`, `AuthMiddleware`, `RateLimitMiddleware`, `TimingMiddleware`, `SandboxedMiddleware` |
| Routing | `Route`, `RouteMatch`, `Router` |
| Store | `MutationEntry`, `StoreSnapshot`, `StoreRegistry`, `TimeTravel` |
| VDOM | `VNode`, `Patch`, `VDOMDiffer`, `VDOMPatcher`, `VDOMEngine` |

## Facade Component Syntax

The compiler recognizes facade component functions and component-local UI
constructs:

```seen
component Panel(title: String) {
    state open: Bool = true
    computed label: String = title
    uiEffect {
        println(label)
    }
}
```

Named arguments, callback block arguments, and trailing/named slot blocks are
parsed as part of the declarative UI surface. The frontend emits diagnostics for
missing and duplicate stable keys in dynamic component children.

See also [Hot Reload](hotreload.md).
