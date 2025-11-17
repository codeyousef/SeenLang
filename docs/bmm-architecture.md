# SeenLang Compiler Architecture

**Generated:** 2025-11-17  
**Document Type:** Technical Architecture  
**Scope:** Complete Compiler System

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [System Architecture](#system-architecture)
3. [Compiler Pipeline](#compiler-pipeline)
4. [Module Breakdown](#module-breakdown)
5. [Code Generation Backends](#code-generation-backends)
6. [Memory Architecture](#memory-architecture)
7. [Runtime System](#runtime-system)
8. [Tooling & IDE Support](#tooling--ide-support)
9. [Critical Gaps & Technical Debt](#critical-gaps--technical-debt)
10. [Future Architecture](#future-architecture)

---

## Executive Summary

SeenLang implements a multi-stage compiler with a traditional frontend (lexer → parser → typechecker) feeding into an intermediate representation (IR) layer, which then branches to multiple backend targets. The architecture supports both JIT compilation via Cranelift and is designed for AOT compilation via LLVM.

### Architecture Pattern
**Multi-backend Compiler Pipeline** with staged compilation and pluggable code generation

### Key Architectural Decisions
- **Rust workspace monorepo** for modular crate organization
- **Chumsky parser combinators** for maintainable parsing
- **Trait-based type system** with Hindley-Milner inference
- **Multiple IR levels:** High-level seen_ir → MLIR → Backend-specific
- **Dual execution modes:** JIT (Cranelift) and AOT (LLVM - incomplete)

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     SeenLang Compiler                        │
└─────────────────────────────────────────────────────────────┘
                            │
                ┌───────────┴───────────┐
                │                       │
         ┌──────▼──────┐         ┌─────▼──────┐
         │   Frontend  │         │   Tooling  │
         │   Pipeline  │         │    Layer   │
         └──────┬──────┘         └────────────┘
                │                      │
    ┌───────────┼───────────┐         ├─ seen_cli
    │           │           │         ├─ seen_lsp
┌───▼───┐  ┌───▼────┐  ┌───▼────┐   └─ seen_tooling
│Lexer  │→ │Parser  │→ │TypeChk │
└───────┘  └────────┘  └───┬────┘
 seen_lexer  seen_parser    │
                            │
                     ┌──────▼──────┐
                     │  seen_ir    │
                     │  (High-level│
                     │     IR)     │
                     └──────┬──────┘
                            │
                ┌───────────┴──────────────┐
                │                          │
         ┌──────▼──────┐            ┌─────▼──────┐
         │  seen_mlir  │            │  Backends  │
         │ (Mid-level) │            │            │
         └──────┬──────┘            └─────┬──────┘
                │                         │
                │          ┌──────────────┼──────────────┐
                │          │              │              │
                │   ┌──────▼───────┐ ┌───▼────┐  ┌──────▼──────┐
                └──→│seen_cranelift│ │ LLVM   │  │seen_runtime │
                    │    (JIT)     │ │Backend │  │             │
                    └──────────────┘ │⚠️MISSING│  └─────────────┘
                                     └────────┘
                                          │
                                     AOT Codegen
                                     (Production)
```

---

## Compiler Pipeline

### Stage 1: Lexical Analysis (seen_lexer)

**Purpose:** Transform source text into token stream

**Key Responsibilities:**
- UTF-8 text processing with NFC normalization
- Multi-language keyword recognition (English/Arabic/Chinese)
- String interpolation lexing
- Comment handling
- Position tracking for error reporting

**Technology:** Custom lexer with Unicode support

**Input:** `.seen` source files  
**Output:** `Token` stream

---

### Stage 2: Syntax Parsing (seen_parser)

**Purpose:** Build Abstract Syntax Tree (AST) from tokens

**Key Responsibilities:**
- Expression parsing with operator precedence
- Statement and declaration parsing
- Pattern matching syntax
- Module and import resolution
- Syntax error recovery

**Technology:** Chumsky parser combinators

**Input:** `Token` stream  
**Output:** `AST` (Abstract Syntax Tree)

---

### Stage 3: Type Checking (seen_typechecker)

**Purpose:** Type inference, checking, and semantic analysis

**Key Responsibilities:**
- Hindley-Milner type inference
- Trait resolution
- Lifetime analysis
- Borrow checking (regions-based)
- Type unification
- Sealed class verification

**Technology:** Custom type system implementation

**Input:** `AST`  
**Output:** Typed `HIR` (High-level IR)

---

### Stage 4: IR Generation (seen_ir)

**Purpose:** Lower to intermediate representation

**Key Responsibilities:**
- HIR → IR lowering
- Control flow graph construction
- SSA form generation
- Function inlining decisions
- Dead code elimination (basic)

**Technology:** Custom IR design

**Input:** Typed `HIR`  
**Output:** `seen_ir::IR`

---

### Stage 5: Mid-level Optimization (seen_mlir)

**Purpose:** MLIR-based optimizations

**Key Responsibilities:**
- E-graph rewriting
- Algebraic simplifications
- Loop optimizations
- Function specialization

**Technology:** MLIR integration

**Input:** `seen_ir::IR`  
**Output:** Optimized `MLIR`

---

### Stage 6: Code Generation Backends

#### Option A: Cranelift JIT (seen_cranelift) ✅ **IMPLEMENTED**

**Purpose:** Fast JIT compilation for development

**Key Responsibilities:**
- IR → Cranelift IR translation
- Machine code generation
- Memory management
- Fast compilation (< 50ms target)

**Technology:** Cranelift compiler backend

**Status:** ✅ **Functional**  
**Use Case:** Development, testing, `seen run`

**Input:** Optimized IR  
**Output:** Executable machine code (in-memory)

---

#### Option B: LLVM Backend ⚠️ **MISSING**

**Purpose:** Production AOT compilation with aggressive optimization

**Key Responsibilities:**
- IR → LLVM IR translation
- LLVM optimization pipeline invocation
- Native object file generation
- Debug info generation
- Cross-platform binary production

**Technology:** LLVM (via llvm-sys or inkwell)

**Status:** ⚠️ **NOT IMPLEMENTED - CRITICAL GAP**  
**Use Case:** Production builds, benchmarks, `seen build`

**Input:** Optimized IR  
**Output:** Native object files (.o) → Linked binary

**Missing Components:**
1. LLVM IR code generator (`seen_ir` → LLVM IR)
2. LLVM context and module management
3. Type mapping (Seen types → LLVM types)
4. Function translation
5. Instruction lowering
6. Calling convention handling
7. Debug metadata generation
8. Object file emission

---

## Module Breakdown

### Core Compiler Modules

| Module | Purpose | Dependencies | Status |
|--------|---------|--------------|--------|
| **seen_support** | Common utilities, error types | None | ✅ Complete |
| **seen_core** | Shared compiler infrastructure | seen_support | ✅ Complete |
| **seen_lexer** | Tokenization | seen_support | ✅ Complete |
| **seen_parser** | AST construction | seen_lexer, chumsky | ✅ Complete |
| **seen_typechecker** | Type inference & checking | seen_parser | ✅ Complete |
| **seen_ir** | IR generation & representation | seen_typechecker | ✅ Complete |
| **seen_mlir** | MLIR optimizations | seen_ir | ✅ Complete |
| **seen_cranelift** | Cranelift JIT backend | seen_ir | ✅ Complete |
| **seen_runtime** | Runtime system | seen_core | ✅ Complete |

### Language Feature Modules

| Module | Purpose | Status |
|--------|---------|--------|
| **seen_oop** | Classes, inheritance, polymorphism | ✅ Complete |
| **seen_concurrency** | Async, tasks, channels | ✅ Complete |
| **seen_reactive** | Reactive programming primitives | ✅ Complete |
| **seen_effects** | Algebraic effects system | ✅ Complete |
| **seen_advanced** | Advanced language features | ✅ Complete |
| **seen_memory_manager** | Memory management utilities | ✅ Complete |
| **seen_shaders** | Shader compilation support | ✅ Complete |

### Tooling Modules

| Module | Purpose | Status |
|--------|---------|--------|
| **seen_cli** | Command-line interface | ✅ Complete |
| **seen_lsp** | Language Server Protocol | ✅ Complete |
| **seen_tooling** | Development tools | ✅ Complete |
| **seen_self_hosting** | Self-hosting infrastructure | ⚠️ In Progress |

---

## Code Generation Backends

### Current State

```
Compilation Modes:
├── JIT Mode (seen run) ✅
│   └── Cranelift → Machine Code (in-memory)
│
└── AOT Mode (seen build) ⚠️ INCOMPLETE
    └── LLVM Backend → Native Binary
        └── ⚠️ NOT IMPLEMENTED
```

### Expected Flow (with LLVM)

```
Source Code (.seen)
    ↓
[Lexer] → Tokens
    ↓
[Parser] → AST
    ↓
[Typechecker] → Typed HIR
    ↓
[IR Gen] → seen_ir
    ↓
[MLIR Opts] → Optimized IR
    ↓
    ├─→ [Cranelift] → JIT Execution ✅
    │
    └─→ [LLVM Backend] ⚠️ MISSING
            ↓
        LLVM IR
            ↓
        [LLVM Optimizer]
            ↓
        Object Files (.o)
            ↓
        [Linker]
            ↓
        Native Binary
```

---

## Memory Architecture

### Region-based Memory Management

**Concept:** Vale-style regions for safe, deterministic memory

**Components:**
- **seen_memory_manager:** Region allocation and tracking
- **RAII semantics:** Automatic cleanup on scope exit
- **Generational references:** Handle-based access with UAF protection
- **No GC:** Deterministic destruction

**Benefits:**
- Memory safety without borrow checker complexity
- Predictable performance
- Zero-cost abstractions

---

## Runtime System

### seen_runtime Components

1. **Task Scheduler**
   - Async/await runtime
   - Work-stealing thread pool
   - Structured concurrency support

2. **Memory Allocator**
   - Region-aware allocation
   - Fast path for small objects
   - Integration with system allocator

3. **Standard Library Bridge**
   - FFI to system libraries
   - Platform abstraction layer

4. **Panic Handler**
   - Abort-on-panic (no unwinding)
   - Stack trace generation

---

## Tooling & IDE Support

### seen_lsp (Language Server Protocol)

**Features:**
- Hover documentation
- Go-to-definition
- Find references
- Syntax highlighting
- Real-time diagnostics
- Code formatting
- Semantic tokens

**Integration:** VSCode extension in `vscode-seen/`

### seen_cli (Command-line Interface)

**Commands:**
- `seen build` - AOT compilation ⚠️ (requires LLVM)
- `seen run` - JIT execution ✅
- `seen test` - Test runner
- `seen fmt` - Code formatter
- `seen check` - Type checking only

---

## Critical Gaps & Technical Debt

### 1. LLVM Backend Implementation ⚠️ **HIGH PRIORITY**

**Impact:** Blocks production use, benchmarks, and AOT compilation

**Required Work:**
- Create `seen_llvm` crate
- Implement IR → LLVM IR translator
- Type system mapping
- Function and instruction lowering
- Debug info generation
- Integration with `seen build` command

**Estimated Effort:** 4-6 weeks (senior compiler engineer)

**Dependencies:**
- LLVM 14+ (via inkwell or llvm-sys)
- Complete seen_ir specification
- Calling convention documentation

---

### 2. Self-hosting Completion

**Status:** Stage 0-2 complete, type errors remaining

**Blockers:**
- Some type inference edge cases
- Module system integration
- Bootstrap verification

**Impact:** Currently requires Rust toolchain

---

### 3. Architecture Documentation

**Gaps:**
- LLVM backend design spec (doesn't exist yet)
- Calling convention documentation
- ABI stability guarantees
- Cross-platform considerations

---

## Future Architecture

### Planned Enhancements

1. **WebAssembly Backend**
   - Direct seen_ir → WASM codegen
   - Browser and server-side execution

2. **Mobile Targets**
   - ARM/ARM64 optimization
   - iOS/Android runtime

3. **Distributed Compilation**
   - Remote caching
   - Parallel builds across machines

4. **Advanced Optimizations**
   - Machine learning-guided optimization
   - Superoptimization integration
   - Profile-guided optimization (PGO)

---

## Development Workflow

### Building the Compiler

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Build specific crate
cargo build -p seen_lexer
```

### Testing

```bash
# Run all tests
cargo test

# Test specific module
cargo test -p seen_parser

# Integration tests
cargo test --test integration

# Benchmarks
cargo bench
```

### Architecture Validation

```bash
# Check for circular dependencies
cargo tree

# Audit dependencies
cargo audit

# Coverage report
cargo tarpaulin
```

---

## Conclusion

The SeenLang compiler implements a robust multi-stage architecture with clear separation of concerns. The primary architectural gap is the **missing LLVM backend**, which blocks production AOT compilation and benchmark execution. Once implemented, the system will support both fast JIT development iteration and highly-optimized production binaries.

---

## Document Metadata

- **Generated by:** BMM document-project workflow
- **Architecture type:** Multi-backend Compiler Pipeline
- **Last updated:** 2025-11-17
- **Next review:** After LLVM backend implementation
