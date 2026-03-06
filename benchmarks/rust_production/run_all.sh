#!/bin/bash
# Run all Rust production benchmarks
# Equivalent to run_production_benchmarks.sh for Seen

set -e
cd "$(dirname "$0")"

echo "=== Rust Production Benchmarks ==="
echo "Building..."
cargo build --release 2>&1 | tail -1

for bench in 01_matrix_mult 02_sieve 03_binary_trees 04_fasta 05_nbody 06_revcomp 07_mandelbrot 08_lru_cache 11_spectral_norm 12_fannkuch; do
    echo ""
    echo "=== $bench ==="
    ./target/release/$bench
done
