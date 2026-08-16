#!/bin/bash
# Production Benchmark Runner - Self-Hosted Seen Compiler
# Runs all 16 benchmarks from benchmarks/production/
# Outputs: markdown report + JSONL machine-readable results

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
SELF_PATH="$SCRIPT_DIR/$(basename -- "${BASH_SOURCE[0]}")"
ARTIFACT_ROOT_HELPER="$SCRIPT_DIR/artifact_root.sh"
ARTIFACT_WRAPPER="$SCRIPT_DIR/run_with_project_artifacts.sh"
HARD_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
BUILDER_CAPABILITY="$SCRIPT_DIR/rebuild_builder_capability.sh"
BUILDER_APPLICABILITY="$SCRIPT_DIR/rebuild_builder_applicability.sh"
SERIALIZER_VERIFY="$SCRIPT_DIR/verify_fork_serializer.sh"
BOUNDED_TOOLCHAIN_PREPARE="$SCRIPT_DIR/prepare_bounded_toolchain.sh"
ORIGINAL_ARGS=("$@")

if [ "$#" -ne 0 ]; then
    echo "ERROR: run_production_benchmarks.sh accepts no arguments" >&2
    exit 2
fi

is_positive_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

derive_main_kb() {
    local total=$1
    local available=$2
    local cap=$((total / 4))
    local available_cap=$((available / 2))
    if [ "$available_cap" -lt "$cap" ]; then cap=$available_cap; fi
    if [ "$cap" -gt 4194304 ]; then cap=4194304; fi
    [ "$cap" -gt 0 ] || cap=1
    printf '%s\n' "$cap"
}

derive_opt_kb() {
    local total=$1
    local main=$2
    local cap=$((total / 10))
    local half_main=$((main / 2))
    if [ "$half_main" -lt "$cap" ]; then cap=$half_main; fi
    if [ "$cap" -gt 2097152 ]; then cap=2097152; fi
    [ "$cap" -gt 0 ] || cap=1
    printf '%s\n' "$cap"
}

