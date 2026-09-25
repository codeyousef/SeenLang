# Repository Instructions

## Publishing

- Do not mention the blocked tool names in commit messages, branch names, pull requests, release notes, or similar publish-facing text unless the user explicitly asks for them.
- Do not add co-author trailers or similar authorship metadata unless the user explicitly asks for them.

## Builds

- Do not run project builds or rebuild scripts without an explicit memory limit derived from current system memory.
- Prefer capped serial or low-memory rebuild paths when available.
- For releases, follow `docs/releasing-three-platforms.md`; Linux x64, macOS arm64, and Windows x64 are one required asset set. Do not publish a partial release.

## Repository hygiene

- Keep generated build/test output and agent-local state in ignored paths; do not stage it.
