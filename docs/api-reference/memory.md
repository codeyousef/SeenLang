# Memory

Modules: `memory/allocation`, `memory/mapped_region`, `memory/pool_region`,
`memory/stack_region`

These modules expose allocation-budget helpers and region-style memory helpers
used by higher-level runtime and application code.

| Type | Module | Purpose |
|------|--------|---------|
| `AllocError` | `memory/allocation` | Allocation failure details for `Result` APIs |
| `MemoryStats` | `memory/allocation` | Current runtime memory limit, usage, peak, remaining budget, and failures |
| `MappedRegion` | `memory/mapped_region` | Memory-mapped allocation region |
| `PoolRegion` | `memory/pool_region` | Pool-backed allocation region |
| `StackRegion` | `memory/stack_region` | Stack-like allocation region |

See [Memory Model](../memory-model.md) for conceptual background.
