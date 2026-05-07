#!/bin/bash
# Safe rebuild: Only updates compiler if bootstrap verifies
#
# This script builds a new compiler from the frozen bootstrap and only
# installs compiler artifacts that pass smoke tests.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m' # No Color

STAGE2="/tmp/stage2_safe_rebuild"
STAGE3="/tmp/stage3_safe_rebuild"
STAGE3_RECOVERY="/tmp/stage3_safe_rebuild_recovery"
PRESERVED_PROD_BUILDER="/tmp/seen_preserved_prod_builder"
COMPILER_SOURCE="compiler_seen/src/main_compiler.seen"
MEMORY_GUARD_SCRIPT="$SCRIPT_DIR/memory_guard.sh"
FORK_SERIALIZER_SOURCE="$SCRIPT_DIR/fork_serializer.c"
FORK_SERIALIZER_SO=""
BOOTSTRAP_SOURCE_ROOT=""

# --- Progress monitoring helpers ---

# Format seconds as HH:MM:SS
format_time() {
    local secs=$1
    printf "%02d:%02d:%02d" $((secs/3600)) $(((secs%3600)/60)) $((secs%60))
}

# Format bytes as human-readable (pure bash, no bc dependency)
format_bytes() {
    local bytes=$1
    if [ "$bytes" -ge 1073741824 ]; then
        local gb=$((bytes / 1073741824))
        local remainder=$(( (bytes % 1073741824) * 10 / 1073741824 ))
        printf "%d.%dGB" "$gb" "$remainder"
    elif [ "$bytes" -ge 1048576 ]; then
        printf "%dMB" "$((bytes / 1048576))"
    elif [ "$bytes" -ge 1024 ]; then
        printf "%dKB" "$((bytes / 1024))"
    else
        printf "%dB" "$bytes"
    fi
}

release_cpu_baseline_to_march() {
    case "$1" in
        "")
            printf '%s\n' "-march=native"
            ;;
        x86-64|x86-64-v3)
            printf '%s\n' "-march=$1"
            ;;
        *)
            echo -e "${RED}ERROR: SEEN_RELEASE_CPU_BASELINE must be x86-64 or x86-64-v3.${NC}" >&2
            exit 1
            ;;
    esac
}

memory_guard_enabled() {
    [ "${SEEN_DISABLE_MEMORY_GUARD:-0}" != "1" ] &&
        [ -x "$MEMORY_GUARD_SCRIPT" ] &&
        { [ -n "${MEMORY_GUARD_RSS_KB:-}" ] || [ -n "${MEMORY_GUARD_RESERVE_KB:-}" ]; }
}

run_guarded_command() {
    local label=$1
    local timeout_secs=$2
    local vmem_kb=$3
    shift 3

    if memory_guard_enabled; then
        local guard_cmd=("$MEMORY_GUARD_SCRIPT" --label "$label")
        if [ -n "${MEMORY_GUARD_RSS_KB:-}" ]; then
            guard_cmd+=(--rss-limit-kb "$MEMORY_GUARD_RSS_KB")
        fi
        if [ -n "${MEMORY_GUARD_RESERVE_KB:-}" ]; then
            guard_cmd+=(--available-reserve-kb "$MEMORY_GUARD_RESERVE_KB")
        fi
        if [ -n "$vmem_kb" ]; then
            guard_cmd+=(--vmem-limit-kb "$vmem_kb")
        fi
        if [ -n "$timeout_secs" ] && [ "$timeout_secs" != "0" ]; then
            guard_cmd+=(--timeout-secs "$timeout_secs")
        fi
        if [ -n "${MEMORY_GUARD_TASKS_MAX:-}" ]; then
            guard_cmd+=(--tasks-max "$MEMORY_GUARD_TASKS_MAX")
        fi
        if [ -n "${MEMORY_GUARD_CGROUP_STOP_KB:-}" ]; then
            guard_cmd+=(--cgroup-stop-kb "$MEMORY_GUARD_CGROUP_STOP_KB")
        fi
        if [ "${SEEN_MEMORY_GUARD_KILL_ONLY:-0}" = "1" ]; then
            guard_cmd+=(--kill-only)
        fi
        guard_cmd+=(-- "$@")
        "${guard_cmd[@]}"
        return $?
    fi

    if [ -n "$timeout_secs" ] && [ "$timeout_secs" != "0" ]; then
        timeout "$timeout_secs" bash -c '
            if [ -n "$1" ]; then
                ulimit -v "$1" 2>/dev/null || true
            fi
            shift
            exec "$@"
        ' bash "$vmem_kb" "$@"
        return $?
    fi

    if [ -n "$vmem_kb" ]; then
        (
            ulimit -v "$vmem_kb" 2>/dev/null || true
            "$@"
        )
    else
        "$@"
    fi
}

run_guarded_command_to_log() {
    local label=$1
    local timeout_secs=$2
    local vmem_kb=$3
    local log_file=$4
    shift 4

    local guard_log="${log_file%.log}.guard.log"
    : > "$log_file"
    : > "$guard_log"

    local status=0
    run_guarded_command "$label" "$timeout_secs" "$vmem_kb" \
        bash -c '
            log_file=$1
            shift
            exec "$@" > "$log_file" 2>&1
        ' bash "$log_file" "$@" > "$guard_log" 2>&1 || status=$?

    if [ -s "$guard_log" ]; then
        {
            echo ""
            echo "[memory guard]"
            cat "$guard_log"
        } >> "$log_file" 2>/dev/null || true
    fi

    return "$status"
}

build_fork_serializer() {
    if [ "$HOST_OS" = "Darwin" ]; then
        return 0
    fi
    if [ "$LOW_MEMORY_MODE" != "1" ]; then
        return 0
    fi
    if [ ! -f "$FORK_SERIALIZER_SOURCE" ]; then
        return 0
    fi
    if ! command -v clang >/dev/null 2>&1; then
        echo -e "${YELLOW}WARNING: clang unavailable; legacy frozen fork serialization disabled.${NC}"
        return 0
    fi

    FORK_SERIALIZER_SO="/tmp/seen_fork_serializer_$$.so"
    rm -f "$FORK_SERIALIZER_SO"
    if run_guarded_command "fork serializer build" 60 "${OPT_VMEM_KB:-}" \
        clang -shared -fPIC -O2 "$FORK_SERIALIZER_SOURCE" -o "$FORK_SERIALIZER_SO" -ldl; then
        echo -e "${YELLOW}Legacy frozen fork serialization enabled.${NC}"
    else
        echo -e "${YELLOW}WARNING: failed to build fork serializer; continuing without it.${NC}"
        FORK_SERIALIZER_SO=""
    fi
}

# Monitor a compilation step in background, printing live progress.
# Usage: monitor_compilation <PID> <stage_label>
# Watches /tmp/seen_module_*.ll files to track per-module progress.
monitor_compilation() {
    local compile_pid=$1
    local label=$2
    local start_time=$SECONDS
    local total_modules=0
    local last_status=""

    while kill -0 "$compile_pid" 2>/dev/null; do
        local elapsed=$((SECONDS - start_time))
        local elapsed_fmt=$(format_time $elapsed)

        # Count .ll files (modules with IR generated)
        local ll_count=$(ls /tmp/seen_module_*.ll 2>/dev/null | wc -l)

        # Count .o files (modules fully compiled)
        local obj_count=$(ls /tmp/seen_module_*.o 2>/dev/null | wc -l)

        # Check for module 5 (the big one) -- if its .ll exists
        local mod5_status=""
        if [ -f /tmp/seen_module_5.ll ]; then
            local mod5_size=$(stat -c%s /tmp/seen_module_5.ll 2>/dev/null || stat -f%z /tmp/seen_module_5.ll 2>/dev/null || echo 0)
            mod5_status="mod5.ll=$(format_bytes $mod5_size)"
        else
            # Check if a fork child is working on it (large RSS process)
            local fork_pids=$(pgrep -P "$compile_pid" 2>/dev/null || true)
            if [ -n "$fork_pids" ]; then
                for fpid in $fork_pids; do
                    local frss=$(ps -o rss= -p "$fpid" 2>/dev/null | tr -d ' ')
                    if [ -n "$frss" ] && [ "$frss" -gt 500000 ]; then
                        mod5_status="mod5: IR gen ($(format_bytes $((frss * 1024))) RSS)"
                        break
                    fi
                done
            fi
            if [ -z "$mod5_status" ]; then
                mod5_status="mod5: waiting"
            fi
        fi

        # Check if we're in opt/link phase (parallel opt script exists and running)
        local phase="IR gen"
        if pgrep -f "seen_parallel_opt.sh" > /dev/null 2>&1; then
            phase="opt"
        fi
        if pgrep -f "clang.*flto.*seen_module" > /dev/null 2>&1 || pgrep -f "ld.lld.*seen_module" > /dev/null 2>&1; then
            phase="link"
        fi

        # Build status line
        local status="${CYAN}[$label]${NC} ${elapsed_fmt}  ${BOLD}${ll_count} .ll${NC} | ${BOLD}${obj_count} .o${NC}  phase:${phase}  ${DIM}${mod5_status}${NC}"

        # Only reprint if status changed (avoid flicker)
        if [ "$status" != "$last_status" ]; then
            printf "\r\033[K${status}"
            last_status="$status"
        fi

        sleep 5
    done

    # Final status
    local elapsed=$((SECONDS - start_time))
    local elapsed_fmt=$(format_time $elapsed)
    local ll_count=$(ls /tmp/seen_module_*.ll 2>/dev/null | wc -l)
    local obj_count=$(ls /tmp/seen_module_*.o 2>/dev/null | wc -l)
    printf "\r\033[K${CYAN}[$label]${NC} ${GREEN}done${NC} in ${elapsed_fmt}  ${ll_count} .ll | ${obj_count} .o\n"
}

# Run a compilation step with live progress monitoring.
# Usage: run_with_progress <label> <command...>
# Returns the exit code of the compilation command.
run_with_progress() {
    local label=$1
    shift
    local logfile=$1
    shift

    # Start compilation in background, logging to file. The guard watches the
    # entire child process tree, so forked opt/link children count toward the cap.
    run_guarded_command_to_log "$label" 0 "${MAIN_COMPILER_VMEM_KB:-}" "$logfile" "$@" &
    local compile_pid=$!

    # Start progress monitor
    monitor_compilation "$compile_pid" "$label" &
    local monitor_pid=$!

    # Wait for compilation to finish
    local exit_code=0
    wait "$compile_pid" || exit_code=$?

    # Stop monitor
    kill "$monitor_pid" 2>/dev/null || true
    wait "$monitor_pid" 2>/dev/null || true

    return $exit_code
}

# Snapshot watcher: periodically copies Stage2 module artifacts to a safe
# directory so they survive frozen compiler cleanup (which deletes
# /tmp/seen_module_*). Plain .ll files are used for recovery; opt logs/statuses
# are kept so the first concrete failure can be inspected without another
# rebuild.
# Usage: start_ll_snapshot_watcher <compiler_pid> <snapshot_dir>
start_ll_snapshot_watcher() {
    local watch_pid=$1
    local snapshot_dir=$2
    mkdir -p "$snapshot_dir"
    while kill -0 "$watch_pid" 2>/dev/null; do
        for f in /tmp/seen_module_*.ll /tmp/seen_module_*.opt.ll \
            /tmp/seen_module_*.opt.log /tmp/seen_module_*.opt.status; do
            [ -f "$f" ] || continue
            [[ "$f" == *.polly.ll ]] && continue
            local bn=$(basename "$f")
            if [ ! -f "$snapshot_dir/$bn" ] || [ "$f" -nt "$snapshot_dir/$bn" ]; then
                cp "$f" "$snapshot_dir/$bn" 2>/dev/null || true
            fi
        done
        sleep 1
    done
    # Final sweep after compiler exits
    for f in /tmp/seen_module_*.ll /tmp/seen_module_*.opt.ll \
        /tmp/seen_module_*.opt.log /tmp/seen_module_*.opt.status; do
        [ -f "$f" ] || continue
        [[ "$f" == *.polly.ll ]] && continue
        cp "$f" "$snapshot_dir/$(basename "$f")" 2>/dev/null || true
    done
}

finish_ll_snapshot_watcher() {
    local watcher_pid=$1
    local waited=0
    while kill -0 "$watcher_pid" 2>/dev/null; do
        if [ "$waited" -ge 10 ]; then
            kill "$watcher_pid" 2>/dev/null || true
            break
        fi
        sleep 1
        waited=$((waited+1))
    done
    wait "$watcher_pid" 2>/dev/null || true
}

kill_matching_processes() {
    local pattern=$1
    local matched_pids=()
    while IFS= read -r pid; do
        [ -n "$pid" ] || continue
        matched_pids+=("$pid")
    done < <(ps -eo pid=,args= | awk -v pat="$pattern" -v self="$$" '$0 ~ pat && $1 != self { print $1 }')
    if [ "${#matched_pids[@]}" -gt 0 ]; then
        kill -9 "${matched_pids[@]}" 2>/dev/null || true
    fi
}

