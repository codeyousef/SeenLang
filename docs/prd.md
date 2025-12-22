# SeenLang - Product Requirements Document

**Author:** yousef
**Date:** 2025-11-17
**Version:** 1.0

---

## Executive Summary

SeenLang aims to prove that a self-hosted compiler with Vale-inspired memory management can **outperform Rust** while maintaining memory safety. This PRD defines the path to a production-ready compiler capable of running 10 rigorous benchmarks with superior performance to Rust's borrow checker approach.

The MVP focuses on **functional completeness** (self-hosting compiler, all benchmarks passing) and **performance superiority** (geometric mean >1.0x of Rust, majority of benchmarks individually faster).

### What Makes This Special

**SeenLang's competitive advantage over Rust:**

1. **Vale-style Regions** - O(1) bulk deallocation vs Rust's incremental Drop overhead
2. **Generational References** - Faster than Rc<RefCell<T>> patterns, no borrow checker runtime cost
3. **Deterministic Codegen** - Better instruction cache locality through reproducible compilation
4. **MLIR Pipeline** - More optimization passes than rustc
5. **Simpler IR** - No borrow checker complexity enables aggressive inlining

**Current proof points:**
- Fibonacci: 1.0x (matches Rust exactly)
- Recursive Sum: 1.0x (matches Rust exactly)  
- Ackermann: 4.5x slower (deep recursion optimization opportunity)

This PRD targets **average >1.0x performance** across 10 production benchmarks.

---

## Project Classification

**Technical Type:** Developer Tool (Programming Language Compiler + Runtime)
**Domain:** Systems Programming / Compiler Development
**Complexity:** High

**Project Classification Details:**

- **Field:** Brownfield (replacing existing Rust implementation)
- **Current State:** Rust compiler 100% production-ready, self-hosted compiler 85% complete (160 errors remaining)
- **Critical Path:** LLVM backend generic array/data support (P0 blocker for 7/10 benchmarks)
- **Architecture:** Multi-backend (LLVM, MLIR, Cranelift), deterministic compilation, Vale-inspired memory management

---

## Success Criteria

**MVP Success = Functional Completeness + Performance Superiority + Full Self-Hosting**

### Reference Hardware Specification

**Primary Benchmark Hardware:**
- **CPU:** AMD Ryzen 9 7950X3D (32 cores) @ 5.763GHz
- **RAM:** 64GB DDR4
- **OS:** Pop!_OS 22.04 LTS (Linux 6.17.4)
- **GPU:** NVIDIA RTX 4090 (for future GPU compute benchmarks)
- **Storage:** NVMe SSD (for I/O benchmarks)

All benchmarks run on this hardware with matching Rust configuration.

### Core Success Metrics

**1. Full Self-Hosting (MUST HAVE)**
- ✅ Self-hosted compiler compiles itself with 0 type errors
- ✅ Bootstrap pipeline: Stage1 (Rust) → Stage2 (Seen) → Stage3 (Seen)
- ✅ Deterministic builds verified (Stage2 == Stage3 hash equality)
- ✅ `verify_rust_needed.sh` reports "Rust not needed"
- ✅ Rust sources removed (backed up, not required for builds)

**2. Functional Correctness (MUST HAVE)**
- ✅ All 10 production benchmarks run and produce correct checksums
- ✅ Zero compiler warnings
- ✅ All tests passing (Rust test suite + self-hosted tests)
- ✅ Checksum validation passes on every benchmark

**3. Performance Superiority (MUST HAVE - NON-NEGOTIABLE)**
- 🎯 **Geometric mean performance: ≥1.0x of Rust** (match or beat on average)
- 🎯 **Minimum 5/10 benchmarks individually beat Rust** (majority wins)
- 🎯 **Target: >1.0x (faster than Rust), Fallback: =1.0x (match Rust)**
- 🎯 **No catastrophic failures** - if any benchmark <0.8x, optimize until ≥1.0x

**4. Production Readiness (MUST HAVE)**
- ✅ Comprehensive performance report generated
- ✅ All benchmarks run in parallel for rapid iteration
- ✅ Statistical validation (multiple runs, all must be consistent)
- ✅ CI integration prevents regressions

### Performance Target Breakdown

