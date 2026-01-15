# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Seen is a self-hosted systems programming language with:
- Multi-language support (English, Arabic, Spanish, Russian, Chinese, French)
- Revolutionary optimization pipeline (E-graph, ML, Superopt, PGO)
- Vale-style memory management (region-based, no GC)
- Self-hosting compiler (100% Seen implementation)

**Current Status:** In active development with working self-hosted compiler and LLVM backend. The compiler is bootstrapped from a Rust implementation and can compile itself.

## Build Commands

### Building from Bootstrap Compiler

The project uses a two-stage build process:

```bash
# Build the Rust bootstrap compiler (in rust_backup/)
cd rust_backup
CARGO_TARGET_DIR=../target-wsl cargo build --release
cd ..

# The bootstrap compiler is now at:
# ./target-wsl/release/seen_cli (or ./target-wsl/debug/seen_cli for debug)
```

### Using the Seen Compiler

```bash
# Compile a Seen program
./target-wsl/release/seen_cli build <source.seen> [output]

# Type check only (no compilation)
./target-wsl/release/seen_cli check <source.seen>

# Run tests
./target-wsl/release/seen_cli test

# JIT execution (interpreter mode)
./target-wsl/release/seen_cli run <source.seen>

# Format code
./target-wsl/release/seen_cli fmt <source.seen>
```

### Building the Self-Hosted Compiler

The self-hosted compiler (written in Seen) is in `compiler_seen/`:

```bash
cd compiler_seen
../target-wsl/release/seen_cli build src/main_compiler.seen -o stage1_compiler
```

### Testing

```bash
# Test the Rust components
cd rust_backup
cargo test --workspace

# Run specific test suites
cargo test -p seen_typechecker
cargo test -p seen_parser
cargo test -p seen_lexer

# Run integration tests
cd compiler_seen
../target-wsl/release/seen_cli run run_tests.seen
```

### Benchmarks

```bash
# Run production benchmarks
./run_production_benchmarks.sh

# Run simple benchmarks
./run_simple_benchmarks.sh

# Individual benchmark
cd benchmarks
../target-wsl/release/seen_cli build matrix_multiply.seen
./matrix_multiply
```

## Architecture

### Two Compilers in the Repository

1. **Rust Bootstrap Compiler** (`rust_backup/`): A full-featured Rust implementation used to bootstrap
   - Workspace with multiple crates: seen_cli, seen_core, seen_lexer, seen_parser, seen_typechecker, seen_ir, seen_interpreter, seen_lsp
   - Provides LLVM backend, interpreter backend, and all tooling
   - Built with `cargo build` in the `rust_backup/` directory

2. **Self-Hosted Seen Compiler** (`compiler_seen/`): The compiler written in Seen itself
   - Located in `compiler_seen/src/`
   - Main entry point: `main.seen` (production) and `main_compiler.seen` (stage1)
   - Written entirely in Seen and compiled by the bootstrap compiler
   - Goal is to fully replace the Rust compiler

### Self-Hosted Compiler Pipeline

The Seen compiler (`compiler_seen/src/`) follows a 5-stage pipeline:

**Stage 1: Lexer** (`compiler_seen/src/lexer/`)
- `SeenLexer` class tokenizes source code
- `KeywordManager` handles multi-language keyword support via TOML files
- Tracks position info for error reporting
- Entry: `tokenize(source: String, language: String) -> Array<Token>`

**Stage 2: Parser** (`compiler_seen/src/parser/`)
- Converts tokens to AST (Abstract Syntax Tree)
- Key node types: `ProgramNode`, `FunctionNode`, `ClassNode`, `StatementNode`, `ParserExpressionNode`
- Two-pass parsing for stability
- Entry: `parse(tokens: Array<Token>) -> ProgramNode`

**Stage 3: Type Checker** (`compiler_seen/src/typechecker/`)
- `TypeChecker` class performs type inference and validation
- `Environment` manages symbol tables
- Smart casting after null checks
- Entry: `check(program: ProgramNode) -> TypeInferenceResult`

**Stage 4: Frontend Integration** (`compiler_seen/src/bootstrap/`)
- `run_frontend()` orchestrates lexer → parser → typechecker
- Produces `FrontendResult` with diagnostics
- Converts errors to `FrontendDiagnostic` with source locations

**Stage 5: Code Generation** (`compiler_seen/src/codegen/`)
- **IR Generator** (`compiler_seen/src/ir/`): Converts AST to LLVM IR
  - `IRBuilder` constructs SSA-form IR
  - `IRGenerator` implements three-pass generation (signatures, types, implementations)
- **C Generator** (`c_gen.seen`): Emits C99 code (fallback backend)
- **LLVM Backend** (`llvm_backend.seen`): Full LLVM compilation with aggressive optimizations
  - Runs `opt -O3` with vectorization, loop unrolling, inlining
  - Compiles with `llc` to object files
  - Links with `clang` using LTO

**Data Flow:**
```
Source (.seen) → Lexer → Parser → TypeChecker → Frontend → IR Generator → LLVM Backend → Executable
                                                         ↘ C Generator → GCC → Executable
```

### Rust Bootstrap Compiler Architecture

Located in `rust_backup/`, this provides the initial compiler used to build the self-hosted version:

**Key Crates:**
- `seen_cli`: Main entry point and CLI
- `seen_core`: Core types and utilities
- `seen_lexer`: Tokenization (uses chumsky parser combinator)
- `seen_parser`: AST generation (uses chumsky)
- `seen_typechecker`: Type inference and validation
- `seen_ir`: LLVM IR generation with SSA
- `seen_interpreter`: JIT interpreter backend
- `seen_lsp`: Language Server Protocol implementation
- `seen_runtime`: Runtime library (C bindings)

**Build System:**
- Cargo workspace with unified dependencies
- LTO enabled for release builds
- Profile-guided optimization support
- Cross-compilation targets defined in workspace

### Language Support

Multi-language keywords defined in `languages/`:
- `en/` - English (default)
- `ar/` - Arabic
- `es/` - Spanish
- `ru/` - Russian
- `zh/` - Chinese
- `fr/` - French

Each language has TOML files for:
- Keywords (control, types, vars, module, literals, async, modifiers, memory, meta, misc)
- Operators
- Standard library names (math, collections, io, env, str)

### Standard Library

Located in `seen_std/`:
- Core types: String, Int, Float, Bool, Char
- Collections: Vec, Array, HashMap, Map
- I/O: File, Path, stdin, stdout
- Process: Command, env
- Math: sqrt, sin, cos, pow, abs, floor, ceil

Standard library is written in Seen and gets linked with compiled programs.

### Key Patterns and Conventions

**Result Types:** Functions return result containers with success flags and error lists
- `FrontendResult`, `TypeInferenceResult`, `CodegenResult`

**Three-Pass Compilation:** Used in both parser and IR generator
1. Collect declarations/signatures
2. Process types/structures
3. Generate implementations

**Environment/Symbol Tables:** Scoped variable and function tracking
- `Environment` class manages nested scopes
- Push/pop scope methods for blocks

**Tracing:** Extensive debug output with `[trace]` and `[DEBUG]` prefixes
- Use these when debugging compiler issues

**Node Naming:** AST nodes end with `Node` suffix
- `ProgramNode`, `FunctionNode`, `StatementNode`, `ParserExpressionNode`

**Backend Selection:** Multiple backends available via `--backend` flag
- LLVM (default, best performance)
- C (fallback, portable)
- Interpreter (fast startup, JIT)

## Important Files and Locations

### Critical Compiler Files
- `compiler_seen/src/main.seen` - Production compiler entry point
- `compiler_seen/src/main_compiler.seen` - Stage1 compiler driver
- `compiler_seen/src/bootstrap/frontend.seen` - Frontend orchestration
- `compiler_seen/src/codegen/llvm_backend.seen` - LLVM code generation (16KB, aggressive opts)
- `compiler_seen/src/codegen/generator.seen` - Backend framework (41KB)
- `compiler_seen/src/ir/ir_generator.seen` - AST to LLVM IR conversion

### Configuration Files
- `Seen.toml` - Root project configuration (test project)
- `compiler_seen/Seen.toml` - Compiler project configuration
- `rust_backup/Cargo.toml` - Rust workspace configuration

### Bootstrap Artifacts
- `target-wsl/` - Symlink to `target/` (build outputs)
- `target-wsl/release/seen_cli` - Rust bootstrap compiler executable
- `stage1_*` - Various stage1 compiler builds for testing

### Documentation
- `README.md` - Comprehensive project documentation
- `IMPLEMENTATION_STATUS.md` - Current implementation status and known issues
- `docs/` - Design documents, specifications, research papers

## Current Implementation Status

**Production Ready:**
- Core language features (let, var, functions, classes, generics)
- Type system with inference and smart casting
- Memory management (region-based, no GC)
- Full compilation pipeline (lexer → parser → typechecker → codegen)
- LLVM backend with aggressive optimizations
- Multi-language keyword support

**In Progress:**
- Self-hosting: Stage1 compiler builds and runs, but has Vec memory corruption issues
- Standard library completion (JSON, networking APIs incomplete)
- Benchmark implementations (2/10 complete)

**Known Issues:**
- Vec memory corruption in self-hosted compiler (`realloc(): invalid next size`)
- TOML file path resolution for language keywords needs absolute paths
- Some stdlib methods missing (String.startsWith, String.endsWith recently fixed)

## Development Workflow

### Adding Language Features

1. Update AST definitions in `compiler_seen/src/parser/ast.seen`
2. Extend parser in `compiler_seen/src/parser/real_parser.seen`
3. Add type checking in `compiler_seen/src/typechecker/`
4. Implement IR generation in `compiler_seen/src/ir/ir_generator.seen`
5. Add tests in `compiler_seen/run_tests.seen`

### Fixing Compiler Issues

When debugging compiler issues:
1. Check the bootstrap compiler first (Rust code in `rust_backup/`)
2. Verify self-hosted compiler behavior matches
3. Use extensive tracing (look for `[trace]` and `[DEBUG]` output)
4. Test with minimal reproducible examples (create `repro_*.seen` files)

### Adding Standard Library Functions

1. Add implementation to `seen_std/src/` in appropriate module
2. Update `seen_std/src/prelude.seen` if it should be auto-imported
3. Add test cases
4. Rebuild compiler to include new functions

### Testing Changes

```bash
# Test Rust compiler components
cd rust_backup && cargo test --workspace

# Test self-hosted compiler
cd compiler_seen
../target-wsl/release/seen_cli run run_tests.seen

# Test specific feature with repro file
echo 'fun main() { println("test") }' > repro_test.seen
../target-wsl/release/seen_cli build repro_test.seen
./repro_test
```

## Common Tasks

### Building a Stage1 Compiler

```bash
cd compiler_seen
../target-wsl/release/seen_cli build src/main_compiler.seen -o ../stage1_new
cd ..
./stage1_new --help
```

### Comparing LLVM IR Output

```bash
# Generate LLVM IR for debugging
./target-wsl/release/seen_cli build test.seen --emit-llvm
cat test.ll

# Or with the self-hosted compiler
cd compiler_seen/src/codegen
cat llvm_backend.seen  # See optimization flags
```

### Running Benchmarks

```bash
# Build benchmark
cd benchmarks
../target-wsl/release/seen_cli build matrix_multiply.seen

# Compare with Rust
cd rust/matrix_mult
cargo build --release
time ../../target/release/matrix_mult
cd ../..
time ./matrix_multiply
```

## Debugging Tips

1. **Compiler Crashes:** Create minimal repro in `repro_*.seen` files at root
2. **Type Errors:** Check `compiler_seen/src/typechecker/` for type inference logic
3. **Codegen Issues:** Look at generated LLVM IR with `--emit-llvm`
4. **Runtime Errors:** Build with debug symbols and use `gdb` or `lldb`
5. **Memory Issues:** The self-hosted compiler has Vec corruption issues under investigation

## Project Philosophy

- **No Stubs:** All features should be fully implemented, not stubbed
- **Self-Hosting First:** Priority is getting the Seen compiler to compile itself
- **Performance Matters:** Target is 1.0x-1.5x Rust performance with 10x faster compilation
- **Multi-Language:** Code should work in English, Arabic, and other languages
- **Developer Experience:** Fast compilation, good error messages, modern tooling

## Debugging Infrastructure

The project has comprehensive debugging/tracing infrastructure. **Always use these tools instead of adding ad-hoc print statements.**

### Environment Variables

| Variable | Purpose | Values |
|----------|---------|--------|
| `SEEN_DEBUG_TYPES` | Type checker debugging | `1` to enable |
| `SEEN_TRACE_LLVM` | LLVM backend tracing | `1`, `all`, `inst`, `values`, `types`, `ir`, `layouts`, `gep`, `boxing` |
| `RUST_LOG` | Standard Rust logging | `warn`, `debug`, `trace` |

### CLI Debug Flags

```bash
# Enable all debugging
./target-wsl/release/seen_cli build source.seen output --debug

# Specific debug modes
./target-wsl/release/seen_cli build source.seen output --trace-llvm
./target-wsl/release/seen_cli build source.seen output --dump-struct-layouts
./target-wsl/release/seen_cli build source.seen output --runtime-debug
```

### Common Debugging Commands

```bash
# Debug type checking issues
SEEN_DEBUG_TYPES=1 ./target-wsl/release/seen_cli build program.seen

# Debug LLVM IR generation (all traces)
SEEN_TRACE_LLVM=all ./target-wsl/release/seen_cli build program.seen

# Debug struct field access (GEP operations)
SEEN_TRACE_LLVM=gep ./target-wsl/release/seen_cli build program.seen

# Dump LLVM IR to debug_ir.ll
SEEN_TRACE_LLVM=ir ./target-wsl/release/seen_cli build program.seen

# Debug boxing/unboxing for generics
SEEN_TRACE_LLVM=boxing ./target-wsl/release/seen_cli build program.seen

# Verbose Rust logging
RUST_LOG=debug ./target-wsl/release/seen_cli build program.seen
```

### Bootstrap Verifier

For multi-stage bootstrap verification, use `BootstrapVerifier` class in `compiler_seen/src/bootstrap/verifier.seen`:
- Logs to stdout and `bootstrap_verification.log`
- Tracks compilation stages, binary comparisons, hashes, and file sizes

### Key Files for Debugging

- `rust_backup/seen_ir/src/llvm_backend.rs` - LLVM trace options (`LlvmTraceOptions` struct)
- `rust_backup/seen_typechecker/src/checker.rs` - Type debug output
- `rust_backup/seen_cli/src/main.rs` - CLI flag parsing
- `compiler_seen/src/bootstrap/verifier.seen` - Bootstrap verification logging
