# Seen Language — Error Model & Diagnostics

## 1. Error Taxonomy
- **Recoverable errors:** encoded via `Result<T, E>`, `Option<T>`, and custom sum types. The `?` operator desugars to `match` and propagates errors deterministically.
- **Unrecoverable errors:** `abort` intrinsic terminates the process without unwinding. Used for compiler invariants or fatal runtime faults.
- **Panics:** Surface-level `panic("...")` expands to `abort` with message metadata; there is no stack unwinding.

## 2. Diagnostics Pipeline
- Lexing/parser/typechecker attach stable, NFC-normalized spans.
- Diagnostics carry:
  - Primary span (source range)
  - Related spans (e.g., borrow origin vs escape site)
  - Error code (`E0xxx`) and deterministic short message
  - Machine-readable notes for IDE tooling
- Formatter is deterministic: re-running formatting does not change diagnostic ordering or content.

## 3. Result Propagation Rules
- `?` only applies to `Result` and `Option`; custom types implement `Try`-style traits for extension.
- For `Result<T, E>`, `?` expands to:
  ```seen
  match value {
    Ok(v) => v,
    Err(e) => return Err(e),
  }
  ```
- In async contexts, `?` respects structured concurrency: if propagation crosses a `scope`, all spawned tasks must complete before the function returns.

## 4. Abort Semantics

- `abort` bypasses destructors for performance-critical paths; use with care. `defer` blocks marked `@[critical]` are
  still executed.
- CLI exposes `--abort-backtrace=off|on` to control metadata emission; deterministic profile defaults to `off` to keep hashes stable.

## 5. Diagnostic Guarantees
- Diagnostics are emitted in source order with stable sorting across platforms.
- CI regression tests under `tests/diagnostics` pin diagnostic JSON fixtures; changes require explicit approval.
- IDE integration via LSP uses the same diagnostic backend ensuring parity with CLI invocations.

## 6. Logging & Tracing
- Structured logs rely on deterministic timestamps (`--profile deterministic`), ensuring identical output for identical inputs.
- `seen trace` command records async scope transitions and region events; trace files embed diagnostic IDs for cross-reference.
