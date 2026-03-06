#!/usr/bin/env bash
set -euo pipefail

# Fair 5-Language Benchmark Comparison: Seen vs Rust vs C (GCC) vs C++ (G++) vs Zig
# All 17 benchmarks use identical algorithms and parameters.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$SCRIPT_DIR"

source params.sh

mkdir -p "$BUILD_DIR" "$RESULTS_DIR"

# ─── Detect compilers ───────────────────────────────────────────────────────

HAS_SEEN=0; HAS_RUST=0; HAS_C=0; HAS_CPP=0; HAS_ZIG=0

if [ -x "$SEEN_COMPILER" ]; then HAS_SEEN=1; echo "[✓] Seen compiler: $SEEN_COMPILER"; else echo "[✗] Seen compiler not found"; fi
if command -v rustc &>/dev/null; then HAS_RUST=1; echo "[✓] Rust: $(rustc --version)"; else echo "[✗] rustc not found"; fi
if command -v gcc &>/dev/null; then HAS_C=1; echo "[✓] GCC: $(gcc --version | head -1)"; else echo "[✗] gcc not found"; fi
if command -v g++ &>/dev/null; then HAS_CPP=1; echo "[✓] G++: $(g++ --version | head -1)"; else echo "[✗] g++ not found"; fi
if command -v zig &>/dev/null; then HAS_ZIG=1; echo "[✓] Zig: $(zig version)"; else echo "[✗] zig not found"; fi

echo ""

# ─── Compile all benchmarks ─────────────────────────────────────────────────

declare -A COMPILE_TIMES

compile_one() {
    local lang="$1" src="$2" out="$3" label="$4"
    local start end elapsed
    start=$(date +%s%N)

    case "$lang" in
        seen)
            # Seen compiler must run from project root (needs seen_runtime/ in CWD)
            (cd "$PROJECT_ROOT" && "$SCRIPT_DIR/$SEEN_COMPILER" $SEEN_CMD "$SCRIPT_DIR/$src" "$SCRIPT_DIR/$out") 2>&1 || { echo "  FAIL: $label"; return 1; }
            ;;
        rust)
            rustc $RUST_FLAGS "$src" -o "$out" 2>&1 || { echo "  FAIL: $label"; return 1; }
            ;;
        c)
            gcc $C_FLAGS "$src" -o "$out" 2>&1 || { echo "  FAIL: $label"; return 1; }
            ;;
        cpp)
            g++ $CPP_FLAGS "$src" -o "$out" 2>&1 || { echo "  FAIL: $label"; return 1; }
            ;;
        zig)
            zig build-exe $ZIG_FLAGS -femit-bin="$out" "$src" 2>&1 || { echo "  FAIL: $label"; return 1; }
            ;;
    esac

    end=$(date +%s%N)
    elapsed=$(( (end - start) / 1000000 ))
    COMPILE_TIMES["$label"]="$elapsed"
    strip "$out" 2>/dev/null || true
    echo "  OK: $label (${elapsed}ms)"
}

echo "=== Compiling ==="

for bench_entry in $BENCHMARKS; do
    num="${bench_entry%%:*}"
    name="${bench_entry##*:}"
    echo "--- ${num}_${name} ---"

    if [ "$HAS_C" = 1 ] && [ -f "c/${num}_${name}.c" ]; then
        compile_one c "c/${num}_${name}.c" "$BUILD_DIR/${num}_${name}_c" "c_${num}" || true
    fi

    if [ "$HAS_CPP" = 1 ] && [ -f "cpp/${num}_${name}.cpp" ]; then
        compile_one cpp "cpp/${num}_${name}.cpp" "$BUILD_DIR/${num}_${name}_cpp" "cpp_${num}" || true
    fi

    if [ "$HAS_RUST" = 1 ] && [ -f "../rust_production/src/bin/${num}_${name}.rs" ]; then
        compile_one rust "../rust_production/src/bin/${num}_${name}.rs" "$BUILD_DIR/${num}_${name}_rust" "rust_${num}" || true
    fi

    if [ "$HAS_ZIG" = 1 ] && [ -f "zig/${num}_${name}.zig" ]; then
        compile_one zig "zig/${num}_${name}.zig" "$BUILD_DIR/${num}_${name}_zig" "zig_${num}" || true
    fi

    if [ "$HAS_SEEN" = 1 ] && [ -f "../production/${num}_${name}.seen" ]; then
        compile_one seen "../production/${num}_${name}.seen" "$BUILD_DIR/${num}_${name}_seen" "seen_${num}" || true
    fi
