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

These package-manager bullets describe the unreleased Seen 0.10 source tree,
not the published Seen 0.9.5 compiler binary.

- Registry dependency versions are exact-only for now.
- `seen pkg publish` writes legacy local-static files for development; it does
  not upload to the hosted service or run hosted ingestion/security gates.
- HTTPS registry resolution currently fails closed until signed metadata
  verification and hardened extraction are implemented.
- `Seen.lock` v2 is currently written as a resolution report; `--locked` and
  `--frozen` enforcement are not implemented yet.
- Package `capabilities` and dependency `allow` are draft contract fields; the
  current compiler does not enforce capability consent yet.
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