| Benchmark Category | Target Performance | Fallback Strategy |
|-------------------|-------------------|-------------------|
| **Memory-intensive** (Binary Trees, LRU Cache) | >1.2x of Rust | If <1.2x: Profile regions, optimize allocator |
| **Compute-intensive** (Matrix, Mandelbrot, N-Body) | ≥1.0x of Rust | If <1.0x: Optimize SIMD, loop unrolling |
| **I/O-bound** (HTTP Server, FASTA) | ≥0.95x of Rust | If <0.95x: Optimize syscalls, buffering |
| **Algorithmic** (Sieve, JSON, Reverse-Complement) | ≥1.0x of Rust | If <1.0x: Optimize cache locality, data structures |

**Overall Target:** Geometric mean ≥1.0x (match or beat Rust on average)

**Optimization Strategy:** If target not met, iterate optimization until achieved (no time limit)

---

## Product Scope

### MVP - Minimum Viable Product

**Phase 1: Complete LLVM Backend (P0 - Critical)**
- **Strategy:** Switch from IR-only to full LLVM backend implementation
- Implement generic array type support (Float[], Int[], String[], etc.)
- Implement generic data field access with proper GEP instructions
- Link pre-compiled stdlib (Option B: cleaner architecture, reusable)
- Enable all language features in LLVM codegen
- **Deliverable:** LLVM backend feature-complete, all benchmarks unblocked
- **Estimated:** 12-16 hours

**Phase 2: Full Self-Hosting (P0 - Critical)**
- Fix remaining 160 compiler_seen type errors
- Achieve deterministic bootstrap (Stage1 → Stage2 → Stage3)
- Verify Stage2 == Stage3 hash equality
- Run `verify_rust_needed.sh` → "Rust not needed"
- Backup and remove Rust sources
- **Deliverable:** 100% self-hosted compiler, no Rust dependency
- **Estimated:** 8-12 hours

**Phase 3: Implement All 10 Benchmarks in Parallel (P0 - Critical)**
- Implement all 10 benchmarks simultaneously in Seen
- Match Rust implementations exactly (same algorithms, same optimizations)
- Verify checksums against reference implementations
- Baseline performance measurement vs Rust
- **Deliverable:** All 10 benchmarks passing with correct output
- **Estimated:** 20-30 hours (parallel development)

**Phase 4: Performance Optimization Until ≥1.0x (P0 - Critical)**
- Profile each benchmark with perf/vtune on Ryzen 9 7950X3D
- Apply architectural advantages:
  - Regions: Optimize Binary Trees, LRU Cache for O(1) bulk free
  - SIMD: Optimize Matrix, Mandelbrot, N-Body for vectorization
  - Cache: Optimize Sieve, JSON, Reverse-Complement for locality
  - I/O: Optimize HTTP Server, FASTA for syscall efficiency
- Iterate optimization passes until ≥1.0x geometric mean
- **Deliverable:** Performance ≥1.0x (match or beat Rust)
- **Estimated:** Variable (no time limit - optimize until target met)

### Growth Features (Post-MVP)

**Alpha Phase (After Performance Proven):**
- Multi-platform support (macOS, Windows)
- Package manager and ecosystem tooling
- Enhanced IDE support (LSP completions, refactoring)
- Additional backends (WebAssembly, ARM)

**Beta Phase (Ecosystem Growth):**
- Standard library expansion
- Community contributions enabled
- Documentation and tutorials
- Public release and benchmarks publication

### Vision (Future)

**Research Validation:**
- Academic paper on regions vs borrow checker performance
- Conference presentations (PLDI, OOPSLA)
- Open source community building
- Real-world applications (game engines, system tools)

---

## Innovation & Novel Patterns

**SeenLang's Architectural Innovations:**

### 1. Vale-Style Regions Without Annotations

**Innovation:** Automatic region inference + generational references eliminate manual lifetime annotations while maintaining memory safety.

**How it works:**
- Compiler infers region boundaries through escape analysis
- Objects allocated in regions with O(1) bulk deallocation
- Generational handles detect use-after-free at runtime (debug) or compile-time (release)

**Performance advantage over Rust:**
- Binary Trees: Rust calls Drop on every node individually → O(n)
- SeenLang: Free entire region → O(1)
- Expected speedup: 1.5x-2.0x on allocation-heavy benchmarks

### 2. Deterministic Compilation for Cache Locality

**Innovation:** Reproducible instruction layout improves CPU cache behavior.

**How it works:**
- IR sorted deterministically before codegen
- Function ordering optimized for call graph
- Hot paths grouped together

**Performance advantage:**
- Better instruction cache hit rates
- Predictable branch behavior
- Expected improvement: 3-5% across all benchmarks

