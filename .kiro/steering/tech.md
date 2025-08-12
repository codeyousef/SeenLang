# Seen Language Technical Stack

## Build System & Toolchain

### Primary Build System
- **Rust Cargo Workspace**: Multi-crate workspace with 13+ crates
- **Self-hosted Compiler**: Bootstrap compiler in `target-wsl/debug/seen`
- **Cross-platform**: Windows (PowerShell), Linux/macOS (Bash)

### Core Commands
```bash
# Build the compiler
./target-wsl/debug/seen build --release

# Run tests
./target-wsl/debug/seen test
cargo test                    # Rust tests
./scripts/run_tests.ps1      # Comprehensive test runner

# Format code
./target-wsl/debug/seen format --all
cargo fmt

# Check code quality
./target-wsl/debug/seen check --all
cargo clippy

# Run benchmarks
./target-wsl/debug/seen benchmark
cargo bench
```

## Technology Stack

### Core Implementation (Rust - Bootstrap Phase)
- **Language**: Rust 2021 edition
- **Parser**: Chumsky parser combinator library
- **String Interning**: Lasso for efficient string handling
- **Collections**: IndexMap, HashBrown for performance
- **Error Handling**: Anyhow, ThisError
- **CLI**: Clap v4 with derive features
- **Serialization**: Serde with TOML support

### Self-hosted Implementation (Seen Language)
- **Location**: `compiler_seen/` directory
- **Language**: Pure Seen language (6,200+ lines)
- **Architecture**: Multi-phase compiler pipeline
- **Performance**: Targets 25M tokens/sec lexing, 80μs/function typechecking

### Testing Framework
- **Unit Tests**: Built-in Rust `#[test]` with enhanced libraries
- **Property Testing**: Proptest for randomized testing
- **Snapshot Testing**: Insta for regression testing
- **Benchmarking**: Criterion with statistical analysis
- **Coverage**: Tarpaulin with HTML reports
- **Integration**: Nextest for fast parallel execution

### Performance Tools
- **Profiling**: Built-in benchmarking framework
- **Memory**: Vale-style region-based memory management
- **Optimization**: E-graph + ML + Superoptimization pipeline
- **SIMD**: Architecture-specific vectorization (AVX-512, RVV, SVE2)

## Language Configuration System

### Multi-language Support
- **Configuration**: TOML files in `languages/` directory
- **Languages**: English (`en.toml`), Arabic (`ar.toml`)
- **Loading**: Perfect hash-based keyword mapping
- **Performance**: Zero runtime overhead (compile-time embedding)

### Project Configuration
- **File**: `Seen.toml` in project root
- **Sections**: `[project]`, `[build]`, `[dependencies]`, `[targets]`
- **Language**: Single language per project (no mixing)

## Architecture Support

### Target Platforms
- **x86_64**: Linux, Windows, macOS
- **ARM64**: Apple Silicon, Linux ARM64
- **RISC-V**: RV32I/RV64I with vector extensions (RVV 1.0)
- **WebAssembly**: WASM32 with SIMD support

### Code Generation
- **Backend**: LLVM IR generation
- **Optimization**: Custom optimization pipeline
- **Debug Info**: Full debugging support
- **Cross-compilation**: Single binary, multiple targets

## Development Workflow

### Quality Gates
1. **Test Suite**: All tests must pass (100+ tests)
2. **Code Formatting**: `cargo fmt` compliance
3. **Linting**: `cargo clippy` with zero warnings
4. **Security**: `cargo audit` checks
5. **Coverage**: Minimum coverage thresholds
6. **Performance**: Benchmark regression detection

### CI/CD Pipeline
- **Platform**: GitHub Actions
- **Matrix**: Multiple OS (Ubuntu, Windows, macOS)
- **Steps**: Build, test, format, lint, security audit
- **Artifacts**: Coverage reports, benchmark results

## IDE Integration

### Language Server Protocol (LSP)
- **Command**: `seen lsp`
- **Features**: Diagnostics, completion, hover, go-to-definition
- **Integration**: VSCode extension in `vscode-seen/`
- **Protocol**: Full LSP 3.17 compliance

### VSCode Extension
- **Location**: `vscode-seen/` directory
- **Features**: Syntax highlighting, IntelliSense, debugging
- **Languages**: Multi-language keyword support
- **Build**: TypeScript with webpack bundling

## Performance Targets

### Compiler Performance
- **Lexer**: 25M tokens/sec (self-hosted target)
- **Parser**: 800K lines/sec
- **Type Checker**: 80μs/function
- **Code Generation**: 300μs/function
- **Memory Overhead**: <10% for self-hosting

### Runtime Performance
- **Compilation**: 10x faster than LLVM
- **Execution**: 20%+ faster than GCC -O3
- **Memory**: Vale-style safety with <1% overhead
- **Startup**: JIT execution under 50ms