validate_benchmark_output_root() {
    local candidate=$1
    local relative_path
    local canonical_candidate

    case "$candidate" in
        "$REPO_ROOT"/*) relative_path=${candidate#"$REPO_ROOT"/} ;;
        *)
            echo "ERROR: benchmark output must stay inside the repository: $candidate" >&2
            return 1
            ;;
    esac
    seen_artifact_assert_safe_relative_path "$relative_path" || return 1
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" "$relative_path" || return 1
    if [ -L "$candidate" ] || { [ -e "$candidate" ] && [ ! -d "$candidate" ]; }; then
        echo "ERROR: unsafe benchmark output root: $candidate" >&2
        return 1
    fi
    if command -v git >/dev/null 2>&1 &&
        git -C "$REPO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then

        git -C "$REPO_ROOT" check-ignore -q -- "$relative_path" || {
            echo "ERROR: benchmark output root must be ignored by Git: $candidate" >&2
            return 1
        }
    else
        case "$relative_path" in
            .seen|.seen/*) ;;
            *)
                echo "ERROR: without Git, benchmark output must stay below .seen/: $candidate" >&2
                return 1
                ;;
        esac
    fi
    if [ -d "$candidate" ]; then
        canonical_candidate=$(seen_artifact_canonical_dir "$candidate") || return 1
        [ "$canonical_candidate" = "$candidate" ] || {
            echo "ERROR: benchmark output root is not canonical: $candidate" >&2
            return 1
        }
    fi
}

[ -f "$ARTIFACT_ROOT_HELPER" ] || {
    echo "ERROR: missing artifact-root helper: $ARTIFACT_ROOT_HELPER" >&2
    exit 1
}
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_HELPER"

total_kb=$(awk '/^MemTotal:/ { print $2; exit }' /proc/meminfo 2>/dev/null || true)
available_kb=$(awk '/^MemAvailable:/ { print $2; exit }' /proc/meminfo 2>/dev/null || true)
if ! is_positive_integer "$total_kb" || ! is_positive_integer "$available_kb"; then
    echo "ERROR: could not derive production-benchmark memory caps" >&2
    exit 1
fi
main_kb=${SEEN_MAIN_VMEM_KB:-$(derive_main_kb "$total_kb" "$available_kb")}
if ! is_positive_integer "$main_kb" || [ "$main_kb" -gt 4194304 ]; then
    echo "ERROR: production-benchmark main cap must be positive and at most 4 GiB" >&2
    exit 1
fi
opt_kb=${SEEN_OPT_VMEM_KB:-$(derive_opt_kb "$total_kb" "$main_kb")}
memory_limit_bytes=${SEEN_MEMORY_LIMIT_BYTES:-$((main_kb * 1024))}
if ! is_positive_integer "$opt_kb" || [ "$opt_kb" -gt 2097152 ] ||
    [ "$opt_kb" -gt "$main_kb" ] ||
    ! is_positive_integer "$memory_limit_bytes" ||
    [ "$memory_limit_bytes" -gt "$((main_kb * 1024))" ]; then

    echo "ERROR: inconsistent production-benchmark optimizer/allocation cap" >&2
    exit 1
fi
export SEEN_LOW_MEMORY=1 SEEN_JOBS=1 SEEN_OPT_JOBS=1
export SEEN_MAIN_VMEM_KB="$main_kb" SEEN_OPT_VMEM_KB="$opt_kb"
export SEEN_MEMORY_LIMIT_BYTES="$memory_limit_bytes"

seen_artifact_root_init "$REPO_ROOT" || exit 1
if [ -z "${SEEN_PRODUCTION_BENCH_OUTPUT_ROOT:-}" ]; then
    if [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" = "1" ] &&
        [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" = "1" ]; then

        SEEN_PRODUCTION_BENCH_OUTPUT_ROOT="$REPO_ROOT/.seen/agent-tools/production-benchmarks"
    else
        SEEN_PRODUCTION_BENCH_OUTPUT_ROOT="$SEEN_ARTIFACT_ROOT/production-benchmarks"
    fi
fi
validate_benchmark_output_root "$SEEN_PRODUCTION_BENCH_OUTPUT_ROOT" || exit 1
export SEEN_PRODUCTION_BENCH_OUTPUT_ROOT

if [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" != "1" ] ||
    [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ]; then

    [ -x "$ARTIFACT_WRAPPER" ] || {
        echo "ERROR: missing project-artifact wrapper: $ARTIFACT_WRAPPER" >&2
        exit 1
    }
    exec "$ARTIFACT_WRAPPER" production-benchmarks -- \
        "$SELF_PATH" "${ORIGINAL_ARGS[@]}"
fi

seen_artifact_root_init "$REPO_ROOT" || exit 1
validate_benchmark_output_root "$SEEN_PRODUCTION_BENCH_OUTPUT_ROOT" || exit 1
if [ "$(uname -s)" = "Linux" ]; then
    namespace_tmp_identity=$(stat -c '%d:%i' /tmp 2>/dev/null || true)
    artifact_root_identity=$(stat -c '%d:%i' "$SEEN_ARTIFACT_ROOT" 2>/dev/null || true)
    if [ -z "$namespace_tmp_identity" ] ||
        [ "$namespace_tmp_identity" != "$artifact_root_identity" ]; then

        echo "ERROR: production-benchmark artifact namespace validation failed" >&2
        exit 1
    fi
fi
cd -- "$REPO_ROOT"

if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then

    [ -x "$HARD_SCOPE_WRAPPER" ] || {
        echo "ERROR: missing hard-memory-scope wrapper: $HARD_SCOPE_WRAPPER" >&2
        exit 1
    }
    exec "$HARD_SCOPE_WRAPPER" --label "Seen production benchmarks" -- \
        "$SELF_PATH" "${ORIGINAL_ARGS[@]}"
fi
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
export SEEN_HARD_MEMORY_SCOPE_ACTIVE
"$HARD_SCOPE_WRAPPER" --label "Seen production benchmarks read-back" \
    --verify-only --
main_kb=$SEEN_MAIN_VMEM_KB
if ! ulimit -S -v "$main_kb" 2>/dev/null; then
    echo "ERROR: could not apply the production-benchmark address-space cap" >&2
    exit 126
fi
active_main_kb=$(ulimit -S -v 2>/dev/null || true)
if ! is_positive_integer "$active_main_kb" || [ "$active_main_kb" -gt "$main_kb" ]; then
    echo "ERROR: production-benchmark address-space cap read-back failed" >&2
    exit 126
fi

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

mkdir -p -- "$SEEN_PRODUCTION_BENCH_OUTPUT_ROOT"
validate_benchmark_output_root "$SEEN_PRODUCTION_BENCH_OUTPUT_ROOT" || exit 1
RESULTS_DIR="$SEEN_PRODUCTION_BENCH_OUTPUT_ROOT/results"
BENCH_BIN_ROOT="$SEEN_ARTIFACT_ROOT/production-benchmark-bin"
for benchmark_directory in "$RESULTS_DIR" "$BENCH_BIN_ROOT"; do
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" \
        "${benchmark_directory#"$REPO_ROOT"/}" || exit 1
done
mkdir -p -- "$RESULTS_DIR" "$BENCH_BIN_ROOT"
for benchmark_directory in "$RESULTS_DIR" "$BENCH_BIN_ROOT"; do
    [ -d "$benchmark_directory" ] && [ ! -L "$benchmark_directory" ] || {
        echo "ERROR: unsafe production-benchmark directory: $benchmark_directory" >&2
        exit 1
    }
    seen_artifact_assert_no_symlink_components "$REPO_ROOT" \
        "${benchmark_directory#"$REPO_ROOT"/}" || exit 1
done
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$RESULTS_DIR/production_comparison_$TIMESTAMP.md"
JSONL_FILE="$RESULTS_DIR/results_$TIMESTAMP.jsonl"
for result_file in "$REPORT_FILE" "$JSONL_FILE"; do
    if [ -L "$result_file" ] || { [ -e "$result_file" ] && [ ! -f "$result_file" ]; }; then
        echo "ERROR: unsafe production-benchmark result file: $result_file" >&2
        exit 1
    fi
    rm -f -- "$result_file"
done

SEEN_COMPILER="$REPO_ROOT/compiler_seen/target/seen"

echo -e "${BLUE}=== Production Benchmark Suite ===${NC}"

if [ ! -x "$SEEN_COMPILER" ] || [ -L "$SEEN_COMPILER" ]; then
    echo -e "${RED}ERROR: Self-hosted compiler not found at $SEEN_COMPILER${NC}"
    echo "Run ./scripts/safe_rebuild.sh to build the compiler first."
    exit 1
fi
FORK_SERIALIZER_SO=${SEEN_FORK_SERIALIZER_SO:-}
FORK_SERIALIZER_ATTESTATION=${SEEN_FORK_SERIALIZER_ATTESTATION:-}
if ! bash "$SERIALIZER_VERIFY" "$FORK_SERIALIZER_SO" \
    "$FORK_SERIALIZER_ATTESTATION" "$SEEN_ARTIFACT_ROOT" \
    "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" >/dev/null; then

    echo -e "${RED}ERROR: production benchmarks require the scope-attested serializer produced by safe_rebuild${NC}" >&2
    exit 126
fi
if ! SEEN_MEMORY_GUARD_IN_SCOPE=1 bash "$BUILDER_APPLICABILITY" \
    "$SEEN_COMPILER" "$FORK_SERIALIZER_SO" >/dev/null; then

    echo -e "${RED}ERROR: production compiler is not serializer-applicable${NC}" >&2
    exit 126
fi
BOUNDED_TOOLCHAIN_DIR=$(bash "$BOUNDED_TOOLCHAIN_PREPARE" "$SEEN_ARTIFACT_ROOT") ||
    exit 126
PATH="$BOUNDED_TOOLCHAIN_DIR:$PATH"
export PATH SEEN_BOUNDED_TOOLCHAIN_DIR="$BOUNDED_TOOLCHAIN_DIR"
compiler_capability_status=0
compiler_capability=$(env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
    -u SEEN_FORK_SERIALIZER_ROOT_PID \
    bash "$BUILDER_CAPABILITY" "$SEEN_COMPILER" 2>/dev/null) ||
    compiler_capability_status=$?
if [ "$compiler_capability_status" -ne 0 ]; then
    echo -e "${RED}ERROR: production compiler schema probe failed with status $compiler_capability_status${NC}" >&2
    exit 126
fi
case "$compiler_capability" in
    advertised-jobs) COMPILER_WORKER_FLAGS=(--jobs 1 --opt-jobs 1) ;;
    advertised-no-fork) COMPILER_WORKER_FLAGS=(--no-fork) ;;
    serializer-required) COMPILER_WORKER_FLAGS=() ;;
    *) echo -e "${RED}ERROR: production compiler schema probe failed${NC}" >&2; exit 126 ;;
esac

abort_on_resource_failure() {
    local status=$1
    local output=$2
    local label=$3
    case "$status" in
        124|125|126|137|143)
            echo "RESOURCE STOP: $label stopped with status $status" >&2
            exit "$status"
            ;;
    esac
    if [ "$status" -ne 0 ] && grep -Eiq \
        '(^|[^[:alnum:]_])(resource stop:|out of memory|cannot allocate memory|could not allocate memory|memory allocation (failed|failure)|allocation failure|std::bad_alloc|bad_alloc|resource temporarily unavailable|cannot fork|can.t fork|fork: retry|fork (failed|failure)|pthread_create([^[:alnum:]_].*)?(failed|failure)|failed to create (a )?thread|can.t create (a )?thread|cannot create (a )?thread|thread creation (failed|failure))([^[:alnum:]_]|$)' \
        <<<"$output"; then

        echo "RESOURCE STOP: $label reported a resource failure" >&2
        exit 126
    fi
}

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
    OUTPUT_BIN="$BENCH_BIN_ROOT/bench_${num}"
    if [ -L "$OUTPUT_BIN" ] || { [ -e "$OUTPUT_BIN" ] && [ ! -f "$OUTPUT_BIN" ]; }; then
        echo "ERROR: unsafe production-benchmark binary path: $OUTPUT_BIN" >&2
        exit 1
    fi
    rm -f -- "$OUTPUT_BIN"

    if [ ! -f "$SEEN_FILE" ]; then
        echo -e "${RED}ERROR: $SEEN_FILE not found${NC}"
        echo "| $num | $display_name | N/A | N/A | N/A | N/A | Missing |" >> "$REPORT_FILE"
        echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":null,\"compile_ms\":null,\"binary_kb\":null,\"peak_rss_kb\":null,\"status\":\"missing\"}" >> "$JSONL_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    if [ "${SEEN_BENCH_COLD_CACHE:-0}" = "1" ]; then
        rm -rf .seen_cache/
    fi

    # Measure compile time
    echo "Compiling..."
    COMPILE_START=$(date +%s%N)
    COMPILE_STATUS=0
    COMPILE_OUTPUT=$(env -u SEEN_FORK_SERIALIZER_ROOT_PID \
        LD_PRELOAD="$FORK_SERIALIZER_SO" \
        SEEN_FORK_SERIALIZER_TARGET="$SEEN_COMPILER" \
        "$SEEN_COMPILER" compile "$SEEN_FILE" "$OUTPUT_BIN" \
        "${COMPILER_WORKER_FLAGS[@]}" 2>&1) || COMPILE_STATUS=$?
    COMPILE_END=$(date +%s%N)
    COMPILE_MS=$(( (COMPILE_END - COMPILE_START) / 1000000 ))
    abort_on_resource_failure "$COMPILE_STATUS" "$COMPILE_OUTPUT" \
        "production compile $file_name"

    if [ "$COMPILE_STATUS" -ne 0 ] ||
        ! echo "$COMPILE_OUTPUT" | tail -1 | grep -q "Build succeeded"; then
        echo -e "${RED}Compilation failed${NC}"
        echo "$COMPILE_OUTPUT" | tail -5
        echo "| $num | $display_name | N/A | $COMPILE_MS | N/A | N/A | Compile Error |" >> "$REPORT_FILE"
        echo "{\"name\":\"$display_name\",\"file\":\"$file_name\",\"runtime_ms\":null,\"compile_ms\":$COMPILE_MS,\"binary_kb\":null,\"peak_rss_kb\":null,\"status\":\"compile_error\"}" >> "$JSONL_FILE"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    if [ ! -f "$OUTPUT_BIN" ] || [ -L "$OUTPUT_BIN" ]; then
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
    RUN_STATUS=0
    if $HAS_GNU_TIME; then
        TIME_OUTPUT=$(/usr/bin/time -v timeout 120 "$OUTPUT_BIN" 2>&1) || RUN_STATUS=$?
        PEAK_RSS=$(echo "$TIME_OUTPUT" | grep "Maximum resident set size" | grep -oP '\d+' || echo "N/A")
        OUTPUT=$(echo "$TIME_OUTPUT" | sed '/Command being timed/,$d')
    else
        OUTPUT=$(timeout 120 "$OUTPUT_BIN" 2>&1) || RUN_STATUS=$?
    fi
    abort_on_resource_failure "$RUN_STATUS" "${TIME_OUTPUT:-$OUTPUT}" \
        "production runtime $file_name"
    if [ "$RUN_STATUS" -eq 0 ]; then RUN_OK=true; else RUN_OK=false; fi

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
        EXIT_CODE=$RUN_STATUS
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
