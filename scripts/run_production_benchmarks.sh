#!/bin/bash
# Production Benchmark Runner - Self-Hosted Seen Compiler
# Runs all 16 benchmarks from benchmarks/production/
# Outputs: markdown report + JSONL machine-readable results

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
JSONL_FILE="$RESULTS_DIR/results_$TIMESTAMP.jsonl"

SEEN_COMPILER="$SCRIPT_DIR/compiler_seen/target/seen"

echo -e "${BLUE}=== Production Benchmark Suite ===${NC}"

if [ ! -f "$SEEN_COMPILER" ]; then
    echo -e "${RED}ERROR: Self-hosted compiler not found at $SEEN_COMPILER${NC}"
    echo "Run ./scripts/safe_rebuild.sh to build the compiler first."
    exit 1
fi

# Check for /usr/bin/time (GNU time) for peak RSS measurement
HAS_GNU_TIME=false
if /usr/bin/time --version 2>&1 | grep -q "GNU"; then
    HAS_GNU_TIME=true
fi

cat > "$REPORT_FILE" << EOF
# Production Benchmark Results: Seen (Self-Hosted Compiler)
Generated: $(date)
System: $(uname -a)
Compiler: Self-hosted Seen compiler (LLVM backend, -O3)

## Configuration
- Compiler: Self-hosted Seen compiler
- Backend: LLVM with -O3 optimizations
- Iterations: 5 per benchmark, minimum time reported
- Warmup: 3 runs before measurement

## Benchmark Results

| # | Benchmark | Min Time (ms) | Compile (ms) | Binary (KB) | Peak RSS (KB) | Status |
|---|-----------|---------------|--------------|-------------|---------------|--------|
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
    "11_spectral_norm:Spectral Norm"
    "12_fannkuch:Fannkuch-Redux"
    "13_great_circle:Great-Circle Distance (Haversine)"
    "14_hyperbolic_pde:Hyperbolic PDE Solver"
    "15_dft_spectrum:DFT Power Spectrum"
    "16_euler_totient:Euler Totient (Number Theory)"
)

SUCCESS_COUNT=0
FAIL_COUNT=0

