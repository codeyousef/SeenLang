# SeenLang MVP - Epics and Stories

**Author:** yousef  
**Date:** 2025-11-17  
**Version:** 1.0  
**Source:** Derived from PRD (docs/prd.md)

---

## Executive Summary

This document breaks down the SeenLang MVP PRD into implementable epics and stories. The goal: **Self-hosted compiler that outperforms Rust on 10 production benchmarks** (≥1.0x geometric mean).

**Total Scope:** 5 Epics, ~50 Stories, estimated 50-70 hours to MVP completion (excluding Phase 4 optimization which has no time limit).

---

## Epic Structure

| Epic | Phase | Priority | Stories | Est. Hours | Dependencies |
|------|-------|----------|---------|------------|--------------|
| **Epic 1:** LLVM Backend | Phase 1 | P0 | 12 | 12-16 | None |
| **Epic 2:** Self-Hosting | Phase 2 | P0 | 8 | 8-12 | Epic 1 |
| **Epic 3:** Benchmarks | Phase 3 | P0 | 20 | 20-30 | Epic 1 |
| **Epic 4:** Measurement | Phase 3 | P0 | 5 | 4-6 | Epic 3 |
| **Epic 5:** Optimization | Phase 4 | P0 | Variable | Variable | Epic 3, 4 |

---

## Epic 1: Complete LLVM Backend (Phase 1)

**Goal:** Fix P0 blocker - implement generic array/struct support in LLVM codegen

**PRD Requirements:** FR16-FR20  
**Priority:** P0 - Critical  
**Estimated Effort:** 12-16 hours  
**Dependencies:** None (foundational work)

**Success Criteria:**
- Generic array types work (Int[], Float[], String[])
- Generic struct field access works
- Stdlib links to compiled programs
- All language features compile via LLVM
- All 10 benchmarks unblocked

### Stories

#### Story 1.1: Generic Array Type System
**As a** compiler developer  
**I want** LLVM backend to handle arrays of any type  
**So that** benchmarks can use Float[], Int[], etc.

**Acceptance Criteria:**
- [ ] IR tracks array element types (not just i8*)
- [ ] LLVM type generation creates correct array types
- [ ] GEP instructions use proper element type metadata
- [ ] Test: Int[], Float[], String[], Bool[] all work

**Technical Tasks:**
- Extend `IRType` to preserve element types
- Map IR array types to LLVM array types
- Generate correct GEP with element size calculation
- Add test fixtures for each array type

**Estimated:** 3-4 hours

---

#### Story 1.2: Array Indexing & Mutation
**As a** benchmark author  
**I want** to read and write array elements  
**So that** I can implement matrix operations

**Acceptance Criteria:**
- [ ] `arr[i]` generates correct GEP + load
- [ ] `arr[i] = value` generates correct GEP + store
- [ ] Bounds checking in debug mode
- [ ] No bounds checking in release mode (-O3)

**Technical Tasks:**
- Implement `ArrayAccess` instruction in LLVM backend
- Implement `ArraySet` instruction in LLVM backend
- Add optional bounds check instrumentation
- Test with various array types

**Estimated:** 2-3 hours

---

#### Story 1.3: Generic Struct Field Access
**As a** compiler developer  
**I want** LLVM to handle struct fields generically  
**So that** any struct type works, not just CommandResult

**Acceptance Criteria:**
- [ ] Struct field access generates proper GEP
- [ ] Field types preserved from typechecker → IR → LLVM
- [ ] Nested struct access works (`obj.inner.field`)
- [ ] Test with multiple struct types

**Technical Tasks:**
- Create struct layout tracker in LLVM backend
- Generate GEP with correct field indices
- Handle nested field access recursively
- Add regression tests for various structs

**Estimated:** 3-4 hours

---

#### Story 1.4: Stdlib Linking Infrastructure
**As a** user  
**I want** compiled programs to link stdlib methods  
**So that** Array.push(), HashMap.get(), etc. work

