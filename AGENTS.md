# Repository Guidelines

## Project Structure & Module Organization

- `seen_*` crates house the Rust toolchain (lexer, parser, typechecker, IR, interpreter, LSP). Each crate owns its unit
  tests and should stay independently buildable.
- `compiler_seen/` contains the self-hosted compiler implemented in Seen; bootstrap entry points live under
  `compiler_seen/src`.
- `docs/` is the source of onboarding material (e.g., `docs/0 - Seen MVP Development Plan.md`, `docs/quickstart.md`)
  plus living design notes.
- `examples/` and `tests/` bundle runnable samples and cross-crate regression suites. Do not check in build artifacts.

## Build, Test, and Development Commands

- `cargo build -p seen_cli --release [--features llvm]` produces the primary CLI; enable `llvm` to activate the LLVM
  backend.
- `cargo test` exercises the full workspace; use `cargo test -p <crate>` for focused loops.
- `target/release/seen_cli run PATH.seen` executes programs with the interpreter;
  `... build PATH.seen --backend llvm --output <bin>` drives the LLVM pipeline.
- `seen determinism PATH.seen -O2` runs the determinism pass to compare IR hashes.

## Coding Style & Naming Conventions

- Rust code follows rustfmt defaults (4-space indent, `snake_case` modules, `CamelCase` types). Run `cargo fmt` before
  submitting patches.
- Seen source uses `fun`/`let`, `CamelCase` types, and NFC-normalized UTF-8 identifiers. Respect project `Seen.toml`
  visibility policy (`caps` vs `explicit` pub modifiers).
- Keep modules small and expression-oriented; prefer explicit visibility over implicit exports when `Seen.toml` requests
  it.

## Testing Guidelines

- Add crate-local unit tests (`mod tests {}`) beside new Rust logic and Seen fixtures under `<crate>/tests`.
- For parser/IR updates, add targeted fixtures and regenerate determinism hashes to confirm stability.
- Exercise bootstrap flows with `scripts/self_host_llvm.sh` once the LLVM backend is touched.

## Commit & Pull Request Guidelines

- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`) and keep patches focused.
- PRs must explain motivation, repro steps, and before/after behavior. Attach determinism or hash output when relevant.
- Never commit generated binaries or `target/` artifacts; rely on `.gitignore` defaults and keep dependencies pinned via
  `Cargo.lock`.

## Security & Configuration Tips

- LLVM tooling must target version 15 (`llvm-config-15`, `LLVM_SYS_150_PREFIX=/usr/lib/llvm-15`). Verify with
  `llvm-config-15 --version`.
- Never embed secrets in scripts or tests. Sanitize inputs passed to external tooling (`__ExecuteCommand`,
  `__WriteFile`) and limit operations to workspace paths.
