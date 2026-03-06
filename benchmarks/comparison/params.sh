#!/usr/bin/env bash
# Standardized benchmark parameters — ALL languages use these exact values.
# Source this file from run_comparison.sh.

# Benchmark list (number:name) — all 17 benchmarks
BENCHMARKS="01:matrix_mult 02:sieve 03:binary_trees 04:fasta 05:nbody 06:revcomp 07:mandelbrot 08:lru_cache 09:json_serialize 10:http_echo 11:spectral_norm 12:fannkuch 13:great_circle 14:hyperbolic_pde 15:dft_spectrum 16:euler_totient 17:fibonacci"

# Compilation flags (parity)
C_FLAGS="-O3 -flto -march=native -lm"
CPP_FLAGS="-O3 -flto -march=native -std=c++17 -lm"
RUST_FLAGS="-C opt-level=3 -C lto=fat -C target-cpu=native"
ZIG_FLAGS="-OReleaseFast -lc"

# Seen compiler
SEEN_COMPILER="../../compiler_seen/target/seen"
SEEN_CMD="compile"

# Output directories
BUILD_DIR="build"
RESULTS_DIR="results"