done

echo ""

# ─── Run benchmarks ─────────────────────────────────────────────────────────

declare -A RUN_TIMES CHECKSUMS BIN_SIZES PEAK_RSS

extract_metric() {
    local output="$1" pattern="$2"
    echo "$output" | grep -i "$pattern" | head -1 | grep -oP '[\-]?[0-9]+\.?[0-9]*' | head -1
}

extract_checksum() {
    local output="$1"
    # Try different checksum patterns
    local cs
    cs=$(echo "$output" | grep -iP '(checksum|total json length|total response length|sum of phi|total distance):' | head -1 | grep -oP '[\-]?[0-9]+\.?[0-9]*' | head -1)
    if [ -z "$cs" ]; then
        # For nbody, use final energy
        cs=$(echo "$output" | grep -i 'final energy' | head -1 | grep -oP '[\-]?[0-9]+\.?[0-9e\+\-]+' | head -1)
    fi
    if [ -z "$cs" ]; then
        # For spectral norm
        cs=$(echo "$output" | grep -i 'spectral norm' | head -1 | grep -oP '[0-9]+\.[0-9]+' | head -1)
    fi
    if [ -z "$cs" ]; then
        # For great circle / hyperbolic PDE
        cs=$(echo "$output" | grep -iP '(total distance|sum of phi):' | head -1 | grep -oP '[\-]?[0-9]+\.?[0-9]*' | head -1)
    fi
    echo "$cs"
}

run_one() {
    local binary="$1" label="$2"
    if [ ! -x "$binary" ]; then return 1; fi

    # Get binary size
    BIN_SIZES["$label"]=$(stat -c%s "$binary" 2>/dev/null || echo "0")

    # Run benchmark and measure RSS via /proc/PID/status VmHWM
    local output rss=0
    if [ -x /usr/bin/time ]; then
        # GNU time available — use it for RSS
        local time_file
        time_file=$(mktemp)
        output=$(/usr/bin/time -v "$binary" 2>"$time_file") || true
        rss=$(grep "Maximum resident" "$time_file" | grep -oP '[0-9]+' | head -1)
        rm -f "$time_file"
    else
        # Fallback: run in background, poll /proc/PID/status for VmHWM
        "$binary" > /tmp/_bench_out_$$ 2>&1 &
        local pid=$!
        local peak=0
        while kill -0 "$pid" 2>/dev/null; do
            local vm
            vm=$(awk '/VmHWM/{print $2}' /proc/$pid/status 2>/dev/null) || true
            if [ -n "$vm" ] && [ "$vm" -gt "$peak" ] 2>/dev/null; then peak=$vm; fi
            sleep 0.1
        done
        wait "$pid" 2>/dev/null || true
        output=$(cat /tmp/_bench_out_$$ 2>/dev/null) || true
        rm -f /tmp/_bench_out_$$
        rss=$peak
    fi
    PEAK_RSS["$label"]="${rss:-0}"

    # Extract min time
    local min_t
    min_t=$(extract_metric "$output" "min time")
    if [ -z "$min_t" ]; then
        # For nbody, extract "Time:"
        min_t=$(extract_metric "$output" "^Time:")
    fi
    RUN_TIMES["$label"]="${min_t:-N/A}"

    # Extract checksum
    CHECKSUMS["$label"]=$(extract_checksum "$output")

    echo "  $label: ${RUN_TIMES[$label]} ms (RSS: ${PEAK_RSS[$label]} KB, checksum: ${CHECKSUMS[$label]:-?})"
}

echo "=== Running benchmarks ==="

LANGS="c cpp rust zig seen"

for bench_entry in $BENCHMARKS; do
    num="${bench_entry%%:*}"
    name="${bench_entry##*:}"
    echo "--- ${num}_${name} ---"

    for lang in $LANGS; do
        [ -x "$BUILD_DIR/${num}_${name}_${lang}" ] && run_one "$BUILD_DIR/${num}_${name}_${lang}" "${lang}_${num}" || true
    done
done

echo ""

# ─── Checksum validation ────────────────────────────────────────────────────

echo "=== Checksum Validation ==="
CHECKSUM_OK=0
CHECKSUM_FAIL=0