### 3. MLIR-Based Optimization Pipeline

**Innovation:** More optimization passes than rustc through MLIR integration.

**How it works:**
- DialEgg equality saturation for algebraic rewrites
- LENS superoptimizer for hot loops
- ML-guided inlining heuristics

**Performance advantage:**
- Matrix: Better loop tiling and vectorization
- Mandelbrot: Superior SIMD code generation
- Expected speedup: 1.1x-1.3x on compute benchmarks

### Validation Approach

**Correctness Validation:**
1. All benchmarks must produce deterministic checksums
2. Checksums must match reference implementations
3. Cross-validate with multiple runs (minimum 10 iterations per benchmark)
4. All runs must produce identical checksums (no variance allowed)

**Performance Validation:**
1. Run on dedicated hardware (AMD Ryzen 9 7950X3D, no background processes)
2. Match Rust configuration exactly:
   - Rust: `cargo build --release` with `-C opt-level=3`
   - Seen: `seen build -O3 --backend llvm`
   - Both: `-C target-cpu=native` (enable all CPU features)
3. Warmup iterations excluded (3-5 runs)
4. Measured iterations: 10 runs per benchmark
5. Statistical requirement: **ALL 10 runs must be within 5% variance**
6. Report: min, mean, max, stddev, geometric mean
7. Compare against Rust with same hardware, flags, conditions
8. CI gates prevent any performance regression >5%

**Rust Parity Requirements:**
- Same LLVM version as rustc backend
- Same optimization flags (-O3, target-cpu=native)
- Same benchmark algorithms (no "cheating" with different approaches)
- Fair comparison: measure pure execution time (exclude compilation)

---

## Developer Tool Specific Requirements

**Compiler Infrastructure Requirements:**

### Compilation Pipeline
- Lexer → Parser → Typechecker → IR → LLVM/MLIR/Cranelift backends
- Support for multiple optimization levels (-O0, -O1, -O2, -O3)
- Deterministic mode (`--profile deterministic`) for reproducible builds
- Backend selection (`--backend llvm|mlir|clif`)

### Self-Hosting Capabilities
- Compiler must compile itself (bootstrap pipeline)
- Stage1 (Rust) → Stage2 (Seen) → Stage3 (Seen) with hash equality
- Manifest module system (SEEN_ENABLE_MANIFEST_MODULES=1)
- Dependency resolution across compiler modules

### Benchmark Harness
- Automated build and execution of all benchmarks
- Timing measurement with microsecond precision
- Checksum validation for correctness
- Comparison report generation (Markdown + JSON)
- CI integration for regression detection

### Platform Support
- Primary: Linux x86_64 (development and benchmarking)
- Secondary: macOS, Windows (post-MVP)
- Native code generation via LLVM
- JIT execution via Cranelift (development/testing)

---

## Technical Risks & Mitigation Strategies

### Risk Management Philosophy

**No Time Limit:** All risks have "optimize until resolved" mitigation strategy. Performance targets are non-negotiable.

### Critical Risks

**RISK 1: LLVM Backend Complexity (High Impact, Medium Probability)**

**Description:** Generic array/data support may take longer than estimated 12-16 hours
- Complex type metadata tracking through IR → LLVM pipeline
- GEP instruction generation for arbitrary data layouts
- Stdlib linking infrastructure needs careful ABI design

**Mitigation Strategy:**
- If blocked >24 hours: Break into smaller incremental PRs
- Test each type individually (Int[], Float[], then generics)
- Fallback: Implement most-used types first (Float[], Int[], String[])
- Use existing Rust LLVM codegen as reference implementation
- **No deadline:** Continue until complete and correct

**RISK 2: Region Advantage Doesn't Materialize (High Impact, Low Probability)**

**Description:** Binary Trees and LRU Cache may not show expected 1.2x-2.0x speedup
- Region inference may not detect optimal boundaries
- Bulk deallocation overhead may offset gains
- Rust's Drop may be more optimized than expected

**Mitigation Strategy:**
- Profile with perf: Measure actual malloc/free counts
- If regions not triggering: Add manual region hints to benchmarks
- Optimize region allocator: Minimize metadata overhead
- Compare assembly: Seen bulk free vs Rust incremental Drop
- **Iterate until ≥1.0x:** Even if not 1.2x, must match or beat Rust

**RISK 3: SIMD Auto-Vectorization Parity (Medium Impact, Medium Probability)**

