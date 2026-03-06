#!/bin/bash
# Language Comparison: Seen vs Rust
# Benchmarks self-time (each does its own warmup/iteration/min internally)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

RESULTS_DIR="benchmark_results"
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$RESULTS_DIR/language_comparison_$TIMESTAMP.md"

SEEN_COMPILER="$SCRIPT_DIR/compiler_seen/target/seen"
PROD_DIR="$SCRIPT_DIR/benchmarks/production"
RUST_DIR="$SCRIPT_DIR/benchmarks/rust"

echo -e "${BLUE}=== Language Comparison: Seen vs Rust ===${NC}"
echo ""

# Benchmark mapping: name|seen_base|rust_name
declare -a BENCHMARKS=(
    "Matrix Multiply|01_matrix_mult|matrix_mult"
    "Sieve of Eratosthenes|02_sieve|sieve"
    "Binary Trees|03_binary_trees|binary_trees"
    "FASTA Generation|04_fasta|fasta"
    "N-Body Simulation|05_nbody|nbody"
    "Reverse Complement|06_revcomp|revcomp"
    "Mandelbrot Set|07_mandelbrot|mandelbrot"
    "LRU Cache|08_lru_cache|lru_cache"
    "JSON Serialization|09_json_serialize|json_serialize"
    "Spectral Norm|11_spectral_norm|spectral_norm"
    "Fannkuch-Redux|12_fannkuch|fannkuch"
    "Great-Circle Distance|13_great_circle|"
    "Hyperbolic PDE|14_hyperbolic_pde|hyperbolic_pde"
    "DFT Spectrum|15_dft_spectrum|"
    "Euler Totient|16_euler_totient|euler_totient"
)

# Build Rust benchmarks if needed (with -C target-cpu=native for fair comparison)
# Use --rebuild-rust or SEEN_REBUILD_RUST=1 to force rebuild with native CPU flags
REBUILD_RUST=0
if [[ "${1:-}" == "--rebuild-rust" ]] || [[ "${SEEN_REBUILD_RUST:-}" == "1" ]]; then
    REBUILD_RUST=1
fi
echo -e "${CYAN}Checking Rust benchmarks...${NC}"
for entry in "${BENCHMARKS[@]}"; do
    IFS='|' read -r name seen_base rust_name <<< "$entry"
    if [ -n "$rust_name" ] && [ -d "$RUST_DIR/$rust_name" ]; then
        bin="$RUST_DIR/$rust_name/target/release/$rust_name"
        if [ ! -f "$bin" ] || [ "$REBUILD_RUST" -eq 1 ]; then
            echo "  Building Rust: $rust_name..."
            (cd "$RUST_DIR/$rust_name" && RUSTFLAGS="-C target-cpu=native" cargo build --release 2>/dev/null)
        fi
    fi
done

# Compile all Seen benchmarks (with timing)
echo -e "${CYAN}Compiling Seen benchmarks...${NC}"
TOTAL_COMPILE_MS=0
for entry in "${BENCHMARKS[@]}"; do
    IFS='|' read -r name seen_base rust_name <<< "$entry"
    seen_src="$PROD_DIR/${seen_base}.seen"
    seen_bin="$PROD_DIR/${seen_base}"
    if [ -f "$seen_src" ]; then
        rm -rf .seen_cache/
        compile_start=$(date +%s%N)
        "$SEEN_COMPILER" compile "$seen_src" "$seen_bin" > /dev/null 2>&1
        compile_end=$(date +%s%N)
        compile_ms=$(( (compile_end - compile_start) / 1000000 ))
        TOTAL_COMPILE_MS=$((TOTAL_COMPILE_MS + compile_ms))
        echo "  $name: ${compile_ms}ms"
    fi
done
echo -e "  ${GREEN}Total compilation: ${TOTAL_COMPILE_MS}ms (avg: $((TOTAL_COMPILE_MS / 15))ms per benchmark)${NC}"
echo ""

# Extract "Min time: X" or "Time: X" from benchmark output
extract_time() {
    local output="$1"
    local t
    t=$(echo "$output" | grep -oP '(?<=Min time: )[0-9.]+' | head -1)
    if [ -z "$t" ]; then
        t=$(echo "$output" | grep -oP '(?<=Time: )[0-9.]+' | head -1)
    fi
    echo "$t"
}

