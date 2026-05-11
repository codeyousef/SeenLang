# Memory Regions

Modules: `memory/mapped_region`, `memory/pool_region`, `memory/stack_region`

These modules expose region-style memory helpers used by higher-level runtime
and application code.

| Type | Module | Purpose |
|------|--------|---------|
| `MappedRegion` | `memory/mapped_region` | Memory-mapped allocation region |
| `PoolRegion` | `memory/pool_region` | Pool-backed allocation region |
| `StackRegion` | `memory/stack_region` | Stack-like allocation region |

See [Memory Model](../memory-model.md) for conceptual background.