**Acceptance Criteria:**
- [ ] Stdlib builds once to `libseen_std.a`
- [ ] LLVM programs link against stdlib
- [ ] Same semantics as interpreter mode
- [ ] ABI stability enforced via `abi_guard`

**Technical Tasks:**
- Build `seen_std` package to static library
- Add linker flags to `seen build` command
- Implement extern function resolution
- Test stdlib methods in compiled code

**Estimated:** 4-5 hours

---

#### Story 1.5: Float Arithmetic in LLVM
**As a** benchmark author  
**I want** float operations to work in compiled code  
**So that** N-Body, Matrix, etc. can run

**Acceptance Criteria:**
- [ ] Float literals → LLVM f64 constants
- [ ] fadd, fsub, fmul, fdiv, frem instructions
- [ ] Mixed Int/Float with promotion
- [ ] fcmp for comparisons

**Technical Tasks:**
- Already mostly complete (recent work)
- Verify all operations tested
- Add mixed type operation tests
- Document float operation semantics

**Estimated:** 1 hour (validation only)

---

### Epic 1 Validation

**Done When:**
- All 5 stories complete
- Simple benchmark compiles and runs (e.g., matrix multiply stub)
- No hardcoded types remain in LLVM backend
- CI tests pass

---

## Epic 2: Full Self-Hosting (Phase 2)

**Goal:** Fix remaining 160 compiler_seen errors, achieve deterministic bootstrap

**PRD Requirements:** FR1-FR5  
**Priority:** P0 - Critical  
**Estimated Effort:** 8-12 hours  
**Dependencies:** Epic 1 (need working LLVM backend)

**Success Criteria:**
- 0 type errors in compiler_seen
- Stage2 == Stage3 (determinism)
- `verify_rust_needed.sh` → "Rust not needed"
- Rust sources backed up and removed

### Stories

#### Story 2.1: Fix Remaining Type Errors (Batch 1)
**As a** compiler developer  
**I want** to fix 50 of the 160 remaining errors  
**So that** bootstrap progresses incrementally

**Acceptance Criteria:**
- [ ] 50 errors fixed (160 → 110)
- [ ] Zero regressions in working code
- [ ] Test suite still passes
- [ ] Document error categories fixed

**Technical Tasks:**
- Focus on highest-frequency error types
- Fix enum variant access issues
- Fix constructor return types
- Test after each batch of 10 fixes

**Estimated:** 2-3 hours

---

#### Story 2.2: Fix Remaining Type Errors (Batch 2)
**As a** compiler developer  
**I want** to fix 60 more errors (110 → 50)  
**So that** we're near completion

**Acceptance Criteria:**
- [ ] 60 errors fixed
- [ ] Full bootstrap attempt (may still fail)
- [ ] Document remaining error patterns

**Technical Tasks:**
- Fix method resolution issues
- Fix default parameter handling
- Fix nullable type comparisons
- Incremental testing

**Estimated:** 3-4 hours

---

#### Story 2.3: Fix Final Type Errors
**As a** compiler developer  
**I want** to fix last 50 errors (50 → 0)  
**So that** bootstrap succeeds

**Acceptance Criteria:**
- [ ] 0 type errors in compiler_seen
- [ ] `SEEN_ENABLE_MANIFEST_MODULES=1 seen build compiler_seen/src/main.seen` succeeds
- [ ] Stage1 → Stage2 completes

**Technical Tasks:**
- Fix edge case type errors
- Fix any language feature gaps
- Full test suite validation

**Estimated:** 2-3 hours

---

#### Story 2.4: Verify Deterministic Bootstrap
**As a** compiler developer  
**I want** Stage2 == Stage3 hash equality  
**So that** builds are reproducible

**Acceptance Criteria:**
- [ ] Stage1 → Stage2 succeeds
- [ ] Stage2 → Stage3 succeeds
- [ ] SHA-256(Stage2) == SHA-256(Stage3)
- [ ] `validate_determinism.sh` passes

