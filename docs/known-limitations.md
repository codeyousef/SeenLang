# Known Limitations

This page tracks public caveats that matter when using the shipped compiler.
Private bootstrap notes and historical recovery plans live outside the public
docs.

## Bootstrap and Rebuilds

- Full compiler rebuilds must be memory-capped. Use the pattern in
  [Bootstrap System](bootstrap.md) instead of running `scripts/safe_rebuild.sh`
  uncapped.
- If a rebuild fails, inspect the first concrete failing module/log before
  retrying. Blind retries can hide deterministic compiler issues.
- `scripts/fix_ir.py` remains a compatibility guard for malformed IR emitted by
  older frozen-bootstrap paths. It is not a substitute for fixing source
  codegen bugs.

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
