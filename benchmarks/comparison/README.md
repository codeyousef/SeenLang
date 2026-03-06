# Fair 4-Language Benchmark Comparison

Compares **Seen**, **Rust**, **C (GCC)**, and **C++ (G++)** on 8 benchmarks with identical algorithms and parameters.

## Quick Start

```bash
cd benchmarks/comparison
chmod +x run_comparison.sh
./run_comparison.sh
```

The script will:
1. Detect available compilers (skips missing ones)
2. Compile all benchmarks with equivalent optimization flags
3. Run each benchmark, extract min time, peak RSS, and checksums
4. Validate checksums match across all languages
5. Generate a markdown report at `results/comparison_report.md`

## Benchmarks

| # | Name | Category | Key Parameters |
|---|------|----------|----------------|
| 01 | Matrix Multiply | FP compute | size=1024, block_size=64 |
| 02 | Sieve | Memory bandwidth | limit=10,000,000 |
| 03 | Binary Trees | Allocation stress | min_depth=4, max_depth=21 (C/C++), 20 (Seen/Rust) |
| 05 | N-Body | FP compute | 50M steps, dt=0.01 |
| 08 | LRU Cache | Hash map heavy | 10M ops, capacity=100K |
| 09 | JSON Serialize | String handling | 1M objects |
| 11 | Spectral Norm | Dense FP math | N=5500, 10 power iterations |
| 12 | Fannkuch | Integer permutations | N=12 |

## Compilation Flags

| Language | Compiler | Flags |
|----------|----------|-------|
| Seen | `compiler_seen/target/seen compile` | Default (opt -O3, llc -O3 -mcpu=native, clang -O3 -flto) |
| Rust | `rustc` | `-C opt-level=3 -C lto=fat -C target-cpu=native` |
| C | `gcc` | `-O3 -flto -march=native -lm` |
| C++ | `g++` | `-O3 -flto -march=native -std=c++17 -lm` |

## Algorithm Parity Rules

- **Same algorithm, line-for-line equivalent control flow** across all languages
- `int64_t` arrays where Seen uses `Array<Int>` (not `char`/`bool`)
- `double` where Seen uses `Float`
- Same PRNG seeds, same iteration counts, same data structures
- Float formatting: `%.6f` (6 decimal places) matching Seen's `fast_f64_to_buf`

## Caveats

1. Seen's pool allocator advantages in allocation-heavy benchmarks (Binary Trees)
2. LRU: different hash map implementations (SwissTable vs linear probing vs std::unordered_map)
3. Fannkuch: Seen uses 64-bit ints for perm arrays; C/C++/Rust use 32-bit
4. Seen's C runtime is compiled with Clang; C/C++ benchmarks use GCC
5. Binary Trees uses different max_depth between C/C++ (21) and Seen/Rust (20)

## Directory Structure

```
comparison/
├── c/                  # C implementations (8 files)
├── cpp/                # C++ implementations (8 files)
├── build/              # Compiled binaries (generated)
├── results/            # Benchmark reports (generated)
├── params.sh           # Shared parameters
├── run_comparison.sh   # Runner script
└── README.md           # This file
```