**Technical Tasks:**
- Run full bootstrap pipeline
- Compare binary hashes
- Debug any non-determinism
- Document hash values

**Estimated:** 1-2 hours

---

#### Story 2.5: Remove Rust Sources
**As a** project maintainer  
**I want** Rust sources removed  
**So that** we're fully self-hosted

**Acceptance Criteria:**
- [ ] `verify_rust_needed.sh` → "Rust not needed"
- [ ] Rust sources backed up
- [ ] Rust sources removed from repo
- [ ] All builds use Seen compiler only

**Technical Tasks:**
- Run verification script
- Create backup archive
- Remove Rust compiler crates
- Update CI to use Stage2 compiler

**Estimated:** 1 hour

---

### Epic 2 Validation

**Done When:**
- 0 type errors
- Deterministic bootstrap verified
- Rust removed
- CI builds with self-hosted compiler

---

## Epic 3: Implement 10 Production Benchmarks (Phase 3)

**Goal:** Implement all 10 benchmarks in Seen, verify correctness

**PRD Requirements:** FR21-FR30  
**Priority:** P0 - Critical  
**Estimated Effort:** 20-30 hours (parallel development)  
**Dependencies:** Epic 1 (LLVM backend working)

**Success Criteria:**
- All 10 benchmarks implemented in Seen
- All checksums match reference implementations
- All benchmarks compile and run
- Baseline performance measured

### Stories (10 Benchmark Stories)

#### Story 3.1: Matrix Multiplication (SGEMM)
**As a** performance analyst  
**I want** matrix multiply benchmark  
**So that** we test SIMD and cache performance

**Acceptance Criteria:**
- [ ] 512×512 Float matrices
- [ ] Cache-blocked tiled algorithm (32×32 tiles)
- [ ] GFLOPS calculation
- [ ] Checksum validates
- [ ] Compiles with `seen build -O3`

**Technical Tasks:**
- Implement matrix data structure
- Implement tiled multiply algorithm
- Add GFLOPS reporting
- Match Rust implementation exactly

**Estimated:** 3-4 hours

---

#### Story 3.2: Sieve of Eratosthenes
**As a** performance analyst  
**I want** prime sieve benchmark  
**So that** we test cache locality and bit operations

**Acceptance Criteria:**
- [ ] Primes up to 10,000,000
- [ ] Bit-packed array (BitSet from stdlib)
- [ ] Segmented sieve
- [ ] Prime count = 664,579
- [ ] Checksum validates

**Technical Tasks:**
- Use stdlib BitSet
- Implement segmented sieve
- Add prime counting
- Verify against reference

**Estimated:** 2-3 hours

---

#### Story 3.3: Binary Trees
**As a** performance analyst  
**I want** tree allocation benchmark  
**So that** we test region-based deallocation

**Acceptance Criteria:**
- [ ] Tree depth 20 (1,048,575 nodes)
- [ ] Recursive allocation
- [ ] Checksum = -1
- [ ] Region inference triggers

**Technical Tasks:**
- Define TreeNode struct
- Implement recursive build
- Implement checksum calculation
- Verify region usage with profiler

**Estimated:** 2 hours

---

#### Story 3.4: FASTA Generation
**As a** performance analyst  
**I want** DNA sequence benchmark  
**So that** we test RNG and string building

**Acceptance Criteria:**
- [ ] 5,000,000 nucleotides
- [ ] LCG random number generator
- [ ] Weighted selection
- [ ] Deterministic with seed=42
- [ ] Checksum validates

**Technical Tasks:**
- Implement LCG from stdlib
- Implement weighted random selection
- Build DNA string efficiently
- Verify determinism

**Estimated:** 2-3 hours

---

#### Story 3.5: N-Body Simulation
**As a** performance analyst  
**I want** physics simulation benchmark  
**So that** we test float precision and math intrinsics

**Acceptance Criteria:**
- [ ] 5 bodies (Sun + 4 planets)
- [ ] 50,000,000 timesteps
- [ ] Double precision Float
- [ ] Energy conservation < 1e-9 error
- [ ] Checksum validates

