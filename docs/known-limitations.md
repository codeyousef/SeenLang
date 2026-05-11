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

- The shipped release command is `seen compile`, not the newer source-only
  wrapper command shape in `compiler_seen/src/main.seen`.
- `seen --version` and `seen --help` are not currently exposed by the shipped
  binary; invoking an unknown command prints usage.
- The shipped backend selector documents LLVM-only behavior, even though older
  docs and source comments mention a C backend.

## Packages

- Registry dependency versions are exact-only for now.
- `seen pkg publish` writes static registry files to a local directory; it does
  not upload to a remote service.
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
