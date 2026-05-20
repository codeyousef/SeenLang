#!/usr/bin/env bash
# Capped Seen performance gate for build, benchmark, release, and package checks.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_TRACE_COMMON="$SCRIPT_DIR/build_trace_common.sh"
MEMORY_GUARD_SCRIPT="$SCRIPT_DIR/memory_guard.sh"

# shellcheck source=scripts/build_trace_common.sh
source "$BUILD_TRACE_COMMON"
seen_build_trace_init "perf_gate"

MODE=""
SUITE="${SEEN_PERF_GATE_SUITE:-build}"
TIER="${SEEN_PERF_GATE_TIER:-quick}"
BENCH="${SEEN_PERF_GATE_BENCH:-}"
VERSION="${SEEN_VERSION:-0.9.0-build}"
BASELINE_ROOT="$ROOT_DIR/target/seen-build/perf-baselines"
TRACE_DIR="$ROOT_DIR/target/seen-build/traces"
RESULT_DIR="$ROOT_DIR/target/seen-build/perf-results"

usage() {
    cat >&2 <<'EOF'
Usage: perf_gate.sh --record-baseline|--compare [options]

Options:
  --suite build|stdlib|runtime|release-lto|packages
  --tier quick|verify|full        Build-suite tier (default: quick)
  --bench NAME[,NAME...]          Benchmark subset for stdlib/runtime suites
  --version VERSION               Package/release version label
  -h, --help

All commands derive memory caps from current system memory unless explicit
SEEN_MAIN_VMEM_KB / SEEN_OPT_VMEM_KB values are provided.
EOF
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

trace_max_detail_field() {
    local trace="$1"
    local key="$2"
    awk -v key="$key" '
        {
            n = split($0, parts, /[^A-Za-z0-9_=-]+/)
            for (i = 1; i <= n; i++) {
                prefix = key "="
                if (substr(parts[i], 1, length(prefix)) == prefix) {
                    value = substr(parts[i], length(prefix) + 1)
                    numeric_value = value + 0
                    if (value ~ /^[0-9]+$/ && numeric_value > max) {
                        max = numeric_value
                    }
                }
            }
        }
        END { print max + 0 }
    ' "$trace" 2>/dev/null || echo 0
}

safe_name() {
    printf '%s' "$1" | tr -c 'A-Za-z0-9_.=-' '_'
}

baseline_path() {
    local suite="$1"
    local name="$2"
    printf '%s/%s/%s.env\n' "$BASELINE_ROOT" "$(safe_name "$suite")" "$(safe_name "$name")"
}

read_metric() {
    local file="$1"
    local key="$2"
    awk -F= -v key="$key" '$1 == key { print substr($0, length(key) + 2); exit }' "$file" 2>/dev/null
}

write_metrics_file() {
    local file="$1"
    local suite="$2"
    local name="$3"
    shift 3

    mkdir -p "$(dirname "$file")"
    {
        printf 'baseline_version=2\n'
        printf 'suite=%s\n' "$suite"
        printf 'name=%s\n' "$name"
        printf 'recorded_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        printf 'branch=%s\n' "$(git -C "$ROOT_DIR" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
        printf 'commit=%s\n' "$(git -C "$ROOT_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
        while [ "$#" -gt 0 ]; do
            printf '%s=%s\n' "$1" "$2"
            shift 2
        done
    } > "$file"
}

fail_regression() {
    local suite="$1"
    local name="$2"
    local metric="$3"
    local threshold="$4"
    local observed="$5"
    local next_step="$6"

    echo "Error: ${suite}/${name} regression in ${metric}; threshold=${threshold}, observed=${observed}. Next: ${next_step}" >&2
    exit 1
}

compare_max_percent() {
    local suite="$1"
    local name="$2"
    local metric="$3"
    local current="$4"
    local baseline="$5"
    local percent="$6"
    local next_step="$7"

    if ! seen_build_positive_integer "$baseline" || [ "$baseline" -eq 0 ]; then
        return 0
    fi
    local threshold=$((baseline * percent / 100))
    if [ "$threshold" -lt 1 ]; then
        threshold=1
    fi
    if [ "$current" -gt "$threshold" ]; then
        fail_regression "$suite" "$name" "$metric" "<=${threshold}" "$current" "$next_step"
    fi
}

compare_min_percent() {
    local suite="$1"
    local name="$2"
    local metric="$3"
    local current="$4"
    local baseline="$5"
    local percent="$6"
    local next_step="$7"

    if ! seen_build_positive_integer "$baseline" || [ "$baseline" -eq 0 ]; then
        return 0
    fi
    local threshold=$((baseline * percent / 100))
    if [ "$current" -lt "$threshold" ]; then
        fail_regression "$suite" "$name" "$metric" ">=$threshold" "$current" "$next_step"
    fi
}

record_or_compare() {
    local suite="$1"
    local name="$2"
    shift 2
    local file
    file=$(baseline_path "$suite" "$name")

    if [ "$MODE" = "record" ]; then
        write_metrics_file "$file" "$suite" "$name" "$@"
        echo "Recorded ${suite}/${name} baseline: $file"
        return 0
    fi

    if [ ! -f "$file" ]; then
        echo "Error: no baseline for ${suite}/${name} at $file. Run --record-baseline first." >&2
        exit 1
    fi

    local current_duration="" current_compile="" current_run="" current_peak="" current_cache_rate=""
    local current_compiler_size="" current_binary_size=""
    local key value
    while [ "$#" -gt 0 ]; do
        key="$1"
        value="$2"
        case "$key" in
            duration_ms) current_duration="$value" ;;
            compile_ms) current_compile="$value" ;;
            run_ms) current_run="$value" ;;
            peak_rss_kb) current_peak="$value" ;;
            cache_hit_rate) current_cache_rate="$value" ;;
            compiler_size|compiler_size_bytes) current_compiler_size="$value" ;;
            binary_size|binary_size_bytes) current_binary_size="$value" ;;
        esac
        shift 2
    done

    echo "${suite}/${name}:"
    if [ -n "$current_duration" ]; then echo "  duration_ms:       $current_duration"; fi
    if [ -n "$current_compile" ]; then echo "  compile_ms:        $current_compile"; fi
    if [ -n "$current_run" ]; then echo "  run_ms:            $current_run"; fi
    if [ -n "$current_peak" ]; then echo "  peak_rss_kb:       $current_peak"; fi
    if [ -n "$current_cache_rate" ]; then echo "  cache_hit_rate:    $current_cache_rate"; fi
    if [ -n "$current_compiler_size" ]; then echo "  compiler_size:     $current_compiler_size"; fi
    if [ -n "$current_binary_size" ]; then echo "  binary_size:       $current_binary_size"; fi
    echo "  baseline:          $file"

    local max_duration_percent="${SEEN_PERF_GATE_MAX_DURATION_PERCENT:-125}"
    local max_memory_percent="${SEEN_PERF_GATE_MAX_MEMORY_PERCENT:-125}"
    local max_size_percent="${SEEN_PERF_GATE_MAX_SIZE_PERCENT:-115}"
    local min_cache_percent="${SEEN_PERF_GATE_MIN_CACHE_RATE_PERCENT:-90}"
    local next_step="inspect trace/logs, confirm cache correctness, then re-record baseline only after intentional improvement"

    if [ -n "$current_duration" ]; then
        compare_max_percent "$suite" "$name" duration_ms "$current_duration" "$(read_metric "$file" duration_ms)" "$max_duration_percent" "$next_step"
    fi
    if [ -n "$current_compile" ]; then
        compare_max_percent "$suite" "$name" compile_ms "$current_compile" "$(read_metric "$file" compile_ms)" "$max_duration_percent" "$next_step"
    fi
    if [ -n "$current_run" ]; then
        compare_max_percent "$suite" "$name" run_ms "$current_run" "$(read_metric "$file" run_ms)" "$max_duration_percent" "$next_step"
    fi
    if [ -n "$current_peak" ]; then
        compare_max_percent "$suite" "$name" peak_rss_kb "$current_peak" "$(read_metric "$file" peak_rss_kb)" "$max_memory_percent" "$next_step"
    fi
    if [ -n "$current_compiler_size" ]; then
        compare_max_percent "$suite" "$name" compiler_size "$current_compiler_size" "$(read_metric "$file" compiler_size)" "$max_size_percent" "$next_step"
    fi
    if [ -n "$current_binary_size" ]; then
        compare_max_percent "$suite" "$name" binary_size "$current_binary_size" "$(read_metric "$file" binary_size)" "$max_size_percent" "$next_step"
    fi
    if [ -n "$current_cache_rate" ]; then
        compare_min_percent "$suite" "$name" cache_hit_rate "$current_cache_rate" "$(read_metric "$file" cache_hit_rate)" "$min_cache_percent" "$next_step"
    fi
}

