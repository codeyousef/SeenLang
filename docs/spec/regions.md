# Seen Language — Regions & Memory Management

## 1. Ownership Model
- **Regions** provide scoped ownership: memory allocated inside a region is released in **O(1)** by destroying the region.
- **RAII:** Values run `Drop` deterministically at end-of-scope in LIFO order; `defer` statements push cleanup actions onto the same stack.
- **Generational handles:** Engine-facing APIs use handle tables keyed by `(index, generation)` to detect stale references.

## 2. Region Syntax
```seen
region stack upload {
  let scratch = allocate_buffer(4 * KB);
  defer free(scratch);
  use_upload_queue(scratch);
}

region bump {
  let tiles = allocate_tiles();
  defer release_tiles(tiles);
}

region cxl_near streaming_cache {
  ingest_large_dataset();
}
```
- Optional **strategy hints** (`stack`, `bump`, `cxl_near`) appear immediately after `region`. A second identifier is treated as the human-readable name; omitting the hint defaults to `auto`.
- Region names scope lifetime hints in diagnostics.
- Regions can be nested; inner regions destroy before outer scope exits.
- `region` blocks integrate with async runtime by emitting suspension points only where proven safe.

## 3. Allocation Strategies
- Inline hints map directly to runtime strategies:
  - `stack`: perfectly nested, O(1) drop semantics; preferred for short-lived scopes with ≤8 allocations.
  - `bump`: amortized O(1) allocation with bulk tear-down at scope exit; selected when regions perform many allocations or spawn child regions.
  - `cxl_near`: pin allocations close to compute when CXL memory tiers are present.
  - `auto` (default): the compiler selects between `stack` and `bump` using lifetime/escape analysis.
- Static analysis promotes small, child-free regions to `stack` automatically even without an explicit hint. Regions that only coordinate control flow but allocate nothing also default to `stack`.
- Debug builds inject assertions when hints cannot be honoured (e.g., `stack` applied to a region with escaping borrows); release builds assume analysis already validated the choice.
- Region descriptors record allocation pressure and the chosen strategy for profiling (`seen trace --regions`).

## 4. Generational References
```rust
struct Handle {
    index: u32,
    generation: u32,
}
```
- Handle tables live in arenas; acquiring/clearing entries bumps the generation counter.
- Runtime validates handles before dereferencing; stale handles trigger deterministic `Abort::StaleHandle` diagnostics tying back to source.
- Region IDs (`RegionId`) use 32-bit indices into a contiguous arena, keeping lookups cache-friendly and deterministic across runs.

## 5. Async & Structured Concurrency Constraints

- Tasks spawned within `scope { ... }` inherit the parent region stack by value. Regions marked `@[region(shared)]`
  permit concurrent borrows; otherwise, mutable access is exclusive.
- Suspension points (`await`) require all borrowed regions to be in a quiescent state; the compiler enforces `await`-free sections when holding `mut &` references.
- Cancelling a task runs deferred actions in reverse order before unwinding the region stack; cancellation cannot observe partially-dropped values.

## 6. Profiling & Instrumentation
- Release builds remove redundant runtime checks when static analysis proves safety.
- `tools/perf_baseline` captures region drop cost and allocation counts; PASS criteria require sub-microsecond drops for hot regions.
- Debug builds surface region overflows, double drops, and long-lived allocations via structured diagnostics.

## 7. Determinism Guarantees
- Region destruction order is fully deterministic; even panic-driven abort paths run deferred actions in registration order.
- Region layout decisions feed into IR hashing; builds that alter layout cause deterministic hash mismatches to alert CI.
