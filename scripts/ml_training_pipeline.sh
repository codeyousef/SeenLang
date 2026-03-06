#!/bin/bash
# ML Training Data Collection Pipeline
# Compiles all 16 benchmarks with --ml-log to collect optimization decisions
# Merges JSONL files into a single training dataset
#
# Usage: scripts/ml_training_pipeline.sh [--output=<file>]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

OUTPUT_FILE="benchmark_results/ml_training_data.jsonl"
for arg in "$@"; do
    case $arg in
        --output=*) OUTPUT_FILE="${arg#*=}" ;;
    esac
done

SEEN_COMPILER="$ROOT_DIR/compiler_seen/target/seen"
if [ ! -f "$SEEN_COMPILER" ]; then
    echo -e "${RED}ERROR: Compiler not found at $SEEN_COMPILER${NC}"
    exit 1
fi

# Clear previous training data
: > "$OUTPUT_FILE"
ML_TMP="/tmp/seen_ml_log_$$.jsonl"

echo -e "${BLUE}=== ML Training Data Collection ===${NC}"
echo "Output: $OUTPUT_FILE"
echo ""

BENCHMARKS=(
    "01_matrix_mult" "02_sieve" "03_binary_trees" "04_fasta"
    "05_nbody" "06_revcomp" "07_mandelbrot" "08_lru_cache"
    "09_json_serialize" "10_http_echo" "11_spectral_norm" "12_fannkuch"
    "13_great_circle" "14_hyperbolic_pde" "15_dft_spectrum" "16_euler_totient"
)

COLLECTED=0

for bench in "${BENCHMARKS[@]}"; do
    SEEN_FILE="benchmarks/production/${bench}.seen"
    if [ ! -f "$SEEN_FILE" ]; then
        echo -e "${YELLOW}Skip: $bench (not found)${NC}"
        continue
    fi

    echo -n "Collecting: $bench... "
    : > "$ML_TMP"
    OUTPUT_BIN="/tmp/ml_bench_$$"

    if "$SEEN_COMPILER" compile "$SEEN_FILE" "$OUTPUT_BIN" --ml-log="$ML_TMP" 2>&1 | tail -1 | grep -q "Build succeeded"; then
        LINES=$(wc -l < "$ML_TMP" 2>/dev/null || echo "0")
        if [ "$LINES" -gt 0 ]; then
            cat "$ML_TMP" >> "$OUTPUT_FILE"
            echo -e "${GREEN}$LINES decisions${NC}"
            COLLECTED=$((COLLECTED + LINES))
        else
            echo -e "${YELLOW}no decisions${NC}"
        fi
    else
        echo -e "${RED}compile failed${NC}"
    fi

    rm -f "$OUTPUT_BIN" "$ML_TMP"
done

echo ""
echo -e "${BLUE}=== Summary ===${NC}"
echo "Total decisions collected: $COLLECTED"
echo "Training data: $OUTPUT_FILE"

# Validate JSONL format
if [ -f "$OUTPUT_FILE" ]; then
    TOTAL_LINES=$(wc -l < "$OUTPUT_FILE")
    echo "Total lines: $TOTAL_LINES"

    # Basic validation: check each line is valid JSON-like
    VALID=0
    while IFS= read -r line; do
        if echo "$line" | grep -q '^{.*}$'; then
            VALID=$((VALID + 1))
        fi
    done < "$OUTPUT_FILE"
    echo "Valid JSONL lines: $VALID"
fi

echo -e "${GREEN}Pipeline complete${NC}"