compiler_for_tier() {
    case "$TIER" in
        quick)
            printf '%s\n' "$ROOT_DIR/compiler_seen/target/seen-dev"
            ;;
        *)
            printf '%s\n' "$ROOT_DIR/compiler_seen/target/seen"
            ;;
    esac
}

find_benchmark_compiler() {
    local candidate
    if [ "${SEEN_BENCH_USE_DEV:-0}" = "1" ] && [ -x "$ROOT_DIR/compiler_seen/target/seen-dev" ]; then
        printf '%s\n' "$ROOT_DIR/compiler_seen/target/seen-dev"
        return 0
    fi
    for candidate in \
        "$ROOT_DIR/compiler_seen/target/seen" \
        "$ROOT_DIR/compiler_seen/target/seen-dev" \
        "$ROOT_DIR/target/release/seen"; do
        if [ -x "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    echo "Error: no Seen compiler found for benchmark gate." >&2
    return 1
}

run_build_suite() {
    local trace_file compiler_path compiler_size status_cache_hits status_cache_misses
    local module_cache_hits module_cache_misses cache_hits cache_misses cache_total cache_hit_rate
    local peak_rss start_ms end_ms duration_ms

    mkdir -p "$TRACE_DIR"
    trace_file="$TRACE_DIR/${MODE}-${SUITE}-${TIER}-$(date +%Y%m%d%H%M%S).jsonl"
    start_ms=$(seen_build_now_ms)
    SEEN_TRACE_BUILD="$trace_file" \
    SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-1}" \
    SEEN_MAIN_VMEM_KB="$main_kb" \
    SEEN_OPT_VMEM_KB="$opt_kb" \
    SEEN_MEMORY_LIMIT_BYTES="$((main_kb * 1024))" \
        "$SCRIPT_DIR/safe_rebuild.sh" --tier "$TIER"
    end_ms=$(seen_build_now_ms)
    duration_ms=$((end_ms - start_ms))

    compiler_path=$(compiler_for_tier)
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
    cache_total=$((cache_hits + cache_misses))
    cache_hit_rate=0
    if [ "$cache_total" -gt 0 ]; then
        cache_hit_rate=$((cache_hits * 100 / cache_total))
    fi
    peak_rss=$(trace_max_detail_field "$trace_file" "peak_rss_kb")

    record_or_compare build "$TIER" \
        tier "$TIER" \
        duration_ms "$duration_ms" \
        peak_rss_kb "$peak_rss" \
        compiler_size "$compiler_size" \
        cache_hits "$cache_hits" \
        cache_misses "$cache_misses" \
        cache_hit_rate "$cache_hit_rate" \
        status_cache_hits "$status_cache_hits" \
        status_cache_misses "$status_cache_misses" \
        module_cache_hits "$module_cache_hits" \
        module_cache_misses "$module_cache_misses" \
        trace "$trace_file"
}

