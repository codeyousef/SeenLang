# Change Log

All notable changes to the Seen VS Code extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-01-28

### Added
- Complete syntax highlighting with TextMate grammar
- LSP integration for IntelliSense, go-to-definition, find references
- Real-time error detection and diagnostics
- Code formatting integration
- Debug Adapter Protocol support
- REPL integration
- Multi-language keyword support for 6 languages (en, ar, es, ru, zh, fr)
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

## [Unreleased]

### Planned
- Semantic highlighting
- Code actions and refactoring support
- Enhanced completion with context-aware suggestions
