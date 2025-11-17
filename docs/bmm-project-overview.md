# SeenLang Project Overview

**Generated:** 2025-11-17  
**Scan Level:** Exhaustive  
**Project Type:** Compiler/Language Implementation

---

## Executive Summary

**SeenLang** is a revolutionary systems programming language designed to combine blazing performance with developer happiness. It features a self-hosted compiler implementation with support for multiple human languages (English, Arabic, Chinese, etc.) and targets universal deployment across native platforms, WebAssembly, and mobile.

### Critical Project Status

⚠️ **LLVM Backend Gap Identified:**  
The project currently lacks a complete LLVM backend for AOT (Ahead-of-Time) compilation, which is preventing production benchmark execution. The JIT (Just-in-Time) compilation path via Cranelift exists, but the LLVM code generation pipeline needs implementation to enable optimized native binary production.

---

## Project Classification

| Attribute | Value |
|-----------|-------|
| **Repository Type** | Monorepo (Cargo workspace) |
| **Primary Language** | Rust (transitioning to self-hosted) |
| **Project Category** | Compiler/Language Implementation |
| **Architecture Pattern** | Compiler Pipeline (Lexer → Parser → Typechecker → IR → Codegen) |
| **Crates** | 24+ modules |
| **Lines of Code** | 50,000+ (estimated) |

---

## Technology Stack

### Core Technologies

| Category | Technology | Version | Purpose |
|----------|------------|---------|---------|
| **Language** | Rust | 2021 Edition | Implementation language |
| **Build System** | Cargo | Workspace | Monorepo management |
| **JIT Backend** | Cranelift | Latest | Just-in-time compilation |
| **IR Optimization** | MLIR | Latest | Multi-level IR transformations |
| **Parser** | Chumsky | 0.10.1 | Parser combinator framework |
| **String Interning** | Lasso | 0.7.3 | Efficient symbol management |
| **Testing** | Proptest, rstest, insta | Various | Property-based & snapshot testing |
| **Benchmarking** | Criterion | 0.5.1 | Performance measurement |

### Missing/Incomplete Components

- **LLVM Backend:** Required for AOT compilation and production benchmarks
- **Full self-hosting:** Transitioning from Rust implementation

---

## Repository Structure

```
seenlang/
├── seen_*/                  # Core compiler crates (24+ modules)
│   ├── seen_lexer/         # Lexical analysis
│   ├── seen_parser/        # Syntax parsing
│   ├── seen_typechecker/   # Type inference & checking
│   ├── seen_ir/            # Intermediate representation
│   ├── seen_mlir/          # MLIR integration
│   ├── seen_cranelift/     # Cranelift JIT backend
│   ├── seen_runtime/       # Runtime system
│   ├── seen_core/          # Core utilities
│   ├── seen_cli/           # Command-line interface
│   ├── seen_lsp/           # Language Server Protocol
│   ├── seen_oop/           # Object-oriented features
│   ├── seen_concurrency/   # Concurrency primitives
│   ├── seen_reactive/      # Reactive programming
│   ├── seen_effects/       # Effect system
│   ├── seen_advanced/      # Advanced language features
│   ├── seen_memory_manager/# Memory management
│   ├── seen_shaders/       # Shader support
│   ├── seen_tooling/       # Development tools
│   └── seen_self_hosting/  # Self-hosting infrastructure
│
├── docs/                    # Comprehensive documentation
│   ├── Seen Language Spec.md
│   ├── Seen Design Document.md
│   ├── 0 - Seen MVP Development Plan.md
│   ├── 1 - Seen Alpha Development Plan.md
│   ├── 2 - Seen Beta Development Plan.md
│   ├── 3 - Seen Release Development Plan.md
│   └── [other planning docs]
│
├── benchmarks/             # Performance benchmarking suite
├── examples/               # Example programs
├── tests/                  # Integration tests
├── tools/                  # Development utilities
│   ├── perf_baseline/     # Performance baseline tool
│   ├── abi_guard/         # ABI stability verification
│   └── sign_bootstrap_artifact/
│
├── vscode-seen/           # VSCode extension
├── installer/             # Installation tooling
└── performance_validation/ # Performance testing

```

---

## Key Features

### Language Capabilities
- **Multi-language syntax:** Write in English, Arabic, Chinese, and more
- **Memory safety:** Vale-style regions + RAII
- **Zero-cost abstractions:** Monomorphization, inline expansion
- **Modern type system:** Hindley-Milner inference, traits, sealed types
- **Concurrency:** Async/await, structured concurrency, channels
- **Effects system:** Algebraic effects and handlers
- **OOP support:** Classes, inheritance, polymorphism
- **Reactive programming:** Built-in reactive primitives

### Compiler Features
- **JIT execution:** Sub-50ms startup via Cranelift
- **AOT compilation:** *Requires LLVM backend implementation*
- **Optimization:** E-graphs, ML-driven optimization, superoptimization, PGO
- **Self-hosting:** 100% implemented in Seen (in progress)
- **LSP support:** Full IDE integration
- **Deterministic builds:** Reproducible compilation

---

## Development Workflow

### Building
```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench
```

### Testing Strategy
- **Unit tests:** Per-crate test suites
- **Integration tests:** Cross-crate functionality
- **Property-based tests:** Using Proptest
- **Snapshot tests:** Using Insta
- **Performance tests:** Criterion benchmarks

### CI/CD
- GitHub Actions workflows for CI
- Automated testing on push/PR
- Performance regression detection
- Cross-platform validation

---

## Project Goals

### Immediate (MVP)
1. ✅ Complete frontend pipeline (lexer, parser, typechecker)
2. ✅ Implement core language features
3. ✅ Self-hosting Stage 0-2
4. ⚠️ **LLVM backend for AOT compilation** ← Current Gap
5. ⏳ Production-ready benchmarks

### Near-term (Alpha)
- Multi-platform native compilation
- Stable ABI
- Package management
- Enhanced tooling

### Long-term (Beta/Release)
- WebAssembly target
- Mobile platform support
- Full optimization pipeline
- Production ecosystem

---

## Critical Technical Debt

### High Priority
1. **LLVM Backend Implementation**
   - **Impact:** Blocks production AOT compilation and benchmarks
   - **Effort:** Significant - requires full LLVM IR generation
   - **Dependencies:** seen_ir module complete

2. **Self-hosting Completion**
   - **Impact:** Enables Rust removal
   - **Effort:** Ongoing - type errors remain
   - **Dependencies:** All compiler stages functional

### Medium Priority
- Comprehensive error messages
- Documentation gaps
- Test coverage improvements
- Performance optimization

---

## Getting Started

### For Contributors
1. Review [Seen Language Spec.md](./Seen Language Spec.md) for language design
2. Read [Seen Design Document.md](./Seen Design Document.md) for architecture
3. Check [0 - Seen MVP Development Plan.md](./0 - Seen MVP Development Plan.md) for roadmap
4. See [CONTRIBUTING.md](../vscode-seen/CONTRIBUTING.md) for contribution guidelines

### For Users
1. Build from source using instructions in README.md
2. Explore [examples/](../examples/) directory
3. Try [quick start guide](./quickstart.md)

---

## External Resources

- **Repository:** https://github.com/seen-lang/seen (referenced)
- **License:** MIT
- **Version:** 1.0.0-alpha

---

## Document Metadata

- **Generated by:** BMM document-project workflow
- **Workflow version:** 1.2.0
- **Scan depth:** Exhaustive
- **Last updated:** 2025-11-17
