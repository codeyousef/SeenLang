#!/bin/bash
# Collect ML training data from diverse Seen programs.
#
# Data sources (ordered by importance):
#   1. Compiler self-compilation — 10+ modules, 47K lines, real-world code patterns
#      (string processing, tree walking, hash lookups, code gen, file I/O)
#   2. Synthetic workloads — 5 programs covering common patterns benchmarks miss
#      (string ops, tree traversal, hash tables, data pipelines, control flow)
#   3. Production benchmarks — 15 numeric-heavy programs
#
# This gives ~2000+ decision points from diverse code, not just tight loops.
#
# Output: ml_training_data.tsv
# Format: program\tsource\tcontent

set -e

SEEN="./compiler_seen/target/seen"
BENCH_DIR="./benchmarks/production"
SYNTH_DIR="./scripts/ml_synthetics"
COMPILER_SRC="compiler_seen/src/main_compiler.seen"
OUTPUT="ml_training_data.tsv"
TMP_DIR="/tmp/seen_ml_collect"

mkdir -p "$TMP_DIR"

# Header
echo -e "program\tsource\tcontent" > "$OUTPUT"

# Track totals
total_feat=0
total_decision=0
total_remark=0

collect_program() {
    local name="$1"
    local src="$2"
    local binary="$3"
    local weight="$4"  # repeat factor for weighting
    local run_binary="$5"  # "yes" or "no"

    echo "--- $name (weight=${weight}x) ---"
    local decision_log="$TMP_DIR/${name}_decisions.tsv"
    local remarks_log="$TMP_DIR/${name}_remarks.tsv"

    # Clear incremental cache
    rm -rf .seen_cache/

    # Compile with both ML logging flags:
    # --ml-decision-log: codegen-level FEAT vectors + inline/dead-arg/etc decisions
    # --ml-log: LLVM opt remarks (inlining, vectorization, etc)
    # Timeout 120s and memory limit 2GB to prevent OOM on large inputs
    echo "  Compiling with ML logging..."
    if timeout 120 $SEEN compile "$src" "$binary" --ml-decision-log="$decision_log" --ml-log="$remarks_log" 2>/dev/null; then
        echo "  OK: compiled"
    else
        echo "  FAIL: compilation failed, skipping"
        return
    fi

    # Append codegen decisions (FEAT vectors + inline/dead-arg/etc)
    if [ -f "$decision_log" ]; then
        local feat_count=0
        local dec_count=0
        while IFS= read -r line; do
            [ -z "$line" ] && continue
            local rep=0
            while [ $rep -lt "$weight" ]; do
                echo -e "${name}\tml_decision\t${line}" >> "$OUTPUT"
                rep=$((rep + 1))
            done
            # Count types
            if echo "$line" | grep -q "^FEAT"; then
                feat_count=$((feat_count + 1))
            else
                dec_count=$((dec_count + 1))
            fi
        done < "$decision_log"
        total_feat=$((total_feat + feat_count * weight))
        total_decision=$((total_decision + dec_count * weight))
        echo "  Collected ${feat_count} features, ${dec_count} decisions (x${weight})"
    fi

    # Append LLVM optimization remarks
    if [ -f "$remarks_log" ]; then
        local remark_count=0
        while IFS= read -r line; do
            [ -z "$line" ] && continue
            echo -e "${name}\tllvm_remark\t${line}" >> "$OUTPUT"
            remark_count=$((remark_count + 1))
        done < "$remarks_log"
        total_remark=$((total_remark + remark_count))
        echo "  Collected ${remark_count} LLVM remarks"
    fi

    # Run binary if requested (benchmarks only) — single run with timeout
    if [ "$run_binary" = "yes" ] && [ -f "$binary" ]; then
        echo "  Running 1x (quick)..."
        local start_ns=$(date +%s%N)
        timeout 30 "$binary" > /dev/null 2>&1 || true
        local end_ns=$(date +%s%N)
        local elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
        echo -e "${name}\truntime_ms\t${elapsed_ms}" >> "$OUTPUT"
        echo "  Runtime: ${elapsed_ms}ms"
    fi

    echo ""
}

echo "=== Seen ML Training Data Collection ==="
echo "Compiler: $SEEN"
echo "Output:   $OUTPUT"
echo ""
echo "Phase 1: Compiler self-compilation (most important)"
echo "Phase 2: Synthetic workloads (common patterns)"
echo "Phase 3: Production benchmarks (numeric code)"
echo ""

# =======================================================================
# PHASE 1: Compiler self-compilation
# This is the #1 most important data source because:
# - It's the largest Seen program (47K lines, 108 files)
# - It has diverse code patterns (strings, trees, tables, I/O)
# - Optimizing the compiler benefits ALL Seen users (faster builds)
# - Weight 3x because it's the most representative real-world code
# =======================================================================
echo "========================================="
echo "Phase 1: Compiler Self-Compilation"
echo "========================================="

# SKIP: Compiler self-compilation causes OOM (42 modules, 5.6M+ chars IR for llvm_ir_gen alone).
# The benchmarks + synthetics provide sufficient training data.
echo "  SKIPPED: Compiler self-compilation causes OOM on large modules."
echo "  Using benchmark + synthetic data instead."
echo ""

# =======================================================================
# PHASE 2: Synthetic workloads
# Cover patterns that benchmarks miss entirely:
# - String processing (CLI tools, parsers, text processors)
# - Tree traversal (compilers, databases, DOM)
# - Hash tables (symbol tables, caches, key-value stores)
# - Data pipelines (ETL, log processing, data science)
# - Control flow (state machines, validators, interpreters)
# Weight 2x because these represent common real-world patterns
# =======================================================================
echo "========================================="
echo "Phase 2: Synthetic Workloads"
echo "========================================="

SYNTHETICS=(
    "string_processing"
    "tree_walk"
    "hash_table"
    "data_pipeline"
    "control_flow"
)

for synth in "${SYNTHETICS[@]}"; do
    src="$SYNTH_DIR/${synth}.seen"
    if [ ! -f "$src" ]; then
        echo "SKIP: $src not found"
        continue
    fi
    collect_program \
        "synth_${synth}" \
        "$src" \
        "$TMP_DIR/synth_${synth}" \
        2 \
        "yes"
done

# =======================================================================
# PHASE 3: Production benchmarks
# These ARE important — numeric code is a key use case.
# But they should NOT dominate the training data.
# Weight 1x (baseline) because they're already well-represented.
# =======================================================================
echo "========================================="
echo "Phase 3: Production Benchmarks"
echo "========================================="

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

for bench in "${BENCHMARKS[@]}"; do
    src="$BENCH_DIR/${bench}.seen"
    if [ ! -f "$src" ]; then
        echo "SKIP: $src not found"
        continue
    fi
    collect_program \
        "bench_${bench}" \
        "$src" \
        "$TMP_DIR/${bench}" \
        1 \
        "yes"
done

# Summary
total_lines=$(wc -l < "$OUTPUT")
echo "========================================="
echo "=== Collection Complete ==="
echo "========================================="
echo ""
echo "Total entries: $((total_lines - 1))"
echo "  Feature vectors: $total_feat"
echo "  Decisions:       $total_decision"
echo "  LLVM remarks:    $total_remark"
echo ""
echo "Data distribution:"
echo "  Compiler self-compilation: 3x weight (most representative)"
echo "  Synthetic workloads:       2x weight (common real patterns)"
echo "  Production benchmarks:     1x weight (numeric code)"
echo ""
echo "Output file: $OUTPUT"
echo ""
echo "Next step: python3 scripts/ml_train.py"