**Technical Tasks:**
- Define Body struct
- Implement symplectic integrator
- Add energy calculation
- Verify conservation law

**Estimated:** 3-4 hours

---

#### Story 3.6: Reverse-Complement
**As a** performance analyst  
**I want** DNA reversal benchmark  
**So that** we test byte manipulation

**Acceptance Criteria:**
- [ ] 25,000,000 base pairs
- [ ] Lookup table for complement
- [ ] Reverse + complement
- [ ] MD5 checksum validates

**Technical Tasks:**
- Build nucleotide lookup table
- Implement reverse algorithm
- Implement complement mapping
- Verify MD5

**Estimated:** 2 hours

---

#### Story 3.7: Mandelbrot Set
**As a** performance analyst  
**I want** fractal rendering benchmark  
**So that** we test complex math and threading

**Acceptance Criteria:**
- [ ] 4000×4000 pixels
- [ ] 1000 max iterations
- [ ] Complex number arithmetic
- [ ] Pixel checksum validates
- [ ] (Future: 8-thread parallelization)

**Technical Tasks:**
- Define Complex struct
- Implement escape-time algorithm
- Calculate pixel values
- Single-threaded first, parallel later

**Estimated:** 3 hours

---

#### Story 3.8: LRU Cache
**As a** performance analyst  
**I want** cache operations benchmark  
**So that** we test data structures

**Acceptance Criteria:**
- [ ] 100,000 capacity
- [ ] 5,000,000 operations (Get/Put 70%/30%)
- [ ] HashMap + LinkedList
- [ ] Sum of Gets validates

**Technical Tasks:**
- Use stdlib HashMap
- Implement LRU eviction with LinkedList
- Generate operation trace
- Verify sum calculation

**Estimated:** 2-3 hours

---

#### Story 3.9: JSON Serialization
**As a** performance analyst  
**I want** JSON encoding benchmark  
**So that** we test string building

**Acceptance Criteria:**
- [ ] 1,000,000 objects
- [ ] Proper escaping
- [ ] MD5 checksum validates
- [ ] Bytes written metric

**Technical Tasks:**
- Define Record struct
- Implement JSON encoder
- Add escape sequences
- Verify MD5

**Estimated:** 2-3 hours

---

#### Story 3.10: HTTP Echo Server
**As a** performance analyst  
**I want** network I/O benchmark  
**So that** we test concurrency

**Acceptance Criteria:**
- [ ] 5,000,000 requests
- [ ] TCP sockets
- [ ] Echo responses
- [ ] Requests/second metric
- [ ] (Initial: single-threaded, Future: concurrent)

**Technical Tasks:**
- Use stdlib TcpListener
- Implement echo logic
- Measure throughput
- Single-threaded MVP

**Estimated:** 3-4 hours

---

### Epic 3 Validation

**Done When:**
- All 10 benchmarks implemented
- All checksums validate
- All benchmarks run successfully
- Baseline report generated

---

## Epic 4: Performance Measurement Infrastructure (Phase 3)

**Goal:** Automate benchmarking with strict measurement protocol

**PRD Requirements:** FR31-FR35  
**Priority:** P0 - Critical  
**Estimated Effort:** 4-6 hours  
**Dependencies:** Epic 3 (benchmarks exist)

### Stories

#### Story 4.1: Benchmark Harness Script
**As a** developer  
**I want** single command to run all benchmarks  
**So that** I can measure performance easily

**Acceptance Criteria:**
- [ ] `./run_all_production_benchmarks.sh` exists
- [ ] Compiles Rust versions
- [ ] Compiles Seen versions
- [ ] Runs all 10 benchmarks
- [ ] Outputs timing data

**Technical Tasks:**
- Extend existing benchmark harness
- Add LLVM compilation step
- Add timing extraction
- Parallelize benchmark runs

**Estimated:** 2 hours

---

