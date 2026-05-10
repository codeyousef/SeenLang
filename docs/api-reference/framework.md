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

See also [Hot Reload](hotreload.md).