# Header
cat > "$REPORT_FILE" << EOF
# Language Comparison: Seen vs Rust
Generated: $(date)
System: $(uname -a)

| # | Benchmark | Seen (ms) | Rust (ms) | Ratio (Seen/Rust) | Winner |
|---|-----------|-----------|-----------|-------------------|--------|
EOF

SEEN_WINS=0
RUST_WINS=0
COMPARED=0
IDX=0

for entry in "${BENCHMARKS[@]}"; do
    IFS='|' read -r name seen_base rust_name <<< "$entry"
    IDX=$((IDX + 1))

    seen_bin="$PROD_DIR/${seen_base}"
    rust_bin=""
    if [ -n "$rust_name" ]; then
        rust_bin="$RUST_DIR/$rust_name/target/release/$rust_name"
    fi

    echo -e "${BLUE}[$IDX/${#BENCHMARKS[@]}] $name${NC}"

    # Run Seen (benchmarks do their own warmup/timing)
    seen_ms=""
    if [ -f "$seen_bin" ]; then
        seen_output=$(timeout 300 "$seen_bin" 2>&1) || true
        seen_ms=$(extract_time "$seen_output")
        if [ -n "$seen_ms" ]; then
            echo -e "  Seen: ${YELLOW}${seen_ms} ms${NC}"
        else
            echo -e "  Seen: ${RED}no timing${NC}"
        fi
    else
        echo -e "  Seen: ${RED}not compiled${NC}"
    fi

    # Run Rust
    rust_ms=""
    if [ -n "$rust_bin" ] && [ -f "$rust_bin" ]; then
        rust_output=$(timeout 300 "$rust_bin" 2>&1) || true
        rust_ms=$(extract_time "$rust_output")
        if [ -n "$rust_ms" ]; then
            echo -e "  Rust: ${YELLOW}${rust_ms} ms${NC}"
        else
            echo -e "  Rust: ${RED}no timing${NC}"
        fi
    else
        echo -e "  Rust: ${RED}no binary${NC}"
    fi

    # Calculate ratio
    ratio="N/A"
    winner=""
    if [ -n "$seen_ms" ] && [ -n "$rust_ms" ]; then
        ratio=$(awk "BEGIN {printf \"%.2f\", $seen_ms / $rust_ms}" 2>/dev/null || echo "N/A")
        if [ "$ratio" != "N/A" ]; then
            cmp=$(awk "BEGIN {print ($seen_ms <= $rust_ms) ? 1 : 0}" 2>/dev/null || echo "0")
            if [ "$cmp" = "1" ]; then
                winner="**Seen**"
                SEEN_WINS=$((SEEN_WINS + 1))
            else
                winner="Rust"
                RUST_WINS=$((RUST_WINS + 1))
            fi
            COMPARED=$((COMPARED + 1))
        fi
        echo -e "  Ratio: ${CYAN}${ratio}x${NC} → ${GREEN}${winner}${NC}"
    fi

    # Write to report
    printf "| %02d | %s | %s | %s | %s | %s |\n" \
        "$IDX" "$name" "${seen_ms:-N/A}" "${rust_ms:-N/A}" "$ratio" "$winner" >> "$REPORT_FILE"
done

echo "" >> "$REPORT_FILE"
echo "## Summary" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "- **Benchmarks compared**: $COMPARED" >> "$REPORT_FILE"
echo "- **Seen wins**: $SEEN_WINS" >> "$REPORT_FILE"
echo "- **Rust wins**: $RUST_WINS" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "Ratio < 1.0 = Seen faster, > 1.0 = Rust faster" >> "$REPORT_FILE"

echo ""
echo -e "${BLUE}=== Results ===${NC}"
echo -e "Compared: $COMPARED benchmarks"
echo -e "Seen wins: ${GREEN}$SEEN_WINS${NC}"
echo -e "Rust wins: ${YELLOW}$RUST_WINS${NC}"
echo -e "${GREEN}Report: $REPORT_FILE${NC}"
