# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Seenlang (س) is a systems programming language targeting performance superiority over C++, Rust, and Zig while providing automated memory safety without garbage collection. It features native bilingual support (English/Arabic) and Kotlin-inspired syntax.

**Current Status**: Phase 0-2 Complete (Core compiler & self-hosting kernel implemented)
**Next Target**: Phase 3 - Self-hosting compiler and FFI

## Build Commands

### Basic Build
```bash
# Set environment for LLVM (required for codegen)
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
export CARGO_TARGET_DIR=target-wsl  # Or /tmp/cargo_target to avoid WSL issues

# Build all packages
cargo build --all

# Build specific packages
cargo build -p seen_lexer
cargo build -p seen_parser
cargo build -p seen_typechecker
cargo build -p seen_interpreter
cargo build -p seen_ir
cargo build -p seen_cli
```

### LLVM Build (for code generation)
```bash
# Use the build script
./build_with_llvm.sh

# Or manually with LLVM environment
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
export CARGO_TARGET_DIR=/tmp/cargo_target
cargo build --package seen_ir
cargo build --package seen_cli
```

### Apply LLVM Fixes (if needed)
```bash
./apply_llvm_fixes.sh
```

## Testing Commands

### Run All Tests
```bash
CARGO_TARGET_DIR=target-wsl cargo test --all
```

### Run Tests for Specific Crates
```bash
CARGO_TARGET_DIR=target-wsl cargo test -p seen_lexer
CARGO_TARGET_DIR=target-wsl cargo test -p seen_parser
CARGO_TARGET_DIR=target-wsl cargo test -p seen_typechecker
CARGO_TARGET_DIR=target-wsl cargo test -p seen_interpreter
CARGO_TARGET_DIR=target-wsl cargo test -p seen_ir
```

### Run Specific Test
```bash
CARGO_TARGET_DIR=target-wsl cargo test -p seen_parser test_parse_struct_literal_basic
```

### Test with Output
```bash
CARGO_TARGET_DIR=target-wsl cargo test -- --nocapture
```

### Quality Reports
```bash
# Generate test quality report
./scripts/test-quality-report.sh

# Run coverage analysis (requires cargo-tarpaulin)
./scripts/coverage.sh

# Run mutation testing (requires cargo-mutants)
./scripts/mutation-test.sh
```

## CLI Usage

### Create New Project
```bash
./target-wsl/debug/seen_cli new myproject
```

### Build Project
```bash
./target-wsl/debug/seen_cli build
```

### Run Project
```bash
./target-wsl/debug/seen_cli run
```

## Code Architecture

### Crate Structure
The project follows a modular architecture with separate crates for each compiler phase:

- **seen_lexer**: Tokenization with bilingual keyword support
  - Reads `keywords.toml` for language mappings
  - UTF-8 source with Arabic/English identifiers
  
- **seen_parser**: AST generation
  - Recursive descent parser
  - Error recovery mechanisms
  - Supports all Phase 2 features (enums, generics, pattern matching)

- **seen_typechecker**: Type system and inference
  - Type checking and inference
  - Struct, enum, and generic type support
  - Comprehensive error reporting

- **seen_interpreter**: Tree-walking interpreter (for testing)
  - Direct AST execution
  - Used for validating semantics before codegen

- **seen_ir**: LLVM IR generation
  - Converts AST to LLVM IR
  - Requires LLVM 18 with Polly
  - Handles basic codegen (functions, primitives, control flow)

- **seen_cli**: Command-line interface
  - Project management (new, build, run)
  - Orchestrates compilation pipeline

### Key Technical Features

1. **Automated GC-Free Memory Model**: Novel memory management combining linear types and generational references
2. **E-Graph Optimization Engine**: Advanced optimization using equality saturation (planned)
3. **Bilingual First-Class Tooling**: English/Arabic keywords throughout the toolchain
4. **Performance-First Design**: Every feature benchmarked against C++/Rust/Zig equivalents

### Important Files

- `keywords.toml`: Bilingual keyword mappings
- `seen.toml`: Project configuration (in each Seen project)
- `specifications/`: Formal language and API specifications
- `docs/Design Document.md`: Comprehensive language design
- `docs/Development Plan.md`: Current roadmap (v6.0)

## Development Workflow

1. **Test-Driven Development**: Write failing tests before any implementation
2. **Performance Benchmarks**: Create benchmarks alongside tests
3. **Bilingual Testing**: Test both English and Arabic variants
4. **Use Proper Target Directory**: Set `CARGO_TARGET_DIR` to avoid WSL issues
5. **LLVM Environment**: Always set LLVM variables when working with codegen

## Current Implementation Status

### Completed (Phase 0-2)
- Core language features (val/var, functions, structs, enums)
- Type system with inference
- Pattern matching and generics
- Basic LLVM code generation
- Bilingual lexer support
- Standard library core (Option, Result, Vec, String)

### In Progress (Phase 3)
- Self-hosting compiler (rewriting in Seen)
- C FFI implementation
- Standard library I/O

### Planned Features
- E-graph optimization engine
- Full memory model implementation
- Advanced concurrency (async/await, M:N scheduler)
- Python interoperability
- LLM assistance integration

## Known Issues & Workarounds

1. **WSL File System**: Use `CARGO_TARGET_DIR=target-wsl` or `/tmp/` directories
2. **LLVM Dependencies**: Ensure `libpolly-18-dev` is installed for LLVM builds
3. **Codegen Limitations**: Some features (advanced patterns, full struct support in IR) are still being implemented

## Testing Philosophy

- **Red-Green-Refactor-Benchmark**: TDD cycle with performance validation
- **No Stubs/TODOs**: Every implementation must be complete
- **Performance Targets**: Must meet or exceed C/Rust equivalents
- **Comprehensive Coverage**: Unit, integration, E2E, and performance tests

## Lint and Type Check Commands

When completing tasks, run these commands to ensure code quality:
```bash
# Format code
cargo fmt --all

# Lint code
cargo clippy --all

# Type check
cargo check --all
```