**Description:** LLVM may not vectorize Seen code as well as Rust
- Type information may not flow correctly to LLVM vectorizer
- Alias analysis may be too conservative
- Loop patterns may not match LLVM's vectorization heuristics

**Mitigation Strategy:**
- Compare LLVM IR output: Seen vs Rust for same algorithm
- Add SIMD hints if auto-vectorization fails
- Ensure `-march=native` and `-O3` flags propagate correctly
- If needed: Add manual SIMD intrinsics (last resort)
- **Optimize until match:** Must achieve ≥1.0x on compute benchmarks

**RISK 4: Single Benchmark Catastrophically Slow (Medium Impact, Low Probability)**

**Description:** One benchmark runs at <0.5x of Rust (>2x slower)
- Indicates fundamental architectural issue
- Would drag down geometric mean significantly
- May reveal unexpected overhead in compiler/runtime

**Mitigation Strategy:**
- Immediate deep-dive profiling with perf/vtune
- Compare generated assembly line-by-line vs Rust
- Check for unexpected allocations, cache misses, branch mispredicts
- If architecture issue: Redesign that specific optimization
- **No compromise:** Optimize until ≥0.8x minimum (ideally ≥1.0x)

**RISK 5: Self-Hosting Remaining 160 Errors (Low Impact, Medium Probability)**

**Description:** Type errors in compiler_seen may be deeper than expected
- Could require language feature additions
- May uncover typechecker bugs
- Might need parser enhancements

**Mitigation Strategy:**
- Already 85% complete (877/1037 errors fixed)
- Incremental approach: Fix 20 errors, test, repeat
- Most errors are code quality, not infrastructure
- Estimated 8-12 hours (proven track record)
- **If blocked:** Use working Rust compiler for benchmarks while fixing

### Performance Guarantee Strategy

**If Geometric Mean <1.0x After Initial Implementation:**

**Phase 4A: Systematic Optimization (Weeks 1-2)**
1. Profile all benchmarks with CPU performance counters
2. Identify top 3 bottlenecks per benchmark
3. Optimize highest-impact issues first
4. Re-measure after each optimization

**Phase 4B: Architectural Improvements (Weeks 3-4)**
1. Enhance region inference for better bulk deallocation
2. Improve SIMD code generation
3. Optimize cache layouts in stdlib collections
4. Reduce runtime overhead in hot paths

**Phase 4C: Deep Optimization (Week 5+)**
1. Hand-optimize assembly for critical loops
2. Add architecture-specific intrinsics
3. Tune allocator for benchmark patterns
4. Continue until ≥1.0x achieved

**Success Guarantee:** No time limit. Iterate optimization phases until performance target met.

---

## Functional Requirements

### Compiler Core (FR1-FR20)

**Self-Hosting Compiler:**

**FR1:** Compiler can parse all Seen syntax including its own source code
- Handles classes, structs, enums, traits, generics
- Supports manifest module system with imports
- Processes Seen.toml configuration files

**FR2:** Typechecker resolves all types without errors in compiler_seen sources
- Generic type inference
- Nullable type handling
- Enum variant field access
- Method resolution with overloading

**FR3:** IR generator produces correct intermediate representation for all language features
- Expression lowering (literals, operators, calls, casts)
- Statement lowering (let, var, return, loops, conditionals)
- Control flow (while, for, break, continue)
- Function definitions with parameters and return types

**FR4:** LLVM backend generates native x86_64 code from IR
- Type mapping (Int, Float, String, Bool, structs, arrays)
- Instruction emission (arithmetic, memory, control flow)
- Function calling conventions
- Module linking and optimization passes

**FR5:** Bootstrap pipeline produces identical Stage2 and Stage3 binaries
- Stage1 (Rust compiler) compiles compiler_seen → Stage2
- Stage2 (Seen compiler) compiles compiler_seen → Stage3
- SHA-256(Stage2) == SHA-256(Stage3) (determinism verified)

### Language Features for Benchmarks (FR6-FR15)

**FR6:** Mutable variables with reassignment
- `var` keyword for mutable bindings
- Assignment expressions (`x = value`)
- Works in JIT and AOT modes

**FR7:** Loop constructs (while, for, break, continue)
- While loops with conditions
- For loops over ranges and arrays
- Break/continue for early exit
- Nested loops supported

