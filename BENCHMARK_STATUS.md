# Seen Production Benchmark Status

## Overview

This document tracks the implementation status of the 10 production benchmarks from `docs/private/benchmarks.md`.

## Current Status (2025-11-16)

### Infrastructure ✅ Complete

- [x] Benchmark runner script (`run_production_benchmarks.sh`)
- [x] All 10 Seen benchmark implementations in `benchmarks/production/`
- [x] Proper benchmark structure (warmup, iterations, checksums)
- [x] LLVM backend enabled in seen_cli

### Language Features Implemented ✅

- [x] `__GetTime()` intrinsic for timing (returns Float seconds)
- [x] `__PrintInt()` intrinsic for integer output
- [x] `__PrintFloat()` intrinsic for float output
- [x] `__Print()` intrinsic for string output
- [x] `__Sqrt()` intrinsic for math
- [x] Cast expression support (Int ↔ Float, etc.)
- [x] Type casting in IR generator
- [x] LLVM intrinsic bindings (clock_gettime for precise timing)

### Benchmarks Created ✅

1. **Matrix Multiplication (SGEMM)** - `01_matrix_mult.seen`
    - Cache-blocked matrix multiplication
    - 512x512 matrices
    - Measures GFLOPS

2. **Sieve of Eratosthenes** - `02_sieve.seen`
    - Bit array optimization
    - Finds primes up to 10M
    - Measures primes/second

3. **Binary Trees** - `03_binary_trees.seen`
    - GC stress test
    - Recursive tree allocation
    - Measures memory throughput

4. **FASTA Generation** - `04_fasta.seen`
    - DNA sequence generation
    - Cumulative probability selection
    - Measures Mbp/second

5. **N-Body Simulation** - `05_nbody.seen`
    - Solar system physics
    - 50M timesteps
    - Measures steps/second

6. **Reverse Complement** - `06_revcomp.seen`
    - DNA reversal with lookup table
    - 25M base pairs
    - Measures Mbp/second

7. **Mandelbrot Set** - `07_mandelbrot.seen`
    - Fractal generation
    - 4000x4000 image
    - Measures pixels/second

8. **LRU Cache** - `08_lru_cache.seen`
    - Cache with hash map
    - 5M operations
    - Measures ops/second

9. **JSON Serialization** - `09_json_serialize.seen`
    - Object to JSON conversion
    - 1M objects
    - Measures throughput MB/s

10. **HTTP Echo Server** - `10_http_echo.seen`
    - Request/response processing
    - 5M requests
    - Measures requests/second

### Known Issues ⚠️

#### LLVM Backend Issues

- Cast instruction lowering incomplete
- Array indexing may not generate proper LLVM IR
- Class field access needs verification
- Integer literal handling in casts

#### Missing Language Features

- [ ] Mutable variables (`var` keyword works in parser but IR/LLVM may have issues)
- [ ] Array methods beyond basic push/length
- [ ] String concatenation operator
- [ ] Class constructor initialization edge cases
- [ ] Match expression with complex patterns
- [ ] Option type full support in LLVM backend

### Next Steps

#### Priority 1: Fix LLVM Cast Lowering

The main blocker is cast instruction lowering in `seen_ir/src/llvm_backend.rs`. Need to:

1. Handle `Instruction::Cast` in the LLVM backend switch statement
2. Generate proper LLVM cast instructions (sitofp, fptosi, etc.)
3. Add type checking for source/target compatibility

#### Priority 2: Array Operations

Benchmarks heavily use arrays. Need to ensure:

1. Array indexing generates proper GEP instructions
2. Array methods (push, length, etc.) link to runtime
3. Bounds checking or unsafe access patterns work

#### Priority 3: Class Field Access

Matrix and other classes need reliable field access:

1. Data GEP generation for class fields
2. Method calls with `this` pointer
3. Constructor initialization

#### Priority 4: Runtime Linking

Ensure compiled binaries link against:

1. libc (malloc, printf, clock_gettime)
2. libm (sqrt, other math functions)
3. Seen runtime (if needed for arrays/strings)

### Testing Strategy

Once LLVM issues are fixed:

```bash
# Test single benchmark
./target/release/seen_cli build benchmarks/production/01_matrix_mult.seen \
    --backend llvm -O3 --output /tmp/matrix
/tmp/matrix

# Run all benchmarks
./run_production_benchmarks.sh
```

Expected output format from each benchmark:

```
<Benchmark Name> Benchmark
<Configuration details>
Warming up (3 runs)...
Running measured iterations...
<Result metrics>
Min time: XXX.XX ms
<Throughput metric>
```

### Success Criteria

- [ ] All 10 benchmarks compile successfully with `--backend llvm -O3`
- [ ] All benchmarks execute without runtime errors
- [ ] All benchmarks produce expected output format
- [ ] Timing results are reasonable (not 0.0 or negative)
- [ ] Checksums prevent dead-code elimination
- [ ] Performance competitive with Rust (within 2-5x initially)

### Performance Goals (Post-Fix)

Initial targets (Seen vs Rust):

- Matrix Mult: ~2x slower (cache optimization)
- Sieve: ~1.5x slower (bit operations)
- Binary Trees: ~3x slower (GC overhead acceptable)
- FASTA: ~1.5x slower (RNG and string ops)
- N-Body: ~1x-1.2x (should be nearly identical, pure math)
- Reverse Complement: ~1.3x slower (lookup table)
- Mandelbrot: ~1x-1.5x (pure computation)
- LRU Cache: ~2x slower (hash map overhead)
- JSON: ~2-3x slower (string allocations)
- HTTP Echo: ~2x slower (string processing)

### Files Created

Benchmark implementations:

- `benchmarks/production/01_matrix_mult.seen`
- `benchmarks/production/02_sieve.seen`
- `benchmarks/production/03_binary_trees.seen`
- `benchmarks/production/04_fasta.seen`
- `benchmarks/production/05_nbody.seen`
- `benchmarks/production/06_revcomp.seen`
- `benchmarks/production/07_mandelbrot.seen`
- `benchmarks/production/08_lru_cache.seen`
- `benchmarks/production/09_json_serialize.seen`
- `benchmarks/production/10_http_echo.seen`

Infrastructure:

- `run_production_benchmarks.sh` - Automated benchmark runner
- `BENCHMARK_STATUS.md` - This file

Compiler changes:

- `seen_typechecker/src/checker.rs` - Added __GetTime, __PrintInt, __PrintFloat intrinsics
- `seen_ir/src/generator.rs` - Added cast expression support
- `seen_ir/src/llvm_backend.rs` - Added __GetTime, __PrintInt, __PrintFloat, __Sqrt LLVM lowering

## Conclusion

The benchmark infrastructure is 100% complete with all 10 benchmarks written in idiomatic Seen code. The primary
remaining work is fixing the LLVM backend to properly lower the IR instructions generated by these benchmarks. The cast
instruction and array operations are the critical path items.

Estimated effort to completion: 4-8 hours for an experienced LLVM backend developer.