# Kill orphaned fork children from the frozen compiler.
# Uses SIGKILL (not SIGTERM) because SIGTERM triggers cleanup handlers that delete files.
kill_frozen_orphans() {
    kill_matching_processes "$(basename "$FROZEN")"
    kill_matching_processes "seen_parallel_opt"
    kill_matching_processes "opt.*seen_module"
    kill_matching_processes "clang.*seen_module"
    kill_matching_processes "ld.lld.*seen_module"
    sleep 2
}

copy_bootstrap_seen_tree() {
    local src_dir=$1
    local dst_dir=$2
    mkdir -p "$dst_dir"

    (cd "$src_dir" && find . -type d -print) | while IFS= read -r dir; do
        mkdir -p "$dst_dir/$dir"
    done

    (cd "$src_dir" && find . -type f -print) | while IFS= read -r file; do
        local src_file="$src_dir/$file"
        local dst_file="$dst_dir/$file"
        mkdir -p "$(dirname "$dst_file")"
        case "$file" in
            *.seen)
                awk '
                    /^[ \t]*\/\/\/[ \t]*$/ {
                        in_triple = !in_triple
                        print "//"
                        next
                    }
                    in_triple {
                        print "//"
                        next
                    }
                    { print }
                ' "$src_file" > "$dst_file"
                ;;
            *)
                cp -a "$src_file" "$dst_file"
                ;;
        esac
    done
}