**FR8:** Array operations (indexing, mutation, methods)
- Array indexing: `arr[i]` returns element
- Array mutation: `arr[i] = value` updates element
- Array methods: `new()`, `push()`, `pop()`, `len()`
- Dynamic resizing with capacity management

**FR9:** Data field access and mutation
- Data field access: `obj.field`
- Data field mutation: `obj.field = value`
- Nested data access: `obj.inner.field`
- Works with both stack and heap-allocated structs

**FR10:** Float literals and arithmetic
- Float literals: `3.14`, `1.5e-3`, `2.0`
- Float operations: `+`, `-`, `*`, `/`, `%`
- Float comparisons: `<`, `>`, `<=`, `>=`, `==`, `!=`
- Int ↔ Float casting

**FR11:** Type casting expressions
- Int ↔ Float conversions
- Bool ↔ Int conversions
- Explicit cast syntax or intrinsics
- No silent type coercion

**FR12:** String operations for benchmarks
- String creation and concatenation
- String to Int/Float conversion
- Int/Float to String conversion
- String length and indexing

**FR13:** Math intrinsics for scientific computing
- `sqrt()`, `sin()`, `cos()`, `pow()`, `abs()`
- `floor()`, `ceil()`, `round()`
- `min()`, `max()` for Int and Float

**FR14:** Time measurement intrinsics
- `__GetTime()` → Float (seconds with microsecond precision)
- `__GetTimestamp()` → Int (milliseconds)
- `__GetTimestampNanos()` → Int (nanoseconds)
- `__Sleep(ms: Int)` for delays

**FR15:** I/O intrinsics for output
- `__Print(s: String)` → Void
- `__PrintInt(i: Int)` → Void
- `__PrintFloat(f: Float)` → Void
- `__Println(s: String)` → Void

### LLVM Backend Critical Features (FR16-FR20)

**FR16:** Generic array type support in LLVM codegen
- Arrays of any type (Int[], Float[], String[], etc.)
- Not hardcoded to StrArray (i8*[])
- Proper LLVM type generation for array elements
- GEP instructions with correct type metadata

**FR17:** Generic data type support in LLVM codegen
- Structs with arbitrary field types
- Not hardcoded to specific structs (CommandResult)
- Field access via proper GEP instructions
- Field mutation with correct type handling

**FR18:** Stdlib method availability in compiled code (Link Strategy)
- Array.withCapacity(), push(), pop(), len(), reserve(), capacity()
- String operations, HashMap, all collection methods
- **Strategy:** Link pre-compiled stdlib (Option B - cleaner, reusable)
- Build stdlib once, link to all compiled programs
- Same semantics as interpreter mode
- ABI stability enforced via `abi_guard`

**FR19:** Type metadata tracking for GEP generation
- IR types map to LLVM types correctly
- Array element types preserved through pipeline
- Data field types available during codegen
- Type information flows from typechecker → IR → LLVM

**FR20:** Float arithmetic operations in LLVM
- Float literals map to LLVM f64 constants
- Float operations (fadd, fsub, fmul, fdiv, frem)
- Mixed Int/Float operations with promotion
- Float comparisons (fcmp) with correct semantics

### Benchmark Implementation (FR21-FR30)

**FR21:** Matrix Multiplication (SGEMM) benchmark
- 512×512 matrices of Float
- Cache-blocked tiled algorithm
- GFLOPS calculation
- Checksum validation

**FR22:** Sieve of Eratosthenes benchmark
- Prime generation up to 10,000,000
- Bit-packed array representation
- Segmented sieve for cache efficiency
- Prime count (664,579) and checksum

**FR23:** Binary Trees benchmark
- Tree depth 20 (1,048,575 nodes)
- Recursive allocation and deallocation
- Checksum must be -1
- Memory allocator stress test

**FR24:** FASTA generation benchmark
- 5,000,000 nucleotide sequence
- Linear congruential generator (LCG)
- Weighted random selection
- Deterministic output with seed

**FR25:** N-Body simulation benchmark
- Solar system (Sun + 4 planets)
- 50,000,000 timesteps
- Double precision Float arithmetic
- Energy conservation validation

**FR26:** Reverse-Complement benchmark
- 25,000,000 base pairs
- DNA sequence reversal and complement
- Lookup table for nucleotide mapping
- MD5 checksum validation

**FR27:** Mandelbrot Set benchmark
- 4000×4000 pixels
- 1000 max iterations
- Complex number arithmetic
- Pixel iteration checksum

