# Known Limitations

This page tracks public caveats that matter when using the shipped compiler.
Private bootstrap notes and historical recovery plans live outside the public
docs.

## Bootstrap and Rebuilds

- Every compiler rebuild must run inside one read-back-verified cgroup v2 scope
  that caps the aggregate process tree, disables swap, and limits tasks. A
  per-process `ulimit` or userspace RSS poller alone is not containment. Use the
  guarded path in [Bootstrap System](bootstrap.md).
- The automatic aggregate rebuild cap is the smaller of 60% of total memory
  and currently available memory after retaining a 10%-of-total system
  reserve, with one compiler and one optimizer worker. The build must fail
  before compiler work if a user systemd scope or any required limit read-back
  is unavailable.
- No equally hard standalone entry point is currently documented for direct
  compiler, prebuild-gate, performance-gate, or package-helper builds. Run those
  phases through `scripts/safe_rebuild.sh`; artifact-root isolation alone does
  not cap memory.
- Frozen and legacy builders are rejected unless capability discovery proves
  bounded `--jobs`/`--opt-jobs`, `--no-fork`, or an explicit serializer. A CLI
  silently accepting an unknown flag does not prove serial execution.
- If a rebuild fails, inspect the first concrete failing module/log before
  retrying. Blind retries can hide deterministic compiler issues.
- `scripts/fix_ir.py` is confined to an explicitly marked frozen Stage-1 IR
  compatibility path. Its output is never production-eligible; it is a
  bootstrap seed only. Current compiler, recovery, cross-build, and release
  paths pass emitted IR to LLVM unchanged and fail closed on rejection.
- Unsupported production source rewriting fails closed. The bootstrap
  source view may relocate files for frozen-manifest and symlink-hardening
  purposes, but every Seen source copy must remain byte-identical to checkout.

## Shipped CLI Shape

- The shipped release command is `seen compile`; `seen build` is not a shipped
  alias.
- `seen --version` / `seen -v` and `seen --help` / `seen -h` are supported by
  the shipped compiler.
- These are not shipped compiler commands yet: `seen init`, `seen fmt`, `seen format`, `seen clean`, and `seen test`. They fail with an explicit unsupported-command diagnostic instead of silently advertising source-wrapper behavior.
- The shipped backend selector is LLVM-only. `--backend=c` is intentionally
  unsupported until a production C backend is wired into the release entrypoint.

## Packages

- The development registry's official root is embedded and its public signed
  metadata and catalog are live. Production remains unprovisioned and fails
  closed without an embedded root.
- Controlled internal publishing is active, but every submitted release remains
  delayed, unavailable, and invisible to public catalog, resolution, and
  download until promotion is implemented.
- Hosted login/logout/whoami, private-package access, yanking, and reporting
  remain inactive.
- Package capability declarations are consent and policy signals, not an
  operating-system sandbox. In particular, FFI, unsafe operations, native
  linking, and process execution require the same review they would in local
  source.
- The compiler and package client are version-coupled. A partial installation
  that omits the matching `seen-pkg` binary, or supplies a client from another
  Seen release, is rejected.
- Hosted registry archives are source-only. Native prebuilt artifacts remain
  local path dependencies and are not accepted as hosted package contents.
- Local prebuilt artifacts are consumed through `{ artifact = "..." }`
  dependencies and are linked from `objects.tsv`.

## Determinism

`HashMap` and `HashSet` iteration order is nondeterministic. In deterministic
mode, use ordered collections such as `BTreeMap`/`BTreeSet` or explicitly mark
the nondeterministic usage where allowed.

## Secret handling

- `SecretBytes.clear` overwrites its mutable byte storage. `SecretString.clear`
  invalidates the logical owner, but immutable Seen string allocator storage
  cannot yet be guaranteed to be overwritten. Use `SecretBytes` when
  best-effort in-process zeroisation is required.
- Secret values redact their own formatting and have no serialization surface,
  but an explicitly revealed ordinary value is again the caller's
  responsibility. Reveal requires `SecretRevealPolicy.Allow` and is never the
  logging or serialization default.

## Language and Code Generation

- Plain enums and exhaustive matching are supported. Payload/data enums remain
  experimental; do not assume every construction, pattern, ownership, and drop
  path is complete without a focused test.
- Closure literals currently lower only when they do not capture enclosing
  locals. A closure environment and its ownership/lifetime rules are not yet a
  shipped feature.
- Sequential and structured branch use-after-move checks are enforced for
  explicit `@move` and `@c_resource` bindings. Ownership transfer through
  closure environments and task captures remains unsupported rather than
  implicitly copied.
- `parallel_for` is capture-free in 0.13.0. Its pthread worker callback has no
  outer-local environment, so examples must not read or mutate enclosing
  locals. Explicit value/reference/move captures and compiler-proven disjoint
  mutation are future work.
- `await` and the blocking async helpers use cooperative polling. Several
  awaited calls do not become concurrent merely by appearing in an async
  function; register coroutine handles with `runtime_spawn` and drive the
  runtime when cooperative interleaving is required.
- `comptime` supports the tested integer/string expressions, target predicates,
  parameters, assertions, and simple block control flow. Arbitrary recursive
  compile-time functions, heap-backed values, I/O, and general macro execution
  are not established release features.

## SIMD and GPU

- Native SIMD coverage is currently `f32x4`, `f32x8`, `f64x2`, `f64x4`,
  `i32x4`, and `i32x8`. Low-level vector load/store builtins take raw addresses,
  not `(Array, offset)` arguments.
- `--emit-glsl` emits inspectable shader/reflection artifacts and invokes
  `glslc` when available, but arbitrary Seen shader bodies are not guaranteed
  to translate faithfully. The generated host wrapper does not construct a
  usable Vulkan pipeline. Real GPU execution still needs explicit shader,
  buffer, pipeline, synchronization, and fallback handling through the runtime.

## Native Interoperability

- Seen `String` uses the `%SeenString` length/data representation and is not a C
  `char *`. Use a reviewed shim or explicit raw-pointer conversion for native C
  text APIs; the compatibility `CString` wrapper does not by itself promise a
  newly allocated NUL-terminated buffer.
- `pub fun` controls Seen visibility; use `@export` for an unmangled native
  symbol. Even then, verify emitted signatures and use `@repr(C)` for supported
  aggregate layouts before publishing a C header.
- Native library declarations belong under `[native.dependencies]`.

## Low-Level Runtime Rules

- Do not stack-allocate escaping `SeenArray` headers; use runtime allocation
  paths.
- Do not mark mutable array data pointers as LLVM `!invariant.load`.
- Runtime C functions that return 0/1 integer values for booleans may need
  explicit `trunc i64 to i1` in codegen.
- Floating-point `isNaN`/`isInfinite` checks must avoid LLVM `fast` flags that
  imply `nnan`/`ninf`.

## Reporting Issues

When reporting a compiler issue, include:

1. A minimal `.seen` reproduction.
2. The exact `seen compile` or `seen check` command.
3. Any relevant capped rebuild log or generated `.ll` artifact.
4. Whether the system-wide `seen` binary or `compiler_seen/target/seen` was used.
