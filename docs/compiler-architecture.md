# Compiler Architecture

The Seen compiler is a 100% self-hosted compiler written in Seen. It follows a 5-stage pipeline and compiles through LLVM for native code generation.

## Pipeline Overview

```
Source (.seen)
  → Lexer (tokenize with language-specific keywords)
  → Parser (recursive descent → AST)
  → Type Checker (inference, validation, smart casts)
  → IR Generator (AST → LLVM IR, three-pass)
  → LLVM Backend (opt → ThinLTO → lld link)
  → Native Binary
```

## Stage 1: Lexer

**Location:** `compiler_seen/src/lexer/`

The `SeenLexer` class tokenizes source code into a stream of tokens.

- `KeywordManager` loads TOML files from `languages/` for multi-language support
- Tracks position (line, column) for error reporting
- Handles string interpolation (`{expr}` inside strings)
- Byte-level access via `byteAt()` for performance

**Entry point:** `tokenize(source: String, language: String) -> Array<Token>`

## Stage 2: Parser

**Location:** `compiler_seen/src/parser/`

Converts the token stream into an AST (Abstract Syntax Tree).

- Recursive descent parser
- Two-pass parsing for stability (declarations first, then bodies)
- Key AST node types: `ProgramNode`, `FunctionNode`, `ClassNode`, `StatementNode`, `ParserExpressionNode`

**Entry point:** `parse(tokens: Array<Token>) -> ProgramNode`

### AST Node Types

| Node | Description |
|------|-------------|
| `ProgramNode` | Root of the AST, contains all top-level declarations |
| `FunctionNode` | Function declaration with parameters, body, return type |
| `ClassNode` | Class with fields, methods, inheritance |
| `StatementNode` | Variable declarations, assignments, control flow |
| `ParserExpressionNode` | Literals, binary ops, method calls, etc. |
| `ItemNode` | Function parameters and items |
| `ParamNode` | Parameter declarations |
| `ImportSymbolNode` | Import declarations |

## Stage 3: Type Checker

**Location:** `compiler_seen/src/typechecker/`

Performs type inference and validation on the AST.

- `TypeChecker` class manages type inference
- `Environment` manages scoped symbol tables (push/pop scope)
- Smart casting after null checks
- Generic type resolution

**Entry point:** `check(program: ProgramNode) -> TypeInferenceResult`

## Stage 4: Frontend Integration

**Location:** `compiler_seen/src/bootstrap/`

Orchestrates the first three stages.

- `run_frontend()` chains lexer → parser → type checker
- Produces `FrontendResult` with diagnostics
- Converts errors to `FrontendDiagnostic` with source locations

## Stage 5: Code Generation

### IR Generator

**Location:** `compiler_seen/src/ir/` and `compiler_seen/src/codegen/`

Converts AST to LLVM IR using a three-pass approach:

1. **Pass 1: Signatures** -- collect all function signatures and type declarations
2. **Pass 2: Types** -- resolve struct layouts, method tables, generics
3. **Pass 3: Bodies** -- generate LLVM IR for function implementations

The IR generator is split across 13 modules:

| Module | Purpose |
|--------|---------|
| `llvm_ir_gen.seen` | Main IR generation (14,300 lines) |
| `ir_declarations.seen` | Runtime function declarations |
| `ir_decl_features.seen` | Feature-specific declarations |
| `ir_decl_parser.seen` | Parser-related declarations |
| `ir_decl_runtime.seen` | Core runtime declarations |
| `ir_method_gen.seen` | Method generation |
| `ir_optimization.seen` | CSE cache, call count, utilities |
| `ir_string_collect.seen` | String constant collection |
| `ir_type_dispatch.seen` | Type dispatch tables |
| `ir_type_info.seen` | Type information |
| `ir_type_mapping.seen` | Seen types → LLVM types |
| `ir_type_ops.seen` | Type operations |
| `ir_type_tables.seen` | Type lookup tables |

### LLVM Backend

**Location:** `compiler_seen/src/codegen/llvm_backend.seen`

Full LLVM compilation pipeline:

```
.ll (LLVM IR)
  → opt -O3 (optimization with vectorization, loop unrolling, inlining)
  → opt --thinlto-bc (ThinLTO bitcode)
  → clang -O3 -flto=thin -fuse-ld=lld (link)
  → Native Binary
```

### C Backend (Fallback)

**Location:** `compiler_seen/src/codegen/c_gen.seen`

Emits C99 code as a portable fallback. Compiled with GCC.

## Fork-Parallel IR Generation

Each module's IR is generated in a forked child process:

1. Parent collects all module source code
2. For each uncached module, fork a child process
3. Child inherits copy-on-write registry snapshot
4. Child generates LLVM IR and writes `.ll` file
5. Parent waits for all children, then links

This provides ~1.74x speedup on cold builds.

## Incremental Compilation

Content-addressed IR cache with per-module granularity:

**Cache key format (v3):**
```
hash("v3:" + declarationsHash + ":" + modulePath + ":" + moduleSourceHash)
```

- `declarationsHash` -- hash of cross-module interfaces (struct layouts, function signatures)
- `moduleSourceHash` -- hash of the module's source code

Editing a function body in one module only recompiles that module. Cross-module interface changes (new functions, changed signatures) invalidate all caches.

**Cache locations:**
- `.seen_cache/` -- source-level cache
- `/tmp/seen_ir_cache` -- IR content-addressed cache
- `/tmp/seen_thinlto_cache` -- ThinLTO linker cache

## Key Source Files

| File | Size | Purpose |
|------|------|---------|
| `compiler_seen/src/main.seen` | -- | Production compiler entry point |
| `compiler_seen/src/main_compiler.seen` | -- | Stage1 compiler driver |
| `compiler_seen/src/bootstrap/frontend.seen` | -- | Frontend orchestration |
| `compiler_seen/src/codegen/llvm_backend.seen` | 16KB | LLVM code generation |
| `compiler_seen/src/codegen/generator.seen` | 41KB | Backend framework |
| `compiler_seen/src/ir/ir_generator.seen` | -- | Main LLVM IR generation |
| `compiler_seen/src/codegen/llvm_ir_gen.seen` | 14K+ lines | Core IR generator |
| `compiler_seen/src/types/struct_layouts.seen` | -- | Canonical struct field layouts |

## Type Representation in LLVM IR

| Seen Type | LLVM IR Type |
|-----------|-------------|
| `Int` | `i64` |
| `Float` | `double` |
| `Bool` | `i1` |
| `String` | `%SeenString` (struct: `{i64, ptr}` = len + data) |
| `Char` | `i64` (Unicode code point) |
| `Array<T>` | `%SeenArray*` (struct: `{i64, i64, i64, ptr}`) |
| `Class` | `ptr` (heap-allocated, handle-based) |
| Simple enum | `i64` |
| Data enum | `ptr` (heap-allocated) |

## Contributing to the Compiler

1. Make changes to `compiler_seen/src/`
2. Run `./scripts/safe_rebuild.sh` to verify bootstrap
3. If verification passes, commit
4. If it fails, fix the bootstrap-breaking issue

See [Bootstrap System](bootstrap.md) for details.
