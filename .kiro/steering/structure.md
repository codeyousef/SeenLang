# Seen Language Project Structure

## Repository Organization

### Root Level Structure
```
seen/
├── Cargo.toml                  # Rust workspace configuration
├── Seen.toml                   # Seen project configuration
├── README.md                   # Main project documentation
├── LICENSE                     # MIT license
├── .gitignore                  # Git ignore patterns
└── target-wsl/debug/seen       # Bootstrap compiler executable
```

## Core Implementation Crates

### Rust Bootstrap Implementation
Located in individual crate directories, each with `Cargo.toml`:

```
seen_cli/                       # Command-line interface
seen_lexer/                     # Lexical analysis
seen_parser/                    # Syntax analysis & AST
seen_typechecker/               # Type system & inference
seen_interpreter/               # JIT execution engine
seen_ir/                        # LLVM IR code generation
seen_memory_manager/            # Vale-style memory management
seen_oop/                       # Object-oriented features
seen_concurrency/               # Async/await, channels, actors
seen_reactive/                  # Reactive programming
seen_advanced/                  # Effects, contracts, metaprogramming
seen_tooling/                   # LSP, formatting, benchmarking
seen_self_hosting/              # Self-hosting capability
```

### Self-hosted Implementation
```
compiler_seen/                  # Self-hosted compiler (6,200+ lines of Seen)
├── Seen.toml                  # Compiler project config
├── src/
│   ├── main.seen              # Compiler entry point
│   ├── lexer/                 # Lexical analysis
│   ├── parser/                # Syntax analysis & AST
│   ├── typechecker/           # Type system
│   ├── codegen/               # Code generation
│   ├── optimization/          # Revolutionary optimization pipeline
│   │   ├── egraph/            # E-graph optimization
│   │   ├── ml/                # Machine learning
│   │   ├── superopt/          # Superoptimization
│   │   ├── pgo/               # Profile-guided optimization
│   │   ├── memory/            # Memory optimization
│   │   └── arch/              # Architecture optimization
│   ├── lsp/                   # Language server
│   └── errors/                # Error handling
└── target/                    # Build artifacts
```

## Language Configuration

### Multi-language Support
```
languages/                      # Language configuration files
├── en.toml                    # English keywords and operators
├── ar.toml                    # Arabic keywords and operators
└── [future languages]         # Additional language support
```

**Key Principles:**
- Each project uses ONE language consistently (no mixing)
- TOML-based configuration for zero runtime overhead
- Perfect hash-based keyword mapping for performance
- Auto-translation system for migrating between languages

## Testing Infrastructure

### Test Organization
```
test/                          # Centralized test suite
├── unit/                      # Unit tests
│   ├── compiler/              # Compiler component tests
│   ├── optimization/          # Optimization pipeline tests
│   └── language/              # Language feature tests
├── integration/               # Integration tests
│   ├── syntax/                # Syntax feature testing
│   ├── memory/                # Memory model tests
│   └── compilation/           # End-to-end compilation tests
├── examples/                  # Test example programs
│   ├── basic/                 # Basic language constructs
│   └── advanced/              # Advanced features
├── fixtures/                  # Test data and fixtures
└── performance/               # Performance benchmarks

tests/                         # Rust integration tests
├── integration_tests.rs       # Cross-component integration

benches/                       # Performance benchmarks
├── compiler_benchmarks.rs     # Compiler performance tests
```

### Test Scripts
```
scripts/                       # Development and testing scripts
├── run_tests.ps1             # Comprehensive test runner
├── quality_gates.ps1         # Quality assurance checks
└── setup_tdd_infrastructure.ps1 # TDD setup
```

## Documentation

### Project Documentation
```
docs/                          # Project documentation
├── 0 - MVP Development Plan.md    # Development roadmap
├── 1 - Alpha Development Plan.md  # Alpha phase details
├── 2 - Beta Development Plan.md   # Beta phase planning
├── 3 - Release Development Plan.md # Release planning
├── Syntax Design.md               # Language syntax specification
├── VSCode Extension Plan.md       # IDE integration plan
└── Installer Plan.md              # Installation strategy
```

### Development Documentation
```
TDD_INFRASTRUCTURE.md          # Test-driven development guide
CLAUDE.md                      # AI assistant integration notes
```

## IDE and Tooling

### VSCode Extension
```
vscode-seen/                   # Visual Studio Code extension
├── package.json               # Extension manifest
├── src/                       # Extension TypeScript source
├── syntaxes/                  # Syntax highlighting definitions
├── snippets/                  # Code snippets
├── test-fixtures/             # Extension test files
└── webpack.config.js          # Build configuration
```

### Installation and Distribution
```
installer/                     # Installation packages
├── windows/                   # Windows MSI installer
├── macos/                     # macOS package
├── linux/                     # Linux packages
├── homebrew/                  # Homebrew formula
├── scoop/                     # Scoop manifest
├── docker/                    # Docker containers
└── scripts/                   # Installation scripts
```

## Example Projects

### Sample Applications
```
examples/                      # Example Seen projects
└── hello_world/               # Basic hello world example

hello_test/                    # Test project structure
├── README.md
├── seen.toml                  # Project configuration
└── src/                       # Source files

bootstrap_test/                # Bootstrap testing project
├── Seen.toml
├── src/
├── benches/                   # Benchmarks
└── target/                    # Build output
```

## Performance Validation

### Benchmarking Infrastructure
```
benchmarks/                    # Comprehensive benchmarking suite
├── competitors/               # Competitor language benchmarks
├── data/                      # Benchmark data sets
├── harness/                   # Benchmarking framework
├── microbenchmarks/           # Micro-performance tests
├── real_world/                # Real-world application benchmarks
├── results/                   # Benchmark results
├── reports/                   # Performance reports
└── *.ps1                      # PowerShell benchmark scripts

performance_validation/        # Performance validation suite
├── benchmarks/                # Validation benchmarks
├── competitors/               # Competitor implementations
├── real_world/                # Real-world test cases
├── results/                   # Validation results
└── scripts/                   # Validation scripts
```

## Build Artifacts

### Build Outputs
```
target/                        # Rust build artifacts (gitignored)
target-wsl/                    # WSL-specific builds
├── debug/
│   └── seen                   # Bootstrap compiler executable
└── release/                   # Release builds
```

## Configuration Files

### Development Configuration
```
.config/
├── nextest.toml              # Test runner configuration

.github/                      # GitHub Actions workflows
├── workflows/
│   ├── ci.yml               # Continuous integration
│   └── release.yml          # Release automation

.vscode/                      # VSCode workspace settings
.idea/                        # IntelliJ IDEA settings
tarpaulin.toml               # Coverage configuration
```

## Naming Conventions

### File Naming
- **Rust files**: `snake_case.rs`
- **Seen files**: `snake_case.seen`
- **Configuration**: `kebab-case.toml`
- **Documentation**: `Title Case.md`
- **Scripts**: `snake_case.ps1` or `snake_case.sh`

### Directory Structure
- **Crates**: `seen_component/` format
- **Source**: `src/` for implementation
- **Tests**: `test/` centralized, `tests/` for Rust integration
- **Documentation**: `docs/` for project docs
- **Examples**: `examples/` for sample projects

### Module Organization
- Each crate has clear single responsibility
- Self-hosted compiler mirrors Rust structure
- Language-specific files in `languages/`
- Cross-platform scripts with appropriate extensions