#!/bin/bash
# Production Benchmark Runner - Seen AOT Mode
# Runs all 10 benchmarks from docs/private/benchmarks.md

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

RESULTS_DIR="benchmark_results"
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$RESULTS_DIR/production_comparison_$TIMESTAMP.md"

echo -e "${BLUE}=== Production Benchmark Suite ===${NC}"
echo "Building Seen compiler with LLVM..."
cargo build --release --bin seen_cli --features llvm

if [ ! -f "target/release/seen_cli" ]; then
    echo -e "${RED}ERROR: Seen compiler not found${NC}"
    exit 1
fi

SEEN_CLI="$SCRIPT_DIR/target/release/seen_cli"

cat > "$REPORT_FILE" << EOF
# Production Benchmark Comparison: Seen (AOT)
Generated: $(date)
System: $(uname -a)
Compiler: seen_cli $(${SEEN_CLI} --version 2>/dev/null || echo "v0.1.0")

## Configuration
- Seen Mode: AOT (LLVM backend with -O3)
- Optimization: Maximum (-O3, target-cpu=native)
- Iterations: 5 per benchmark, minimum time reported
- Warmup: 3 runs before measurement

## Benchmark Results

| # | Benchmark | Min Time (ms) | Throughput | Status |
|---|-----------|---------------|------------|--------|
EOF

BENCHMARKS=(
    "01_matrix_mult:Matrix Multiplication (SGEMM)"
    "02_sieve:Sieve of Eratosthenes"
    "03_binary_trees:Binary Trees"
    "04_fasta:FASTA Generation"
    "05_nbody:N-Body Simulation"
    "06_revcomp:Reverse Complement"
    "07_mandelbrot:Mandelbrot Set"
    "08_lru_cache:LRU Cache"
    "09_json_serialize:JSON Serialization"
    "10_http_echo:HTTP Echo Server"
)

SUCCESS_COUNT=0
FAIL_COUNT=0

for benchmark in "${BENCHMARKS[@]}"; do
    IFS=':' read -r file_name display_name <<< "$benchmark"
    num=$(echo "$file_name" | grep -oP '^\d+')
    
    echo ""
    echo -e "${BLUE}=== [$num/10] $display_name ===${NC}"
    
    SEEN_FILE="benchmarks/production/${file_name}.seen"
    OUTPUT_BIN="benchmarks/production/${file_name}"
    
    if [ ! -f "$SEEN_FILE" ]; then
        echo -e "${RED}ERROR: $SEEN_FILE not found${NC}"
        echo "| $num | $display_name | N/A | N/A | ❌ Missing |" >> "$REPORT_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi
    
    echo "Compiling with LLVM -O3..."
    if ! "$SEEN_CLI" build "$SEEN_FILE" --backend llvm -O3 --output "$OUTPUT_BIN" 2>&1 | grep -q "Compilation successful\|Finished"; then
        echo -e "${RED}Compilation failed${NC}"
        echo "| $num | $display_name | N/A | N/A | ❌ Compile Error |" >> "$REPORT_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi
    
    if [ ! -f "$OUTPUT_BIN" ]; then
        echo -e "${RED}ERROR: Binary not generated${NC}"
        echo "| $num | $display_name | N/A | N/A | ❌ No Binary |" >> "$REPORT_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi
    
    echo "Running benchmark..."
    if OUTPUT=$("$OUTPUT_BIN" 2>&1); then
        MIN_TIME=$(echo "$OUTPUT" | grep -oP "(?<=Min time: )[0-9.]+" || echo "$OUTPUT" | grep -oP "(?<=Time: )[0-9.]+" || echo "N/A")
        THROUGHPUT=$(echo "$OUTPUT" | grep -E "(GFLOPS|per second|Throughput|MB/s)" | head -1 | grep -oP "[0-9.]+ [A-Za-z/]+" || echo "N/A")
        
        if [ "$MIN_TIME" != "N/A" ]; then
            echo -e "${GREEN}✓ Success${NC}"
            echo -e "  Time: ${YELLOW}${MIN_TIME} ms${NC}"
            echo -e "  Throughput: ${YELLOW}${THROUGHPUT}${NC}"
            echo "| $num | $display_name | $MIN_TIME | $THROUGHPUT | ✅ Pass |" >> "$REPORT_FILE"
            SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
        else
            echo -e "${YELLOW}⚠ Completed but no timing found${NC}"
            echo "| $num | $display_name | N/A | N/A | ⚠️  No Timing |" >> "$REPORT_FILE"
            FAIL_COUNT=$((FAIL_COUNT + 1))
        fi
    else
        echo -e "${RED}✗ Runtime error${NC}"
        echo "| $num | $display_name | N/A | N/A | ❌ Runtime Error |" >> "$REPORT_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
    
    rm -f "$OUTPUT_BIN"
done

cat >> "$REPORT_FILE" << EOF

## Summary

- **Total Benchmarks**: 10
- **Successful**: $SUCCESS_COUNT
- **Failed**: $FAIL_COUNT
- **Success Rate**: $((SUCCESS_COUNT * 100 / 10))%

## Notes

All benchmarks compiled with:
\`\`\`
seen build <file>.seen --backend llvm -O3 --output <binary>
\`\`\`

Each benchmark includes:
- Deterministic inputs (fixed seeds)
- Warmup iterations (3 runs)
- Measured iterations (5 runs, minimum time reported)
- Checksums to prevent dead-code elimination
- Maximum optimizations (-O3)

EOF

echo ""
echo -e "${BLUE}=== Results Summary ===${NC}"
echo -e "Successful: ${GREEN}$SUCCESS_COUNT${NC}/10"
echo -e "Failed: ${RED}$FAIL_COUNT${NC}/10"
echo ""
echo -e "${GREEN}Report saved to: $REPORT_FILE${NC}"
