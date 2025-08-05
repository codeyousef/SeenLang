# [[Seen]] Language MVP Phase Development Plan

## Overview: Foundation & Core Functionality

**Duration**: Months 0-3 **Goal**: Self-hosting compiler with basic language features and cargo-like toolchain

**Core MVP Requirements:**

- Complete lexer, parser, and type system
- Basic memory model implementation
- LLVM code generation
- Cargo-like command interface (`seen build`, `seen run`, `seen test`)
- C interoperability foundation
- Self-hosting capability

## Phase Structure

### Milestone 1: Foundation (Months 0-1)

#### Step 1: Repository Structure & Build System

**Tests Written First:**

- [ ] Test: `seen build` compiles simple programs successfully
- [ ] Test: `seen clean` removes all build artifacts
- [ ] Test: `seen check` validates syntax without building
- [ ] Test: Workspace structure supports multiple crates
- [ ] Test: Language files load from TOML configuration
- [ ] Test: Hot reload completes in <50ms

**Implementation:**

- [ ] **Core Build Commands:**
    - [ ] `seen build [target]` - Compile to native/WASM/JS
    - [ ] `seen build --release` - Optimized builds
    - [ ] `seen build --debug` - Debug builds with symbols
    - [ ] `seen clean` - Remove target/ directory
    - [ ] `seen check` - Fast syntax/type checking only
- [ ] Modular crate structure with clear boundaries
- [ ] Dynamic language loading system (English/Arabic via TOML)
- [ ] TOML-based project configuration (Seen.toml)
- [ ] Target specification system (native, WASM, JS)
- [ ] Dependency resolution framework
- [ ] Incremental compilation infrastructure

**Performance Benchmarks:**

```rust
#[bench]
fn bench_build_command_startup(b: &mut Bencher) {
    let project = create_simple_project();
    b.iter(|| {
        let build_time = measure_time(|| {
            seen_build(&project, BuildMode::Debug)
        });
        assert!(build_time < Duration::from_millis(100)); // <100ms startup
    });
}

#[bench]
fn bench_incremental_build(b: &mut Bencher) {
    let project = setup_large_project(1000_files);
    seen_build(&project, BuildMode::Debug); // Initial build
    
    modify_single_file(&project);
    b.iter(|| {
        let incremental_time = measure_time(|| {
            seen_build(&project, BuildMode::Debug)
        });
        assert!(incremental_time < Duration::from_secs(1)); // <1s incremental
    });
}
```

#### Step 2: Lexical Analysis

**Tests Written First:**

- [ ] Test: Lexer processes >10M tokens/second
- [ ] Test: All operators tokenized correctly
- [ ] Test: String literals handle escapes properly
- [ ] Test: Comments preserved for documentation
- [ ] Test: Unicode identifiers work (Arabic/English)
- [ ] Test: Error recovery produces helpful messages

**Implementation:**

- [ ] High-performance lexer with SIMD optimizations
- [ ] Complete token set (keywords, operators, literals)
- [ ] Multilingual keyword support via language registry
- [ ] Error recovery and reporting system
- [ ] Source location tracking with span information
- [ ] Memory-efficient token stream representation

#### Step 3: Parsing & AST Construction

**Tests Written First:**

- [ ] Test: Parser handles >1M lines/second
- [ ] Test: AST nodes properly typed and structured
- [ ] Test: Error recovery maintains parse state
- [ ] Test: Precedence rules match Kotlin exactly
- [ ] Test: Memory usage scales linearly with input

**Implementation:**

- [ ] Recursive descent parser with operator precedence
- [ ] Complete AST node hierarchy with proper typing
- [ ] Error recovery using panic mode + synchronization
- [ ] Memory-efficient AST representation
- [ ] Source-to-AST mapping for IDE support
- [ ] Parse tree validation and consistency checks

### Milestone 2: Core Language (Months 1-2)

#### Step 4: Type System Foundation

**Tests Written First:**

- [ ] Test: Type inference completes in <100μs per function
- [ ] Test: Generic type resolution works correctly
- [ ] Test: C type mapping is bidirectional and lossless
- [ ] Test: Error messages exceed Rust quality

**Implementation:**

- [ ] Hindley-Milner type inference engine
- [ ] Generic type system with constraints
- [ ] C interop type mapping (automatic header parsing)
- [ ] Type error reporting with suggestions
- [ ] Incremental type checking for IDE responsiveness

#### Step 5: Memory Model (Vale-style)

**Tests Written First:**

- [ ] Test: Region inference prevents all memory errors
- [ ] Test: Performance overhead <5% vs unsafe code
- [ ] Test: No false positive safety errors
- [ ] Test: Automatic lifetime management works

**Implementation:**

- [ ] Region-based memory management
- [ ] Generational references with zero runtime cost
- [ ] Automatic memory safety verification
- [ ] Linear capability tracking
- [ ] Compile-time memory leak detection

#### Step 6: Basic Code Generation

**Tests Written First:**

- [ ] Test: Generated code matches C performance
- [ ] Test: Debug info complete and accurate
- [ ] Test: C calling conventions respected
- [ ] Test: LLVM IR is well-formed and optimal

**Implementation:**

- [ ] LLVM backend with efficient IR generation
- [ ] Debug information generation (DWARF)
- [ ] C ABI compatibility layer
- [ ] Basic optimization pipeline
- [ ] Cross-compilation support

### Milestone 3: Self-Hosting Preparation (Months 2-3)

#### Step 7: Standard Library Core

**Tests Written First:**

- [ ] Test: Core types match Rust performance
- [ ] Test: Collections beat C implementations
- [ ] Test: I/O system achieves full bandwidth
- [ ] Test: C library interop seamless

**Implementation:**

