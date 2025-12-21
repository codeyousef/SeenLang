# Production Benchmark Comparison: Seen (AOT)
Generated: Thu 20 Nov 18:13:17 +03 2025
System: Linux pop-os 6.17.4-76061704-generic #202510191616~1762410050~22.04~898873a SMP PREEMPT_DYNAMIC Thu N x86_64 x86_64 x86_64 GNU/Linux
Compiler: seen_cli seen 0.1.0

## Configuration
- Seen Mode: AOT (LLVM backend with -O3)
- Optimization: Maximum (-O3, target-cpu=native)
- Iterations: 5 per benchmark, minimum time reported
- Warmup: 3 runs before measurement

## Benchmark Results

| # | Benchmark | Min Time (ms) | Throughput | Status |
|---|-----------|---------------|------------|--------|
| 01 | Matrix Multiplication (SGEMM) | N/A | N/A | ❌ Compile Error |
| 02 | Sieve of Eratosthenes | N/A | N/A | ❌ Compile Error |
| 03 | Binary Trees | N/A | N/A | ❌ Compile Error |
| 04 | FASTA Generation | N/A | N/A | ❌ Compile Error |
| 05 | N-Body Simulation | N/A | N/A | ❌ Compile Error |
| 06 | Reverse Complement | N/A | N/A | ❌ Compile Error |
| 07 | Mandelbrot Set | N/A | N/A | ❌ Compile Error |
| 08 | LRU Cache | N/A | N/A | ❌ Compile Error |
| 09 | JSON Serialization | N/A | N/A | ❌ Compile Error |
| 10 | HTTP Echo Server | N/A | N/A | ❌ Compile Error |

## Summary

- **Total Benchmarks**: 10
- **Successful**: 0
- **Failed**: 10
- **Success Rate**: 0%

## Notes

All benchmarks compiled with:
```
seen build <file>.seen --backend llvm -O3 --output <binary>
```

Each benchmark includes:
- Deterministic inputs (fixed seeds)
- Warmup iterations (3 runs)
- Measured iterations (5 runs, minimum time reported)
- Checksums to prevent dead-code elimination
- Maximum optimizations (-O3)

