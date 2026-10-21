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

## Optional aggregate values

An `Option<Record>` or `Record?` containing a `repr(C)` record owns its container
and a payload box. Creating it from a record value copies that value; creating
it from a record literal transfers the literal's temporary storage. Aggregate
payload layouts are bounded at 16 MiB; invalid or larger layouts report
`E_OPTION_LAYOUT`.

`unwrap()` checks presence and returns a record value copy. Repeated unwraps
remain valid while the Option owner is alive. Call `.free()` on the Option once
to release its payload box and container; a None Option releases its container.
An unwrapped value remains usable after that cleanup.

Pointer fields remain borrowed. Keep their buffer or region owner alive while
using the copied record. Releasing the Option leaves those pointees owned by
their original buffer or region, as with `Span<T>` and `PackedView`.

Passing or returning an aggregate record literal copies its value and releases
its temporary storage. Local aggregate initialization follows the same rule.
The copy preserves borrowed pointer fields; it does not extend their owners'
lifetimes. Both pooled and larger literal storage follow this cleanup rule.

`inference.span.packedGet` checks untrusted packed-view geometry and returns an
owned Option. `unsafePackedGet` returns a byte without allocating; its caller
must first prove the data owner remains alive, the index and bit multiplication
are in range, the bit width is 2..8, and every accessed byte is covered. The
bulk SIMD unpacker proves the full buffer range before using that primitive.
