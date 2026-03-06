#!/bin/bash
# A/B validation of ML-trained weights vs default thresholds.
# Compiles and runs all benchmarks twice:
#   A) Without .seen_ml_heuristics (default thresholds)
#   B) With .seen_ml_heuristics (trained weights)
# Reports wins/losses/ties.
#
# Usage: ./scripts/ml_validate.sh

set -e

SEEN="./compiler_seen/target/seen"
BENCH_DIR="./benchmarks/production"
TMP_DIR="/tmp/seen_ml_validate"
HEURISTICS=".seen_ml_heuristics"
RUNS=5

mkdir -p "$TMP_DIR"

BENCHMARKS=(
    "01_matrix_mult"
    "02_sieve"
    "03_binary_trees"
    "04_fasta"
    "05_nbody"
    "06_revcomp"
    "07_mandelbrot"
    "08_lru_cache"
    "09_json_serialize"
    "11_spectral_norm"
    "12_fannkuch"
    "13_great_circle"
    "14_hyperbolic_pde"
    "15_dft_spectrum"
    "16_euler_totient"
)

run_benchmark() {
    local binary="$1"
    local best=999999
    for i in $(seq 1 $RUNS); do
        local start_ns=$(date +%s%N)
        timeout 60 "$binary" > /dev/null 2>&1 || true
        local end_ns=$(date +%s%N)
        local elapsed=$(( (end_ns - start_ns) / 1000000 ))
        if [ "$elapsed" -lt "$best" ]; then
            best=$elapsed
        fi
    done
    echo "$best"
}

echo "=== Seen ML A/B Validation ==="
echo "Compiler: $SEEN"
echo "Runs per benchmark: $RUNS"
echo ""

# Check if heuristics file exists
if [ ! -f "$HEURISTICS" ]; then
    echo "ERROR: $HEURISTICS not found. Run scripts/ml_train.py first."
    exit 1
fi

wins=0
losses=0
ties=0
total_a=0
total_b=0

printf "%-25s %10s %10s %10s %s\n" "Benchmark" "Default" "ML" "Ratio" "Result"
printf "%-25s %10s %10s %10s %s\n" "---------" "-------" "--" "-----" "------"

for bench in "${BENCHMARKS[@]}"; do
    src="$BENCH_DIR/${bench}.seen"
    if [ ! -f "$src" ]; then
        continue
    fi

    # A) Compile without ML weights
    rm -rf .seen_cache/
    mv "$HEURISTICS" "${HEURISTICS}.bak" 2>/dev/null || true
    binary_a="$TMP_DIR/${bench}_default"
    if ! $SEEN build "$src" "$binary_a" 2>/dev/null; then
        printf "%-25s %10s\n" "$bench" "COMPILE_FAIL"
        mv "${HEURISTICS}.bak" "$HEURISTICS" 2>/dev/null || true
        continue
    fi

    # B) Compile with ML weights
    rm -rf .seen_cache/
    mv "${HEURISTICS}.bak" "$HEURISTICS" 2>/dev/null || true
    binary_b="$TMP_DIR/${bench}_ml"
    if ! $SEEN build "$src" "$binary_b" 2>/dev/null; then
        printf "%-25s %10s %10s\n" "$bench" "OK" "COMPILE_FAIL"
        continue
    fi

    # Run both
    time_a=$(run_benchmark "$binary_a")
    time_b=$(run_benchmark "$binary_b")

    total_a=$((total_a + time_a))
    total_b=$((total_b + time_b))

    # Calculate ratio (ML time / default time)
    if [ "$time_a" -gt 0 ]; then
        ratio=$(echo "scale=3; $time_b / $time_a" | bc 2>/dev/null || echo "N/A")
    else
        ratio="N/A"
    fi

    # Determine result (5% threshold for significance)
    result="tie"
    if [ "$time_b" -lt "$((time_a * 95 / 100))" ]; then
        result="WIN"
        wins=$((wins + 1))
    elif [ "$time_b" -gt "$((time_a * 105 / 100))" ]; then
        result="LOSS"
        losses=$((losses + 1))
    else
        ties=$((ties + 1))
    fi

    printf "%-25s %8dms %8dms %10s %s\n" "$bench" "$time_a" "$time_b" "${ratio}x" "$result"
done

echo ""
echo "=== Summary ==="
echo "Wins:   $wins"
echo "Losses: $losses"
echo "Ties:   $ties"
if [ "$total_a" -gt 0 ]; then
    overall=$(echo "scale=3; $total_b / $total_a" | bc 2>/dev/null || echo "N/A")
    echo "Overall: ${overall}x (total_default=${total_a}ms, total_ml=${total_b}ms)"
fi

echo ""
if [ "$wins" -gt "$losses" ]; then
    echo "VERDICT: ML weights are a net improvement. Consider keeping .seen_ml_heuristics."
elif [ "$losses" -gt "$wins" ]; then
    echo "VERDICT: ML weights are a net regression. Consider reverting to defaults."
    echo "  rm .seen_ml_heuristics"
else
    echo "VERDICT: Neutral. ML weights show no significant difference."
fi