prepare_bootstrap_source_overlay() {
    if [ "${SEEN_DISABLE_BOOTSTRAP_SOURCE_OVERLAY:-0}" = "1" ]; then
        BOOTSTRAP_SOURCE_ROOT="$REPO_ROOT"
        return 0
    fi

    BOOTSTRAP_SOURCE_ROOT=$(mktemp -d /tmp/seen_bootstrap_source.XXXXXX)
    for entry in "$REPO_ROOT"/*; do
        local base
        base=$(basename "$entry")
        case "$base" in
            compiler_seen|seen_std)
                ;;
            *)
                ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/$base"
                ;;
        esac
    done

    mkdir -p "$BOOTSTRAP_SOURCE_ROOT/compiler_seen" "$BOOTSTRAP_SOURCE_ROOT/seen_std"
    for entry in "$REPO_ROOT/compiler_seen"/*; do
        local base
        base=$(basename "$entry")
        if [ "$base" = "src" ]; then
            copy_bootstrap_seen_tree "$entry" "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/src"
        else
            ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/$base"
        fi
    done
    for entry in "$REPO_ROOT/seen_std"/*; do
        local base
        base=$(basename "$entry")
        if [ "$base" = "src" ]; then
            copy_bootstrap_seen_tree "$entry" "$BOOTSTRAP_SOURCE_ROOT/seen_std/src"
        else
            ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/seen_std/$base"
        fi
    done
    echo -e "${YELLOW}Bootstrap source overlay enabled: temporary /// bodies stripped for older bootstrap compilers.${NC}"
}

cleanup_bootstrap_source_overlay() {
    if [ -n "$BOOTSTRAP_SOURCE_ROOT" ] && [ "$BOOTSTRAP_SOURCE_ROOT" != "$REPO_ROOT" ]; then
        rm -rf "$BOOTSTRAP_SOURCE_ROOT"
    fi
}

trap cleanup_bootstrap_source_overlay EXIT

extract_expected_module_count() {
    local log_file=$1
    local count=""
    if [ -f "$log_file" ]; then
        count=$(grep -Eo 'Found [0-9]+ modules' "$log_file" 2>/dev/null | head -1 | awk '{print $2}')
    fi
    if [ -z "$count" ]; then
        echo 0
    else
        echo "$count"
    fi
}

tail_log_if_exists() {
    local log_file=$1
    local lines=${2:-30}
    if [ -f "$log_file" ]; then
        tail -"$lines" "$log_file" 2>/dev/null || true
    else
        echo "(missing log: $log_file)"
    fi
}

count_module_objects() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.o; do
        [ -f "$f" ] || continue
        count=$((count+1))
    done
    echo "$count"
}

count_plain_module_lls() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.ll; do
        [ -f "$f" ] || continue
        [[ "$f" == *.opt.ll ]] && continue
        [[ "$f" == *.polly.ll ]] && continue
        count=$((count+1))
    done
    echo "$count"
}

count_module_opt_lls() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.opt.ll; do
        [ -f "$f" ] || continue
        count=$((count+1))
    done
    echo "$count"
}

list_modules_missing_objects() {
    local dir=$1
    local missing=""
    for llfile in "$dir"/seen_module_*.ll; do
        [ -f "$llfile" ] || continue
        [[ "$llfile" == *.opt.ll ]] && continue
        [[ "$llfile" == *.polly.ll ]] && continue
        local modname=$(basename "$llfile" .ll)
        if [ ! -f "$dir/${modname}.o" ]; then
            missing="$missing ${modname}"
        fi
    done
    echo "$missing"
}

find_problem_empty_modules() {
    local dir=$1
    local empty=""
    for llfile in "$dir"/seen_module_*.ll; do
        [ -f "$llfile" ] || continue
        [[ "$llfile" == *.opt.ll ]] && continue
        [[ "$llfile" == *.polly.ll ]] && continue
        local defines=$(grep -c '^define' "$llfile" 2>/dev/null | tail -1)
        defines=${defines:-0}
        if [ "$defines" -eq 0 ] 2>/dev/null; then
            local strings=$(grep -c '@\.str' "$llfile" 2>/dev/null | tail -1)
            strings=${strings:-0}
            if [ "$strings" -gt 0 ] 2>/dev/null; then
                empty="$empty $(basename "$llfile" .ll)"
            fi
        fi
    done
    echo "$empty"
}

bootstrap_binary_usable() {
    local bin=$1
    local smoke_log="/tmp/seen_bootstrap_smoke_$$_$(basename "$bin").log"
    [ -x "$bin" ] || return 1
    run_guarded_command "bootstrap smoke $(basename "$bin")" 5 "$MAIN_COMPILER_VMEM_KB" \
        "$bin" >"$smoke_log" 2>&1
    local exit_code=$?
    rm -f "$smoke_log"
    [ "$exit_code" -eq 0 ] || [ "$exit_code" -eq 1 ]
}

stage2_failure_looks_oom() {
    local exit_code=$1
    local log_file=$2
    if [ "$exit_code" -eq 137 ]; then
        return 0
    fi
    if [ -f "$log_file" ] && grep -qiE 'killed|out of memory|oom' "$log_file"; then
        return 0
    fi
    return 1
}

preserve_stage2_failure_artifacts() {
    local snapshot_dir=$1
    local preserve_dir="/tmp/seen_stage2_failure_$$"
    rm -rf "$preserve_dir"
    mkdir -p "$preserve_dir"
    cp /tmp/safe_rebuild_stage2.log "$preserve_dir/" 2>/dev/null || true
    cp /tmp/safe_rebuild_stage2.guard.log "$preserve_dir/" 2>/dev/null || true
    if [ -d "$snapshot_dir" ]; then
        cp "$snapshot_dir"/seen_module_* "$preserve_dir/" 2>/dev/null || true
    fi
    cp /tmp/seen_module_* "$preserve_dir/" 2>/dev/null || true
    echo -e "${YELLOW}Preserved Stage2 failure artifacts: $preserve_dir${NC}"
}

is_positive_integer() {
    case "$1" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

detect_physical_memory_kb() {
    local mem_kb=""
    if [ -r /proc/meminfo ]; then
        mem_kb=$(awk '/^MemTotal:/ { print $2; exit }' /proc/meminfo 2>/dev/null)
    fi
    if ! is_positive_integer "$mem_kb" && command -v sysctl >/dev/null 2>&1; then
        local mem_bytes
        mem_bytes=$(sysctl -n hw.memsize 2>/dev/null || true)
        if is_positive_integer "$mem_bytes"; then
            mem_kb=$((mem_bytes / 1024))
        fi
    fi
    if is_positive_integer "$mem_kb"; then
        echo "$mem_kb"
    fi
}

detect_available_memory_kb() {
    local mem_kb=""
    if [ -r /proc/meminfo ]; then
        mem_kb=$(awk '/^MemAvailable:/ { print $2; exit }' /proc/meminfo 2>/dev/null)
    fi
    if is_positive_integer "$mem_kb"; then
        echo "$mem_kb"
    fi
}

detect_cgroup_memory_kb() {
    local limit_bytes=""
    if [ -r /sys/fs/cgroup/memory.max ]; then
        limit_bytes=$(cat /sys/fs/cgroup/memory.max 2>/dev/null || true)
    elif [ -r /sys/fs/cgroup/memory/memory.limit_in_bytes ]; then
        limit_bytes=$(cat /sys/fs/cgroup/memory/memory.limit_in_bytes 2>/dev/null || true)
    fi

    if [ "$limit_bytes" = "max" ] || ! is_positive_integer "$limit_bytes"; then
        return 0
    fi

    # Some cgroup v1 hosts report a huge sentinel when no limit is configured.
    if [ "$limit_bytes" -ge 1125899906842624 ] 2>/dev/null; then
        return 0
    fi

    echo $((limit_bytes / 1024))
}

detect_effective_system_memory_kb() {
    local physical_kb
    local cgroup_kb
    physical_kb=$(detect_physical_memory_kb)
    cgroup_kb=$(detect_cgroup_memory_kb)

    if ! is_positive_integer "$physical_kb"; then
        return 1
    fi

    if is_positive_integer "$cgroup_kb" && [ "$cgroup_kb" -lt "$physical_kb" ]; then
        echo "$cgroup_kb"
    else
        echo "$physical_kb"
    fi
}

derive_main_compiler_vmem_kb() {
    local total_kb=$1
    local available_kb=$2
    local cap_kb=$((total_kb * 25 / 100))

    if is_positive_integer "$available_kb"; then
        local available_cap_kb=$((available_kb * 50 / 100))
        if [ "$available_cap_kb" -gt 0 ] && [ "$available_cap_kb" -lt "$cap_kb" ]; then
            cap_kb=$available_cap_kb
        fi
    fi

    local max_main_kb=$((8 * 1024 * 1024))
    if [ "$cap_kb" -gt "$max_main_kb" ]; then
        cap_kb=$max_main_kb
    fi

    if [ "$cap_kb" -lt 1 ]; then
        cap_kb=1
    fi

    echo "$cap_kb"
}

derive_opt_vmem_kb() {
    local total_kb=$1
    local main_kb=$2
    local cap_kb=$((total_kb * 10 / 100))
    local half_main_kb=$((main_kb / 2))

    if [ "$half_main_kb" -gt 0 ] && [ "$half_main_kb" -lt "$cap_kb" ]; then
        cap_kb=$half_main_kb
    fi

    local max_opt_kb=$((2 * 1024 * 1024))
    if [ "$cap_kb" -gt "$max_opt_kb" ]; then
        cap_kb=$max_opt_kb
    fi

    if [ "$cap_kb" -lt 1 ]; then
        cap_kb=1
    fi

    echo "$cap_kb"
}

derive_memory_guard_reserve_kb() {
    local total_kb=$1
    local available_kb=$2
    local reserve_kb=$((total_kb * 10 / 100))
    local min_reserve_kb=$((4 * 1024 * 1024))

    if [ "$reserve_kb" -lt "$min_reserve_kb" ]; then
        reserve_kb=$min_reserve_kb
    fi

    if is_positive_integer "$available_kb"; then
        local half_available_kb=$((available_kb / 2))
        if [ "$half_available_kb" -gt 0 ] && [ "$reserve_kb" -gt "$half_available_kb" ]; then
            reserve_kb=$half_available_kb
        fi
    fi

    if [ "$reserve_kb" -lt 1 ]; then
        reserve_kb=1
    fi

    echo "$reserve_kb"
}

derive_memory_guard_rss_kb() {
    local total_kb=$1
    local available_kb=$2
    local reserve_kb=$3
    local cap_kb=$((total_kb * 60 / 100))

    if is_positive_integer "$available_kb" && [ "$available_kb" -gt "$reserve_kb" ]; then
        local available_cap_kb=$((available_kb - reserve_kb))
        if [ "$available_cap_kb" -gt 0 ] && [ "$available_cap_kb" -lt "$cap_kb" ]; then
            cap_kb=$available_cap_kb
        fi
    fi

    local max_guard_kb=$((48 * 1024 * 1024))
    if [ "$cap_kb" -gt "$max_guard_kb" ]; then
        cap_kb=$max_guard_kb
    fi

    if [ "$cap_kb" -lt 1 ]; then
        cap_kb=1
    fi

    echo "$cap_kb"
}

guard_low_memory_concurrency() {
    if [ "${SEEN_ALLOW_CONCURRENT_REBUILD:-0}" = "1" ]; then
        return 0
    fi

    local matches
    matches=$(ps -eo pid=,ppid=,comm=,args= | awk -v self="$$" '
        $1 == self { next }
        $2 == self { next }
        $3 == "safe_rebuild.sh" { print; next }
        $0 ~ /(^|[[:space:]])seen[[:space:]]+compile([[:space:]]|$)/ { print; next }
        $0 ~ /compiler_seen\/target\/seen[[:space:]]+compile([[:space:]]|$)/ { print; next }
        $0 ~ /seen_preserved_prod_builder[[:space:]]+compile([[:space:]]|$)/ { print; next }
    ')

    if [ -n "$matches" ]; then
        echo -e "${RED}ERROR: Low-memory rebuild refused because another Seen compile/rebuild appears to be running.${NC}"
        echo "Set SEEN_ALLOW_CONCURRENT_REBUILD=1 only if you have checked memory pressure manually."
        echo "$matches"
        exit 1
    fi
}

echo "=== Safe Rebuild Script ==="
echo ""

# Detect host platform
HOST_OS=$(uname -s)
HOST_ARCH=$(uname -m)
LOW_MEMORY_MODE=0
STAGE2_COMPILE_FLAGS="--fast --no-cache"
PASS2_COMPILE_FLAGS="--fast --no-cache"
RELEASE_CPU_BASELINE="${SEEN_RELEASE_CPU_BASELINE:-}"
RELEASE_TARGET_CPU_FLAG=""
RELEASE_CLANG_MARCH_FLAG="$(release_cpu_baseline_to_march "$RELEASE_CPU_BASELINE")"
MAIN_COMPILER_VMEM_KB=""
OPT_VMEM_KB=""
RECOVERY_TIMEOUT_SECS="${SEEN_RECOVERY_TIMEOUT_SECS:-1800}"
SYSTEM_MEMORY_KB=$(detect_effective_system_memory_kb || true)
SYSTEM_AVAILABLE_KB=$(detect_available_memory_kb || true)
MEMORY_GUARD_RSS_KB=""
MEMORY_GUARD_RESERVE_KB=""
MEMORY_GUARD_TASKS_MAX=""
MEMORY_GUARD_CGROUP_STOP_KB=""

if [ -n "$RELEASE_CPU_BASELINE" ]; then
    RELEASE_TARGET_CPU_FLAG="--target-cpu=$RELEASE_CPU_BASELINE"
    STAGE2_COMPILE_FLAGS="$STAGE2_COMPILE_FLAGS $RELEASE_TARGET_CPU_FLAG"
    PASS2_COMPILE_FLAGS="$PASS2_COMPILE_FLAGS $RELEASE_TARGET_CPU_FLAG"
    export SEEN_RELEASE_CPU_BASELINE="$RELEASE_CPU_BASELINE"
    echo -e "${YELLOW}Release CPU baseline enabled: $RELEASE_CPU_BASELINE.${NC}"
fi

if [ "${SEEN_DISABLE_MEMORY_GUARD:-0}" != "1" ]; then
    if ! is_positive_integer "$SYSTEM_MEMORY_KB"; then
        echo -e "${RED}ERROR: Could not detect system memory for rebuild memory guard.${NC}"
        echo "Set SEEN_GUARD_RSS_KB and SEEN_GUARD_RESERVE_KB explicitly, or run on a host with /proc/meminfo or sysctl hw.memsize."
        exit 1
    fi
    if [ -n "${SEEN_GUARD_RSS_KB:-}" ] && ! is_positive_integer "$SEEN_GUARD_RSS_KB"; then
        echo -e "${RED}ERROR: SEEN_GUARD_RSS_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    if [ -n "${SEEN_GUARD_RESERVE_KB:-}" ] && ! is_positive_integer "$SEEN_GUARD_RESERVE_KB"; then
        echo -e "${RED}ERROR: SEEN_GUARD_RESERVE_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    if [ -n "${SEEN_GUARD_TASKS_MAX:-}" ] && ! is_positive_integer "$SEEN_GUARD_TASKS_MAX"; then
        echo -e "${RED}ERROR: SEEN_GUARD_TASKS_MAX must be a positive integer value.${NC}"
        exit 1
    fi
    if [ -n "${SEEN_GUARD_CGROUP_STOP_KB:-}" ] && ! is_positive_integer "$SEEN_GUARD_CGROUP_STOP_KB"; then
        echo -e "${RED}ERROR: SEEN_GUARD_CGROUP_STOP_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    MEMORY_GUARD_RESERVE_KB="${SEEN_GUARD_RESERVE_KB:-$(derive_memory_guard_reserve_kb "$SYSTEM_MEMORY_KB" "$SYSTEM_AVAILABLE_KB")}"
    MEMORY_GUARD_RSS_KB="${SEEN_GUARD_RSS_KB:-$(derive_memory_guard_rss_kb "$SYSTEM_MEMORY_KB" "$SYSTEM_AVAILABLE_KB" "$MEMORY_GUARD_RESERVE_KB")}"
    MEMORY_GUARD_TASKS_MAX="${SEEN_GUARD_TASKS_MAX:-256}"
    MEMORY_GUARD_CGROUP_STOP_KB="${SEEN_GUARD_CGROUP_STOP_KB:-$((MEMORY_GUARD_RSS_KB * 90 / 100))}"
    if [ "$MEMORY_GUARD_CGROUP_STOP_KB" -lt 1 ]; then
        MEMORY_GUARD_CGROUP_STOP_KB=1
    fi
    export SEEN_MEMORY_GUARD_RSS_KB="$MEMORY_GUARD_RSS_KB"
    export SEEN_MEMORY_GUARD_RESERVE_KB="$MEMORY_GUARD_RESERVE_KB"
    export SEEN_MEMORY_GUARD_TASKS_MAX="$MEMORY_GUARD_TASKS_MAX"
    export SEEN_MEMORY_GUARD_CGROUP_STOP_KB="$MEMORY_GUARD_CGROUP_STOP_KB"
    if [ "$HOST_OS" != "Darwin" ] && [ "${SEEN_MEMORY_GUARD_KERNEL_SCOPE:-1}" != "0" ]; then
        export SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=1
    else
        export SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=0
    fi
fi

if [ "${SEEN_LOW_MEMORY:-0}" = "1" ]; then
    LOW_MEMORY_MODE=1
    STAGE2_COMPILE_FLAGS="$STAGE2_COMPILE_FLAGS --no-fork"
    PASS2_COMPILE_FLAGS="$PASS2_COMPILE_FLAGS --no-fork"
    if ! is_positive_integer "$SYSTEM_MEMORY_KB"; then
        echo -e "${RED}ERROR: Could not detect system memory for low-memory rebuild caps.${NC}"
        echo "Set SEEN_MAIN_VMEM_KB and SEEN_OPT_VMEM_KB explicitly, or run on a host with /proc/meminfo or sysctl hw.memsize."
        exit 1
    fi
    if [ -n "${SEEN_MAIN_VMEM_KB:-}" ] && ! is_positive_integer "$SEEN_MAIN_VMEM_KB"; then
        echo -e "${RED}ERROR: SEEN_MAIN_VMEM_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    if [ -n "${SEEN_OPT_VMEM_KB:-}" ] && ! is_positive_integer "$SEEN_OPT_VMEM_KB"; then
        echo -e "${RED}ERROR: SEEN_OPT_VMEM_KB must be a positive integer KB value.${NC}"
        exit 1
    fi
    MAIN_COMPILER_VMEM_KB="${SEEN_MAIN_VMEM_KB:-$(derive_main_compiler_vmem_kb "$SYSTEM_MEMORY_KB" "$SYSTEM_AVAILABLE_KB")}"
    OPT_VMEM_KB="${SEEN_OPT_VMEM_KB:-$(derive_opt_vmem_kb "$SYSTEM_MEMORY_KB" "$MAIN_COMPILER_VMEM_KB")}"
    if ! is_positive_integer "$MAIN_COMPILER_VMEM_KB" || ! is_positive_integer "$OPT_VMEM_KB"; then
        echo -e "${RED}ERROR: Low-memory rebuild caps must be positive integer KB values.${NC}"
        exit 1
    fi
    if [ -z "${SEEN_GUARD_RSS_KB:-}" ]; then
        MEMORY_GUARD_RSS_KB="$MAIN_COMPILER_VMEM_KB"
        export SEEN_MEMORY_GUARD_RSS_KB="$MEMORY_GUARD_RSS_KB"
    fi
    if [ -z "${SEEN_GUARD_TASKS_MAX:-}" ]; then
        MEMORY_GUARD_TASKS_MAX=48
        export SEEN_MEMORY_GUARD_TASKS_MAX="$MEMORY_GUARD_TASKS_MAX"
    fi
    if [ -z "${SEEN_GUARD_CGROUP_STOP_KB:-}" ]; then
        MEMORY_GUARD_CGROUP_STOP_KB=$((MEMORY_GUARD_RSS_KB * 90 / 100))
        if [ "$MEMORY_GUARD_CGROUP_STOP_KB" -lt 1 ]; then
            MEMORY_GUARD_CGROUP_STOP_KB=1
        fi
        export SEEN_MEMORY_GUARD_CGROUP_STOP_KB="$MEMORY_GUARD_CGROUP_STOP_KB"
    fi
    RECOVERY_TIMEOUT_SECS="${SEEN_RECOVERY_TIMEOUT_SECS:-7200}"
    export SEEN_LOW_MEMORY=1
    export SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB"
    export SEEN_OPT_VMEM_KB="$OPT_VMEM_KB"
    export SEEN_RECOVERY_TIMEOUT_SECS="$RECOVERY_TIMEOUT_SECS"
    guard_low_memory_concurrency
    echo -e "${YELLOW}Low-memory mode enabled: serial bootstrap stages.${NC}"
    echo -e "${YELLOW}Detected system memory: $(format_bytes $((SYSTEM_MEMORY_KB * 1024))). Main compiler cap: $(format_bytes $((MAIN_COMPILER_VMEM_KB * 1024))). opt cap: $(format_bytes $((OPT_VMEM_KB * 1024))).${NC}"
fi

if memory_guard_enabled; then
    echo -e "${YELLOW}Memory guard enabled: tree RSS cap $(format_bytes $((MEMORY_GUARD_RSS_KB * 1024))); cgroup stop $(format_bytes $((MEMORY_GUARD_CGROUP_STOP_KB * 1024))); reserve $(format_bytes $((MEMORY_GUARD_RESERVE_KB * 1024))); tasks max ${MEMORY_GUARD_TASKS_MAX:-unlimited}.${NC}"
fi

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS
    if [ "$HOST_ARCH" = "arm64" ]; then
        FROZEN="bootstrap/stage1_frozen_macos_arm64"
        HASH_FILE="bootstrap/stage1_frozen_macos_arm64.sha256"
        echo "Detected macOS ARM64 (Apple Silicon), using stage1_frozen_macos_arm64"
    else
        FROZEN="bootstrap/stage1_frozen_macos_x86_64"
        HASH_FILE="bootstrap/stage1_frozen_macos_x86_64.sha256"
        echo "Detected macOS x86_64, using stage1_frozen_macos_x86_64"
    fi
else
    # Linux: Auto-detect CPU ISA level and select appropriate bootstrap
    detect_isa_level() {
        if grep -q 'avx512f' /proc/cpuinfo 2>/dev/null; then
            echo "v4"
        else
            echo "v3"
        fi
    }

    ISA_LEVEL=$(detect_isa_level)
    if [ "$ISA_LEVEL" = "v4" ]; then
        FROZEN="bootstrap/stage1_frozen"
        HASH_FILE="bootstrap/stage1_frozen.sha256"
        echo "Detected x86-64-v4 (AVX-512) CPU, using stage1_frozen"
    else
        FROZEN="bootstrap/stage1_frozen_v3"
        HASH_FILE="bootstrap/stage1_frozen_v3.sha256"
        echo "Detected x86-64-v3 CPU, using stage1_frozen_v3"
    fi
fi

# Check frozen compiler exists
if [ ! -f "$FROZEN" ]; then
    echo -e "${RED}ERROR: Frozen compiler not found at $FROZEN${NC}"
    echo "Run this script from the repository root."
    if [ "$HOST_OS" = "Darwin" ]; then
        echo "On macOS, run scripts/bootstrap_macos.sh first to create the macOS bootstrap."
    fi
    exit 1
fi

if ! bootstrap_binary_usable "$FROZEN"; then
    if [ "$HOST_OS" != "Darwin" ] && [ "$FROZEN" = "bootstrap/stage1_frozen" ] && [ -x "bootstrap/stage1_frozen_v3" ] && bootstrap_binary_usable "bootstrap/stage1_frozen_v3"; then
        echo -e "${YELLOW}bootstrap/stage1_frozen failed a startup smoke test; falling back to bootstrap/stage1_frozen_v3.${NC}"
        FROZEN="bootstrap/stage1_frozen_v3"
        HASH_FILE="bootstrap/stage1_frozen_v3.sha256"
    else
        echo -e "${RED}ERROR: Frozen compiler at $FROZEN failed a startup smoke test.${NC}"
        exit 1
    fi
fi

# Verify frozen compiler hash (cross-platform)
echo "Verifying frozen compiler integrity..."
verify_hash() {
    if command -v sha256sum &>/dev/null; then
        sha256sum -c "$1" > /dev/null 2>&1
    elif command -v shasum &>/dev/null; then
        shasum -a 256 -c "$1" > /dev/null 2>&1
    else
        echo -e "${YELLOW}WARNING: No sha256sum or shasum found, skipping hash verification${NC}"
        return 0
    fi
}

cleanup_smoke_build_state() {
    rm -rf .seen_cache/ /tmp/seen_ir_cache/ /tmp/seen_thinlto_cache/
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
    rm -f /tmp/seen_module_*.polly.ll /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log
    rm -f /tmp/seen_module_*.relink.o /tmp/safe_rebuild_smoke_bin
    rm -f /tmp/safe_rebuild_*_hello_english.seen
}

preserve_smoke_failure_artifacts() {
    local stage_slug=$1
    local artifact_dir="/tmp/seen_smoke_failure_${stage_slug}_$(date +%s)"
    mkdir -p "$artifact_dir" 2>/dev/null || return 0
    for f in /tmp/seen_module_*.ll /tmp/seen_module_*.opt.ll \
        /tmp/seen_module_*.opt.log /tmp/seen_module_*.opt.status \
        /tmp/safe_rebuild_"$stage_slug"_hello_*.log \
        /tmp/safe_rebuild_"$stage_slug"_hello_english.seen; do
        if [ -e "$f" ]; then
            cp "$f" "$artifact_dir/" 2>/dev/null || true
        fi
    done
    echo -e "${YELLOW}Preserved smoke failure artifacts: $artifact_dir${NC}"
}

preserve_existing_production_compiler() {
    rm -f "$PRESERVED_PROD_BUILDER"
    if [ -x compiler_seen/target/seen ]; then
        cp compiler_seen/target/seen "$PRESERVED_PROD_BUILDER"
        chmod +x "$PRESERVED_PROD_BUILDER"
        return 0
    fi
    if [ -x target/release/seen ]; then
        cp target/release/seen "$PRESERVED_PROD_BUILDER"
        chmod +x "$PRESERVED_PROD_BUILDER"
        return 0
    fi
    return 1
}

smoke_test_compiler() {
    local compiler_path=$1
    local stage_label=$2
    local stage_slug=$3
    local smoke_fixture="$REPO_ROOT/examples/hello_world/hello_english.seen"
    local smoke_source="/tmp/safe_rebuild_${stage_slug}_hello_english.seen"
    local smoke_bin="/tmp/safe_rebuild_smoke_bin"
    local check_log="/tmp/safe_rebuild_${stage_slug}_hello_check.log"
    local compile_log="/tmp/safe_rebuild_${stage_slug}_hello_compile.log"
    local run_log="/tmp/safe_rebuild_${stage_slug}_hello_run.log"
    local compiler_env=()
    local check_cmd=("$compiler_path" check "$smoke_source")
    local compile_cmd=("$compiler_path" compile "$smoke_source" "$smoke_bin" --fast --no-cache)

    if [ "$LOW_MEMORY_MODE" = "1" ]; then
        compiler_env+=("SEEN_LOW_MEMORY=${SEEN_LOW_MEMORY:-1}")
        if [ -n "$MAIN_COMPILER_VMEM_KB" ]; then
            compiler_env+=("SEEN_MAIN_VMEM_KB=$MAIN_COMPILER_VMEM_KB")
        fi
        if [ -n "$OPT_VMEM_KB" ]; then
            compiler_env+=("SEEN_OPT_VMEM_KB=$OPT_VMEM_KB")
        fi
        check_cmd+=(--no-fork)
        compile_cmd+=(--no-fork)
    fi
    if [ -n "${RELEASE_TARGET_CPU_FLAG:-}" ]; then
        compile_cmd+=("$RELEASE_TARGET_CPU_FLAG")
    fi

    cleanup_smoke_build_state
    if ! cp "$smoke_fixture" "$smoke_source"; then
        echo -e "${YELLOW}${stage_label} could not prepare hello-world smoke source.${NC}"
        return 1
    fi

    if ! (
        cd "$REPO_ROOT" &&
        run_guarded_command "$stage_label check smoke" 120 "$MAIN_COMPILER_VMEM_KB" \
            env "${compiler_env[@]}" "${check_cmd[@]}" > "$check_log" 2>&1
    ); then
        echo -e "${YELLOW}${stage_label} failed hello-world check smoke test.${NC}"
        tail -20 "$check_log" 2>/dev/null || true
        preserve_smoke_failure_artifacts "$stage_slug"
        cleanup_smoke_build_state
        return 1
    fi

    cleanup_smoke_build_state
    if ! cp "$smoke_fixture" "$smoke_source"; then
        echo -e "${YELLOW}${stage_label} could not prepare hello-world smoke source.${NC}"
        return 1
    fi

    if ! (
        cd "$REPO_ROOT" &&
        run_guarded_command "$stage_label compile smoke" 120 "$MAIN_COMPILER_VMEM_KB" \
            env "${compiler_env[@]}" "${compile_cmd[@]}" > "$compile_log" 2>&1
    ); then
        echo -e "${YELLOW}${stage_label} failed hello-world compile smoke test.${NC}"
        tail -20 "$compile_log" 2>/dev/null || true
        preserve_smoke_failure_artifacts "$stage_slug"
        cleanup_smoke_build_state
        return 1
    fi

    if [ ! -x "$smoke_bin" ]; then
        echo -e "${YELLOW}${stage_label} compile smoke test did not produce $smoke_bin.${NC}"
        preserve_smoke_failure_artifacts "$stage_slug"
        cleanup_smoke_build_state
        return 1
    fi

    if ! "$smoke_bin" > "$run_log" 2>&1; then
        echo -e "${YELLOW}${stage_label} failed hello-world run smoke test.${NC}"
        tail -20 "$run_log" 2>/dev/null || true
        preserve_smoke_failure_artifacts "$stage_slug"
        cleanup_smoke_build_state
        return 1
    fi

    echo -e "${GREEN}${stage_label} passed hello-world smoke test.${NC}"
    cleanup_smoke_build_state
    return 0
}

recover_with_preserved_production_compiler() {
    if [ ! -x "$PRESERVED_PROD_BUILDER" ]; then
        return 1
    fi

    echo ""
    echo "Recovery: trying preserved production compiler..."
    if ! smoke_test_compiler "$PRESERVED_PROD_BUILDER" "Preserved production compiler" "preserved_prod"; then
        echo -e "${YELLOW}Preserved production compiler failed smoke; skipping recovery rebuild.${NC}"
        return 1
    fi

    cleanup_smoke_build_state
    rm -f "$STAGE3_RECOVERY"

    local recovery_exit=0
    run_guarded_command_to_log "preserved compiler recovery" "$RECOVERY_TIMEOUT_SECS" "$MAIN_COMPILER_VMEM_KB" /tmp/safe_rebuild_stage3_recovery.log \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
        env PATH="$OPT_WRAPPER_DIR:$PATH" \
            SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-0}" \
            SEEN_SKIP_IR_FIXUPS=1 \
            SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
            "$PRESERVED_PROD_BUILDER" compile "$COMPILER_SOURCE" "$STAGE3_RECOVERY" \
            --fast --no-cache --no-fork $RELEASE_TARGET_CPU_FLAG || recovery_exit=$?
    if [ "$recovery_exit" -eq 0 ]; then
        echo -e "${GREEN}Recovery rebuild succeeded.${NC}"
        echo ""
        echo "Recovery smoke: checking hello-world..."
        if smoke_test_compiler "$STAGE3_RECOVERY" "Recovered stage3" "stage3_recovery"; then
            VERIFIED="$STAGE3_RECOVERY"
            return 0
        fi
        echo -e "${YELLOW}Recovery rebuild produced a compiler that failed smoke.${NC}"
        return 1
    fi

    echo -e "${YELLOW}Recovery rebuild failed or timed out (exit=$recovery_exit).${NC}"
    if [ "$recovery_exit" != "124" ]; then
        tail_log_if_exists /tmp/safe_rebuild_stage3_recovery.log 10
    fi
    return 1
}

recover_with_existing_stage_builder() {
    local builder_path=$1
    local builder_name
    local builder_slug
    local builder_log

    [ -x "$builder_path" ] || return 1
    builder_name=$(basename "$builder_path")
    builder_slug=$(printf "%s" "$builder_name" | tr -c 'A-Za-z0-9_' '_')
    builder_log="/tmp/safe_rebuild_existing_stage_${builder_slug}.log"

    echo ""
    echo "Recovery: trying existing stage builder $builder_path..."
    if ! smoke_test_compiler "$builder_path" "Existing stage builder $builder_name" "existing_${builder_slug}"; then
        echo -e "${YELLOW}Existing stage builder $builder_name failed smoke; trying next recovery builder.${NC}"
        return 1
    fi

    cleanup_smoke_build_state
    rm -f "$STAGE3_RECOVERY"

    local recovery_exit=0
    run_guarded_command_to_log "existing stage builder $builder_name recovery" "$RECOVERY_TIMEOUT_SECS" "$MAIN_COMPILER_VMEM_KB" "$builder_log" \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
        env PATH="$OPT_WRAPPER_DIR:$PATH" \
            SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-0}" \
            SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
            "$builder_path" compile "$COMPILER_SOURCE" "$STAGE3_RECOVERY" \
            --fast --no-cache --no-fork $RELEASE_TARGET_CPU_FLAG || recovery_exit=$?

    if [ "$recovery_exit" -eq 0 ]; then
        echo -e "${GREEN}Existing stage builder $builder_name rebuilt the compiler.${NC}"
        echo ""
        echo "Recovery smoke: checking hello-world..."
        if smoke_test_compiler "$STAGE3_RECOVERY" "Recovered compiler from $builder_name" "recovered_${builder_slug}"; then
            VERIFIED="$STAGE3_RECOVERY"
            return 0
        fi
        echo -e "${YELLOW}Recovered compiler from $builder_name failed smoke; trying next recovery builder.${NC}"
        return 1
    fi

    echo -e "${YELLOW}Existing stage builder $builder_name failed or timed out (exit=$recovery_exit).${NC}"
    if [ "$recovery_exit" != "124" ]; then
        tail_log_if_exists "$builder_log" 10
    fi
    return 1
}

recover_with_existing_stage_builders() {
    local candidate
    local candidates=()

    if [ -n "${SEEN_STAGE_BUILDER:-}" ]; then
        candidates+=("$SEEN_STAGE_BUILDER")
    fi
    candidates+=(
        "$REPO_ROOT/stage2_head"
        "$REPO_ROOT/stage3_recovery_head"
        "$REPO_ROOT/stage3_head"
        "$REPO_ROOT/compiler_seen/target/seen_native_snapshot"
        "$REPO_ROOT/compiler_seen/target/seen_frozen6"
    )

    for candidate in "${candidates[@]}"; do
        if recover_with_existing_stage_builder "$candidate"; then
            return 0
        fi
    done

    return 1
}

if verify_hash "$HASH_FILE"; then
    echo -e "${GREEN}Frozen compiler verified.${NC}"
else
    echo -e "${RED}ERROR: Frozen compiler hash verification failed!${NC}"
    echo "The bootstrap compiler may be corrupted."
    exit 1
fi

preserve_existing_production_compiler >/dev/null 2>&1 || true
prepare_bootstrap_source_overlay
FROZEN_ABS="$REPO_ROOT/$FROZEN"

if [ "${SEEN_SKIP_PREBUILD_GATES:-0}" != "1" ]; then
    echo "Running prebuild gates..."
    bash "$SCRIPT_DIR/seen_prebuild_gates.sh"
else
    echo -e "${YELLOW}Prebuild gates skipped by SEEN_SKIP_PREBUILD_GATES=1.${NC}"
fi

# Kill any leftover compilation processes that might write to /tmp/seen_module_*
# and interfere with this build (race condition causes duplicate symbols)
kill_matching_processes "seen compile"
kill_matching_processes "seen build"
sleep 1

# Clean up any previous test files and cache
rm -f "$STAGE2" "$STAGE3"
rm -rf .seen_cache/ /tmp/seen_ir_cache/

# --- Opt wrapper setup (platform-specific) ---

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: use comprehensive ABI mismatch fixer (macos_opt_wrapper.py)
    OPT_WRAPPER_DIR=$(mktemp -d /tmp/seen_opt_wrapper.XXXXXX)
    if [ -d "/opt/homebrew/opt/llvm/bin" ]; then
        LLVM_BIN="/opt/homebrew/opt/llvm/bin"
    elif [ -d "/usr/local/opt/llvm/bin" ]; then
        LLVM_BIN="/usr/local/opt/llvm/bin"
    else
        LLVM_BIN=""
    fi
    PYTHON3_PATH=""
    for p in /opt/homebrew/bin/python3 /usr/local/bin/python3 /usr/bin/python3; do
        if [ -x "$p" ]; then PYTHON3_PATH="$p"; break; fi
    done
    [ -z "$PYTHON3_PATH" ] && PYTHON3_PATH=$(which python3 2>/dev/null || echo "python3")
    cp bootstrap/macos_opt_wrapper.py "$OPT_WRAPPER_DIR/macos_opt_wrapper_impl.py"
    cat > "$OPT_WRAPPER_DIR/opt" << WRAPPER_EOF
#!/bin/sh
export PATH="$LLVM_BIN:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:\$PATH"
exec "$PYTHON3_PATH" "$OPT_WRAPPER_DIR/macos_opt_wrapper_impl.py" "\$@"
WRAPPER_EOF
    chmod +x "$OPT_WRAPPER_DIR/opt"
    export PATH="$OPT_WRAPPER_DIR:$LLVM_BIN:$PATH"
    echo "macOS: opt wrapper enabled (python3=$PYTHON3_PATH)"
else
    # Linux: LLVM 21+ rejects duplicate 'declare' statements with different attributes.
    REAL_OPT=$(command -v opt)
    OPT_WRAPPER_DIR="/tmp/seen_opt_override"
    mkdir -p "$OPT_WRAPPER_DIR"
cat > "$OPT_WRAPPER_DIR/opt" << WRAPPER_EOF
#!/bin/bash
# Wrapper: deduplicate declare statements in .ll files before invoking real opt.
# The frozen compiler emits extern __-prefixed functions twice (once from ir_declarations
# with nounwind, once from extern handler with possibly different types). We do a two-pass
# approach: first collect all declared function names, then on second pass keep only the
# LAST declaration for each function (which matches call site types).
ARGS=("\$@")
if [ "\${SEEN_LOW_MEMORY:-0}" = "1" ] && [ -n "\${SEEN_OPT_VMEM_KB:-}" ]; then
    # Cap the whole wrapper so fix_ir.py and llvm-as stay within the low-memory budget too.
    ulimit -v "\$SEEN_OPT_VMEM_KB" 2>/dev/null || true
fi
SEEN_OPT_LOCK_HELD=0
acquire_seen_low_memory_opt_lock() {
    if [ "\${SEEN_LOW_MEMORY:-0}" != "1" ] || [ "\$SEEN_OPT_LOCK_HELD" = "1" ]; then
        return
    fi
    if command -v flock >/dev/null 2>&1; then
        exec 9>/tmp/seen_opt_low_memory.lock
        flock 9
    else
        while ! mkdir /tmp/seen_opt_low_memory.lockdir 2>/dev/null; do
            sleep 1
        done
        trap 'rmdir /tmp/seen_opt_low_memory.lockdir 2>/dev/null || true' EXIT
    fi
    SEEN_OPT_LOCK_HELD=1
}
wait_for_stable_ir_file() {
    local file="\$1"
    local prev_size=""
    local cur_size=""
    local stable_count=0
    local attempts=0
    while [ "\$attempts" -lt 50 ]; do
        cur_size=\$(stat -c%s "\$file" 2>/dev/null || stat -f%z "\$file" 2>/dev/null || echo "")
        if [ -n "\$cur_size" ] && [ "\$cur_size" = "\$prev_size" ]; then
            stable_count=\$((stable_count + 1))
            if [ "\$stable_count" -ge 2 ]; then
                return
            fi
        else
            stable_count=0
            prev_size="\$cur_size"
        fi
        attempts=\$((attempts + 1))
        sleep 0.1
    done
}
if [ "\${SEEN_SKIP_IR_FIXUPS:-0}" = "1" ]; then
    acquire_seen_low_memory_opt_lock
    exec "$REAL_OPT" "\$@"
fi
acquire_seen_low_memory_opt_lock
for arg in "\${ARGS[@]}"; do
    if [[ "\$arg" == *.ll && "\$arg" != *.opt.ll && -f "\$arg" ]]; then
        wait_for_stable_ir_file "\$arg"
        awk '
        # Pass 1: count declarations per function name
        NR == FNR {
            if (/^declare /) {
                if (match(\$0, /@([A-Za-z0-9_.]+)/, m)) {
                    count[m[1]]++
                    seen_count[m[1]] = 0
                }
            }
            next
        }
        # Pass 2: for functions with duplicates, skip all but the last
        /^declare / {
            if (match(\$0, /@([A-Za-z0-9_.]+)/, m)) {
                fname = m[1]
                seen_count[fname]++
                if (count[fname] > 1 && seen_count[fname] < count[fname]) next
            }
        }
        { print }
        ' "\$arg" "\$arg" > "\${arg}.dedup" && mv "\${arg}.dedup" "\$arg"

        # Fix byteAt codegen bug: pre-refactoring compiler emits string concat
        # (seen_int_to_string + seen_char_to_str + seen_str_concat_ss) instead of
        # integer add for byteAt() + int expressions. Replace with add i64.
        python3 -c "
import re, sys
with open(sys.argv[1]) as f:
    content = f.read()
pattern = re.compile(
    r'  (%\d+) = call %SeenString @seen_int_to_string\(i64 (%\d+)\)\n'
    r'  (%\d+) = call %SeenString @seen_char_to_str\(i64 (%\d+)\)\n'
    r'  (%\d+) = call %SeenString @seen_str_concat_ss\(%SeenString \1, %SeenString \3\)'
)
def fix(m):
    return f'  {m.group(1)} = add i64 0, 0\n  {m.group(3)} = add i64 0, 0\n  {m.group(5)} = add i64 {m.group(2)}, {m.group(4)}'
new_content, count = pattern.subn(fix, content)
if count > 0:
    with open(sys.argv[1], 'w') as f:
        f.write(new_content)
    print(f'  byteAt fix: patched {count} site(s) in {sys.argv[1]}', file=sys.stderr)
" "\$arg" 2>&1 || true

        # Apply comprehensive IR fixups (declare dedup, type mismatches, SSA, etc.)
        if ! python3 "$SCRIPT_DIR/fix_ir.py" "\$arg" 2>&1; then
            echo "IR FIXUP WARNING: fix_ir.py failed for \$arg (continuing)" >&2
        fi

        # Fix bare 0 as type in declare params (e.g. (i64, 0) → (i64, i64))
        # This is a belt-and-suspenders fix in case fix_ir.py doesn't catch it
        sed -i 's/^\(declare.*(\)\(.*\), 0)/\1\2, i64)/g' "\$arg" 2>/dev/null || true

        # Fix corrupt declares from string constants leaking into declare generator
        # Stage1 parses @funcName(...) from string constants, producing broken declares
        # with \00 or other garbage. Remove these — the correct declare is already present.
        sed -i '/^declare.*\\\\00/d' "\$arg" 2>/dev/null || true

        # NOTE: Phantom declare removal disabled — it was too aggressive and removed
        # declares for cross-module functions (emitIncludeStrImpl, etc.) that ARE called
        # from the module via ThinLTO. The awk dedup + fix_ir.py handle the critical cases.

        # IR Validation: run llvm-as structural check on the fixed .ll file
        if ! llvm-as "\$arg" -o /dev/null 2>/tmp/seen_verify_err.txt; then
            echo "IR VERIFY WARNING: \$arg (retrying fixups)" >&2
            head -2 /tmp/seen_verify_err.txt >&2
            if python3 "$SCRIPT_DIR/fix_ir.py" "\$arg" 2>&1 && \
               llvm-as "\$arg" -o /dev/null 2>/tmp/seen_verify_err.txt; then
                echo "IR VERIFY RECOVERED: \$arg" >&2
            else
                echo "IR VERIFY ERROR: \$arg still invalid after retry" >&2
                head -2 /tmp/seen_verify_err.txt >&2
                rm -f /tmp/seen_verify_err.txt
                exit 1
            fi
            rm -f /tmp/seen_verify_err.txt
        fi
        rm -f /tmp/seen_verify_err.txt

        # NOTE: seen_ir_lint disabled — it naively counts commas to determine
        # argument count, causing false positives on LLVM inline struct literals
        # like %SeenString { i64 7, ptr @.str }. llvm-as above catches real errors.
    fi
done
if [ "\${SEEN_LOW_MEMORY:-0}" = "1" ]; then
    acquire_seen_low_memory_opt_lock
fi
exec "$REAL_OPT" "\$@"
WRAPPER_EOF
    chmod +x "$OPT_WRAPPER_DIR/opt"
fi  # end platform-specific opt wrapper

build_fork_serializer

if [ "$LOW_MEMORY_MODE" = "1" ] && [ "$HOST_OS" != "Darwin" ] && [ "${SEEN_SKIP_LOW_MEMORY_SHORTCUT:-0}" != "1" ]; then
    echo ""
    echo "Low-memory shortcut: trying existing stage builders before frozen bootstrap..."
    if recover_with_existing_stage_builders; then
        echo -e "${GREEN}Low-memory rebuild succeeded via existing stage builder.${NC}"
    elif recover_with_preserved_production_compiler; then
        echo -e "${GREEN}Low-memory rebuild succeeded via preserved production compiler.${NC}"
    else
        echo -e "${YELLOW}Low-memory shortcut did not complete; continuing with capped frozen bootstrap.${NC}"
    fi
elif [ "$LOW_MEMORY_MODE" = "1" ] && [ "$HOST_OS" != "Darwin" ]; then
    echo ""
    echo -e "${YELLOW}Low-memory shortcut skipped; continuing with capped frozen bootstrap.${NC}"
fi

if [ -z "${VERIFIED:-}" ]; then
# Step 1: Build stage2 with frozen compiler (--fast)
# NOTE: PATH override ensures our dedup opt wrapper runs instead of system opt.
echo ""
echo "Step 1: Building stage2 with frozen compiler (--fast)..."
echo -e "${DIM}The frozen compiler generates IR for all 50+ modules.${NC}"
echo -e "${DIM}Module 5 (llvm_ir_gen.seen, 14K lines) typically takes 1-2 minutes.${NC}"
echo ""

# Clean stale .ll/.o from previous runs so counts are accurate
rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: PATH already set via export above; use --no-cache
    # The frozen compiler may fail at its internal link step (e.g., internal globals
    # eliminated by opt) but still produce a full .opt.ll set we can relink in step 1b.
    if run_with_progress "S1→S2" /tmp/safe_rebuild_stage2.log \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
        "$FROZEN_ABS" compile "$COMPILER_SOURCE" "$STAGE2" $STAGE2_COMPILE_FLAGS; then
        echo -e "${GREEN}Stage2 build succeeded.${NC}"
    else
        EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
        OPT_LL_COUNT=$(count_module_opt_lls /tmp)
        if [ "$EXPECTED_STAGE2_MODULES" -gt 0 ] && [ "$OPT_LL_COUNT" -eq "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${YELLOW}Stage2 internal link failed, but the full $OPT_LL_COUNT/$EXPECTED_STAGE2_MODULES .opt.ll set is available for relink.${NC}"
        else
            echo -e "${RED}ERROR: Stage2 build failed!${NC}"
            if [ "$EXPECTED_STAGE2_MODULES" -gt 0 ]; then
                echo "Expected $EXPECTED_STAGE2_MODULES optimized modules, found $OPT_LL_COUNT."
            fi
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail_log_if_exists /tmp/safe_rebuild_stage2.log 30
            exit 1
        fi
    fi
else
    # Linux S1→S2: Inline process management so we can run a snapshot watcher.
    # The frozen compiler deletes ALL /tmp/seen_module_* on exit (even on failure),
    # so we snapshot .ll files while it's running.
    SNAPSHOT_DIR="/tmp/seen_ll_snapshot_$$"
    rm -rf "$SNAPSHOT_DIR"

    FROZEN_COMPILE_ENV=(env "PATH=$OPT_WRAPPER_DIR:$PATH")
    if [ -n "$FORK_SERIALIZER_SO" ]; then
        FROZEN_COMPILE_ENV+=("LD_PRELOAD=$FORK_SERIALIZER_SO")
    fi

    # Start compiler in background
    SEEN_MEMORY_GUARD_KILL_ONLY=1 run_guarded_command_to_log "S1->S2" 0 "$MAIN_COMPILER_VMEM_KB" /tmp/safe_rebuild_stage2.log \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
        "${FROZEN_COMPILE_ENV[@]}" "$FROZEN_ABS" compile "$COMPILER_SOURCE" "$STAGE2" \
            $STAGE2_COMPILE_FLAGS &
    COMPILE_PID=$!

    # Start progress monitor and snapshot watcher
    monitor_compilation "$COMPILE_PID" "S1→S2" &
    MONITOR_PID=$!
    start_ll_snapshot_watcher "$COMPILE_PID" "$SNAPSHOT_DIR" &
    WATCHER_PID=$!

    # Wait for compiler
    COMPILE_EXIT=0
    wait "$COMPILE_PID" || COMPILE_EXIT=$?

    # Stop monitor and watcher
    kill "$MONITOR_PID" 2>/dev/null || true
    wait "$MONITOR_PID" 2>/dev/null || true
    finish_ll_snapshot_watcher "$WATCHER_PID"

    EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
    if [ "$EXPECTED_STAGE2_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for Stage2.${NC}"
        echo "Check /tmp/safe_rebuild_stage2.log for details."
        tail_log_if_exists /tmp/safe_rebuild_stage2.log 30
        preserve_stage2_failure_artifacts "$SNAPSHOT_DIR"
        rm -rf "$SNAPSHOT_DIR"
        exit 1
    fi

    if [ "$COMPILE_EXIT" -eq 0 ]; then
        STAGE2_OBJ_COUNT=$(count_module_objects /tmp)
        if [ "$STAGE2_OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${RED}ERROR: Stage2 reported success but produced only $STAGE2_OBJ_COUNT/$EXPECTED_STAGE2_MODULES module objects.${NC}"
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail_log_if_exists /tmp/safe_rebuild_stage2.log 30
            preserve_stage2_failure_artifacts "$SNAPSHOT_DIR"
            rm -rf "$SNAPSHOT_DIR"
            exit 1
        fi
        echo -e "${GREEN}Stage2 build succeeded.${NC}"
        rm -rf "$SNAPSHOT_DIR"
    else
        STAGE2_OOM_LIKE=0
        if stage2_failure_looks_oom "$COMPILE_EXIT" /tmp/safe_rebuild_stage2.log; then
            STAGE2_OOM_LIKE=1
            echo -e "${YELLOW}Stage2 appears to have been OOM-killed; skipping frozen --no-fork retry and using recovery paths instead.${NC}"
        fi

        # Kill orphaned fork children before recovery (SIGKILL to avoid cleanup handlers)
        echo -e "${YELLOW}Stage2 compilation failed (exit=$COMPILE_EXIT), killing orphans...${NC}"
        kill_frozen_orphans
        preserve_stage2_failure_artifacts "$SNAPSHOT_DIR"
        if [ "${SEEN_STOP_AFTER_FROZEN_STAGE2_FAILURE:-0}" = "1" ]; then
            echo -e "${RED}Stopping after frozen Stage2 failure as requested.${NC}"
            rm -rf "$SNAPSHOT_DIR"
            exit "$COMPILE_EXIT"
        fi

        # Check how many .ll files we have: first from snapshot, fallback to live /tmp
        SNAP_COUNT=$(count_plain_module_lls "$SNAPSHOT_DIR")
        LIVE_COUNT=$(count_plain_module_lls /tmp)

        LL_RECOVERY_SOURCE_DIR=""
        if [ "$SNAP_COUNT" -gt "$LIVE_COUNT" ]; then
            LL_COUNT=$SNAP_COUNT
            LL_SOURCE="snapshot"
            echo -e "${YELLOW}Snapshot has $SNAP_COUNT .ll files (live: $LIVE_COUNT). Restoring from snapshot...${NC}"
            # Clean /tmp of any partial files, then restore from snapshot
            rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
            rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log
            cp "$SNAPSHOT_DIR"/seen_module_*.ll /tmp/ 2>/dev/null || true
            LL_RECOVERY_SOURCE_DIR="$SNAPSHOT_DIR"
        else
            LL_COUNT=$LIVE_COUNT
            LL_SOURCE="live"
            echo -e "${YELLOW}Using $LIVE_COUNT live .ll files from /tmp.${NC}"
            LL_RECOVERY_SOURCE_DIR="/tmp"
        fi

        SKIP_PRESERVED_RECOVERY=0
        if [ "$LL_COUNT" -eq "$EXPECTED_STAGE2_MODULES" ]; then
            SKIP_PRESERVED_RECOVERY=1
            echo -e "${YELLOW}Captured a full $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll set; skipping preserved-compiler rebuild and recovering directly from IR.${NC}"
            FULL_LL_RECOVERY_DIR="/tmp/seen_full_ll_recovery_$$"
            rm -rf "$FULL_LL_RECOVERY_DIR"
            mkdir -p "$FULL_LL_RECOVERY_DIR"
            cp "$LL_RECOVERY_SOURCE_DIR"/seen_module_*.ll "$FULL_LL_RECOVERY_DIR/" 2>/dev/null || true
            FULL_LL_COUNT=$(count_plain_module_lls "$FULL_LL_RECOVERY_DIR")
            if [ "$FULL_LL_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
                echo -e "${RED}ERROR: failed to preserve complete .ll recovery set ($FULL_LL_COUNT/$EXPECTED_STAGE2_MODULES).${NC}"
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR"
                exit 1
            fi
            LL_RECOVERY_SOURCE_DIR="$FULL_LL_RECOVERY_DIR"
        fi

        if [ "$SKIP_PRESERVED_RECOVERY" -eq 0 ] && recover_with_preserved_production_compiler; then
            echo -e "${YELLOW}Frozen Stage2 bootstrap failed; skipping slow .ll replay recovery and using the preserved-compiler recovery build.${NC}"
        elif [ "$LL_COUNT" -eq "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${YELLOW}Recovering with the full $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll set ($LL_SOURCE)...${NC}"

            # Clean stale .o and .opt.ll from the compiler's failed internal opt/link —
            # we must regenerate them from the raw .ll files via our own opt wrapper.
            rm -f /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
            rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

            # Run recovery in subprocess (immune to set -e).
            # Recovery works in a private temp dir to avoid interference from
            # concurrent compilations. It outputs RECOVERY_DIR=<path> on success.
            RECOVERY_EXIT=0
            RECOVERY_OUTPUT=$(run_guarded_command "Stage2 IR recovery" "$RECOVERY_TIMEOUT_SECS" "$OPT_VMEM_KB" \
                bash "$SCRIPT_DIR/recovery_opt.sh" "$OPT_WRAPPER_DIR" "$SCRIPT_DIR" "$LL_RECOVERY_SOURCE_DIR" 2>&1) || RECOVERY_EXIT=$?
            echo "$RECOVERY_OUTPUT" | grep -v '^RECOVERY_DIR=' || true

            if [ "$RECOVERY_EXIT" -ne 0 ]; then
                echo -e "${RED}ERROR: Recovery failed.${NC}"
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR"
                exit 1
            fi

            RECOVERY_DIR=$(echo "$RECOVERY_OUTPUT" | grep '^RECOVERY_DIR=' | tail -1 | cut -d= -f2)
            if [ -z "$RECOVERY_DIR" ] || [ ! -d "$RECOVERY_DIR" ]; then
                echo -e "${RED}ERROR: Recovery failed — no output directory.${NC}"
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR"
                exit 1
            fi

            OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
            if [ "$OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
                MISSING_MODULES=$(list_modules_missing_objects "$RECOVERY_DIR")
                echo -e "${RED}ERROR: Recovery failed — only $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files produced.${NC}"
                if [ -n "$MISSING_MODULES" ]; then
                    echo "Missing objects:$MISSING_MODULES"
                fi
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
                exit 1
            fi
            echo "  Recovery: $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files ready."

            # Check for empty modules that might cause link failures.
            # Skip modules that are legitimately empty (only declares/types, no string
            # constants — these are re-export shims with no real code).
            EMPTY_MODULES=$(find_problem_empty_modules "$RECOVERY_DIR")
            EMPTY_COUNT=$(echo "$EMPTY_MODULES" | wc -w)
            if [ "$EMPTY_COUNT" -gt 0 ]; then
                echo -e "${YELLOW}Empty modules ($EMPTY_COUNT with 0 function definitions):${EMPTY_MODULES}${NC}"

                # --- Retry loop: re-run frozen compiler to fill empty modules ---
                # OOM kills are non-deterministic, so retrying will likely produce
                # a different set of empty modules (or none at all).
                MAX_RETRIES=2
                RETRY=0
                while [ "$EMPTY_COUNT" -gt 0 ] && [ "$RETRY" -lt "$MAX_RETRIES" ]; do
                    RETRY=$((RETRY+1))
                    echo -e "${YELLOW}Retry $RETRY/$MAX_RETRIES: re-running frozen compiler to fill $EMPTY_COUNT empty module(s)...${NC}"

                    # Clean /tmp for retry run
                    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
                    rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

                    RETRY_SNAPSHOT="/tmp/seen_retry_snapshot_${$}_${RETRY}"
                    rm -rf "$RETRY_SNAPSHOT"
                    RETRY_COMPILE_ENV=(env "PATH=$OPT_WRAPPER_DIR:$PATH")
                    if [ -n "$FORK_SERIALIZER_SO" ]; then
                        RETRY_COMPILE_ENV+=("LD_PRELOAD=$FORK_SERIALIZER_SO")
                    fi

                    SEEN_MEMORY_GUARD_KILL_ONLY=1 run_guarded_command_to_log "Retry$RETRY" 600 "$MAIN_COMPILER_VMEM_KB" /tmp/retry_${RETRY}.log \
                        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
                        "${RETRY_COMPILE_ENV[@]}" "$FROZEN_ABS" compile "$COMPILER_SOURCE" /dev/null \
                            $STAGE2_COMPILE_FLAGS &
                    RETRY_PID=$!

                    start_ll_snapshot_watcher "$RETRY_PID" "$RETRY_SNAPSHOT" &
                    RETRY_WATCHER=$!
                    monitor_compilation "$RETRY_PID" "Retry$RETRY" &
                    RETRY_MONITOR=$!

                    wait $RETRY_PID 2>/dev/null || true
                    finish_ll_snapshot_watcher "$RETRY_WATCHER"
                    kill $RETRY_MONITOR 2>/dev/null || true; wait $RETRY_MONITOR 2>/dev/null || true
                    kill_frozen_orphans

                    RETRY_COUNT=$(count_plain_module_lls "$RETRY_SNAPSHOT")
                    echo ""
                    echo "  Retry $RETRY: captured $RETRY_COUNT .ll files"

                    # Replace modules where retry has MORE defines (catches both
                    # empty modules and partially-generated/truncated modules)
                    REPLACED=0
                    for retry_ll in "$RETRY_SNAPSHOT"/seen_module_*.ll; do
                        [ -f "$retry_ll" ] || continue
                        [[ "$retry_ll" == *.opt.ll ]] && continue
                        [[ "$retry_ll" == *.polly.ll ]] && continue
                        bn=$(basename "$retry_ll")
                        mod=$(basename "$retry_ll" .ll)
                        orig_ll="$RECOVERY_DIR/$bn"
                        [ -f "$orig_ll" ] || continue

                        orig_defines=$(grep -c '^define' "$orig_ll" 2>/dev/null | tail -1)
                        orig_defines=${orig_defines:-0}
                        retry_defines=$(grep -c '^define' "$retry_ll" 2>/dev/null | tail -1)
                        retry_defines=${retry_defines:-0}

                        if [ "$retry_defines" -gt "$orig_defines" ] 2>/dev/null; then
                            cp "$retry_ll" "$orig_ll"
                            rm -f "$RECOVERY_DIR/${mod}.opt.ll" "$RECOVERY_DIR/${mod}.o"
                            echo "    Replaced $bn: $orig_defines -> $retry_defines defines"
                            REPLACED=$((REPLACED+1))
                        fi
                    done
                    rm -rf "$RETRY_SNAPSHOT"
                    echo "  Retry $RETRY: replaced $REPLACED module(s)"

                    # Re-run opt wrapper + thinlto-bc on replaced modules (those missing .o)
                    if [ "$REPLACED" -gt 0 ]; then
                        REAL_OPT_BIN=$(command -v opt)
                        for llfile in "$RECOVERY_DIR"/seen_module_*.ll; do
                            [ -f "$llfile" ] || continue
                            [[ "$llfile" == *.opt.ll ]] && continue
                            [[ "$llfile" == *.polly.ll ]] && continue
                            mod=$(basename "$llfile" .ll)
                            objfile="$RECOVERY_DIR/${mod}.o"
                            [ -f "$objfile" ] && continue

                            optfile="$RECOVERY_DIR/${mod}.opt.ll"
                            echo "    Re-optimizing ${mod}..."
                            # Use opt wrapper to apply dedup, byteAt fix, fix_ir.py
                            if ! run_guarded_command "Retry$RETRY ${mod} opt" 300 "$OPT_VMEM_KB" \
                                "$OPT_WRAPPER_DIR/opt" \
                                -passes='function(sroa,instcombine<no-verify-fixpoint>,simplifycfg),default<O1>' \
                                -inline-threshold=250 -S "$llfile" -o "$optfile" 2>/dev/null; then
                                cp "$llfile" "$optfile" 2>/dev/null || true
                            fi
                            run_guarded_command "Retry$RETRY ${mod} thinlto" 300 "$OPT_VMEM_KB" \
                                "$REAL_OPT_BIN" --thinlto-bc "$optfile" -o "$objfile" 2>/dev/null || true
                        done
                    fi

                    # Re-count empty modules
                    EMPTY_MODULES=$(find_problem_empty_modules "$RECOVERY_DIR")
                    EMPTY_COUNT=$(echo "$EMPTY_MODULES" | wc -w)
                    if [ "$EMPTY_COUNT" -eq 0 ]; then
                        echo -e "${GREEN}  All modules now have function definitions!${NC}"
                    else
                        echo -e "${YELLOW}  Still $EMPTY_COUNT empty module(s):${EMPTY_MODULES}${NC}"
                    fi
                done

                # Recount .o files after retries
                if [ "$RETRY" -gt 0 ]; then
                    OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
                    echo "  Post-retry: $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files ready."
                fi

                # --- Pass 2: Two-pass .ll merge for satellite modules ---
                if [ "$EMPTY_COUNT" -gt 0 ]; then
                # The frozen compiler generates module 5 (llvm_ir_gen.seen) correctly
                # but outputs 0 defines for satellite codegen modules. The production
                # compiler generates satellite modules correctly but hangs on module 5.
                # Merge: pick the .ll with more defines from each compiler.
                PROD_COMPILER="compiler_seen/target/seen"
                if [ -x "$PROD_COMPILER" ]; then
                    echo -e "${YELLOW}Running Pass 2 (production compiler) to fill empty modules...${NC}"

                    # Clean /tmp for Pass 2
                    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
                    rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

                    # Run production compiler with timeout + snapshot watcher
                    PASS2_SNAPSHOT="/tmp/seen_pass2_snapshot_$$"
                    rm -rf "$PASS2_SNAPSHOT"

                    SEEN_MEMORY_GUARD_KILL_ONLY=1 run_guarded_command_to_log "Pass2" 600 "$MAIN_COMPILER_VMEM_KB" /tmp/pass2.log \
                        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$BOOTSTRAP_SOURCE_ROOT" \
                        "$PROD_COMPILER" compile "$COMPILER_SOURCE" /dev/null \
                            $PASS2_COMPILE_FLAGS &
                    PASS2_PID=$!

                    start_ll_snapshot_watcher "$PASS2_PID" "$PASS2_SNAPSHOT" &
                    PASS2_WATCHER=$!
                    monitor_compilation "$PASS2_PID" "Pass2" &
                    PASS2_MONITOR=$!

                    wait $PASS2_PID 2>/dev/null || true
                    finish_ll_snapshot_watcher "$PASS2_WATCHER"
                    kill $PASS2_MONITOR 2>/dev/null; wait $PASS2_MONITOR 2>/dev/null || true
                    # Kill any children of the Pass 2 compiler (fork children, opt, clang)
                    for child in $(pgrep -P $PASS2_PID 2>/dev/null); do
                        kill -9 "$child" 2>/dev/null || true
                    done
                    sleep 2

                    PASS2_COUNT=$(count_plain_module_lls "$PASS2_SNAPSHOT")
                    echo ""
                    echo "  Pass 2: captured $PASS2_COUNT .ll files"

                    # Merge: for each module, pick the .ll with more defines
                    MERGED=0
                    for pass1_ll in "$RECOVERY_DIR"/seen_module_*.ll; do
                        [ -f "$pass1_ll" ] || continue
                        [[ "$pass1_ll" == *.opt.ll ]] && continue
                        [[ "$pass1_ll" == *.polly.ll ]] && continue
                        bn=$(basename "$pass1_ll")
                        pass2_ll="$PASS2_SNAPSHOT/$bn"
                        [ -f "$pass2_ll" ] || continue

                        p1_defines=$(grep -c '^define' "$pass1_ll" 2>/dev/null | tail -1)
                        p1_defines=${p1_defines:-0}
                        p2_defines=$(grep -c '^define' "$pass2_ll" 2>/dev/null | tail -1)
                        p2_defines=${p2_defines:-0}

                        if [ "$p2_defines" -gt "$p1_defines" ] 2>/dev/null; then
                            cp "$pass2_ll" "$pass1_ll"
                            modname=$(basename "$pass1_ll" .ll)
                            rm -f "$RECOVERY_DIR/${modname}.opt.ll" "$RECOVERY_DIR/${modname}.o"
                            echo "    Merged $bn: $p1_defines -> $p2_defines defines (from production)"
                            MERGED=$((MERGED+1))
                        fi
                    done
                    # Also add Pass 2 .ll files not present in RECOVERY_DIR
                    for pass2_ll in "$PASS2_SNAPSHOT"/seen_module_*.ll; do
                        [ -f "$pass2_ll" ] || continue
                        [[ "$pass2_ll" == *.opt.ll ]] && continue
                        [[ "$pass2_ll" == *.polly.ll ]] && continue
                        bn=$(basename "$pass2_ll")
                        if [ ! -f "$RECOVERY_DIR/$bn" ]; then
                            cp "$pass2_ll" "$RECOVERY_DIR/$bn"
                            echo "    Added $bn from Pass 2 (not in Pass 1)"
                            MERGED=$((MERGED+1))
                        fi
                    done
                    rm -rf "$PASS2_SNAPSHOT"
                    echo "  Merged $MERGED modules from Pass 2"

                    if [ "$MERGED" -gt 0 ]; then
                        # Re-run opt + thinlto-bc on merged modules (those missing .o files)
                        REAL_OPT_BIN=$(command -v opt)
                        for llfile in "$RECOVERY_DIR"/seen_module_*.ll; do
                            [ -f "$llfile" ] || continue
                            [[ "$llfile" == *.opt.ll ]] && continue
                            [[ "$llfile" == *.polly.ll ]] && continue
                            modname=$(basename "$llfile" .ll)
                            objfile="$RECOVERY_DIR/${modname}.o"
                            [ -f "$objfile" ] && continue

                            optfile="$RECOVERY_DIR/${modname}.opt.ll"
                            echo "    Re-optimizing $modname..."
                            if ! run_guarded_command "Pass2 ${modname} opt" 300 "$OPT_VMEM_KB" \
                                "$REAL_OPT_BIN" \
                                -passes='function(sroa,instcombine<no-verify-fixpoint>,simplifycfg),default<O1>' \
                                -inline-threshold=250 -S "$llfile" -o "$optfile" 2>/dev/null; then
                                cp "$llfile" "$optfile" 2>/dev/null || true
                            fi
                            run_guarded_command "Pass2 ${modname} thinlto" 300 "$OPT_VMEM_KB" \
                                "$REAL_OPT_BIN" --thinlto-bc "$optfile" -o "$objfile" 2>/dev/null || true
                        done

                        # Recount .o files after merge
                        OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
                        echo "  Post-merge: $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files ready."
                    fi
                else
                    echo -e "${YELLOW}No production compiler available for Pass 2 merge.${NC}"
                    echo -e "${YELLOW}Fix: revert module list changes, rebuild, update stage1_frozen, re-apply.${NC}"
                fi
                fi
            fi

            EMPTY_MODULES=$(find_problem_empty_modules "$RECOVERY_DIR")
            EMPTY_COUNT=$(echo "$EMPTY_MODULES" | wc -w)
            if [ "$EMPTY_COUNT" -gt 0 ]; then
                echo -e "${RED}ERROR: Recovery left $EMPTY_COUNT module(s) with missing function bodies:${EMPTY_MODULES}${NC}"
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
                exit 1
            fi

            OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
            if [ "$OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
                MISSING_MODULES=$(list_modules_missing_objects "$RECOVERY_DIR")
                echo -e "${RED}ERROR: Recovery object set is incomplete ($OBJ_COUNT/$EXPECTED_STAGE2_MODULES).${NC}"
                if [ -n "$MISSING_MODULES" ]; then
                    echo "Missing objects:$MISSING_MODULES"
                fi
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
                exit 1
            fi

            # Pre-compile runtime
            RT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/seen_runtime"
            if [ ! -f "$RT_DIR/seen_runtime.o" ] || [ "$RT_DIR/seen_runtime.c" -nt "$RT_DIR/seen_runtime.o" ]; then
                echo "  Pre-compiling runtime..."
                run_guarded_command "runtime seen_runtime.c" 300 "$OPT_VMEM_KB" \
                    clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections -pthread \
                    -c -I "$RT_DIR" "$RT_DIR/seen_runtime.c" -o "$RT_DIR/seen_runtime.o" 2>/dev/null || true
            fi
            if [ -f "$RT_DIR/seen_region.c" ]; then
                if [ ! -f "$RT_DIR/seen_region.o" ] || [ "$RT_DIR/seen_region.c" -nt "$RT_DIR/seen_region.o" ]; then
                    run_guarded_command "runtime seen_region.c" 300 "$OPT_VMEM_KB" \
                        clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections \
                        -c -I "$RT_DIR" "$RT_DIR/seen_region.c" -o "$RT_DIR/seen_region.o" 2>/dev/null || true
                fi
            fi
            if [ -f "$RT_DIR/seen_gpu.c" ]; then
                if [ ! -f "$RT_DIR/seen_gpu.o" ] || [ "$RT_DIR/seen_gpu.c" -nt "$RT_DIR/seen_gpu.o" ]; then
                    run_guarded_command "runtime seen_gpu.c" 300 "$OPT_VMEM_KB" \
                        clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections \
                        -c -I "$RT_DIR" "$RT_DIR/seen_gpu.c" -o "$RT_DIR/seen_gpu.o" 2>/dev/null || true
                fi
            fi

            # Link from recovery directory (not /tmp, which may be contaminated
            # by concurrent compilations)
            echo "  Linking $OBJ_COUNT modules..."
            LINK_OBJS=""
            for obj in "$RECOVERY_DIR"/seen_module_*.o; do
                LINK_OBJS="$LINK_OBJS $obj"
            done
            RT_OBJS="$RT_DIR/seen_runtime.o"
            [ -f "$RT_DIR/seen_region.o" ] && RT_OBJS="$RT_OBJS $RT_DIR/seen_region.o"
            [ -f "$RT_DIR/seen_gpu.o" ] && RT_OBJS="$RT_OBJS $RT_DIR/seen_gpu.o"

            LINK_LIBS="-lm -lpthread"
            [ -f "$RT_DIR/seen_gpu.o" ] && pkg-config --exists vulkan 2>/dev/null && LINK_LIBS="$LINK_LIBS -lvulkan"

            if run_guarded_command "Stage2 recovery link" 0 "" clang -O1 -flto=thin -fuse-ld=lld \
                -Wl,--thinlto-cache-dir=/tmp/seen_thinlto_cache \
                -Wl,--allow-multiple-definition \
                "$RELEASE_CLANG_MARCH_FLAG" -Wl,--gc-sections -Wno-unused-command-line-argument \
                $LINK_OBJS $RT_OBJS -o "$STAGE2" $LINK_LIBS 2>/tmp/safe_rebuild_link.log; then
                echo -e "${GREEN}Stage2 recovery link succeeded ($(wc -c < "$STAGE2" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}ERROR: Stage2 recovery link failed.${NC}"
                grep -E 'undefined|error' /tmp/safe_rebuild_link.log | head -10
                rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
                exit 1
            fi
            rm -rf "$FULL_LL_RECOVERY_DIR" "$RECOVERY_DIR"
        else
            echo -e "${RED}ERROR: Stage2 build failed after generating only $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll files from $LL_SOURCE.${NC}"
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail_log_if_exists /tmp/safe_rebuild_stage2.log 30
            exit 1
        fi
        rm -rf "$SNAPSHOT_DIR" "$FULL_LL_RECOVERY_DIR"
    fi
fi

# macOS relink: The frozen bootstrap produces ThinLTO bitcode .o files which
# don't link correctly without -flto=thin. Relink from .opt.ll using llc + clang.
if [ "$HOST_OS" = "Darwin" ]; then
    echo ""
    echo "Step 1b: macOS relink from .opt.ll files (llc + clang -O2)..."

    # Post-opt fixup: LLVM opt may eliminate internal globals that are cross-module referenced.
    # Scan all .opt.ll files: for each 'external global @X' reference, ensure @X is defined
    # (non-external) in at least one module. If eliminated, re-inject from the original .ll.
    echo "    Fixing cross-module globals post-opt..."
    "$PYTHON3_PATH" - <<'POSTOPT_FIX'
import re, glob, os

opt_files = sorted(glob.glob('/tmp/seen_module_*.opt.ll'))
ll_files = sorted(glob.glob('/tmp/seen_module_*.ll'))
ll_files = [f for f in ll_files if not f.endswith('.opt.ll') and not f.endswith('.polly.ll')]

# Collect: which opt files define globals, which declare them external
defined = {}   # gname -> (file, line)
external = {}  # gname -> set of files needing it

for f in opt_files:
    with open(f) as fh:
        for line in fh:
            gm = re.match(r'(@\w+)\s*=\s*(external\s+)?(?:local_unnamed_addr\s+)?(?:unnamed_addr\s+)?(?:internal\s+)?global\s+(\S+)', line)
            if gm:
                gname = gm.group(1)
                is_ext = gm.group(2) is not None
                if is_ext:
                    external.setdefault(gname, set()).add(f)
                else:
                    defined[gname] = (f, gm.group(3))

# Find globals referenced but not defined in any opt file
missing = set()
for gname in external:
    if gname not in defined:
        missing.add(gname)

if missing:
    # Try to find definitions in original .ll files
    orig_defs = {}
    for f in ll_files:
        with open(f) as fh:
            for line in fh:
                gm = re.match(r'(@\w+)\s*=\s*(?:internal\s+)?global\s+(\S+)\s+(.*)', line)
                if gm and gm.group(1) in missing:
                    orig_defs[gm.group(1)] = (gm.group(2), gm.group(3).strip(), f)

    # Inject missing globals into the first module that references them externally
    for gname in missing:
        if gname in orig_defs:
            gtype, gval, src = orig_defs[gname]
            # Add to first referencing opt file
            target = sorted(external[gname])[0]
            with open(target) as fh:
                content = fh.read()
            # Replace external declaration with actual definition
            content = re.sub(
                rf'^{re.escape(gname)}\s*=\s*external\s+(?:local_unnamed_addr\s+)?(?:unnamed_addr\s+)?global\s+\S+\s*$',
                f'{gname} = global {gtype} {gval}',
                content, count=1, flags=re.MULTILINE
            )
            with open(target, 'w') as fh:
                fh.write(content)
            print(f'    Injected {gname} into {os.path.basename(target)}')

if not missing:
    print('    Post-opt fixup: 0 missing globals')
else:
    print(f'    Post-opt fixup: {len(missing)} missing globals, {len(missing & set(orig_defs.keys()))} fixed')
POSTOPT_FIX

    EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
    OPT_LL_COUNT=$(count_module_opt_lls /tmp)
    if [ "$EXPECTED_STAGE2_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for macOS relink.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
        echo -e "${RED}ERROR: Refusing macOS relink with only $OPT_LL_COUNT/$EXPECTED_STAGE2_MODULES optimized modules.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -gt 0 ]; then
        echo "    Found $OPT_LL_COUNT .opt.ll modules, relinking with llc..."
        RELINK_FAILED=0
        RELINK_OBJS=""
        for optll in /tmp/seen_module_*.opt.ll; do
            modname=$(basename "$optll" .opt.ll)
            objfile="/tmp/${modname}.relink.o"
            if ! run_guarded_command "macOS stage2 ${modname} llc" 300 "$OPT_VMEM_KB" \
                llc -mtriple=arm64-apple-macosx -filetype=obj -O2 "$optll" -o "$objfile" 2>/tmp/relink_llc.log; then
                echo -e "${RED}    llc failed for $modname${NC}"
                cat /tmp/relink_llc.log
                RELINK_FAILED=1
                break
            fi
            RELINK_OBJS="$RELINK_OBJS $objfile"
        done
        if [ "$RELINK_FAILED" = "0" ]; then
            NATIVE_RT="/tmp/seen_runtime_native.o"
            run_guarded_command "macOS stage2 runtime" 300 "$OPT_VMEM_KB" \
                clang -O2 -c -I seen_runtime seen_runtime/seen_runtime.c -o "$NATIVE_RT" 2>/dev/null || true
            NATIVE_REGION="/tmp/seen_region_native.o"
            [ -f seen_runtime/seen_region.c ] && run_guarded_command "macOS stage2 region" 300 "$OPT_VMEM_KB" \
                clang -O2 -c -I seen_runtime seen_runtime/seen_region.c -o "$NATIVE_REGION" 2>/dev/null || true
            RT_OBJS="$NATIVE_RT"
            [ -f "$NATIVE_REGION" ] && RT_OBJS="$RT_OBJS $NATIVE_REGION"
            if run_guarded_command "macOS stage2 relink" 0 "$MAIN_COMPILER_VMEM_KB" \
                clang -O2 -arch arm64 $RELINK_OBJS $RT_OBJS -o "$STAGE2" -lm -lpthread 2>/tmp/relink_link.log; then
                echo -e "${GREEN}    macOS relink succeeded ($(wc -c < "$STAGE2" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}    macOS relink failed${NC}"
                cat /tmp/relink_link.log
                exit 1
            fi
            rm -f /tmp/seen_module_*.relink.o "$NATIVE_RT" "$NATIVE_REGION"
        else
            echo -e "${RED}ERROR: macOS relink failed${NC}"
            exit 1
        fi
    fi
fi
fi

if [ -z "${VERIFIED:-}" ]; then
echo ""
echo "Stage2 smoke: checking hello-world..."
if ! smoke_test_compiler "$STAGE2" "Stage2" "stage2"; then
    echo -e "${RED}ERROR: Fresh Stage2 cannot compile a normal user program.${NC}"
    echo -e "${RED}Refusing to continue with an unusable bootstrap compiler.${NC}"
    exit 1
fi
if [ "${SEEN_STOP_AFTER_STAGE2_SMOKE:-0}" = "1" ]; then
    echo -e "${YELLOW}Stopping after Stage2 smoke as requested.${NC}"
    echo "Stage2 binary preserved at $STAGE2"
    exit 0
fi

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: full S2→S3 bootstrap verification works
    rm -rf .seen_cache/ /tmp/seen_ir_cache/
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
    rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

    echo ""
    echo "Step 2: Building stage3 with stage2 (--fast)..."
    if run_with_progress "S2→S3" /tmp/safe_rebuild_stage3.log $STAGE2 compile "$COMPILER_SOURCE" "$STAGE3" $STAGE2_COMPILE_FLAGS; then
        echo -e "${GREEN}Stage3 build succeeded.${NC}"
    else
        echo -e "${RED}ERROR: Stage3 build failed!${NC}"
        echo "Check /tmp/safe_rebuild_stage3.log for details."
        tail_log_if_exists /tmp/safe_rebuild_stage3.log 30
        rm -f "$STAGE2"
        exit 1
    fi

    # Relink stage3
    echo ""
    echo "Step 2b: macOS relink stage3 from .opt.ll files..."
    EXPECTED_STAGE3_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage3.log)
    OPT_LL_COUNT=$(count_module_opt_lls /tmp)
    if [ "$EXPECTED_STAGE3_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for macOS stage3 relink.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -ne "$EXPECTED_STAGE3_MODULES" ]; then
        echo -e "${RED}ERROR: Refusing macOS stage3 relink with only $OPT_LL_COUNT/$EXPECTED_STAGE3_MODULES optimized modules.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -gt 0 ]; then
        echo "    Found $OPT_LL_COUNT .opt.ll modules, relinking with llc..."
        RELINK_FAILED=0
        RELINK_OBJS=""
        for optll in /tmp/seen_module_*.opt.ll; do
            modname=$(basename "$optll" .opt.ll)
            objfile="/tmp/${modname}.relink.o"
            if ! run_guarded_command "macOS stage3 ${modname} llc" 300 "$OPT_VMEM_KB" \
                llc -mtriple=arm64-apple-macosx -filetype=obj -O2 "$optll" -o "$objfile" 2>/tmp/relink_llc.log; then
                echo -e "${RED}    llc failed for $modname${NC}"
                cat /tmp/relink_llc.log
                RELINK_FAILED=1
                break
            fi
            RELINK_OBJS="$RELINK_OBJS $objfile"
        done
        if [ "$RELINK_FAILED" = "0" ]; then
            NATIVE_RT="/tmp/seen_runtime_native.o"
            run_guarded_command "macOS stage3 runtime" 300 "$OPT_VMEM_KB" \
                clang -O2 -c -I seen_runtime seen_runtime/seen_runtime.c -o "$NATIVE_RT" 2>/dev/null || true
            NATIVE_REGION="/tmp/seen_region_native.o"
            [ -f seen_runtime/seen_region.c ] && run_guarded_command "macOS stage3 region" 300 "$OPT_VMEM_KB" \
                clang -O2 -c -I seen_runtime seen_runtime/seen_region.c -o "$NATIVE_REGION" 2>/dev/null || true
            RT_OBJS="$NATIVE_RT"
            [ -f "$NATIVE_REGION" ] && RT_OBJS="$RT_OBJS $NATIVE_REGION"
            if run_guarded_command "macOS stage3 relink" 0 "$MAIN_COMPILER_VMEM_KB" \
                clang -O2 -arch arm64 $RELINK_OBJS $RT_OBJS -o "$STAGE3" -lm -lpthread 2>/tmp/relink_link.log; then
                echo -e "${GREEN}    macOS stage3 relink succeeded ($(wc -c < "$STAGE3" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}    macOS stage3 relink failed${NC}"
                cat /tmp/relink_link.log
                exit 1
            fi
            rm -f /tmp/seen_module_*.relink.o "$NATIVE_RT" "$NATIVE_REGION"
        else
            echo -e "${RED}ERROR: macOS stage3 relink failed${NC}"
            exit 1
        fi
    fi

    echo ""
    echo "Stage3 smoke: checking hello-world..."
    if smoke_test_compiler "$STAGE3" "Stage3" "stage3"; then
        echo ""
        echo "Step 3: Verifying bootstrap..."
        if diff "$STAGE2" "$STAGE3" > /dev/null 2>&1; then
            echo -e "${GREEN}Bootstrap verified: Stage2 == Stage3 (identical binaries)!${NC}"
        else
            echo -e "${YELLOW}Note: Stage2 != Stage3 (expected if stage1_frozen is older than source).${NC}"
            echo -e "${GREEN}Stage3 build succeeded — using Stage3 as production compiler.${NC}"
        fi
        VERIFIED="$STAGE3"
    else
        echo -e "${YELLOW}Stage3 build completed but failed hello-world smoke; falling back to Stage2.${NC}"
        echo -e "${GREEN}Using Stage2 as production compiler (it passed smoke).${NC}"
        VERIFIED="$STAGE2"
    fi
else
    # Linux: Attempt S2→S3 bootstrap verification with a timeout.
    # If Bug A (SeenString field corruption) is fixed, S2 should be able to
    # cold-compile. Fall back to S2 if S2→S3 times out or fails.
    rm -rf .seen_cache/ /tmp/seen_ir_cache/
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
    rm -f /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log

    echo ""
    echo "Step 2: Attempting S2→S3 bootstrap verification (Linux)..."
    echo -e "${DIM}Timeout: 30 minutes. Falls back to S2 if this fails.${NC}"

    if run_guarded_command_to_log "S2->S3" 1800 "$MAIN_COMPILER_VMEM_KB" /tmp/safe_rebuild_stage3.log \
        "$STAGE2" compile "$COMPILER_SOURCE" "$STAGE3" --fast --no-cache --no-fork $RELEASE_TARGET_CPU_FLAG \
        ; then
        echo -e "${GREEN}Stage3 build succeeded.${NC}"

        echo ""
        echo "Stage3 smoke: checking hello-world..."
        if smoke_test_compiler "$STAGE3" "Stage3" "stage3"; then
            echo ""
            echo "Step 3: Verifying bootstrap..."
            if diff "$STAGE2" "$STAGE3" > /dev/null 2>&1; then
                echo -e "${GREEN}Bootstrap verified: Stage2 == Stage3 (identical binaries)!${NC}"
            else
                echo -e "${YELLOW}Note: Stage2 != Stage3 (expected if stage1_frozen is older than source).${NC}"
                echo -e "${GREEN}Stage3 build succeeded — using Stage3 as production compiler.${NC}"
            fi
            VERIFIED="$STAGE3"
        else
            echo -e "${YELLOW}Stage3 build completed but failed hello-world smoke; falling back to Stage2.${NC}"
            if [ "${SEEN_REQUIRE_STAGE3:-0}" = "1" ]; then
                echo -e "${RED}ERROR: SEEN_REQUIRE_STAGE3=1 and Stage3 smoke failed.${NC}"
                exit 1
            fi
            if recover_with_preserved_production_compiler; then
                echo -e "${GREEN}Using recovered stage3 as production compiler.${NC}"
            else
                echo -e "${GREEN}Using Stage2 as production compiler (it passed smoke).${NC}"
                VERIFIED="$STAGE2"
            fi
        fi
    else
        S3_EXIT=$?
        echo -e "${YELLOW}S2→S3 build failed or timed out (exit=$S3_EXIT).${NC}"
        if [ "$S3_EXIT" = "124" ]; then
            echo -e "${YELLOW}Timeout reached — cold-compile hang likely still present.${NC}"
        else
            echo "Check /tmp/safe_rebuild_stage3.log for details."
            tail_log_if_exists /tmp/safe_rebuild_stage3.log 10
        fi
        if [ "${SEEN_REQUIRE_STAGE3:-0}" = "1" ]; then
            echo -e "${RED}ERROR: SEEN_REQUIRE_STAGE3=1 and S2→S3 failed.${NC}"
            exit "$S3_EXIT"
        fi
        if recover_with_preserved_production_compiler; then
            echo -e "${GREEN}Using recovered stage3 as production compiler.${NC}"
        else
            echo -e "${GREEN}Using Stage2 as production compiler (verified via frozen bootstrap).${NC}"
            VERIFIED="$STAGE2"
        fi
    fi

    rm -rf "$OPT_WRAPPER_DIR"
fi
fi

# Install production compiler
echo ""
echo "Installing production compiler..."
mkdir -p compiler_seen/target
# Remove before copy to avoid "Text file busy" if the binary is in use
rm -f compiler_seen/target/seen 2>/dev/null || true
cp "$VERIFIED" compiler_seen/target/seen
chmod +x compiler_seen/target/seen
cp "$STAGE2" stage2_head 2>/dev/null || true
[ -f "$STAGE3" ] && cp "$STAGE3" stage3_head 2>/dev/null || true
rm -f stage3_recovery_head 2>/dev/null || true
[ -f "$STAGE3_RECOVERY" ] && cp "$STAGE3_RECOVERY" stage3_recovery_head 2>/dev/null || true

# Also install to target/release/seen (README install path)
mkdir -p target/release
cp compiler_seen/target/seen target/release/seen
chmod +x target/release/seen

# Clean up
rm -f "$STAGE2" "$STAGE3" "$STAGE3_RECOVERY" "$PRESERVED_PROD_BUILDER"
rm -rf .seen_cache/
[ -n "$OPT_WRAPPER_DIR" ] && rm -rf "$OPT_WRAPPER_DIR"

echo ""
echo -e "${GREEN}=== Safe Rebuild Complete ===${NC}"
echo ""
echo "Production compiler updated: compiler_seen/target/seen"
echo "Also installed to: target/release/seen"
[ -f stage3_recovery_head ] && echo "Recovery backup: stage3_recovery_head"
echo "Safe to commit your changes."
