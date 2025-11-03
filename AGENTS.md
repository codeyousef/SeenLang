# Repository Guidelines

## Project Structure & Module Organization
- `seen_*` crates contain the Rust pipeline: lexer, parser, typechecker, IR, interpreter, CLI, etc. Treat each crate as an independent lib with local tests.
- `compiler_seen/` holds the self-hosting compiler written in Seen; bootstrap targets live under `compiler_seen/src`.
- `docs/` carries plans and onboarding references (`docs/0 - Seen MVP Development Plan.md`, `docs/quickstart.md`).
- `examples/` and `tests/` provide runnable samples and cross-crate regression suites. Keep new assets minimal and committed as source, not build outputs.

## Build, Test, and Development Commands
- `cargo build -p seen_cli --release [--features llvm]` builds the primary CLI (enable `llvm` when exercising the LLVM backend).
- `cargo test` runs the entire workspace; use `cargo test -p crate_name` for targeted loops.
- Interpreter/CLI smoke: `target/release/seen_cli run examples/hello.seen`, `... build compiler_seen/src/main.seen --backend llvm --output stage1_seen`.
- Determinism helpers: `seen determinism <file.seen> -O2` (Rust pipeline) and `scripts/self_host_llvm.sh` for staged bootstrap checks.

## Coding Style & Naming Conventions
- Rust follows rustfmt defaults (4-space indent, module `snake_case`, types `CamelCase`). Run `cargo fmt` before submitting.
- Seen source uses `fun`/`let`, `CamelCase` types, and UTF-8 identifiers normalized to NFC (lexer enforces this). Keep files small and expression-oriented.
- Prefer explicit visibility: configure per-project via `Seen.toml` (`visibility = "caps"` or `"explicit"`); parser rejects mismatches.

## Testing Guidelines
- Add crate-local unit tests (`mod tests {}`) alongside new logic. Integration tests live under `<crate>/tests`.
- Exercise parser/IR changes with targeted fixtures; confirm runtime features via interpreter tests (see `seen_interpreter/tests`).
- For bootstrap work, regenerate stage artifacts and compare hashes (Stage2 vs Stage3) once LLVM path stabilizes.

## Commit & Pull Request Guidelines
- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`). Keep patches surgical—avoid sweeping refactors across crates unless planned.
- PRs must describe motivation, reproduction steps, and before/after behavior. Link issues, attach determinism/hash output where relevant.
- Never commit generated binaries or `target/` directories; rely on `.gitignore` defaults.

## Security & Configuration Tips
- Keep host dependencies pinned via `Cargo.lock`; LLVM builds require LLVM 15 (`llvm-config-15`, `LLVM_SYS_150_PREFIX`).
- Do not embed secrets in scripts or tests. When invoking external tooling (`__ExecuteCommand`, `__WriteFile`), sanitize inputs and limit scope to workspace temp paths.