bench_list_for_suite() {
    case "$1" in
        stdlib)
            printf '%s\n' collections string_json math_sort
            ;;
        runtime)
            printf '%s\n' allocation simd
            ;;
        *)
            return 1
            ;;
    esac
}

bench_path() {
    local suite="$1"
    local bench="$2"
    printf '%s/benchmarks/gates/%s_%s.seen\n' "$ROOT_DIR" "$suite" "$bench"
}

run_guarded_gate_command() {
    local label="$1"
    local timeout_secs="$2"
    local metrics_file="$3"
    shift 3

    "$MEMORY_GUARD_SCRIPT" \
        --label "$label" \
        --rss-limit-kb "$main_kb" \
        --available-reserve-kb "$reserve_kb" \
        --vmem-limit-kb "$main_kb" \
        --timeout-secs "$timeout_secs" \
        --metrics-file "$metrics_file" \
        -- "$@"
}

metric_value() {
    local file="$1"
    local key="$2"
    awk -F= -v key="$key" '$1 == key { print $2; exit }' "$file" 2>/dev/null || echo 0
}

run_one_benchmark() {
    local suite="$1"
    local bench="$2"
    local fixture compiler bin metrics_compile metrics_run compile_start compile_end run_start run_end
    local compile_ms run_ms compile_peak run_peak binary_size

    fixture=$(bench_path "$suite" "$bench")
    if [ ! -f "$fixture" ]; then
        echo "Error: missing benchmark fixture $fixture" >&2
        exit 1
    fi
    compiler=$(find_benchmark_compiler)
    mkdir -p "$RESULT_DIR/bin" "$RESULT_DIR/metrics"
    bin="$RESULT_DIR/bin/${suite}-${bench}"
    metrics_compile="$RESULT_DIR/metrics/${suite}-${bench}-compile.env"
    metrics_run="$RESULT_DIR/metrics/${suite}-${bench}-run.env"
    rm -f "$bin" "$metrics_compile" "$metrics_run"

    compile_start=$(seen_build_now_ms)
    run_guarded_gate_command "${suite}/${bench} compile" 300 "$metrics_compile" \
        "$compiler" compile "$fixture" "$bin" --fast
    compile_end=$(seen_build_now_ms)
    compile_ms=$((compile_end - compile_start))

    if [ ! -x "$bin" ]; then
        chmod +x "$bin" 2>/dev/null || true
    fi
    if [ ! -x "$bin" ]; then
        echo "Error: benchmark ${suite}/${bench} did not produce executable $bin" >&2
        exit 1
    fi

    run_start=$(seen_build_now_ms)
    run_guarded_gate_command "${suite}/${bench} run" 120 "$metrics_run" "$bin" >/dev/null
    run_end=$(seen_build_now_ms)
    run_ms=$((run_end - run_start))

    compile_peak=$(metric_value "$metrics_compile" peak_rss_kb)
    run_peak=$(metric_value "$metrics_run" peak_rss_kb)
    binary_size=$(stat -c%s "$bin" 2>/dev/null || stat -f%z "$bin" 2>/dev/null || echo 0)

    record_or_compare "$suite" "$bench" \
        compile_ms "$compile_ms" \
        compile_peak_rss_kb "$compile_peak" \
        run_ms "$run_ms" \
        run_peak_rss_kb "$run_peak" \
        peak_rss_kb "$compile_peak" \
        binary_size "$binary_size" \
        compiler "$compiler" \
        fixture "$fixture"
}