for benchmark in "${BENCHMARKS[@]}"; do
    IFS=':' read -r file_name display_name <<< "$benchmark"
    num=$(echo "$file_name" | grep -oP '^\d+')

    echo ""
    echo -e "${BLUE}=== [$num/16] $display_name ===${NC}"

    SEEN_FILE="benchmarks/production/${file_name}.seen"
    OUTPUT_BIN="/tmp/bench_${num}"

    if [ ! -f "$SEEN_FILE" ]; then
        echo -e "${RED}ERROR: $SEEN_FILE not found${NC}"
        echo "| $num | $display_name | N/A | N/A | N/A | N/A | Missing |" >> "$REPORT_FILE"
        echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":null,\"compile_ms\":null,\"binary_kb\":null,\"peak_rss_kb\":null,\"status\":\"missing\"}" >> "$JSONL_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    # Clear cache to avoid stale objects from previous benchmark
    rm -rf .seen_cache/

    # Measure compile time
    echo "Compiling..."
    COMPILE_START=$(date +%s%N)
    COMPILE_OUTPUT=$("$SEEN_COMPILER" compile "$SEEN_FILE" "$OUTPUT_BIN" 2>&1 || true)
    COMPILE_END=$(date +%s%N)
    COMPILE_MS=$(( (COMPILE_END - COMPILE_START) / 1000000 ))

    if ! echo "$COMPILE_OUTPUT" | tail -1 | grep -q "Build succeeded"; then
        echo -e "${RED}Compilation failed${NC}"
        echo "$COMPILE_OUTPUT" | tail -5
        echo "| $num | $display_name | N/A | $COMPILE_MS | N/A | N/A | Compile Error |" >> "$REPORT_FILE"
        echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":null,\"compile_ms\":$COMPILE_MS,\"binary_kb\":null,\"peak_rss_kb\":null,\"status\":\"compile_error\"}" >> "$JSONL_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    if [ ! -f "$OUTPUT_BIN" ]; then
        echo -e "${RED}ERROR: Binary not generated${NC}"
        echo "| $num | $display_name | N/A | $COMPILE_MS | N/A | N/A | No Binary |" >> "$REPORT_FILE"
        echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":null,\"compile_ms\":$COMPILE_MS,\"binary_kb\":null,\"peak_rss_kb\":null,\"status\":\"no_binary\"}" >> "$JSONL_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    # Measure binary size
    BINARY_KB=$(du -k "$OUTPUT_BIN" | cut -f1)

    # Run benchmark with optional peak RSS measurement
    echo "Running benchmark..."
    PEAK_RSS="N/A"
    if $HAS_GNU_TIME; then
        TIME_OUTPUT=$(/usr/bin/time -v timeout 120 "$OUTPUT_BIN" 2>&1) && RUN_OK=true || RUN_OK=false
        PEAK_RSS=$(echo "$TIME_OUTPUT" | grep "Maximum resident set size" | grep -oP '\d+' || echo "N/A")
        OUTPUT=$(echo "$TIME_OUTPUT" | sed '/Command being timed/,$d')
    else
        OUTPUT=$(timeout 120 "$OUTPUT_BIN" 2>&1) && RUN_OK=true || RUN_OK=false
    fi

    if $RUN_OK; then
        MIN_TIME=$(echo "$OUTPUT" | grep -oP "(?<=Min time: )[0-9.]+" || echo "$OUTPUT" | grep -oP "(?<=Time: )[0-9.]+" || echo "N/A")
        THROUGHPUT=$(echo "$OUTPUT" | grep -E "(GFLOPS|per second|Throughput|MB/s|Mbp/s)" | head -1 | sed 's/.*: //' || echo "N/A")

        if [ "$MIN_TIME" != "N/A" ]; then
            echo -e "${GREEN}Pass${NC}"
            echo -e "  Time: ${YELLOW}${MIN_TIME} ms${NC}  Compile: ${YELLOW}${COMPILE_MS} ms${NC}  Binary: ${YELLOW}${BINARY_KB} KB${NC}  RSS: ${YELLOW}${PEAK_RSS} KB${NC}"
            echo "| $num | $display_name | $MIN_TIME | $COMPILE_MS | $BINARY_KB | $PEAK_RSS | Pass |" >> "$REPORT_FILE"
            # JSONL output - use null for N/A values
            PEAK_RSS_JSON="$PEAK_RSS"
            if [ "$PEAK_RSS" = "N/A" ]; then PEAK_RSS_JSON="null"; fi
            echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":$MIN_TIME,\"compile_ms\":$COMPILE_MS,\"binary_kb\":$BINARY_KB,\"peak_rss_kb\":$PEAK_RSS_JSON,\"status\":\"pass\"}" >> "$JSONL_FILE"
            SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
        else
            echo -e "${YELLOW}Completed but no timing found${NC}"
            echo "$OUTPUT" | head -10
            echo "| $num | $display_name | N/A | $COMPILE_MS | $BINARY_KB | $PEAK_RSS | No Timing |" >> "$REPORT_FILE"
            echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":null,\"compile_ms\":$COMPILE_MS,\"binary_kb\":$BINARY_KB,\"peak_rss_kb\":null,\"status\":\"no_timing\"}" >> "$JSONL_FILE"
            FAIL_COUNT=$((FAIL_COUNT + 1))
        fi
    else
        EXIT_CODE=$?
        if [ $EXIT_CODE -eq 124 ]; then
            echo -e "${RED}Timeout (120s)${NC}"
            echo "| $num | $display_name | N/A | $COMPILE_MS | $BINARY_KB | N/A | Timeout |" >> "$REPORT_FILE"
            echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":null,\"compile_ms\":$COMPILE_MS,\"binary_kb\":$BINARY_KB,\"peak_rss_kb\":null,\"status\":\"timeout\"}" >> "$JSONL_FILE"
        else
            echo -e "${RED}Runtime error (exit $EXIT_CODE)${NC}"
            echo "| $num | $display_name | N/A | $COMPILE_MS | $BINARY_KB | N/A | Runtime Error |" >> "$REPORT_FILE"
            echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":null,\"compile_ms\":$COMPILE_MS,\"binary_kb\":$BINARY_KB,\"peak_rss_kb\":null,\"status\":\"runtime_error\"}" >> "$JSONL_FILE"
        fi
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi

    rm -f "$OUTPUT_BIN"
done

# Write summary JSON line
SYSTEM_INFO=$(uname -srm)
echo "{\"_summary\":true,\"timestamp\":\"$TIMESTAMP\",\"system\":\"$SYSTEM_INFO\",\"total\":16,\"passed\":$SUCCESS_COUNT,\"failed\":$FAIL_COUNT}" >> "$JSONL_FILE"

cat >> "$REPORT_FILE" << EOF

## Summary

- **Total Benchmarks**: 16
- **Successful**: $SUCCESS_COUNT
- **Failed**: $FAIL_COUNT
- **Success Rate**: $((SUCCESS_COUNT * 100 / 16))%

## Notes

All benchmarks compiled with the self-hosted Seen compiler:
\`\`\`
./compiler_seen/target/seen compile <file>.seen <output>
\`\`\`

Each benchmark includes:
- Deterministic inputs (fixed seeds)
- Warmup iterations (3 runs)
- Measured iterations (5 runs, minimum time reported)
- Checksums to prevent dead-code elimination

JSONL results: \`$JSONL_FILE\`

EOF

echo ""
echo -e "${BLUE}=== Results Summary ===${NC}"
echo -e "Successful: ${GREEN}$SUCCESS_COUNT${NC}/16"
echo -e "Failed: ${RED}$FAIL_COUNT${NC}/16"
echo ""
echo -e "${GREEN}Report saved to: $REPORT_FILE${NC}"
echo -e "${GREEN}JSONL saved to:  $JSONL_FILE${NC}"