- [ ] Primitive types with optimal memory layout
- [ ] High-performance collections (Vec, HashMap, etc.)
- [ ] String handling (UTF-8 native)
- [ ] File and network I/O
- [ ] C library binding utilities
- [ ] Error handling types (Result, Option)

#### Step 8: Testing Framework & Document Formatting

**Tests Written First:**

- [ ] Test: `seen test` discovers and runs all tests
- [ ] Test: Test runner reports timing and memory usage
- [ ] Test: Benchmark framework integrates with CI
- [ ] Test: Code coverage tracking works
- [ ] Test: `seen format` handles all document types (.md, .toml, .seen)
- [ ] Test: Document formatting preserves semantic meaning
- [ ] Test: Format command integrates with IDE workflows

**Implementation:**

- [ ] **Testing & Documentation Commands:**
    - [ ] `seen test` - Run all unit tests
    - [ ] `seen test --bench` - Run benchmarks
    - [ ] `seen test --coverage` - Generate coverage reports
    - [ ] `seen test [filter]` - Run specific tests
    - [ ] `seen format` - Format all project documents
    - [ ] `seen format --check` - Check formatting without changes
    - [ ] `seen format [path]` - Format specific files/directories
- [ ] Built-in test framework with assertions
- [ ] Benchmark infrastructure with statistical analysis
- [ ] Code coverage tracking and reporting
- [ ] Property-based testing support
- [ ] Test discovery and parallel execution
- [ ] Document formatter for Markdown, TOML, and Seen code
- [ ] Configurable formatting rules via Seen.toml
- [ ] Integration with version control pre-commit hooks

#### Step 9: Self-Hosting Compiler & Development Transition

**Tests Written First:**

- [ ] Test: Seen compiler can compile itself
- [ ] Test: Self-compiled version is byte-identical
- [ ] Test: Bootstrap cycle completes successfully
- [ ] Test: Self-hosted compiler has same performance
- [ ] Test: Development workflow fully operational in Seen
- [ ] Test: All future development can proceed in Seen

**Implementation:**

- [ ] Port lexer from Rust to Seen
- [ ] Port parser from Rust to Seen
- [ ] Port type system from Rust to Seen
- [ ] Port code generation from Rust to Seen
- [ ] Bootstrap process automation
- [ ] Verification of compiler correctness
- [ ] **CRITICAL: Development Language Transition**
    - [ ] After self-hosting success, ALL future development in Seen
    - [ ] Convert existing Rust codebase to Seen systematically
    - [ ] Update build scripts to use self-hosted compiler
    - [ ] Migrate all development tools to Seen implementation
    - [ ] Establish Seen-only development workflow
    - [ ] Archive Rust implementation as bootstrap-only

## MVP Command Interface

### Primary Commands

```bash
# Build and run commands
seen build                    # Build current project (debug mode)
seen build --release         # Build optimized version
seen build --target wasm     # Build for WebAssembly
seen run                     # JIT compile and run (script mode)
seen run main.seen           # Run specific file

# Development commands  
seen check                   # Fast syntax and type checking
seen clean                   # Remove build artifacts
seen test                    # Run all tests
seen test --bench           # Run benchmarks

# Project management
seen init <name>            # Create new project
seen add <dependency>       # Add dependency
seen update                 # Update dependencies
```

### Configuration Files

**Seen.toml** (Project Configuration):

```toml
[project]
name = "my-app"
version = "0.1.0"
author = "Developer Name"
language = "en"  # or "ar" or custom

[dependencies]
http = "0.1"

[build]
targets = ["native", "wasm"]
optimize = "speed"  # or "size"

[format]
line-width = 100
indent = 4
trailing-comma = true
document-types = [".seen", ".md", ".toml"]

[dev-dependencies]
test-framework = "0.1"
```

**languages/en.toml** (Language Definition):

```toml
[keywords]
func = "TokenFunc"
if = "TokenIf" 
else = "TokenElse"
return = "TokenReturn"
# ... complete keyword mapping

[operators]
"+" = "TokenPlus"
"-" = "TokenMinus"
# ... complete operator set
```

## Success Criteria

### Performance Targets (All Must Pass)

- [ ] Lexer: >10M tokens/second
- [ ] Parser: >1M lines/second
- [ ] Type checking: <100μs per function
- [ ] JIT startup: <50ms cold start
- [ ] Memory usage: ≤ equivalent Rust programs
- [ ] Build time: <10s for 100K line projects

### Functional Requirements

- [ ] Self-hosting compiler compiles itself
- [ ] All cargo-equivalent commands implemented
- [ ] C interop works without manual bindings
- [ ] Multilingual syntax support functional
- [ ] Test framework catches all error types
- [ ] Debug information complete and accurate

### Code Quality Standards

- [ ] 100% test coverage for all core components
- [ ] All benchmarks passing with required performance
- [ ] Zero memory leaks in compiler and generated code
- [ ] Complete error handling with helpful messages
- [ ] Documentation coverage >90%
- [ ] CI/CD pipeline with regression testing

## Risk Mitigation

### Technical Risks

- **LLVM Integration Complexity**: Start with simple IR generation, expand gradually
- **C Interop Edge Cases**: Focus on common use cases first, expand coverage
- **Memory Model Complexity**: Implement subset first, validate with extensive testing
- **Performance Regression**: Continuous benchmarking with automatic alerts

### Schedule Risks

- **Self-hosting Timeline**: Parallel development of Rust and Seen versions
- **Feature Creep**: Strict MVP scope, defer advanced features to Alpha
- **Integration Issues**: Early integration testing, weekly builds

## Next Phase Preview

**Alpha Phase** will add:

- Advanced optimization pipeline
- Complete standard library
- LSP server for IDE support
- Package manager and registry
- Advanced C++ interoperability
- WebAssembly optimization
- Production debugging tools