#### Story 4.2: Statistical Validation
**As a** performance analyst  
**I want** strict measurement protocol  
**So that** results are statistically valid

**Acceptance Criteria:**
- [ ] 3-5 warmup runs (discarded)
- [ ] 10 measured runs
- [ ] ALL runs within 5% variance
- [ ] Re-run if variance >5%
- [ ] Report min, mean, max, stddev, median

**Technical Tasks:**
- Implement warmup loop
- Implement measurement loop
- Calculate variance
- Add retry logic
- Statistical reporting

**Estimated:** 2 hours

---

#### Story 4.3: Performance Report Generation
**As a** stakeholder  
**I want** comprehensive comparison report  
**So that** I can see Seen vs Rust performance

**Acceptance Criteria:**
- [ ] Markdown report generated
- [ ] Per-benchmark timing tables
- [ ] Geometric mean calculation
- [ ] Ratios (Seen/Rust)
- [ ] Identifies wins/losses

**Technical Tasks:**
- Implement report generator
- Add table formatting
- Calculate geometric mean
- Generate visualizations (optional)

**Estimated:** 1-2 hours

---

#### Story 4.4: Checksum Validation Automation
**As a** developer  
**I want** automatic checksum verification  
**So that** incorrect optimizations are caught

**Acceptance Criteria:**
- [ ] Extract checksums from output
- [ ] Compare Seen vs reference
- [ ] Fail build if mismatch
- [ ] Report which benchmarks failed

**Technical Tasks:**
- Parse benchmark output
- Extract checksum values
- Implement comparison logic
- Add failure reporting

**Estimated:** 1 hour

---

#### Story 4.5: CI Integration
**As a** maintainer  
**I want** benchmarks in CI  
**So that** regressions are detected

**Acceptance Criteria:**
- [ ] GitHub Actions workflow
- [ ] Runs on every PR
- [ ] Performance gates (>5% = fail)
- [ ] Archives reports

**Technical Tasks:**
- Create `.github/workflows/benchmarks.yml`
- Configure dedicated runner
- Add performance thresholds
- Upload artifacts

**Estimated:** 2 hours

---

### Epic 4 Validation

**Done When:**
- Automated harness works
- Statistical validation passes
- Reports generated
- CI integrated

---

## Epic 5: Performance Optimization Until ≥1.0x (Phase 4)

**Goal:** Iterate optimization until geometric mean ≥1.0x of Rust

**PRD Requirements:** PR1-PR5  
**Priority:** P0 - Critical  
**Estimated Effort:** Variable (no time limit)  
**Dependencies:** Epic 3, 4 (benchmarks + measurement)

**Success Criteria:**
- Geometric mean ≥1.0x
- Minimum 5/10 benchmarks beat Rust
- No benchmark <0.8x
- All optimizations validated

### Optimization Stories (Created Dynamically)

**Note:** These stories are created on-demand as performance gaps are identified.

#### Story 5.1: Profile All Benchmarks
**As a** optimization engineer  
**I want** detailed performance profiles  
**So that** I know where to optimize

**Acceptance Criteria:**
- [ ] perf profiles for all 10 benchmarks
- [ ] CPU counters captured (cache misses, branches, etc.)
- [ ] Assembly compared to Rust
- [ ] Top 3 bottlenecks identified per benchmark

**Technical Tasks:**
- Run `perf record` on each benchmark
- Analyze `perf report` output
- Compare Seen vs Rust assembly
- Document bottlenecks

**Estimated:** 4-6 hours

---

#### Story 5.2: Optimize Binary Trees (Region Advantage)
**As a** optimization engineer  
**I want** Binary Trees to beat Rust by >1.2x  
**So that** we prove region advantage

**Acceptance Criteria:**
- [ ] Current: Measure baseline performance
- [ ] Target: >1.2x of Rust
- [ ] Profile allocation/deallocation
- [ ] Verify region bulk-free triggers
- [ ] Re-measure and validate

