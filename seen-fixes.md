# Seen Compiler Bug Catalog

Status as of 2026-04-08 after re-verifying the absolute-path direct-entry project case against the current compiler tree.

---

## Fixed in this tree

### C12. Direct-entry project module emitted invalid pointer-versus-integer LLVM IR — Fixed in this tree

**Severity**: Critical

**Root cause**:

- The compiler was resolving `Seen.toml`, `project.modules`, and `[build].entry` relative to the process working directory instead of the input file's project root.
- Absolute-path direct-entry builds could therefore drop sibling project modules, mis-shape type/layout information, and eventually emit the invalid pointer-versus-integer LLVM IR described in the original report.
- The fix now resolves the nearest `Seen.toml` from the input path, applies `project.modules` only to declared project members, keeps `[build].entry` import seeding for non-member project files, and updates the stale `seen_std` manifest entry from `src/async/task.seen` to `src/async/runtime.seen`.

**Compiler/test coverage delivered**:

- `compiler_seen/src/main_compiler.seen`
- `compiler_seen/src/main.seen`
- `tests/misc_root_tests/seen_fix_regressions.sh`
- `seen_std/Seen.toml`
- `seen_std/Seen.lock`

**Observed local result**:

- The exact absolute-path repro now succeeds both from the project root and from an external working directory, and both runs log `Reading Seen.toml...` plus `Found 2 modules`.
- A new non-member absolute-path regression proves standalone files under a project tree no longer incorrectly absorb unrelated `project.modules`.
- The current tree also passes the focused Seen-fixes regression harness, the 66-case multilingual E2E suite, native smoke validation, and the broader platform matrix.

---

## Summary Table

| ID  | Category | Severity | Summary | Status |
|-----|----------|----------|---------|--------|
| C12 | Modules  | Critical | Absolute-path direct-entry project module now resolves the correct project graph and no longer emits invalid pointer/integer LLVM IR | Fixed |

**Open blockers in this file**: none.
