# Repository Guidelines

These contributor guidelines are specific to this repository’s layout and tooling. Keep changes focused and minimal; prefer small, reviewable patches.

## Project Structure & Modules
- `seen_*`: Rust crates (lexer, parser, typechecker, IR, CLI, etc.).
- `compiler_seen/`: Self‑hosting compiler pieces written in Seen.
- `docs/`: Plans and guides (see `docs/SELF_HOSTING_PLAN.md`, `docs/quickstart.md`).
- `examples/`, `tests/`: Samples and tests.

## Build, Test, Dev Commands
- Build CLI: `cargo build -p seen_cli --release`
- Run: `seen run <file.seen>` (interprets)
- Check: `seen check <file.seen>` (syntax + types)
- Emit IR: `seen build <file.seen> --output a.ir`
- Build native (LLVM): `seen build <file.seen> --backend llvm --output a.out`
- Unit tests (workspace): `cargo test`

## Coding Style & Naming
- Rust: rustfmt default; 4‑space indent; modules use `snake_case`; types `CamelCase`.
- Seen: `fun` for functions; `CamelCase` types; files end with `.seen`.
- Keep functions small; prefer clear names over brevity.

## Testing Guidelines
- Rust: add tests near code (`mod tests {}`) or under `tests/`.
- Seen: keep examples minimal and runnable via `seen run` or `seen check`.
- Name tests for behavior (e.g., `parses_imports_ok`).

## Commit & PR Guidelines
- Use Conventional Commits: `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`.
- PRs: include a clear description, linked issues, and repro steps. Add before/after where applicable.

## Agent‑Specific Instructions (Agents Ignore)
- Do not commit binaries or generated artifacts; ignore:
  - `target/`, `**/target/`, `stage1.c`, `stage*.seen`, `*.exe`, `*.o`.
- Avoid editing performance/benchmark dumps or large generated files.
- When bootstrapping, do not rewrite historical docs; update `docs/SELF_HOSTING_PLAN.md` instead.
- Keep changes surgical: avoid renames, global refactors, or file moves unless requested.

## Security & Config Tips
- No secrets in sources or scripts.
- Keep dependencies pinned via `Cargo.lock`.
- Prefer local builds; only symlink `seen` into PATH if you understand the impact.