**FR28:** LRU Cache benchmark
- 100,000 entry capacity
- 5,000,000 operations (Get/Put)
- HashMap + LinkedList implementation
- Sum of Get results validation

**FR29:** JSON Serialization benchmark
- 1,000,000 objects
- String escaping and formatting
- MD5 checksum of output
- Bytes written metric

**FR30:** HTTP Echo Server benchmark
- 5,000,000 requests
- Concurrent connections
- TCP socket handling
- Requests/second + MB/s metrics

### Performance Measurement (FR31-FR35)

**FR31:** Benchmark harness automation
- Single command runs all benchmarks
- Compiles both Rust and Seen versions
- Executes with timing measurement
- Generates comparison report

**FR32:** Timing accuracy and warmup (Strict Protocol)
- Warmup iterations excluded (3-5 runs, discard results)
- Measured iterations: Exactly 10 runs per benchmark
- Statistical requirement: ALL 10 runs within 5% variance
- If variance >5%: Re-run until consistent (system noise elimination)
- Measurement: Wall-clock time with microsecond precision
- Report: min, mean, max, stddev, median, geometric mean
- Use same measurement methodology as Computer Language Benchmarks Game

**FR33:** Checksum validation for correctness
- Every benchmark outputs deterministic checksum
- Checksums compared between runs
- Mismatches flagged as failures
- Prevents incorrect optimizations

**FR34:** Performance report generation
- Markdown report with tables and analysis
- Per-benchmark timing breakdown
- Geometric mean calculation
- Comparison to Rust with ratios

**FR35:** CI integration for regression detection
- Automated benchmark runs on commits
- Performance gates (no >5% regressions)
- Report archiving for historical comparison
- Alerts on performance degradation

---

## Non-Functional Requirements

### Performance

**PR1: Overall Performance Target**
- **Requirement:** Geometric mean of all 10 benchmarks must be >1.0x of Rust
- **Measurement:** `geometric_mean([seen_time[i]/rust_time[i] for i in 1..10]) > 1.0`
- **Rationale:** Proves SeenLang's architectural advantages deliver real-world performance gains

**PR2: Individual Benchmark Wins**
- **Requirement:** Minimum 5 out of 10 benchmarks must individually beat Rust
- **Measurement:** Count benchmarks where `seen_time < rust_time`
- **Rationale:** Not just optimizing one benchmark, but broadly competitive

**PR3: No Catastrophic Regressions**
- **Requirement:** No benchmark may be slower than 0.8x of Rust (max 20% slower)
- **Measurement:** `min([seen_time[i]/rust_time[i] for i in 1..10]) >= 0.8`
- **Rationale:** Ensures no pathological cases where architecture hurts performance

**PR4: Memory-Intensive Benchmark Target**
- **Requirement:** Binary Trees and LRU Cache must be >1.2x of Rust
- **Rationale:** These benchmarks specifically test region-based deallocation advantage
- **Expected:** O(1) bulk free vs O(n) incremental Drop overhead

**PR5: Compilation Time**
- **Requirement:** Compiler self-hosting build time <5 minutes on AMD Ryzen 9 7950X3D
- **Measurement:** Time for Stage1 → Stage2 full build
- **Rationale:** Fast iteration cycles for development
- **Hardware:** 32 cores allow parallel compilation for rapid builds

### Scalability

**SC1: Benchmark Data Sizes**
- Must handle benchmark-specified data sizes without OOM errors
- Matrix: 512×512 (1 MB)
- Binary Trees: Depth 20 (~8 MB)
- N-Body: 50M iterations (~400 MB working set)

**SC2: Concurrent Execution**
- HTTP Server: Handle 10,000 concurrent connections
- Mandelbrot: Utilize 8 CPU cores efficiently (>7x speedup)

### Integration

**INT1: LLVM Integration**
- Generate LLVM IR compatible with LLVM 14+
- Support optimization passes (-O0 through -O3)
- Link with system libraries (libc, libm)

**INT2: Build System Integration**
- `seen build` command compiles benchmarks
- `seen run` command executes in JIT mode
- `--backend llvm` flag selects LLVM codegen

**INT3: CI/CD Integration**
- Automated builds on every commit
- Performance benchmarks on PR merges
- Release artifacts generated for tags

---

_This PRD captures the essence of SeenLang - a self-hosted compiler proving that Vale-inspired regions can outperform Rust's borrow checker through rigorous production benchmarks._

_Created through collaborative discovery between yousef and AI facilitator (John, Product Manager Agent)._
