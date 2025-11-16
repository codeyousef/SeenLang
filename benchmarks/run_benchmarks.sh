#!/bin/bash
# Benchmark Runner: Rust vs Seen Performance Comparison

set -e

RUST_DIR="rust"
SEEN_DIR="seen"
RESULTS_FILE="benchmark_results.md"

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║          Rust vs Seen Performance Benchmarks                     ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

# Create results file header
cat > "$RESULTS_FILE" << 'HEADER'
# Rust vs Seen Performance Benchmark Results

**Date:** $(date '+%Y-%m-%d %H:%M:%S')  
**Machine:** $(uname -a)  
**Rust Version:** $(rustc --version)  
**Seen Compiler:** target/release/seen_cli

---

HEADER

# Compile Rust benchmarks
echo "Compiling Rust benchmarks..."
cd "$RUST_DIR"
for bench in bench_*.rs; do
    name="${bench%.rs}"
    echo "  - $name"
    rustc -O -C target-cpu=native "$bench" -o "$name" 2>&1 | head -5
done
cd ..

echo ""
echo "Running benchmarks..."
echo ""

# Run Rust benchmarks
echo "=== RUST RESULTS ===" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

for bench in "$RUST_DIR"/bench_*[^.rs]; do
    if [ -x "$bench" ]; then
        name=$(basename "$bench")
        echo "Running $name..." | tee -a "$RESULTS_FILE"
        "$bench" | tee -a "$RESULTS_FILE"
        echo "" | tee -a "$RESULTS_FILE"
    fi
done

echo ""
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║  Benchmark results saved to: $RESULTS_FILE"
echo "╚══════════════════════════════════════════════════════════════════╝"
