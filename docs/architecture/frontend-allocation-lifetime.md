# GPU frontend allocation lifetime

`lowerGpuSource` is the owning source-to-IR boundary. Its frontend result and AST
are borrowed from an exclusive `OwnedFrontend` session. GPU lowering clones the
data that survives in the returned IR or diagnostics, then releases the session.
Callers release that independent result through its existing ownership API.

The Seen allocation owner records allocator provenance, addresses and byte sizes
in a fixed open-addressed table. It permits at most 262,144 live records and
256 MiB of temporary allocations. Exceeding either limit stops with
`E_GPU_FRONTEND_LIMIT`. Tombstones permit repeated allocation/free cycles without
growing metadata. Capture is paused during lowering so independent IR does not
enter the temporary owner. Notifications remain attached while paused to account
for explicit frees or reallocations of captured storage.

The runtime-v14 callback ABI only reports allocation, reallocation and release
events. Registration is exclusive and local to the calling thread; callbacks
cannot recursively observe their own storage. Seen decides retention, lookup,
limits, allocator selection and final cleanup. Metadata is allocated before
registration and released after removing the observer. Aligned, pool, budgeted
and unbudgeted allocations retain their original release convention.

`parseOwnedGpuArtifactCli` applies the same owner to GPU option parsing, including
inline-value copies and error construction. Its `OwnedGpuCli` result borrows argv
and exposes an immutable parsed view until release. It detaches notifications
after parsing so a source-frontend owner can run independently. Captured storage
must not be individually freed or reallocated while detached; values that escape
the snapshot must be copied. Releasing the snapshot frees its recorded storage
and metadata without disturbing another observer.

Parser UI registries and recovery counters are swapped for private session
scratch, then restored before releasing captured storage. Sessions are serial:
the legacy parser has global state and is not safe for concurrent frontend
sessions. AST views must not escape session release. CPU compiler frontend
lifetimes remain governed by their existing compilation scope.
Parser recovery makes the owned result unsuccessful even when the legacy
frontend leaves its success flag set. Source-to-IR lowering rejects that partial
tree with `E_GPU_SOURCE` and releases the same temporary owner.

The frontend ownership contract repeats successful and rejected source lowering,
checks surviving IR after frontend cleanup, and requires exact allocation
baselines. The allocation owner contract grows all four native hash-map storage
shapes, verifies lookups and tombstone replacement, and requires the same baseline
after releasing each session. Both run in fast, ThinLTO, UBSan and ASan profiles;
ASan retains leak detection.