for bench_entry in $BENCHMARKS; do
    num="${bench_entry%%:*}"
    name="${bench_entry##*:}"

    ref_cs=""
    ref_lang=""
    all_match=1

    for lang in $LANGS; do
        key="${lang}_${num}"
        cs="${CHECKSUMS[$key]:-}"
        if [ -n "$cs" ]; then
            if [ -z "$ref_cs" ]; then
                ref_cs="$cs"
                ref_lang="$lang"
            elif [ "$cs" != "$ref_cs" ]; then
                all_match=0
            fi
        fi
    done

    if [ "$all_match" = 1 ] && [ -n "$ref_cs" ]; then
        echo "  [OK] ${num}_${name}: $ref_cs"
        CHECKSUM_OK=$((CHECKSUM_OK + 1))
    elif [ -n "$ref_cs" ]; then
        echo "  [MISMATCH] ${num}_${name}:"
        for lang in $LANGS; do
            key="${lang}_${num}"
            cs="${CHECKSUMS[$key]:-N/A}"
            echo "    $lang: $cs"
        done
        CHECKSUM_FAIL=$((CHECKSUM_FAIL + 1))
    else
        echo "  [SKIP] ${num}_${name}: no results"
    fi
done

echo ""
echo "Checksums: $CHECKSUM_OK OK, $CHECKSUM_FAIL mismatched"
echo ""

# ─── Generate markdown report ───────────────────────────────────────────────

REPORT="$RESULTS_DIR/comparison_report.md"

cat > "$REPORT" << 'HEADER'
# Fair 5-Language Benchmark Comparison

All 17 benchmarks use identical algorithms and parameters across Seen, Rust, C (GCC), C++ (G++), and Zig.

## Compilation Flags

| Language | Compiler | Flags |
|----------|----------|-------|
| Seen | seen compile | Default (opt -O3, llc -O3 -mcpu=native, clang -O3 -flto -march=native) |
| Rust | rustc | -C opt-level=3 -C lto=fat -C target-cpu=native |
| C | gcc | -O3 -flto -march=native -lm |
| C++ | g++ | -O3 -flto -march=native -std=c++17 -lm |
| Zig | zig build-exe | -OReleaseFast |

HEADER

# Wall time table
{
    echo "## Wall Time (ms, min of measured iterations)"
    echo ""
    echo "| Benchmark | Seen | Rust | C (GCC) | C++ (G++) | Zig | Seen/C | Seen/Rust | Seen/Zig |"
    echo "|-----------|------|------|---------|-----------|-----|--------|-----------|----------|"

    for bench_entry in $BENCHMARKS; do
        num="${bench_entry%%:*}"
        name="${bench_entry##*:}"
        display_name=$(echo "$name" | tr '_' ' ' | sed 's/\b\(.\)/\u\1/g')

        seen_t="${RUN_TIMES[seen_${num}]:-N/A}"
        rust_t="${RUN_TIMES[rust_${num}]:-N/A}"
        c_t="${RUN_TIMES[c_${num}]:-N/A}"
        cpp_t="${RUN_TIMES[cpp_${num}]:-N/A}"
        zig_t="${RUN_TIMES[zig_${num}]:-N/A}"

        ratio_c="N/A"
        ratio_rust="N/A"
        ratio_zig="N/A"
        if [[ "$seen_t" != "N/A" && "$c_t" != "N/A" ]]; then
            ratio_c=$(awk "BEGIN{printf \"%.2f\", $seen_t / $c_t}" 2>/dev/null || echo "N/A")
        fi
        if [[ "$seen_t" != "N/A" && "$rust_t" != "N/A" ]]; then
            ratio_rust=$(awk "BEGIN{printf \"%.2f\", $seen_t / $rust_t}" 2>/dev/null || echo "N/A")
        fi
        if [[ "$seen_t" != "N/A" && "$zig_t" != "N/A" ]]; then
            ratio_zig=$(awk "BEGIN{printf \"%.2f\", $seen_t / $zig_t}" 2>/dev/null || echo "N/A")
        fi

        # Format times to 3 decimal places for readability
        fmt_seen=$([ "$seen_t" = "N/A" ] && echo "N/A" || awk "BEGIN{printf \"%.3f\", $seen_t}")
        fmt_rust=$([ "$rust_t" = "N/A" ] && echo "N/A" || awk "BEGIN{printf \"%.3f\", $rust_t}")
        fmt_c=$([ "$c_t" = "N/A" ] && echo "N/A" || awk "BEGIN{printf \"%.3f\", $c_t}")
        fmt_cpp=$([ "$cpp_t" = "N/A" ] && echo "N/A" || awk "BEGIN{printf \"%.3f\", $cpp_t}")
        fmt_zig=$([ "$zig_t" = "N/A" ] && echo "N/A" || awk "BEGIN{printf \"%.3f\", $zig_t}")

        echo "| ${display_name} | ${fmt_seen} | ${fmt_rust} | ${fmt_c} | ${fmt_cpp} | ${fmt_zig} | ${ratio_c}x | ${ratio_rust}x | ${ratio_zig}x |"
    done
    echo ""
} >> "$REPORT"

