#!/usr/bin/env bash
# Record and compare Seen build-performance baselines.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_TRACE_COMMON="$SCRIPT_DIR/build_trace_common.sh"

# shellcheck source=scripts/build_trace_common.sh
source "$BUILD_TRACE_COMMON"
seen_build_trace_init "build_perf_gate"

BASELINE_FILE="$ROOT_DIR/target/seen-build/build-perf-baseline.env"
TRACE_DIR="$ROOT_DIR/target/seen-build/traces"
MODE=""
TIER="${SEEN_PERF_GATE_TIER:-quick}"

usage() {
    echo "Usage: $0 --record-baseline|--compare [--tier quick|verify|full]"
    echo ""
    echo "Records/compares build duration, trace path, compiler size, and cache counters"
    echo "using memory caps derived from current system memory."
}

detect_memory_kb() {
    awk '/^MemTotal:/ {print $2; exit}' /proc/meminfo 2>/dev/null || echo ""
}

detect_available_kb() {
    awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo 2>/dev/null || echo ""
}

derive_main_kb() {
    local total="$1" avail="$2" cap
    cap=$((total * 25 / 100))
    if seen_build_positive_integer "$avail"; then
        local avail_cap=$((avail * 50 / 100))
        if [ "$avail_cap" -gt 0 ] && [ "$avail_cap" -lt "$cap" ]; then
            cap="$avail_cap"
        fi
    fi
    if [ "$cap" -gt 10485760 ]; then
        cap=10485760
    fi
    if [ "$cap" -lt 1 ]; then
        cap=1
    fi
    printf '%s\n' "$cap"
}

derive_opt_kb() {
    local total="$1" main="$2" cap half_main
    cap=$((total * 10 / 100))
    half_main=$((main / 2))
    if [ "$half_main" -gt 0 ] && [ "$half_main" -lt "$cap" ]; then
        cap="$half_main"
    fi
    if [ "$cap" -gt 2097152 ]; then
        cap=2097152
    fi
    if [ "$cap" -lt 1 ]; then
        cap=1
    fi
    printf '%s\n' "$cap"
}

trace_count_status() {
    local trace="$1"
    local status="$2"
    grep -c "\"status\":\"$status\"" "$trace" 2>/dev/null || true
}

trace_sum_detail_field() {
    local trace="$1"
    local key="$2"
    awk -v key="$key" '
        {
            n = split($0, parts, /[^A-Za-z0-9_=-]+/)
            for (i = 1; i <= n; i++) {
                prefix = key "="
                if (substr(parts[i], 1, length(prefix)) == prefix) {
                    value = substr(parts[i], length(prefix) + 1)
                    if (value ~ /^[0-9]+$/) {
                        total += value
                    }
                }
            }
        }
        END { print total + 0 }
    ' "$trace" 2>/dev/null || echo 0
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --record-baseline) MODE="record"; shift ;;
        --compare) MODE="compare"; shift ;;
        --tier) TIER="$2"; shift 2 ;;
        --tier=*) TIER="${1#--tier=}"; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
    esac
done

if [ -z "$MODE" ]; then
    usage >&2
    exit 1
fi
case "$TIER" in
    quick|verify|full) ;;
    *) echo "Invalid tier: $TIER" >&2; exit 1 ;;
esac

total_kb=$(detect_memory_kb)
avail_kb=$(detect_available_kb)
if ! seen_build_positive_integer "$total_kb"; then
    echo "Error: could not derive memory caps from /proc/meminfo." >&2
    exit 1
fi
main_kb="${SEEN_MAIN_VMEM_KB:-$(derive_main_kb "$total_kb" "$avail_kb")}"
opt_kb="${SEEN_OPT_VMEM_KB:-$(derive_opt_kb "$total_kb" "$main_kb")}"

mkdir -p "$(dirname "$BASELINE_FILE")" "$TRACE_DIR"
trace_file="$TRACE_DIR/${MODE}-${TIER}-$(date +%Y%m%d%H%M%S).jsonl"

start_ms=$(seen_build_now_ms)
SEEN_TRACE_BUILD="$trace_file" \
SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-1}" \
SEEN_MAIN_VMEM_KB="$main_kb" \
SEEN_OPT_VMEM_KB="$opt_kb" \
SEEN_MEMORY_LIMIT_BYTES="$((main_kb * 1024))" \
    "$SCRIPT_DIR/safe_rebuild.sh" --tier "$TIER"
end_ms=$(seen_build_now_ms)
duration_ms=$((end_ms - start_ms))

compiler_path="$ROOT_DIR/compiler_seen/target/seen"
if [ "$TIER" = "quick" ]; then
    compiler_path="$ROOT_DIR/compiler_seen/target/seen-dev"
fi
compiler_size=0
if [ -f "$compiler_path" ]; then
    compiler_size=$(stat -c%s "$compiler_path" 2>/dev/null || stat -f%z "$compiler_path" 2>/dev/null || echo 0)
fi
status_cache_hits=$(trace_count_status "$trace_file" "hit")
status_cache_misses=$(trace_count_status "$trace_file" "miss")
module_cache_hits=$(trace_sum_detail_field "$trace_file" "cached")
module_cache_misses=$(trace_sum_detail_field "$trace_file" "uncached")
cache_hits=$((status_cache_hits + module_cache_hits))
cache_misses=$((status_cache_misses + module_cache_misses))

if [ "$MODE" = "record" ]; then
    {
        printf 'baseline_version=1\n'
        printf 'tier=%s\n' "$TIER"
        printf 'duration_ms=%s\n' "$duration_ms"
        printf 'compiler_size=%s\n' "$compiler_size"
        printf 'cache_hits=%s\n' "$cache_hits"
        printf 'cache_misses=%s\n' "$cache_misses"
        printf 'status_cache_hits=%s\n' "$status_cache_hits"
        printf 'status_cache_misses=%s\n' "$status_cache_misses"
        printf 'module_cache_hits=%s\n' "$module_cache_hits"
        printf 'module_cache_misses=%s\n' "$module_cache_misses"
        printf 'trace=%s\n' "$trace_file"
        printf 'recorded_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    } > "$BASELINE_FILE"
    echo "Recorded build baseline: $BASELINE_FILE"
else
    if [ ! -f "$BASELINE_FILE" ]; then
        echo "Error: no baseline at $BASELINE_FILE. Run --record-baseline first." >&2
        exit 1
    fi
    baseline_recorded_duration=$(awk -F= '/^duration_ms=/ {print $2; exit}' "$BASELINE_FILE")
    if ! seen_build_positive_integer "$baseline_recorded_duration"; then
        echo "Error: invalid baseline duration in $BASELINE_FILE" >&2
        exit 1
    fi
    echo "Current duration:  ${duration_ms} ms"
    echo "Baseline duration: ${baseline_recorded_duration} ms"
    echo "Current compiler:  ${compiler_size} bytes"
    echo "Cache hits:        ${cache_hits} (${module_cache_hits} module, ${status_cache_hits} status)"
    echo "Cache misses:      ${cache_misses} (${module_cache_misses} module, ${status_cache_misses} status)"
    echo "Trace:             $trace_file"
    if [ "$duration_ms" -gt $((baseline_recorded_duration * 125 / 100)) ]; then
        echo "Error: build duration regressed by more than 25%." >&2
        exit 1
    fi
fi