**Technical Tasks:**
- Profile malloc/free counts
- Verify region inference working
- Optimize region allocator if needed
- Add manual region hints if needed
- Iterate until target met

**Estimated:** Variable (4-8 hours typical)

---

#### Story 5.3: Optimize Matrix (SIMD)
**As a** optimization engineer  
**I want** Matrix to match Rust  
**So that** SIMD auto-vectorization works

**Acceptance Criteria:**
- [ ] Current: Measure baseline
- [ ] Target: ≥1.0x of Rust
- [ ] Verify LLVM vectorization
- [ ] Compare LLVM IR output
- [ ] Add SIMD hints if needed

**Technical Tasks:**
- Check if loops vectorize
- Compare Seen vs Rust LLVM IR
- Adjust loop structure if needed
- Add explicit SIMD if required
- Validate performance gain

**Estimated:** Variable (3-6 hours typical)

---

**Additional optimization stories created as needed...**

### Epic 5 Validation

**Done When:**
- Geometric mean ≥1.0x (REQUIRED)
- Minimum 5/10 benchmarks beat Rust
- No benchmark <0.8x
- Performance report published

---

## Story Point Summary

### By Epic

| Epic | Stories | Story Points | Hours (Est.) |
|------|---------|--------------|--------------|
| Epic 1: LLVM Backend | 5 | 13 | 12-16 |
| Epic 2: Self-Hosting | 5 | 10 | 8-12 |
| Epic 3: Benchmarks | 10 | 25 | 20-30 |
| Epic 4: Measurement | 5 | 6 | 4-6 |
| Epic 5: Optimization | Variable | Variable | Variable |
| **Total (Phases 1-3)** | **25** | **54** | **44-64** |

### By Priority

| Priority | Stories | Hours |
|----------|---------|-------|
| P0 - Critical | All | All |
| P1 - Important | 0 | 0 |
| P2 - Nice to Have | 0 | 0 |

**Everything is P0 for MVP.**

---

## Implementation Schedule (Suggested)

### Week 1: Foundation
- **Days 1-2:** Epic 1 (LLVM Backend) - 12-16 hours
- **Days 3-4:** Epic 2 (Self-Hosting) - 8-12 hours

### Week 2-3: Benchmarks
- **Days 5-14:** Epic 3 (Benchmarks) - 20-30 hours parallel
- **Days 13-14:** Epic 4 (Measurement) - 4-6 hours

### Week 4+: Optimization
- **Ongoing:** Epic 5 (Optimize until ≥1.0x) - Variable

**Total to Baseline:** ~2-3 weeks  
**Total to ≥1.0x:** Variable (no deadline)

---

## Dependencies Graph

```
Epic 1 (LLVM Backend)
   ↓
   ├─→ Epic 2 (Self-Hosting)
   └─→ Epic 3 (Benchmarks)
         ↓
         Epic 4 (Measurement)
         ↓
         Epic 5 (Optimization)
```

---

## Risk Mitigation Stories

### If LLVM Backend Takes >24 Hours
- Break Story 1.1-1.4 into smaller incremental tasks
- Test each type individually before moving to generics

### If Performance <1.0x After Phase 3
- Execute Epic 5 systematically
- Profile → Identify → Optimize → Measure → Repeat
- No deadline - continue until target met

---

## Acceptance Criteria Summary

**MVP is DONE when:**
1. ✅ All 25 stories in Epics 1-4 complete
2. ✅ All 10 benchmarks passing with correct checksums
3. ✅ Self-hosted compiler (0 Rust dependency)
4. ✅ Geometric mean ≥1.0x (Epic 5 complete)
5. ✅ Minimum 5/10 benchmarks beat Rust
6. ✅ No benchmark <0.8x
7. ✅ Performance report generated and validated

---

_This epic breakdown transforms the PRD's 35 functional requirements into actionable development tasks with clear acceptance criteria and effort estimates._

_Created by: Product Manager Agent (John)_  
_Date: 2025-11-17_  
_Source: docs/prd.md_