# Binary size table
{
    echo "## Binary Size (bytes, stripped)"
    echo ""
    echo "| Benchmark | Seen | Rust | C (GCC) | C++ (G++) | Zig |"
    echo "|-----------|------|------|---------|-----------|-----|"

    for bench_entry in $BENCHMARKS; do
        num="${bench_entry%%:*}"
        name="${bench_entry##*:}"
        display_name=$(echo "$name" | tr '_' ' ' | sed 's/\b\(.\)/\u\1/g')

        seen_s="${BIN_SIZES[seen_${num}]:-N/A}"
        rust_s="${BIN_SIZES[rust_${num}]:-N/A}"
        c_s="${BIN_SIZES[c_${num}]:-N/A}"
        cpp_s="${BIN_SIZES[cpp_${num}]:-N/A}"
        zig_s="${BIN_SIZES[zig_${num}]:-N/A}"

        echo "| ${display_name} | ${seen_s} | ${rust_s} | ${c_s} | ${cpp_s} | ${zig_s} |"
    done
    echo ""
} >> "$REPORT"

# Compile time table
{
    echo "## Compile Time (ms)"
    echo ""
    echo "| Benchmark | Seen | Rust | C (GCC) | C++ (G++) | Zig |"
    echo "|-----------|------|------|---------|-----------|-----|"

    for bench_entry in $BENCHMARKS; do
        num="${bench_entry%%:*}"
        name="${bench_entry##*:}"
        display_name=$(echo "$name" | tr '_' ' ' | sed 's/\b\(.\)/\u\1/g')

        seen_ct="${COMPILE_TIMES[seen_${num}]:-N/A}"
        rust_ct="${COMPILE_TIMES[rust_${num}]:-N/A}"
        c_ct="${COMPILE_TIMES[c_${num}]:-N/A}"
        cpp_ct="${COMPILE_TIMES[cpp_${num}]:-N/A}"
        zig_ct="${COMPILE_TIMES[zig_${num}]:-N/A}"

        echo "| ${display_name} | ${seen_ct} | ${rust_ct} | ${c_ct} | ${cpp_ct} | ${zig_ct} |"
    done
    echo ""
} >> "$REPORT"

# Peak RSS table
{
    echo "## Peak RSS (KB)"
    echo ""
    echo "| Benchmark | Seen | Rust | C (GCC) | C++ (G++) | Zig |"
    echo "|-----------|------|------|---------|-----------|-----|"

    for bench_entry in $BENCHMARKS; do
        num="${bench_entry%%:*}"
        name="${bench_entry##*:}"
        display_name=$(echo "$name" | tr '_' ' ' | sed 's/\b\(.\)/\u\1/g')

        seen_r="${PEAK_RSS[seen_${num}]:-N/A}"
        rust_r="${PEAK_RSS[rust_${num}]:-N/A}"
        c_r="${PEAK_RSS[c_${num}]:-N/A}"
        cpp_r="${PEAK_RSS[cpp_${num}]:-N/A}"
        zig_r="${PEAK_RSS[zig_${num}]:-N/A}"

        echo "| ${display_name} | ${seen_r} | ${rust_r} | ${c_r} | ${cpp_r} | ${zig_r} |"
    done
    echo ""
} >> "$REPORT"

# Caveats
cat >> "$REPORT" << 'CAVEATS'
## Caveats

1. **Pool allocator**: Seen uses a pool allocator for objects ≤80 bytes (advantage in Binary Trees)
2. **Bounds checking**: Seen has no runtime bounds checking (like C/C++, unlike Rust in debug mode; Rust release elides most checks)
3. **LRU hash map**: Rust uses SwissTable (`HashMap`), Seen uses linear probing, C uses manual open-addressing, C++ uses `std::unordered_map`, Zig uses `std.HashMap`
4. **Seen runtime**: ~2000 lines of pre-compiled C linked with Clang -O3; C/C++ benchmarks compiled with GCC
5. **DFT trig functions**: Seen uses inlined Taylor-series trig; other languages use libm/compiler intrinsics
6. **Zig compilation**: Uses -OReleaseFast (equivalent to -O3 with safety checks disabled)
CAVEATS

echo "=== Report written to $REPORT ==="
echo ""
cat "$REPORT"
