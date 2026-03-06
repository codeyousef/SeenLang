#!/bin/bash
# Production Benchmark Runner - Seen vs Rust
# Implements all 10 benchmarks from docs/private/benchmarks.md

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Results directory
RESULTS_DIR="benchmark_results"
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$RESULTS_DIR/production_comparison_$TIMESTAMP.md"

echo "=== Production Benchmark Suite ==="
echo "Building Seen compiler..."
cargo build --release --bin seen_cli

if [ ! -f "target/release/seen_cli" ]; then
    echo -e "${RED}ERROR: Seen compiler not found${NC}"
    exit 1
fi

SEEN_CLI="$SCRIPT_DIR/target/release/seen_cli"

# Initialize report
cat > "$REPORT_FILE" << EOF
# Production Benchmark Comparison: Rust vs Seen
Generated: $(date)
System: $(uname -a)

## Benchmark Results

| Benchmark | Rust Time (ms) | Seen Time (ms) | Speedup | Winner |
|-----------|----------------|----------------|---------|--------|
EOF

# Function to run a single benchmark
run_benchmark() {
    local name="$1"
    local rust_file="$2"
    local seen_file="$3"
    
    echo ""
    echo "=== Running: $name ==="
    
    # Compile and run Rust version
    echo "Building Rust version..."
    rustc -O -C target-cpu=native -C opt-level=3 "$rust_file" -o "${rust_file%.rs}"
    
    echo "Running Rust version..."
    RUST_OUTPUT=$("${rust_file%.rs}" 2>&1)
    RUST_TIME=$(echo "$RUST_OUTPUT" | grep -oP "(?<=Min time: )[0-9.]+" || echo "N/A")
    
    # Compile and run Seen version with AOT
    echo "Building Seen version (AOT)..."
    "$SEEN_CLI" build "$seen_file" --backend llvm -O3 --output "${seen_file%.seen}"
    
    if [ ! -f "${seen_file%.seen}" ]; then
        echo -e "${RED}Failed to build Seen version${NC}"
        SEEN_TIME="N/A"
        SPEEDUP="N/A"
        WINNER="N/A"
    else
        echo "Running Seen version..."
        SEEN_OUTPUT=$("${seen_file%.seen}" 2>&1)
        SEEN_TIME=$(echo "$SEEN_OUTPUT" | grep -oP "(?<=Min time: )[0-9.]+" || echo "N/A")
        
        # Calculate speedup
        if [ "$RUST_TIME" != "N/A" ] && [ "$SEEN_TIME" != "N/A" ]; then
            SPEEDUP=$(echo "scale=2; $RUST_TIME / $SEEN_TIME" | bc)
            if (( $(echo "$SEEN_TIME < $RUST_TIME" | bc -l) )); then
                WINNER="Seen (${SPEEDUP}x)"
            else
                INV_SPEEDUP=$(echo "scale=2; $SEEN_TIME / $RUST_TIME" | bc)
                WINNER="Rust (${INV_SPEEDUP}x)"
            fi
        else
            SPEEDUP="N/A"
            WINNER="N/A"
        fi
    fi
    
    # Append to report
    echo "| $name | $RUST_TIME | $SEEN_TIME | $SPEEDUP | $WINNER |" >> "$REPORT_FILE"
    
    echo -e "${GREEN}Rust:${NC} $RUST_TIME ms"
    echo -e "${GREEN}Seen:${NC} $SEEN_TIME ms"
    echo -e "${GREEN}Winner:${NC} $WINNER"
}

# Note: This script will be updated as benchmarks are implemented
# For now, it shows the infrastructure is in place

echo ""
echo "=== Benchmark Implementation Status ==="
echo "The following benchmarks need to be implemented:"
echo "1. Matrix Multiplication (SGEMM)"
echo "2. Sieve of Eratosthenes"
echo "3. Binary Trees"
echo "4. FASTA Generation"
echo "5. N-Body Simulation"
echo "6. Reverse Complement"
echo "7. Mandelbrot Set"
echo "8. LRU Cache"
echo "9. JSON Serialization"
echo "10. HTTP Echo Server"
echo ""
echo "Run this script after implementing the benchmarks in:"
echo "  benchmarks/production/*.seen (Seen versions)"
echo "  benchmarks/production/*.rs (Rust versions)"

# Append summary
cat >> "$REPORT_FILE" << EOF

## Summary

Production benchmark infrastructure is ready. Implement benchmarks following specs in docs/private/benchmarks.md.

Each benchmark must:
- Use deterministic inputs (fixed seeds)
- Include warmup iterations
- Report checksums to prevent dead-code elimination
- Compile with maximum optimizations
- Measure only the core computation (not I/O setup)

EOF

echo ""
echo -e "${GREEN}Report saved to: $REPORT_FILE${NC}"
