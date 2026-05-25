# Change Log

All notable changes to the Seen VS Code extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.6.0] - 2026-05-18

### Added
- Target configuration coverage for Seen 0.9.1 native and cross targets, including `linux-riscv64`.
- Syntax highlighting coverage for new 0.9.1 collection, byte buffer, primitive buffer, memory, and priority queue types.
- Snippets for StringBuilder, ByteBuffer, HashMap direct lookup, and integer priority queues.

### Changed
- Compiler auto-download platform detection now recognizes Linux RISC-V hosts.
- Marketplace documentation now points users at the current target list and 0.9.1 tooling surface.

## [1.5.0] - 2026-05-10

### Added
- Package workflow commands for `seen pkg fetch`, `pack`, `prebuild`, and `publish`.
- Snippets and highlighting coverage for package imports, `effect(Token)`, `@using`, `@operator`, `in`, sealed classes, shared-module calls, and hot reload helpers.
- Editor parity guards for language lists, unsupported custom LSP requests, grammar tokens, snippets, and LSP source drift.

### Changed
- Replaced stale language choices with Japanese across extension settings and commands.
- Translation and language switching now use compiler CLI/config behavior instead of unsupported custom LSP requests.
- Standalone diagnostics now call supported `seen check` commands and parse stdout plus stderr.

## [Unreleased]

### Planned
- Semantic highlighting
- Code actions and refactoring support
- Enhanced completion with context-aware suggestions

## [1.4.1] - 2026-05-01

### Added
- Syntax and snippets for `///` block comments and `@export` functions.
- Hot reload snippets for importing, loading, and calling shared modules.
- Shared-module compile command for PIC object and object-manifest workflows.

### Changed
- Documented PIC/object-manifest extension settings for shared-library builds.

## [1.1.0] - 2026-01-28

### Added
- Complete syntax highlighting with TextMate grammar
- LSP integration for IntelliSense, go-to-definition, find references
- Real-time error detection and diagnostics
- Code formatting integration
- Debug Adapter Protocol support
- REPL integration
- Multi-language keyword support for 6 languages (en, ar, es, ru, zh, ja)
- Language switching and code translation commands
- Build, run, test, and check commands
- 24 code snippets matching actual Seen syntax
- Benchmark CodeLens integration
- Project initialization command
- Problem matcher for compiler output
- File icons for light and dark themes

### Fixed
- Extension no longer blocks activation when compiler is not installed
- Removed deprecated `client.onReady()` usage in LSP client
- Snippets now use correct Seen syntax (no semicolons, no parentheses on control flow, `r:` return types)
