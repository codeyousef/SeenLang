# Seen Language Project - Claude Code Context

**Version**: 2.1 | **Phase**: MVP Development | **Target**: Self-hosting compiler in 3 months

## Project Overview

Seen (س) is a revolutionary systems programming language designed to be the world's most performant language while providing intuitive developer experience. Key innovations:

- **Dual execution**: `seen run` (JIT <50ms) + `seen build` (AOT beats C/Rust)
- **Vale-style memory model**: Zero overhead safety without borrow checker complexity
- **Universal deployment**: Same codebase for backend, web (WASM), mobile (iOS/Android), desktop
- **Zig-style C interop**: Import C headers directly, no bindings needed
- **Self-hosting goal**: Compiler written in Seen within 3 months
- **Multi-target**: Native, WASM, mobile from single source

## Critical Development Rules

### Rule 0: Complete Implementation Only
- **NO MOCKS, STUBS, OR TODOs** - Ever. Every function must be fully implemented
- **NO WORKAROUNDS** - Solve problems correctly the first time
- **REAL CODE ONLY** - All examples must be actual working implementations
- **NO PLACEHOLDERS** - Every component must be production-ready

### Rule 1: Test-Driven Development (Mandatory)
- **Tests written FIRST** - No exceptions. Write failing test, then implement
- **Real implementation testing** - No mocking internal functions
- **Performance benchmarks required** - All components must meet performance targets
- **Integration tests mandatory** - Test actual system behavior, not isolated units

### Rule 2: Single Responsibility Principle
- **One purpose per file** - Each file should have a single, clear responsibility
- **Maximum 500 lines per file** - Split larger files into focused modules
- **Clear naming conventions** - Names should immediately convey purpose
- **Minimal dependencies** - Each module should have minimal external dependencies

### Rule 3: Performance First
- **All code must meet performance targets** - No performance regressions allowed
- **Benchmark everything** - Every optimization claim must be proven
- **Memory efficiency mandatory** - Track and optimize memory usage continuously
- **Profile-guided development** - Use profiling data to guide implementation decisions

## Project Structure

```
seenlang/
├── CLAUDE.md                    # This file
├── Development Plans/           # Phase-based development roadmaps
│   ├── 0-MVP Development Plan.md
│   ├── 1-Alpha Development Plan.md  
│   ├── 2-Beta Development Plan.md
│   └── 3-Release Development Plan.md
├── compiler_seen/              # Self-hosted Seen compiler (target)
│   ├── src/                    # Seen compiler source (written in Seen)
│   ├── tests/                  # Comprehensive test suite
│   └── benchmarks/             # Performance benchmarks
├── compiler_bootstrap/         # Bootstrap Rust compiler (temporary)
│   ├── lexer/                  # Lexical analysis
│   ├── parser/                 # Syntax analysis & AST
│   ├── typechecker/            # Type system & inference
│   ├── ir/                     # Intermediate representation
│   └── codegen/                # Code generation (LLVM/MLIR)
├── seen_std/                   # Standard library
│   ├── core/                   # Core types & primitives
│   ├── collections/            # Data structures
│   ├── io/                     # I/O operations
│   └── net/                    # Networking
├── seen_runtime/               # Runtime system
│   ├── memory/                 # Memory management
│   ├── gc/                     # Region-based allocation
│   └── threading/              # Concurrency primitives
├── tools/                      # Development tools
│   ├── lsp/                    # Language server
│   ├── formatter/              # Code formatter
│   └── debugger/               # Debug support
├── tests/                      # Integration tests
│   ├── performance/            # Performance test suite
│   ├── compatibility/          # C interop tests
│   └── showcase/               # Real-world applications
└── docs/                       # Documentation
    ├── language_spec/          # Language specification
    ├── api/                    # API documentation
    └── tutorials/              # Learning materials
```

## Essential Commands

### Build Commands
```bash
# Core development (Bootstrap phase - Rust)
cargo build --release          # Build bootstrap compiler
cargo test                     # Run all tests
cargo bench                    # Run benchmarks

# Self-hosted development (Post-MVP - Seen) 
seen build                     # Build current project
seen build --release          # Optimized build
seen build --target wasm      # WebAssembly build
seen test                      # Run all tests
seen test --bench             # Run benchmarks
seen format                    # Format all code/docs
seen check                     # Fast syntax/type check
seen clean                     # Clean build artifacts
```

### Development Workflow
```bash
# Before making changes
seen test                      # Ensure all tests pass
seen format --check           # Verify formatting

# After implementation  
seen test                      # Verify new tests pass
seen bench                     # Check performance targets
seen format                    # Format all changes
git add -A && git commit       # Commit with descriptive message
```

## Code Quality Standards

### Testing Requirements
- **100% test coverage** for all core components
- **Performance tests required** for every optimization
- **Integration tests mandatory** for language features
- **Cross-platform testing** for all targets (native, WASM, mobile)
- **Regression testing** to prevent performance degradation

### Performance Targets (Non-negotiable)
- **Lexer**: >10M tokens/second
- **Parser**: >1M lines/second
- **Type checking**: <100μs per function
- **JIT startup**: <50ms cold start
- **Memory usage**: ≤ equivalent Rust programs
- **C interop**: