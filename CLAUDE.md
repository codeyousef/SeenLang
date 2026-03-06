# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Seen is a **100% self-hosted** systems programming language with:
- Multi-language support (English, Arabic, Spanish, Russian, Chinese, French)
- Revolutionary optimization pipeline (E-graph, ML, Superopt, PGO)
- Vale-style memory management (region-based, no GC)
- Self-hosting compiler written entirely in Seen

**Current Status:** The compiler is fully self-hosted. Bootstrap verification confirms Stage 3 == Stage 4 (fixed-point achieved) with no external dependencies.

## Build Commands

### Using the Seen Compiler

The self-hosted compiler is at `compiler_seen/target/seen`:

```bash
# Compile a Seen program
./compiler_seen/target/seen build <source.seen> [output]

# Type check only (no compilation)
./compiler_seen/target/seen check <source.seen>

# JIT execution
./compiler_seen/target/seen run <source.seen>

# Format code
./compiler_seen/target/seen fmt <source.seen>
```

### Building the Compiler from Source

**Recommended: Use the safe rebuild script:**

```bash
# Safe rebuild with bootstrap verification
./scripts/safe_rebuild.sh
```

This script:
1. Builds stage2 from the frozen bootstrap compiler
2. Builds stage3 from stage2
3. Verifies bootstrap (stage2 == stage3 or stage3 == stage4)
4. Only updates production compiler if verification passes

**Manual rebuild:**

```bash
# Use frozen bootstrap to build a new compiler
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen stage2_new

# Verify bootstrap (should produce identical binaries)
./stage2_new compile compiler_seen/src/main_compiler.seen stage3_new
diff stage2_new stage3_new  # Should be identical
```

### Testing

```bash
# Run compiler tests
cd compiler_seen
../stage1_verified run run_tests.seen

# Test specific feature with repro file
echo 'fun main() { println("test") }' > repro_test.seen
./stage1_verified build repro_test.seen
./repro_test
```

### Benchmarks

```bash
# Run production benchmarks
./run_production_benchmarks.sh

# Run simple benchmarks
./run_simple_benchmarks.sh

# Individual benchmark
cd benchmarks
../stage1_verified build matrix_multiply.seen
./matrix_multiply
```

## Architecture

### Compiler Pipeline

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
- `run_frontend()` orchestrates lexer -> parser -> typechecker
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
Source (.seen) -> Lexer -> Parser -> TypeChecker -> Frontend -> IR Generator -> LLVM Backend -> Executable
                                                              \-> C Generator -> GCC -> Executable
```

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

### Runtime

Located in `seen_runtime/`:
- C runtime library providing low-level primitives
- Memory allocation, string operations, I/O
- Linked with all compiled Seen programs

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

### Bootstrap Artifacts
- `bootstrap/stage1_frozen` - Frozen bootstrap compiler (verified, committed to repo)
- `bootstrap/stage1_frozen.sha256` - Hash for integrity verification
- `compiler_seen/target/seen` - Production compiler
- `stage2_head`, `stage3_head` - Latest verified bootstrap stages (may be outdated)

### Documentation
- `README.md` - Comprehensive project documentation
- `RUST_MIGRATION_COMPLETE.md` - Documents the successful self-hosting
- `docs/` - Design documents, specifications, research papers

## Development Workflow

### Bootstrap-Safe Development

**CRITICAL:** Before committing compiler changes, verify bootstrap:

```bash
# Run safe rebuild to verify changes don't break bootstrap
./scripts/safe_rebuild.sh

# Or enable pre-commit hook for automatic verification
git config core.hooksPath .githooks
```

The pre-commit hook will:
1. Detect changes to `compiler_seen/src/`
2. Run bootstrap verification
3. Block commits that break self-hosting

### Adding Language Features

1. Update AST definitions in `compiler_seen/src/parser/ast.seen`
2. **Update struct layouts** in `compiler_seen/src/types/struct_layouts.seen` (if adding fields)
3. Extend parser in `compiler_seen/src/parser/real_parser.seen`
4. Add type checking in `compiler_seen/src/typechecker/`
5. Implement IR generation in `compiler_seen/src/ir/ir_generator.seen`
6. Add tests in `compiler_seen/run_tests.seen`
7. **Run `./scripts/safe_rebuild.sh` before committing**

### Fixing Compiler Issues

When debugging compiler issues:
1. Use extensive tracing (look for `[trace]` and `[DEBUG]` output)
2. Test with minimal reproducible examples (create `repro_*.seen` files)
3. Compare output at each compilation stage
4. Use environment variables to enable detailed logging

### Adding Standard Library Functions

1. Add implementation to `seen_std/src/` in appropriate module
2. Update `seen_std/src/prelude.seen` if it should be auto-imported
3. Add test cases
4. Rebuild compiler to include new functions

### Testing Changes

```bash
# Test self-hosted compiler
cd compiler_seen
../stage1_verified run run_tests.seen

# Test specific feature with repro file
echo 'fun main() { println("test") }' > repro_test.seen
../stage1_verified build repro_test.seen
./repro_test
```

## Common Tasks

### Building a New Stage Compiler

```bash
./stage1_verified build compiler_seen/src/main_compiler.seen -o stage2_new
./stage2_new build compiler_seen/src/main_compiler.seen -o stage3_new
# stage2_new and stage3_new should be identical (bootstrap fixed-point)
```

### Comparing LLVM IR Output

```bash
# Generate LLVM IR for debugging
./stage1_verified build test.seen --emit-llvm
cat test.ll

# Or examine the LLVM backend source
cat compiler_seen/src/codegen/llvm_backend.seen
```

### Running Benchmarks

```bash
# Build benchmark
cd benchmarks
../stage1_verified build matrix_multiply.seen
time ./matrix_multiply
```

## Debugging Tips

1. **Compiler Crashes:** Create minimal repro in `repro_*.seen` files at root
2. **Type Errors:** Check `compiler_seen/src/typechecker/` for type inference logic
3. **Codegen Issues:** Look at generated LLVM IR with `--emit-llvm`
4. **Runtime Errors:** Build with debug symbols and use `gdb` or `lldb`

## Project Philosophy

- **No Stubs:** All features should be fully implemented, not stubbed
- **Self-Hosting:** The compiler compiles itself - this is achieved
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

### CLI Debug Flags

```bash
# Enable all debugging
./stage1_verified build source.seen output --debug

# Specific debug modes
./stage1_verified build source.seen output --trace-llvm
./stage1_verified build source.seen output --dump-struct-layouts
./stage1_verified build source.seen output --runtime-debug
```

### Common Debugging Commands

```bash
# Debug type checking issues
SEEN_DEBUG_TYPES=1 ./stage1_verified build program.seen

# Debug LLVM IR generation (all traces)
SEEN_TRACE_LLVM=all ./stage1_verified build program.seen

# Debug struct field access (GEP operations)
SEEN_TRACE_LLVM=gep ./stage1_verified build program.seen

# Dump LLVM IR to debug_ir.ll
SEEN_TRACE_LLVM=ir ./stage1_verified build program.seen

# Debug boxing/unboxing for generics
SEEN_TRACE_LLVM=boxing ./stage1_verified build program.seen
```

### Bootstrap Verifier

For multi-stage bootstrap verification, use `BootstrapVerifier` class in `compiler_seen/src/bootstrap/verifier.seen`:
- Logs to stdout and `bootstrap_verification.log`
- Tracks compilation stages, binary comparisons, hashes, and file sizes

### Key Files for Debugging

- `compiler_seen/src/codegen/llvm_backend.seen` - LLVM trace options
- `compiler_seen/src/typechecker/type_checker.seen` - Type debug output
- `compiler_seen/src/main_compiler.seen` - CLI flag parsing
- `compiler_seen/src/bootstrap/verifier.seen` - Bootstrap verification logging

## Bootstrap Resilience System

The project uses a bootstrap resilience system to ensure permanent Rust independence.

### Components

| File | Purpose |
|------|---------|
| `bootstrap/stage1_frozen` | Frozen, verified bootstrap compiler |
| `bootstrap/stage1_frozen.sha256` | Hash for integrity verification |
| `bootstrap/README.md` | Usage documentation |
| `scripts/safe_rebuild.sh` | Safe rebuild with bootstrap verification |
| `.githooks/pre-commit` | Automatic bootstrap check on commit |
| `compiler_seen/src/types/struct_layouts.seen` | Canonical struct field layouts |

### Safe Development Workflow

1. **Make changes** to compiler code
2. **Run `./scripts/safe_rebuild.sh`** to verify bootstrap
3. **If verification passes**, commit your changes
4. **If verification fails**, fix the bootstrap-breaking issue

### Updating the Frozen Compiler

Only update when you have a verified working compiler:

```bash
# 1. Verify current compiler passes bootstrap
./scripts/safe_rebuild.sh

# 2. If successful, update frozen compiler
cp stage2_head bootstrap/stage1_frozen
sha256sum bootstrap/stage1_frozen > bootstrap/stage1_frozen.sha256

# 3. Commit
git add bootstrap/
git commit -m "Update frozen bootstrap compiler"
```

### Common Bootstrap Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "undefined value" error | New method not in old compiler | Remove new feature temporarily, rebuild, then add |
| GEP index out of bounds | Struct field count mismatch | Update struct_layouts.seen, verify field order |
| stage2 != stage3 != stage4 | Non-deterministic codegen | Check for HashMap/time usage in compiler |

### Emergency Recovery

If bootstrap is broken and the production compiler doesn't work:

```bash
# Use the frozen bootstrap compiler
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen recovery_compiler

# If that fails, check out a known-good commit
git checkout ead1940 -- compiler_seen/src/
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen recovery_compiler
```