split_benches() {
    local value="$1"
    if [ -z "$value" ]; then
        bench_list_for_suite "$SUITE"
    else
        printf '%s\n' "$value" | tr ',' '\n'
    fi
}

run_benchmark_suite() {
    local bench
    while IFS= read -r bench; do
        [ -n "$bench" ] || continue
        run_one_benchmark "$SUITE" "$bench"
    done < <(split_benches "$BENCH")
}

run_release_lto_suite() {
    run_release_lto_variant default merged
    run_release_lto_variant optout optout --no-merged-release-lto
}

run_packages_suite() {
    local compiler output_dir trace_file metrics_file start_ms end_ms duration_ms peak_rss
    local artifact_hits artifact_misses artifact_reused package_bytes

    compiler=$(find_benchmark_compiler)
    output_dir="$RESULT_DIR/package-dist"
    trace_file="$TRACE_DIR/${MODE}-packages-linux-$(date +%Y%m%d%H%M%S).jsonl"
    metrics_file="$RESULT_DIR/metrics/packages-linux.env"
    mkdir -p "$output_dir" "$RESULT_DIR/metrics" "$TRACE_DIR"
    rm -f "$metrics_file" "$trace_file"

    start_ms=$(seen_build_now_ms)
    run_guarded_gate_command "packages/linux" 300 "$metrics_file" \
        env \
        SEEN_BUILD_TRACE="$trace_file" \
        SEEN_TRACE_BUILD="$trace_file" \
        SEEN_RELEASE_SKIP_VERIFY=1 \
        "$SCRIPT_DIR/build_release.sh" \
        --version "$VERSION" \
        --output-dir "$output_dir" \
        --compiler "$compiler" \
        --cpu-baseline x86-64 \
        --artifact-suffix linux-x64 \
        --skip-verify
    end_ms=$(seen_build_now_ms)
    duration_ms=$((end_ms - start_ms))
    peak_rss=$(metric_value "$metrics_file" peak_rss_kb)
    artifact_hits=$(trace_count_status "$trace_file" "hit")
    artifact_misses=$(trace_count_status "$trace_file" "miss")
    artifact_reused=0
    if grep -q '"phase":"package artifact cache","status":"hit"' "$trace_file" 2>/dev/null; then
        artifact_reused=1
    fi
    package_bytes=$(find "$output_dir" -maxdepth 1 -type f \( -name '*.tar.gz' -o -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -printf '%s\n' 2>/dev/null | awk '{total += $1} END {print total + 0}')

    record_or_compare packages linux \
        duration_ms "$duration_ms" \
        peak_rss_kb "$peak_rss" \
        package_bytes "$package_bytes" \
        cache_hits "$artifact_hits" \
        cache_misses "$artifact_misses" \
        package_artifact_reused "$artifact_reused" \
        compiler "$compiler" \
        trace "$trace_file"
}

trace_release_lto_status() {
    local trace="$1"
    grep '"phase":"release lto mode"' "$trace" 2>/dev/null | tail -1 || true
}

run_release_lto_variant() {
    local name="$1"
    local expected="$2"
    shift 2
    local compiler fixture bin trace_file metrics_file start_ms end_ms duration_ms peak_rss binary_size status_line

    compiler=$(find_benchmark_compiler)
    fixture="$ROOT_DIR/benchmarks/gates/release_lto_entry.seen"
    if [ ! -f "$fixture" ]; then
        echo "Error: missing release LTO fixture $fixture" >&2
        exit 1
    fi
    mkdir -p "$RESULT_DIR/bin" "$RESULT_DIR/metrics" "$TRACE_DIR"
    bin="$RESULT_DIR/bin/release-lto-${name}"
    trace_file="$TRACE_DIR/${MODE}-release-lto-${name}-$(date +%Y%m%d%H%M%S).jsonl"
    metrics_file="$RESULT_DIR/metrics/release-lto-${name}-compile.env"
    rm -f "$bin" "$trace_file" "$metrics_file"

    start_ms=$(seen_build_now_ms)
    run_guarded_gate_command "release-lto/${name} compile" 300 "$metrics_file" \
        env SEEN_BUILD_TRACE="$trace_file" SEEN_TRACE_BUILD="$trace_file" \
        "$compiler" compile "$fixture" "$bin" --release --no-cache "$@"
    end_ms=$(seen_build_now_ms)
    duration_ms=$((end_ms - start_ms))
    peak_rss=$(metric_value "$metrics_file" peak_rss_kb)
    binary_size=$(stat -c%s "$bin" 2>/dev/null || stat -f%z "$bin" 2>/dev/null || echo 0)
    status_line=$(trace_release_lto_status "$trace_file")

    if [ "$expected" = "merged" ]; then
        if ! printf '%s\n' "$status_line" | grep -q '"status":"merged"'; then
            echo "Error: release-lto/default expected merged LTO, observed: ${status_line:-none}" >&2
            exit 1
        fi
    else
        if ! printf '%s\n' "$status_line" | grep -q '"status":"optout"'; then
            echo "Error: release-lto/optout expected explicit opt-out, observed: ${status_line:-none}" >&2
            exit 1
        fi
    fi

    if [ -x "$bin" ]; then
        "$bin" >/dev/null
    else
        echo "Error: release-lto/${name} did not produce executable $bin" >&2
        exit 1
    fi

    record_or_compare release-lto "$name" \
        duration_ms "$duration_ms" \
        peak_rss_kb "$peak_rss" \
        binary_size "$binary_size" \
        release_lto_mode "$expected" \
        compiler "$compiler" \
        fixture "$fixture" \
        trace "$trace_file"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --record-baseline) MODE="record"; shift ;;
        --compare) MODE="compare"; shift ;;
        --suite) SUITE="${2:-}"; shift 2 ;;
        --suite=*) SUITE="${1#--suite=}"; shift ;;
        --tier) TIER="${2:-}"; shift 2 ;;
        --tier=*) TIER="${1#--tier=}"; shift ;;
        --bench) BENCH="${2:-}"; shift 2 ;;
        --bench=*) BENCH="${1#--bench=}"; shift ;;
        --version) VERSION="${2:-}"; shift 2 ;;
        --version=*) VERSION="${1#--version=}"; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
    esac
done

if [ -z "$MODE" ]; then
    usage
    exit 1
fi
case "$SUITE" in
    build|stdlib|runtime|release-lto|packages) ;;
    *) echo "Invalid suite: $SUITE" >&2; exit 1 ;;
esac
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
reserve_kb="${SEEN_MEMORY_GUARD_RESERVE_KB:-$((total_kb / 10))}"

mkdir -p "$BASELINE_ROOT" "$TRACE_DIR" "$RESULT_DIR"

case "$SUITE" in
    build)
        run_build_suite
        ;;
    stdlib|runtime)
        run_benchmark_suite
        ;;
    release-lto)
        run_release_lto_suite
        ;;
    packages)
        run_packages_suite
        ;;
esac
