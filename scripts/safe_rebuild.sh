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
BOOTSTRAP_PREFLIGHT_DONE=0
FROZEN_ABS=""
BUILD_TRACE_COMMON="$SCRIPT_DIR/build_trace_common.sh"
REBUILD_TIER="full"
CLEAN_CACHE=0
PACKAGE_CLIENT_BUILD_OUTPUT="$REPO_ROOT/target/seen-build/package-client/seen-pkg"

if [ -f "$BUILD_TRACE_COMMON" ]; then
    # shellcheck source=scripts/build_trace_common.sh
    source "$BUILD_TRACE_COMMON"
    seen_build_trace_init "safe_rebuild"
fi

safe_rebuild_usage() {
    echo "Usage: $0 [--tier quick|verify|full] [--clean-cache] [--help]"
    echo ""
    echo "Tiers:"
    echo "  quick   Cache-enabled developer rebuild to compiler_seen/target/seen-dev; smoke only."
    echo "  verify  Cache-enabled production rebuild; targeted checks; install after verification."
    echo "  full    Cold staged bootstrap verification. This is the default for compatibility."
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --tier)
            if [ "$#" -lt 2 ]; then
                echo -e "${RED:-}ERROR: --tier requires quick, verify, or full.${NC:-}" >&2
                exit 1
            fi
            REBUILD_TIER="$2"
            shift 2
            ;;
        --tier=*)
            REBUILD_TIER="${1#--tier=}"
            shift
            ;;
        --clean-cache)
            CLEAN_CACHE=1
            shift
            ;;
        -h|--help)
            safe_rebuild_usage
            exit 0
            ;;
        *)
            echo -e "${RED:-}ERROR: unknown safe rebuild option: $1${NC:-}" >&2
            safe_rebuild_usage >&2
            exit 1
            ;;
    esac
done

case "$REBUILD_TIER" in
    quick|verify|full) ;;
    *)
        echo -e "${RED:-}ERROR: --tier must be quick, verify, or full.${NC:-}" >&2
        exit 1
        ;;
esac

safe_rebuild_cleanup() {
    cleanup_bootstrap_source_overlay
    if declare -F seen_build_trace_summary >/dev/null 2>&1; then
        seen_build_trace_summary
    fi
}

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

latest_plain_module_ll() {
    local dir=$1
    local latest=""
    for f in "$dir"/seen_module_*.ll; do
        [ -f "$f" ] || continue
        [[ "$f" == *.opt.ll ]] && continue
        [[ "$f" == *.polly.ll ]] && continue
        if [ -z "$latest" ] || [ "$f" -nt "$latest" ]; then
            latest="$f"
        fi
    done
    if [ -n "$latest" ]; then
        basename "$latest"
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

user_memory_scope_available() {
    command -v systemd-run >/dev/null 2>&1 || return 1
    command -v systemctl >/dev/null 2>&1 || return 1
    systemctl --user show-environment >/dev/null 2>&1
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
        if [ -n "${SEEN_MEMORY_GUARD_METRICS_FILE:-}" ]; then
            guard_cmd+=(--metrics-file "$SEEN_MEMORY_GUARD_METRICS_FILE")
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
    local guard_metrics="${log_file%.log}.guard.metrics"
    local trace_start=""
    if declare -F seen_build_trace_step_start >/dev/null 2>&1; then
        trace_start=$(seen_build_trace_step_start "$label")
    fi
    : > "$log_file"
    : > "$guard_log"
    rm -f "$guard_metrics"

    local status=0
    SEEN_MEMORY_GUARD_METRICS_FILE="$guard_metrics" \
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

    if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
        local trace_detail="log=$log_file"
        local peak_rss_kb peak_cgroup_kb guard_state guard_status
        if [ -f "$guard_metrics" ]; then
            peak_rss_kb=$(awk -F= '/^peak_rss_kb=/ {print $2; exit}' "$guard_metrics" 2>/dev/null || true)
            peak_cgroup_kb=$(awk -F= '/^peak_cgroup_kb=/ {print $2; exit}' "$guard_metrics" 2>/dev/null || true)
            guard_state=$(awk -F= '/^state=/ {print $2; exit}' "$guard_metrics" 2>/dev/null || true)
            guard_status=$(awk -F= '/^command_status=/ {print $2; exit}' "$guard_metrics" 2>/dev/null || true)
            if [ -n "$peak_rss_kb" ]; then
                trace_detail="$trace_detail peak_rss_kb=$peak_rss_kb"
            fi
            if [ -n "$peak_cgroup_kb" ]; then
                trace_detail="$trace_detail peak_cgroup_kb=$peak_cgroup_kb"
            fi
            if [ -n "$guard_state" ]; then
                trace_detail="$trace_detail guard_state=$guard_state"
            fi
            if [ -n "$guard_status" ]; then
                trace_detail="$trace_detail guard_status=$guard_status"
            fi
        fi
        if [ "$status" -eq 0 ]; then
            seen_build_trace_step_end "$label" "$trace_start" "ok" "$trace_detail"
        else
            seen_build_trace_step_end "$label" "$trace_start" "failed:$status" "$trace_detail"
        fi
    fi
    return "$status"
}

log_failure_signal_pattern() {
    printf '%s\n' 'Fatal Lexer Error|Fatal Parser Error|IR VERIFY|llvm-as:|/usr/bin/opt:|clang: error|ld\.lld: error|LLVM ERROR|Error: optimization failed|Segmentation fault|core dumped|Traceback \(most recent call last\)|(^|[[:space:]])Error:'
}

start_log_failure_watcher() {
    local label=$1
    local log_file=$2
    local watched_pid=$3

    if [ "${SEEN_ABORT_ON_FIRST_FAILURE_SIGNAL:-1}" = "0" ]; then
        echo ""
        return 0
    fi

    (
        local pattern
        pattern=$(log_failure_signal_pattern)
        local interval="${SEEN_FAILURE_WATCH_INTERVAL_SECS:-2}"
        local match=""

        while kill -0 "$watched_pid" 2>/dev/null; do
            if [ -f "$log_file" ]; then
                match=$(grep -n -m1 -E "$pattern" "$log_file" 2>/dev/null || true)
                if [ -n "$match" ]; then
                    printf "\n%s[%s]%s first failure signal: %s\n" "$YELLOW" "$label" "$NC" "$match" >&2
                    printf "%s[%s]%s stopping build step early; set SEEN_ABORT_ON_FIRST_FAILURE_SIGNAL=0 to wait for the full command.\n" "$YELLOW" "$label" "$NC" >&2
                    kill -TERM "$watched_pid" 2>/dev/null || true
                    sleep 5
                    kill -KILL "$watched_pid" 2>/dev/null || true
                    exit 0
                fi
            fi
            sleep "$interval"
        done
    ) &
    echo "$!"
}

run_guarded_command_to_log_with_failure_watch() {
    local label=$1
    local timeout_secs=$2
    local vmem_kb=$3
    local log_file=$4
    shift 4

    : > "$log_file"
    run_guarded_command_to_log "$label" "$timeout_secs" "$vmem_kb" "$log_file" "$@" &
    local command_pid=$!
    local failure_watcher_pid
    failure_watcher_pid=$(start_log_failure_watcher "$label" "$log_file" "$command_pid")

    local status=0
    wait "$command_pid" || status=$?
    if [ -n "$failure_watcher_pid" ]; then
        kill "$failure_watcher_pid" 2>/dev/null || true
        wait "$failure_watcher_pid" 2>/dev/null || true
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

        # Count plain generated IR separately from fixed/optimized IR so progress
        # doesn't look like the compiler is discovering new source modules forever.
        local ll_count=$(count_plain_module_lls /tmp)
        local opt_ll_count=$(count_module_opt_lls /tmp)

        # Count .o files (modules fully compiled)
        local obj_count=$(count_module_objects /tmp)
        local latest_ll=$(latest_plain_module_ll /tmp)

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

        local ll_status="${BOLD}${ll_count} raw.ll${NC}"
        if [ "$opt_ll_count" -gt 0 ]; then
            ll_status="${ll_status}/${BOLD}${opt_ll_count} opt.ll${NC}"
        fi

        local latest_status=""
        if [ -n "$latest_ll" ]; then
            latest_status=" latest=${latest_ll}"
        fi

        # Build status line
        local status="${CYAN}[$label]${NC} ${elapsed_fmt}  ${ll_status} | ${BOLD}${obj_count} .o${NC}  phase:${phase}  ${DIM}${mod5_status}${latest_status}${NC}"

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
    local ll_count=$(count_plain_module_lls /tmp)
    local opt_ll_count=$(count_module_opt_lls /tmp)
    local obj_count=$(count_module_objects /tmp)
    printf "\r\033[K${CYAN}[$label]${NC} ${GREEN}done${NC} in ${elapsed_fmt}  ${ll_count} raw.ll | ${opt_ll_count} opt.ll | ${obj_count} .o\n"
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
    : > "$logfile"
    run_guarded_command_to_log "$label" 0 "${MAIN_COMPILER_VMEM_KB:-}" "$logfile" "$@" &
    local compile_pid=$!

    # Start progress monitor
    monitor_compilation "$compile_pid" "$label" &
    local monitor_pid=$!
    local failure_watcher_pid
    failure_watcher_pid=$(start_log_failure_watcher "$label" "$logfile" "$compile_pid")

    # Wait for compilation to finish
    local exit_code=0
    wait "$compile_pid" || exit_code=$?

    # Stop monitor
    kill "$monitor_pid" 2>/dev/null || true
    wait "$monitor_pid" 2>/dev/null || true
    if [ -n "$failure_watcher_pid" ]; then
        kill "$failure_watcher_pid" 2>/dev/null || true
        wait "$failure_watcher_pid" 2>/dev/null || true
    fi

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
                        print ""
                        next
                    }
                    in_triple {
                        print ""
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
            bootstrap|compiler_seen|seen_std)
                ;;
            *)
                ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/$base"
                ;;
        esac
    done

    if [ -d "$REPO_ROOT/bootstrap" ]; then
        mkdir -p "$BOOTSTRAP_SOURCE_ROOT/bootstrap"
        for entry in "$REPO_ROOT/bootstrap"/*; do
            [ -e "$entry" ] || continue
            local base
            base=$(basename "$entry")
            case "$base" in
                stage1_frozen*|seen_frozen*)
                    cp -pL "$entry" "$BOOTSTRAP_SOURCE_ROOT/bootstrap/$base"
                    chmod +x "$BOOTSTRAP_SOURCE_ROOT/bootstrap/$base" 2>/dev/null || true
                    ;;
                *)
                    ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/bootstrap/$base"
                    ;;
            esac
        done
    fi

    mkdir -p "$BOOTSTRAP_SOURCE_ROOT/compiler_seen" "$BOOTSTRAP_SOURCE_ROOT/seen_std"
    for entry in "$REPO_ROOT/compiler_seen"/*; do
        local base
        base=$(basename "$entry")
        if [ "$base" = "src" ]; then
            copy_bootstrap_seen_tree "$entry" "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/src"
        elif [ "$base" = "target" ]; then
            mkdir -p "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/target"
            for bin in "$entry"/seen "$entry"/seen_frozen* "$entry"/stage1_frozen* "$entry"/seen_native_snapshot; do
                [ -f "$bin" ] || continue
                cp -pL "$bin" "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/target/$(basename "$bin")"
                chmod +x "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/target/$(basename "$bin")" 2>/dev/null || true
            done
        else
            ln -s "$entry" "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/$base"
        fi
    done
    # seen_std is a local package dependency. Keep its overlay symlink-free so
    # the package client's local-source hardening sees the same regular-file
    # layout that a published package archive contains.
    copy_bootstrap_seen_tree "$REPO_ROOT/seen_std" "$BOOTSTRAP_SOURCE_ROOT/seen_std"
    echo -e "${YELLOW}Bootstrap source overlay enabled: temporary /// bodies stripped for older bootstrap compilers.${NC}"
}

cleanup_bootstrap_source_overlay() {
    if [ -n "$BOOTSTRAP_SOURCE_ROOT" ] && [ "$BOOTSTRAP_SOURCE_ROOT" != "$REPO_ROOT" ]; then
        # Hardened package views are read-only. Restore owner write permission
        # inside the disposable overlay so trap cleanup can remove them.
        chmod -R u+w "$BOOTSTRAP_SOURCE_ROOT/compiler_seen/.seen" 2>/dev/null || true
        rm -rf "$BOOTSTRAP_SOURCE_ROOT"
    fi
}

trap safe_rebuild_cleanup EXIT

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

summarize_stage2_failure_log() {
    local log_file=$1
    if [ ! -f "$log_file" ]; then
        echo "(missing Stage2 log: $log_file)"
        return 0
    fi

    echo -e "${YELLOW}First Stage2 failure signals:${NC}"
    local matches
    matches=$(grep -n -E 'IR VERIFY|llvm-as:|/usr/bin/opt:|clang: error|ld.lld: error|LLVM ERROR|Error: optimization failed|error:' "$log_file" 2>/dev/null | head -40 || true)
    if [ -n "$matches" ]; then
        echo "$matches"
    else
        echo "(no targeted error markers found; tailing recent log output)"
        tail_log_if_exists "$log_file" 40
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

find_latest_compile_ll_dir_with_count() {
    local expected_count=$1
    local newer_than=${2:-}
    local latest=""
    local dir
    local count

    for dir in /tmp/seen_compile_*; do
        [ -d "$dir" ] || continue
        if [ -n "$newer_than" ] && [ ! "$dir" -nt "$newer_than" ]; then
            continue
        fi
        count=$(count_plain_module_lls "$dir")
        if [ "$count" -eq "$expected_count" ] 2>/dev/null; then
            if [ -z "$latest" ] || [ "$dir" -nt "$latest" ]; then
                latest="$dir"
            fi
        fi
    done

    echo "$latest"
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

    local max_main_kb=$((10 * 1024 * 1024))
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

configure_adaptive_rebuild_workers() {
    local jobs opt_jobs

    if [ -n "${SEEN_JOBS:-}" ] && ! is_positive_integer "$SEEN_JOBS"; then
        echo -e "${RED}ERROR: SEEN_JOBS must be a positive integer.${NC}" >&2
        exit 1
    fi
    if [ -n "${SEEN_OPT_JOBS:-}" ] && ! is_positive_integer "$SEEN_OPT_JOBS"; then
        echo -e "${RED}ERROR: SEEN_OPT_JOBS must be a positive integer.${NC}" >&2
        exit 1
    fi

    if declare -F seen_build_derive_jobs >/dev/null 2>&1; then
        read -r jobs opt_jobs < <(seen_build_derive_jobs "$MAIN_COMPILER_VMEM_KB" "$OPT_VMEM_KB")
    else
        jobs=2
        opt_jobs=1
    fi

    if [ -z "${SEEN_JOBS:-}" ]; then
        export SEEN_JOBS="$jobs"
    fi
    if [ -z "${SEEN_OPT_JOBS:-}" ]; then
        export SEEN_OPT_JOBS="$opt_jobs"
    fi

    if declare -F seen_build_trace_event >/dev/null 2>&1; then
        seen_build_trace_event "worker budget" "ok" "SEEN_JOBS=$SEEN_JOBS SEEN_OPT_JOBS=$SEEN_OPT_JOBS"
    fi
}

clean_rebuild_caches() {
    local reason="${1:-requested}"
    local trace_start=""
    if declare -F seen_build_trace_step_start >/dev/null 2>&1; then
        trace_start=$(seen_build_trace_step_start "cache cleanup")
    fi
    rm -rf .seen_cache/ /tmp/seen_ir_cache/ /tmp/seen_thinlto_cache/ /tmp/seen_testmain_obj_cache/
    if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
        seen_build_trace_step_end "cache cleanup" "$trace_start" "ok" "$reason"
    fi
}

echo "=== Safe Rebuild Script ==="
echo ""
echo "Tier: $REBUILD_TIER"
if [ "$CLEAN_CACHE" = "1" ]; then
    echo "Cache cleanup: requested"
fi

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
TIER_TIMEOUT_SECS="${SEEN_TIER_TIMEOUT_SECS:-2700}"
IR_RECOVERY_DISABLED="${SEEN_DISABLE_IR_RECOVERY:-0}"
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
    if [ "$HOST_OS" != "Darwin" ] && [ "${SEEN_MEMORY_GUARD_KERNEL_SCOPE:-1}" != "0" ] &&
        user_memory_scope_available; then
        export SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=1
    else
        export SEEN_MEMORY_GUARD_KERNEL_SCOPE=0
        export SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=0
    fi
fi

if ! is_positive_integer "$TIER_TIMEOUT_SECS"; then
    echo -e "${RED}ERROR: SEEN_TIER_TIMEOUT_SECS must be a positive integer number of seconds.${NC}" >&2
    exit 1
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
    MAIN_COMPILER_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:-$((MAIN_COMPILER_VMEM_KB * 1024))}"
    export SEEN_LOW_MEMORY=1
    export SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB"
    export SEEN_OPT_VMEM_KB="$OPT_VMEM_KB"
    export SEEN_MEMORY_LIMIT_BYTES="$MAIN_COMPILER_MEMORY_LIMIT_BYTES"
    export SEEN_RECOVERY_TIMEOUT_SECS="$RECOVERY_TIMEOUT_SECS"
    guard_low_memory_concurrency
    if [ "$REBUILD_TIER" = "full" ]; then
        echo -e "${YELLOW}Low-memory mode enabled: serial full-bootstrap stages.${NC}"
    else
        echo -e "${YELLOW}Low-memory mode enabled: adaptive bounded worker tiers.${NC}"
    fi
    echo -e "${YELLOW}Detected system memory: $(format_bytes $((SYSTEM_MEMORY_KB * 1024))). Main compiler cap: $(format_bytes $((MAIN_COMPILER_VMEM_KB * 1024))). tracked allocation budget: $(format_bytes "$MAIN_COMPILER_MEMORY_LIMIT_BYTES"). opt cap: $(format_bytes $((OPT_VMEM_KB * 1024))).${NC}"
fi

configure_adaptive_rebuild_workers

if memory_guard_enabled; then
    echo -e "${YELLOW}Memory guard enabled: tree RSS cap $(format_bytes $((MEMORY_GUARD_RSS_KB * 1024))); cgroup stop $(format_bytes $((MEMORY_GUARD_CGROUP_STOP_KB * 1024))); reserve $(format_bytes $((MEMORY_GUARD_RESERVE_KB * 1024))); tasks max ${MEMORY_GUARD_TASKS_MAX:-unlimited}.${NC}"
fi
if [ "$IR_RECOVERY_DISABLED" = "1" ]; then
    echo -e "${YELLOW}Strict rebuild validation enabled: direct IR recovery is disabled.${NC}"
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
if [ "$REBUILD_TIER" = "full" ]; then
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
elif declare -F seen_build_trace_event >/dev/null 2>&1; then
    seen_build_trace_event "bootstrap preflight" "deferred" "$REBUILD_TIER tier"
fi

# Verify frozen compiler hash (cross-platform)
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

ensure_bootstrap_preflight() {
    if [ "$BOOTSTRAP_PREFLIGHT_DONE" = "1" ]; then
        return 0
    fi

    local trace_start=""
    if declare -F seen_build_trace_step_start >/dev/null 2>&1; then
        trace_start=$(seen_build_trace_step_start "bootstrap preflight")
    fi

    if [ ! -f "$FROZEN" ]; then
        echo -e "${RED}ERROR: Frozen compiler not found at $FROZEN${NC}"
        echo "Run this script from the repository root."
        if [ "$HOST_OS" = "Darwin" ]; then
            echo "On macOS, run scripts/bootstrap_macos.sh first to create the macOS bootstrap."
        fi
        if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
            seen_build_trace_step_end "bootstrap preflight" "$trace_start" "failed" "missing=$FROZEN"
        fi
        return 1
    fi

    if ! bootstrap_binary_usable "$FROZEN"; then
        if [ "$HOST_OS" != "Darwin" ] && [ "$FROZEN" = "bootstrap/stage1_frozen" ] && [ -x "bootstrap/stage1_frozen_v3" ] && bootstrap_binary_usable "bootstrap/stage1_frozen_v3"; then
            echo -e "${YELLOW}bootstrap/stage1_frozen failed a startup smoke test; falling back to bootstrap/stage1_frozen_v3.${NC}"
            FROZEN="bootstrap/stage1_frozen_v3"
            HASH_FILE="bootstrap/stage1_frozen_v3.sha256"
        else
            echo -e "${RED}ERROR: Frozen compiler at $FROZEN failed a startup smoke test.${NC}"
            if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
                seen_build_trace_step_end "bootstrap preflight" "$trace_start" "failed" "startup=$FROZEN"
            fi
            return 1
        fi
    fi

    echo "Verifying frozen compiler integrity..."
    if verify_hash "$HASH_FILE"; then
        echo -e "${GREEN}Frozen compiler verified.${NC}"
    else
        echo -e "${RED}ERROR: Frozen compiler hash verification failed!${NC}"
        echo "The bootstrap compiler may be corrupted."
        if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
            seen_build_trace_step_end "bootstrap preflight" "$trace_start" "failed" "hash=$HASH_FILE"
        fi
        return 1
    fi

    prepare_bootstrap_source_overlay
    if [ "$BOOTSTRAP_SOURCE_ROOT" != "$REPO_ROOT" ] && [ -f "$BOOTSTRAP_SOURCE_ROOT/$FROZEN" ]; then
        FROZEN_ABS="$BOOTSTRAP_SOURCE_ROOT/$FROZEN"
    else
        FROZEN_ABS="$REPO_ROOT/$FROZEN"
    fi
    BOOTSTRAP_PREFLIGHT_DONE=1
    if declare -F seen_build_trace_step_end >/dev/null 2>&1; then
        seen_build_trace_step_end "bootstrap preflight" "$trace_start" "ok" "frozen=$FROZEN source_root=$BOOTSTRAP_SOURCE_ROOT"
    fi
}

cleanup_smoke_build_state() {
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

hash_paths_for_cache() {
    if declare -F seen_build_hash_paths >/dev/null 2>&1; then
        seen_build_hash_paths "$@"
        return
    fi
    local path
    for path in "$@"; do
        [ -e "$path" ] || continue
        if [ -f "$path" ]; then
            sha256sum "$path"
        else
            find "$path" -type f -print0 | sort -z | xargs -0 sha256sum 2>/dev/null
        fi
    done | sha256sum | awk '{print $1}'
}

smoke_compile_cache_key() {
    local compiler_path=$1
    local smoke_fixture=$2
    local smoke_flags=$3
    local compiler_hash fixture_hash runtime_hash std_hash

    compiler_hash=$(hash_paths_for_cache "$compiler_path")
    fixture_hash=$(hash_paths_for_cache "$smoke_fixture")
    runtime_hash=$(hash_paths_for_cache "$REPO_ROOT/seen_runtime")
    std_hash=$(hash_paths_for_cache "$REPO_ROOT/seen_std/src")
    {
        printf 'smoke-cache-v1\n'
        printf 'compiler=%s\n' "$compiler_hash"
        printf 'fixture=%s\n' "$fixture_hash"
        printf 'runtime=%s\n' "$runtime_hash"
        printf 'stdlib=%s\n' "$std_hash"
        printf 'flags=%s\n' "$smoke_flags"
        printf 'host_os=%s\n' "${HOST_OS:-unknown}"
        printf 'host_arch=%s\n' "${HOST_ARCH:-unknown}"
    } | sha256sum | awk '{print $1}'
}

smoke_cache_default_enabled() {
    case "${SEEN_SMOKE_CACHE:-}" in
        0|false|False|FALSE|no|No|NO)
            return 1
            ;;
        1|true|True|TRUE|yes|Yes|YES)
            return 0
            ;;
    esac
    [ "$REBUILD_TIER" != "full" ]
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
    local smoke_flags="--fast --no-cache"
    local smoke_cache_key smoke_cache_dir smoke_cache_bin

    if [ "$LOW_MEMORY_MODE" = "1" ]; then
        compiler_env+=("SEEN_LOW_MEMORY=${SEEN_LOW_MEMORY:-1}")
        if [ -n "$MAIN_COMPILER_VMEM_KB" ]; then
            compiler_env+=("SEEN_MAIN_VMEM_KB=$MAIN_COMPILER_VMEM_KB")
        fi
        if [ -n "$OPT_VMEM_KB" ]; then
            compiler_env+=("SEEN_OPT_VMEM_KB=$OPT_VMEM_KB")
        fi
        if [ -n "${SEEN_MEMORY_LIMIT_BYTES:-}" ]; then
            compiler_env+=("SEEN_MEMORY_LIMIT_BYTES=$SEEN_MEMORY_LIMIT_BYTES")
        fi
        check_cmd+=(--no-fork)
        compile_cmd+=(--no-fork)
        smoke_flags="$smoke_flags --no-fork"
    fi
    if [ -n "${RELEASE_TARGET_CPU_FLAG:-}" ]; then
        compile_cmd+=("$RELEASE_TARGET_CPU_FLAG")
        smoke_flags="$smoke_flags $RELEASE_TARGET_CPU_FLAG"
    fi

    cleanup_smoke_build_state
    if ! cp "$smoke_fixture" "$smoke_source"; then
        echo -e "${YELLOW}${stage_label} could not prepare hello-world smoke source.${NC}"
        return 1
    fi

    if ! (
        cd "$REPO_ROOT" &&
        run_guarded_command_to_log_with_failure_watch "$stage_label check smoke" 120 "$MAIN_COMPILER_VMEM_KB" "$check_log" \
            env "${compiler_env[@]}" "${check_cmd[@]}"
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

    if smoke_cache_default_enabled; then
        smoke_cache_key=$(smoke_compile_cache_key "$compiler_path" "$smoke_fixture" "$smoke_flags")
        smoke_cache_dir="$REPO_ROOT/target/seen-build/smoke-cache/$smoke_cache_key"
        smoke_cache_bin="$smoke_cache_dir/safe_rebuild_smoke_bin"
        if [ -x "$smoke_cache_bin" ]; then
            cp "$smoke_cache_bin" "$smoke_bin"
            chmod +x "$smoke_bin" 2>/dev/null || true
            printf 'smoke compile cache hit: %s\n' "$smoke_cache_key" > "$compile_log"
            if declare -F seen_build_trace_event >/dev/null 2>&1; then
                seen_build_trace_event "$stage_label compile smoke cache" "hit" "key=$smoke_cache_key"
            fi
        else
            if declare -F seen_build_trace_event >/dev/null 2>&1; then
                seen_build_trace_event "$stage_label compile smoke cache" "miss" "key=$smoke_cache_key"
            fi
            if ! (
                cd "$REPO_ROOT" &&
                run_guarded_command_to_log_with_failure_watch "$stage_label compile smoke" 120 "$MAIN_COMPILER_VMEM_KB" "$compile_log" \
                    env "${compiler_env[@]}" "${compile_cmd[@]}"
            ); then
                echo -e "${YELLOW}${stage_label} failed hello-world compile smoke test.${NC}"
                tail -20 "$compile_log" 2>/dev/null || true
                preserve_smoke_failure_artifacts "$stage_slug"
                cleanup_smoke_build_state
                return 1
            fi
            if [ -x "$smoke_bin" ]; then
                mkdir -p "$smoke_cache_dir"
                cp "$smoke_bin" "$smoke_cache_bin" 2>/dev/null || true
                chmod +x "$smoke_cache_bin" 2>/dev/null || true
            fi
        fi
    else
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "$stage_label compile smoke cache" "disabled" "tier=$REBUILD_TIER"
        fi
        if ! (
            cd "$REPO_ROOT" &&
            run_guarded_command_to_log_with_failure_watch "$stage_label compile smoke" 120 "$MAIN_COMPILER_VMEM_KB" "$compile_log" \
                env "${compiler_env[@]}" "${compile_cmd[@]}"
        ); then
            echo -e "${YELLOW}${stage_label} failed hello-world compile smoke test.${NC}"
            tail -20 "$compile_log" 2>/dev/null || true
            preserve_smoke_failure_artifacts "$stage_slug"
            cleanup_smoke_build_state
            return 1
        fi
    fi

    if [ -f "$smoke_bin" ] && [ ! -x "$smoke_bin" ]; then
        chmod +x "$smoke_bin" 2>/dev/null || true
    fi

    if [ ! -x "$smoke_bin" ]; then
        echo -e "${YELLOW}${stage_label} compile smoke test did not produce executable $smoke_bin.${NC}"
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

tier_source_root_for_builder() {
    local builder_path=$1
    local builder_name
    builder_name=$(basename "$builder_path")
    if [ "${SEEN_TIER_USE_BOOTSTRAP_OVERLAY:-${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}}" = "1" ]; then
        printf '%s\n' "$BOOTSTRAP_SOURCE_ROOT"
        return 0
    fi
    case "$builder_name" in
        seen_frozen*|stage1_frozen*)
            printf '%s\n' "$BOOTSTRAP_SOURCE_ROOT"
            ;;
        *)
            printf '%s\n' "$REPO_ROOT"
            ;;
    esac
}

tier_builder_requires_bootstrap_preflight() {
    if [ "${SEEN_TIER_USE_BOOTSTRAP_OVERLAY:-${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}}" = "1" ]; then
        return 0
    fi
    case "$(basename "$1")" in
        seen_frozen*|stage1_frozen*)
            return 0
            ;;
    esac
    return 1
}

tier_builder_path_for_source_root() {
    local builder_path=$1
    local source_root=$2
    if [ "$source_root" = "$BOOTSTRAP_SOURCE_ROOT" ] &&
        [ "${SEEN_TIER_USE_BOOTSTRAP_OVERLAY:-${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}}" = "1" ]; then
        local overlay_builder="$BOOTSTRAP_SOURCE_ROOT/compiler_seen/target/seen_tier_builder"
        cp -pL "$builder_path" "$overlay_builder" || return 1
        chmod +x "$overlay_builder" 2>/dev/null || true
        printf '%s\n' "$overlay_builder"
        return 0
    fi
    case "$builder_path" in
        "$REPO_ROOT"/*)
            if [ "$source_root" = "$BOOTSTRAP_SOURCE_ROOT" ]; then
                printf '%s\n' "$BOOTSTRAP_SOURCE_ROOT/${builder_path#$REPO_ROOT/}"
                return 0
            fi
            ;;
    esac
    printf '%s\n' "$builder_path"
}

tier_builder_candidates() {
    if [ -n "${SEEN_STAGE_BUILDER:-}" ]; then
        printf '%s\n' "$SEEN_STAGE_BUILDER"
    fi
    printf '%s\n' \
        "$REPO_ROOT/compiler_seen/target/seen-dev" \
        "$REPO_ROOT/stage2_head" \
        "$REPO_ROOT/stage3_recovery_head" \
        "$REPO_ROOT/stage3_head" \
        "$REPO_ROOT/compiler_seen/target/seen" \
        "$REPO_ROOT/target/release/seen" \
        "$REPO_ROOT/bootstrap/stage1_frozen_v3" \
        "$REPO_ROOT/bootstrap/stage1_frozen"
}

tier_builder_supports_jobs() {
    local builder_path=$1
    "$builder_path" --help 2>/dev/null | grep -q -- '--jobs'
}

tier_legacy_builder_requires_guarded_fork() {
    case "$(basename "$1")" in
        stage2_head)
            return 0
            ;;
    esac
    return 1
}

run_tier_prebuild_gates_if_needed() {
    if [ "$REBUILD_TIER" != "verify" ]; then
        return 0
    fi
    if [ "${SEEN_SKIP_PREBUILD_GATES:-0}" = "1" ]; then
        echo -e "${YELLOW}Prebuild gates skipped by SEEN_SKIP_PREBUILD_GATES=1.${NC}"
        return 0
    fi

    echo "Running verify-tier prebuild gates..."
    if run_guarded_command_to_log_with_failure_watch "prebuild gates" 900 "$MAIN_COMPILER_VMEM_KB" \
        /tmp/safe_rebuild_verify_prebuild_gates.log \
        bash "$SCRIPT_DIR/seen_prebuild_gates.sh"; then
        return 0
    fi
    echo -e "${RED}ERROR: verify-tier prebuild gates failed.${NC}"
    tail_log_if_exists /tmp/safe_rebuild_verify_prebuild_gates.log 30
    return 1
}

run_tier_targeted_checks() {
    local compiler_path=$1

    if [ "$REBUILD_TIER" != "verify" ]; then
        return 0
    fi

    if [ -f "$REPO_ROOT/compiler_seen/tests/dead_code_warnings.seen" ]; then
        if ! run_guarded_command_to_log_with_failure_watch "verify dead-code test check" 300 "$MAIN_COMPILER_VMEM_KB" \
            /tmp/safe_rebuild_verify_dead_code.log \
            "$compiler_path" check "$REPO_ROOT/compiler_seen/tests/dead_code_warnings.seen"; then
            echo -e "${RED}ERROR: verify-tier dead-code test check failed.${NC}"
            tail_log_if_exists /tmp/safe_rebuild_verify_dead_code.log 30
            return 1
        fi
    fi

    return 0
}

prepare_package_client() {
    local expected_version helper
    expected_version=$(awk -F'"' '/^version = / { print $2; exit }' "$REPO_ROOT/Seen.toml")
    if [ -z "$expected_version" ]; then
        echo -e "${RED}ERROR: could not read the Seen version for package-client coupling.${NC}" >&2
        return 1
    fi

    if [ -n "${SEEN_PACKAGE_CLIENT:-}" ]; then
        helper="$SEEN_PACKAGE_CLIENT"
        if [ ! -x "$helper" ]; then
            echo -e "${RED}ERROR: SEEN_PACKAGE_CLIENT is not executable: $helper${NC}" >&2
            return 1
        fi
    else
        mkdir -p "$(dirname "$PACKAGE_CLIENT_BUILD_OUTPUT")"
        "$SCRIPT_DIR/build_package_client.sh" \
            --version "$expected_version" \
            --output "$PACKAGE_CLIENT_BUILD_OUTPUT" || return 1
        helper="$PACKAGE_CLIENT_BUILD_OUTPUT"
    fi

    if ! "$helper" --expect-version "$expected_version" version --machine >/dev/null 2>&1; then
        echo -e "${RED}ERROR: package-client version handshake failed for Seen $expected_version.${NC}" >&2
        return 1
    fi
    SEEN_PACKAGE_CLIENT="$(cd "$(dirname "$helper")" && pwd)/$(basename "$helper")"
    PACKAGE_CLIENT_BUILD_OUTPUT="$SEEN_PACKAGE_CLIENT"
    export SEEN_PACKAGE_CLIENT
    echo "Package client: $SEEN_PACKAGE_CLIENT"
}

install_tier_verified_compiler() {
    local compiler_path=$1

    echo ""
    echo "Installing verified compiler..."
    mkdir -p "$REPO_ROOT/compiler_seen/target" "$REPO_ROOT/target/release"
    rm -f "$REPO_ROOT/compiler_seen/target/seen" 2>/dev/null || true
    cp "$compiler_path" "$REPO_ROOT/compiler_seen/target/seen"
    chmod +x "$REPO_ROOT/compiler_seen/target/seen"
    cp "$REPO_ROOT/compiler_seen/target/seen" "$REPO_ROOT/target/release/seen"
    chmod +x "$REPO_ROOT/target/release/seen"
    cp "$PACKAGE_CLIENT_BUILD_OUTPUT" "$REPO_ROOT/compiler_seen/target/seen-pkg"
    chmod +x "$REPO_ROOT/compiler_seen/target/seen-pkg"
    cp "$PACKAGE_CLIENT_BUILD_OUTPUT" "$REPO_ROOT/target/release/seen-pkg"
    chmod +x "$REPO_ROOT/target/release/seen-pkg"
}

run_tiered_rebuild() {
    local output_path final_output_path label compile_log candidate source_root builder_for_root
    local compile_status compile_flags

    label="$REBUILD_TIER"
    if [ "$REBUILD_TIER" = "quick" ]; then
        output_path="/tmp/seen_quick_rebuild"
        final_output_path="$REPO_ROOT/compiler_seen/target/seen-dev"
        compile_log="/tmp/safe_rebuild_quick.log"
    else
        output_path="/tmp/seen_verify_rebuild"
        final_output_path="$output_path"
        compile_log="/tmp/safe_rebuild_verify.log"
    fi

    if [ "$CLEAN_CACHE" = "1" ]; then
        clean_rebuild_caches "$REBUILD_TIER --clean-cache"
    fi
    run_tier_prebuild_gates_if_needed || return 1

    echo ""
    echo "Tiered rebuild: $REBUILD_TIER"
    echo "  SEEN_JOBS=$SEEN_JOBS SEEN_OPT_JOBS=$SEEN_OPT_JOBS"
    echo "  Output: $final_output_path"

    rm -f "$output_path"
    mkdir -p "$(dirname "$output_path")" "$(dirname "$final_output_path")"

    while IFS= read -r candidate; do
        [ -n "$candidate" ] || continue
        [ -x "$candidate" ] || continue
        if tier_builder_requires_bootstrap_preflight "$candidate"; then
            ensure_bootstrap_preflight || return 1
        fi
        source_root=$(tier_source_root_for_builder "$candidate")
        builder_for_root=$(tier_builder_path_for_source_root "$candidate" "$source_root")
        [ -x "$builder_for_root" ] || continue

        echo "  Trying builder: $candidate"
        compile_status=0
        compile_flags=(--fast)
        if [ -n "${RELEASE_TARGET_CPU_FLAG:-}" ]; then
            compile_flags+=("$RELEASE_TARGET_CPU_FLAG")
        fi
        if [ "${SEEN_FORCE_SERIAL_REBUILD:-0}" = "1" ]; then
            compile_flags+=(--no-fork)
        elif [ "$LOW_MEMORY_MODE" = "1" ] && ! tier_builder_supports_jobs "$builder_for_root" &&
            ! tier_legacy_builder_requires_guarded_fork "$candidate"; then
            echo "    Builder lacks worker-budget flags; using --no-fork under low-memory cap."
            compile_flags+=(--no-fork)
        elif [ "$LOW_MEMORY_MODE" = "1" ] && ! tier_builder_supports_jobs "$builder_for_root"; then
            echo "    Legacy stage builder requires forked codegen; memory guard remains active."
        fi

        run_guarded_command_to_log_with_failure_watch "$label compile" "$TIER_TIMEOUT_SECS" "$MAIN_COMPILER_VMEM_KB" "$compile_log" \
            bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$source_root" \
            env \
                SEEN_COMPILER_SOURCE_ROOT="$source_root" \
                SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-0}" \
                SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
                SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
                SEEN_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:-}" \
                SEEN_JOBS="$SEEN_JOBS" \
                SEEN_OPT_JOBS="$SEEN_OPT_JOBS" \
                "$builder_for_root" compile "$COMPILER_SOURCE" "$output_path" \
                "${compile_flags[@]}" || compile_status=$?

        if grep -qE 'Fatal Lexer Error|Fatal Parser Error' "$compile_log" 2>/dev/null; then
            echo -e "${YELLOW}  Builder emitted a fatal frontend diagnostic; rejecting its output.${NC}"
            compile_status=1
        fi

        if [ "$compile_status" -ne 0 ]; then
            echo -e "${YELLOW}  Builder failed for $REBUILD_TIER tier (exit=$compile_status); trying next candidate.${NC}"
            tail_log_if_exists "$compile_log" 10
            rm -f "$output_path"
            if [ "${SEEN_STAGE_BUILDER_ONLY:-0}" = "1" ] && [ -n "${SEEN_STAGE_BUILDER:-}" ] && [ "$candidate" = "$SEEN_STAGE_BUILDER" ]; then
                return "$compile_status"
            fi
            continue
        fi

        if smoke_test_compiler "$output_path" "$REBUILD_TIER compiler" "$REBUILD_TIER"; then
            run_tier_targeted_checks "$output_path" || return 1
            if [ "$REBUILD_TIER" = "verify" ]; then
                install_tier_verified_compiler "$output_path"
            else
                cp "$output_path" "$final_output_path"
                chmod +x "$final_output_path"
                cp "$PACKAGE_CLIENT_BUILD_OUTPUT" "$(dirname "$final_output_path")/seen-pkg"
                chmod +x "$(dirname "$final_output_path")/seen-pkg"
            fi
            echo -e "${GREEN}${REBUILD_TIER} rebuild complete.${NC}"
            if [ "$REBUILD_TIER" = "quick" ]; then
                echo "Developer compiler: compiler_seen/target/seen-dev"
            else
                echo "Production compiler updated: compiler_seen/target/seen"
                echo "Also installed to: target/release/seen"
            fi
            return 0
        fi

        echo -e "${YELLOW}  Built compiler failed smoke; trying next builder.${NC}"
        rm -f "$output_path"
    done < <(tier_builder_candidates)

    echo -e "${RED}ERROR: no trusted stage builder completed the $REBUILD_TIER rebuild.${NC}"
    return 1
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

    local recovery_source_root="$REPO_ROOT"
    local source_mode="${SEEN_EXISTING_BUILDER_SOURCE_ROOT:-auto}"
    if [ "${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}" = "1" ]; then
        source_mode="overlay"
    fi
    case "$source_mode" in
        overlay)
            recovery_source_root="$BOOTSTRAP_SOURCE_ROOT"
            ;;
        real)
            recovery_source_root="$REPO_ROOT"
            ;;
        auto)
            case "$(basename "$PRESERVED_PROD_BUILDER")" in
                seen_frozen*|stage1_frozen*) recovery_source_root="$BOOTSTRAP_SOURCE_ROOT" ;;
            esac
            ;;
        *)
            echo -e "${RED}ERROR: SEEN_EXISTING_BUILDER_SOURCE_ROOT must be auto, real, or overlay.${NC}" >&2
            return 1
            ;;
    esac
    local recovery_builder_path="$PRESERVED_PROD_BUILDER"
    case "$recovery_builder_path" in
        "$REPO_ROOT"/*)
            if [ "$recovery_source_root" = "$BOOTSTRAP_SOURCE_ROOT" ]; then
                recovery_builder_path="$BOOTSTRAP_SOURCE_ROOT/${recovery_builder_path#$REPO_ROOT/}"
            fi
            ;;
    esac

    local recovery_exit=0
    local recovery_marker=""
    recovery_marker=$(mktemp -d /tmp/seen_preserved_recovery_marker.XXXXXX 2>/dev/null || true)
    run_guarded_command_to_log "preserved compiler recovery" "$RECOVERY_TIMEOUT_SECS" "$MAIN_COMPILER_VMEM_KB" /tmp/safe_rebuild_stage3_recovery.log \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$recovery_source_root" \
        env PATH="$OPT_WRAPPER_DIR:$PATH" \
            SEEN_COMPILER_SOURCE_ROOT="$recovery_source_root" \
            SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-0}" \
            SEEN_SKIP_IR_FIXUPS=1 \
            SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
            SEEN_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:-}" \
            "$recovery_builder_path" compile "$COMPILER_SOURCE" "$STAGE3_RECOVERY" \
            --fast --no-cache --no-fork $RELEASE_TARGET_CPU_FLAG || recovery_exit=$?
    if [ "$recovery_exit" -eq 0 ]; then
        rm -rf "$recovery_marker"
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
    local preserved_expected_modules=0
    local preserved_ll_dir=""
    preserved_expected_modules=$(extract_expected_module_count /tmp/safe_rebuild_stage3_recovery.log)
    if is_positive_integer "$preserved_expected_modules" && [ "$preserved_expected_modules" -gt 0 ]; then
        preserved_ll_dir=$(find_latest_compile_ll_dir_with_count "$preserved_expected_modules" "$recovery_marker")
        if [ -n "$preserved_ll_dir" ]; then
            LL_COUNT="$preserved_expected_modules"
            LL_SOURCE="preserved compiler recovery"
            LL_RECOVERY_SOURCE_DIR="$preserved_ll_dir"
            EXPECTED_STAGE2_MODULES="$preserved_expected_modules"
            echo -e "${YELLOW}Preserved compiler left a complete $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll set at $LL_RECOVERY_SOURCE_DIR; falling back to direct IR recovery.${NC}"
        fi
    fi
    rm -rf "$recovery_marker"
    return 1
}

link_recovered_compiler() {
    local output_path=$1
    local recovery_dir=$2
    local label=$3

    local obj_count
    obj_count=$(count_module_objects "$recovery_dir")
    echo "  ${label}: $obj_count recovered module objects ready."

    local rt_dir
    rt_dir="$(cd "$SCRIPT_DIR/.." && pwd)/seen_runtime"
    if [ ! -f "$rt_dir/seen_runtime.o" ] || [ "$rt_dir/seen_runtime.c" -nt "$rt_dir/seen_runtime.o" ]; then
        echo "  Pre-compiling runtime..."
        run_guarded_command "${label} runtime seen_runtime.c" 300 "$OPT_VMEM_KB" \
            clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections -pthread \
            -c -I "$rt_dir" "$rt_dir/seen_runtime.c" -o "$rt_dir/seen_runtime.o" 2>/dev/null || true
    fi
    if [ -f "$rt_dir/seen_region.c" ]; then
        if [ ! -f "$rt_dir/seen_region.o" ] || [ "$rt_dir/seen_region.c" -nt "$rt_dir/seen_region.o" ]; then
            run_guarded_command "${label} runtime seen_region.c" 300 "$OPT_VMEM_KB" \
                clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections \
                -c -I "$rt_dir" "$rt_dir/seen_region.c" -o "$rt_dir/seen_region.o" 2>/dev/null || true
        fi
    fi
    if [ -f "$rt_dir/seen_gpu.c" ]; then
        if [ ! -f "$rt_dir/seen_gpu.o" ] || [ "$rt_dir/seen_gpu.c" -nt "$rt_dir/seen_gpu.o" ]; then
            run_guarded_command "${label} runtime seen_gpu.c" 300 "$OPT_VMEM_KB" \
                clang -O3 -flto=thin "$RELEASE_CLANG_MARCH_FLAG" -ffunction-sections -fdata-sections \
                -c -I "$rt_dir" "$rt_dir/seen_gpu.c" -o "$rt_dir/seen_gpu.o" 2>/dev/null || true
        fi
    fi

    local link_objs=""
    local obj
    for obj in "$recovery_dir"/seen_module_*.o; do
        link_objs="$link_objs $obj"
    done

    local rt_objs="$rt_dir/seen_runtime.o"
    [ -f "$rt_dir/seen_region.o" ] && rt_objs="$rt_objs $rt_dir/seen_region.o"
    [ -f "$rt_dir/seen_gpu.o" ] && rt_objs="$rt_objs $rt_dir/seen_gpu.o"

    local link_libs="-lm -lpthread"
    [ -f "$rt_dir/seen_gpu.o" ] && pkg-config --exists vulkan 2>/dev/null && link_libs="$link_libs -lvulkan"

    echo "  Linking $obj_count modules..."
    if run_guarded_command "${label} recovery link" 0 "" clang -O1 -fuse-ld=lld \
        -Wl,--allow-multiple-definition \
        "$RELEASE_CLANG_MARCH_FLAG" -Wl,--gc-sections -Wno-unused-command-line-argument \
        $link_objs $rt_objs -o "$output_path" $link_libs 2>/tmp/safe_rebuild_link.log; then
        echo -e "${GREEN}${label} recovery link succeeded ($(wc -c < "$output_path" | tr -d ' ') bytes).${NC}"
        return 0
    fi

    echo -e "${RED}ERROR: ${label} recovery link failed.${NC}"
    grep -E 'undefined|error' /tmp/safe_rebuild_link.log | head -10
    return 1
}

recover_complete_ll_set_to_compiler() {
    local expected_modules=$1
    local marker_dir=$2
    local output_path=$3
    local label=$4

    if [ "$IR_RECOVERY_DISABLED" = "1" ]; then
        echo -e "${RED}ERROR: ${label} failed and SEEN_DISABLE_IR_RECOVERY=1 forbids direct IR recovery.${NC}"
        return 1
    fi

    if ! is_positive_integer "$expected_modules" || [ "$expected_modules" -le 0 ]; then
        return 1
    fi

    local ll_dir
    ll_dir=$(find_latest_compile_ll_dir_with_count "$expected_modules" "$marker_dir")
    if [ -z "$ll_dir" ]; then
        return 1
    fi

    echo -e "${YELLOW}${label} left a complete $expected_modules/$expected_modules .ll set at $ll_dir; falling back to direct IR recovery.${NC}"

    local recovery_exit=0
    local recovery_log="/tmp/seen_${label//[^A-Za-z0-9_]/_}_recovery_$$.log"
    set +e
    run_guarded_command "${label} IR recovery" "$RECOVERY_TIMEOUT_SECS" "$OPT_VMEM_KB" \
        bash "$SCRIPT_DIR/recovery_opt.sh" "$OPT_WRAPPER_DIR" "$SCRIPT_DIR" "$ll_dir" 2>&1 | tee "$recovery_log"
    recovery_exit=${PIPESTATUS[0]}
    set -e

    local recovery_output
    recovery_output=$(cat "$recovery_log" 2>/dev/null || true)
    rm -f "$recovery_log"

    if [ "$recovery_exit" -ne 0 ]; then
        echo -e "${RED}ERROR: ${label} IR recovery failed.${NC}"
        return 1
    fi

    local recovery_dir
    recovery_dir=$(echo "$recovery_output" | grep '^RECOVERY_DIR=' | tail -1 | cut -d= -f2)
    if [ -z "$recovery_dir" ] || [ ! -d "$recovery_dir" ]; then
        echo -e "${RED}ERROR: ${label} IR recovery did not return an output directory.${NC}"
        return 1
    fi

    local obj_count
    obj_count=$(count_module_objects "$recovery_dir")
    if [ "$obj_count" -ne "$expected_modules" ]; then
        local missing_modules
        missing_modules=$(list_modules_missing_objects "$recovery_dir")
        echo -e "${RED}ERROR: ${label} IR recovery produced only $obj_count/$expected_modules objects.${NC}"
        if [ -n "$missing_modules" ]; then
            echo "Missing objects:$missing_modules"
        fi
        rm -rf "$recovery_dir"
        return 1
    fi

    if link_recovered_compiler "$output_path" "$recovery_dir" "$label"; then
        rm -rf "$recovery_dir"
        return 0
    fi

    rm -rf "$recovery_dir"
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

    local recovery_source_root="$REPO_ROOT"
    local source_mode="${SEEN_EXISTING_BUILDER_SOURCE_ROOT:-auto}"
    if [ "${SEEN_EXISTING_BUILDER_USE_BOOTSTRAP_OVERLAY:-0}" = "1" ]; then
        source_mode="overlay"
    fi
    case "$source_mode" in
        overlay)
            recovery_source_root="$BOOTSTRAP_SOURCE_ROOT"
            ;;
        real)
            recovery_source_root="$REPO_ROOT"
            ;;
        auto)
            case "$builder_name" in
                seen_frozen*|stage1_frozen*) recovery_source_root="$BOOTSTRAP_SOURCE_ROOT" ;;
            esac
            ;;
        *)
            echo -e "${RED}ERROR: SEEN_EXISTING_BUILDER_SOURCE_ROOT must be auto, real, or overlay.${NC}" >&2
            return 1
            ;;
    esac
    local recovery_builder_path="$builder_path"
    case "$recovery_builder_path" in
        "$REPO_ROOT"/*)
            if [ "$recovery_source_root" = "$BOOTSTRAP_SOURCE_ROOT" ]; then
                recovery_builder_path="$BOOTSTRAP_SOURCE_ROOT/${recovery_builder_path#$REPO_ROOT/}"
            fi
            ;;
    esac

    local recovery_exit=0
    run_guarded_command_to_log "existing stage builder $builder_name recovery" "$RECOVERY_TIMEOUT_SECS" "$MAIN_COMPILER_VMEM_KB" "$builder_log" \
        bash -c 'cd "$1" || exit 1; shift; exec "$@"' bash "$recovery_source_root" \
        env PATH="$OPT_WRAPPER_DIR:$PATH" \
            SEEN_COMPILER_SOURCE_ROOT="$recovery_source_root" \
            SEEN_LOW_MEMORY="${SEEN_LOW_MEMORY:-0}" \
            SEEN_MAIN_VMEM_KB="$MAIN_COMPILER_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$OPT_VMEM_KB" \
            SEEN_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:-}" \
            "$recovery_builder_path" compile "$COMPILER_SOURCE" "$STAGE3_RECOVERY" \
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
        if [ "${SEEN_STAGE_BUILDER_ONLY:-0}" = "1" ] && [ -n "${SEEN_STAGE_BUILDER:-}" ] && [ "$candidate" = "$SEEN_STAGE_BUILDER" ]; then
            return 1
        fi
    done

    return 1
}

preserve_existing_production_compiler >/dev/null 2>&1 || true

# Every compiler produced from 0.10 onward invokes this exact, version-matched
# helper during dependency preparation. Build and verify it before any stage
# can become a builder for the next stage.
prepare_package_client || exit 1

if [ "$REBUILD_TIER" != "full" ]; then
    run_tiered_rebuild
    exit $?
fi

ensure_bootstrap_preflight || exit 1

clean_rebuild_caches "full tier cold verification"

if [ "${SEEN_SKIP_PREBUILD_GATES:-0}" != "1" ]; then
    echo "Running prebuild gates..."
    if ! run_guarded_command_to_log_with_failure_watch "prebuild gates" 900 "$MAIN_COMPILER_VMEM_KB" \
        /tmp/safe_rebuild_prebuild_gates.log \
        bash "$SCRIPT_DIR/seen_prebuild_gates.sh"; then
        echo -e "${RED}ERROR: prebuild gates failed.${NC}"
        tail_log_if_exists /tmp/safe_rebuild_prebuild_gates.log 30
        exit 1
    fi
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
repair_stale_builder_ir() {
    local file="\$1"
    # Fix C-style void parameters emitted by older builders. LLVM IR spells an
    # empty parameter list as (), and has no void parameter type.
    sed -i 's/, void)/)/g; s/(void, /(/g; s/, void, /, /g; s/(void)\([[:space:]]*\)/()\1/g' "\$file" 2>/dev/null || true

    # Drop impossible dead casts from void-returning calls. Older builders can
    # emit these after void statements, and LLVM rejects void as a value type.
    sed -i '/^[[:space:]]*%[0-9][0-9]* = bitcast void %[0-9][0-9]* to /d' "\$file" 2>/dev/null || true

    # Older builders can also leave invalid aggregate returns in unreachable
    # blocks, for example a ret of an i1 value from a SeenString function.
    # Replace only those verifier-invalid returns with a zero aggregate value.
    python3 - "\$file" <<'PY_RET_FIX' 2>&1 || true
import re
import sys

path = sys.argv[1]
try:
    with open(path, "r", encoding="utf-8") as fh:
        content = fh.read()
except OSError:
    sys.exit(0)

def fix_function_ret_values(function_text):
    i1_values = set(re.findall(r"^\s*(%\d+)\s*=\s*icmp\b", function_text, re.M))
    if not i1_values:
        return function_text

    def fix_ret(match):
        aggregate_type = match.group(1)
        value = match.group(2)
        if value in i1_values:
            return "  ret " + aggregate_type + " zeroinitializer"
        return match.group(0)

    return re.sub(
        r"^\s*ret\s+(%[A-Za-z0-9_.]+)\s+(%\d+)\s*$",
        fix_ret,
        function_text,
        flags=re.M,
    )

parts = []
last = 0
for match in re.finditer(r"^define\b", content, re.M):
    start = match.start()
    if start < last:
        continue
    end_match = re.search(r"^}\s*$", content[start:], re.M)
    if not end_match:
        continue
    end = start + end_match.end()
    parts.append(content[last:start])
    parts.append(fix_function_ret_values(content[start:end]))
    last = end
parts.append(content[last:])
fixed = "".join(parts)
count = 1 if fixed != content else 0
if count > 0 and fixed != content:
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(fixed)
    print("  stale-builder aggregate ret fix applied to " + path, file=sys.stderr)
PY_RET_FIX
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

        repair_stale_builder_ir "\$arg"

        # Fix corrupt declares from string constants leaking into declare generator
        # Stage1 parses @funcName(...) from string constants, producing broken declares
        # with \00 or other garbage. Remove these — the correct declare is already present.
        sed -i '/^declare.*\\\\00/d' "\$arg" 2>/dev/null || true

        # IR call-shape validation: compare direct call sites with declared/defined
        # signatures before opt, so aggregate/scalar ABI drift fails with context.
        if [ "\${SEEN_SKIP_IR_CALL_SHAPE_VERIFY:-0}" != "1" ]; then
            if ! python3 "$SCRIPT_DIR/verify_ir_call_shapes.py" "\$arg" 2>/tmp/seen_call_shape_err.txt; then
                echo "IR CALL SHAPE ERROR: \$arg" >&2
                head -20 /tmp/seen_call_shape_err.txt >&2
                rm -f /tmp/seen_call_shape_err.txt
                exit 1
            fi
            rm -f /tmp/seen_call_shape_err.txt
        fi

        # NOTE: Phantom declare removal disabled — it was too aggressive and removed
        # declares for cross-module functions (emitIncludeStrImpl, etc.) that ARE called
        # from the module via ThinLTO. The awk dedup + fix_ir.py handle the critical cases.

        # IR Validation: run llvm-as structural check on the fixed .ll file
        if ! llvm-as "\$arg" -o /dev/null 2>/tmp/seen_verify_err.txt; then
            echo "IR VERIFY WARNING: \$arg (retrying fixups)" >&2
            head -2 /tmp/seen_verify_err.txt >&2
            if python3 "$SCRIPT_DIR/fix_ir.py" "\$arg" 2>&1 && \
               repair_stale_builder_ir "\$arg" && \
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
        # like %SeenString { i64 7, ptr @.str }. verify_ir_call_shapes.py handles
        # direct call signatures without those comma-splitting false positives.
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
    elif [ "${SEEN_STAGE_BUILDER_ONLY:-0}" = "1" ]; then
        echo -e "${RED}ERROR: SEEN_STAGE_BUILDER_ONLY=1 and the selected stage builder failed.${NC}"
        exit 1
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
        env SEEN_COMPILER_SOURCE_ROOT="$BOOTSTRAP_SOURCE_ROOT" \
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

    FROZEN_COMPILE_ENV=(env "PATH=$OPT_WRAPPER_DIR:$PATH" "SEEN_COMPILER_SOURCE_ROOT=$BOOTSTRAP_SOURCE_ROOT")
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
    FAILURE_WATCHER_PID=$(start_log_failure_watcher "S1→S2" /tmp/safe_rebuild_stage2.log "$COMPILE_PID")
    start_ll_snapshot_watcher "$COMPILE_PID" "$SNAPSHOT_DIR" &
    WATCHER_PID=$!

    # Wait for compiler
    COMPILE_EXIT=0
    wait "$COMPILE_PID" || COMPILE_EXIT=$?

    # Stop monitor and watcher
    kill "$MONITOR_PID" 2>/dev/null || true
    wait "$MONITOR_PID" 2>/dev/null || true
    if [ -n "$FAILURE_WATCHER_PID" ]; then
        kill "$FAILURE_WATCHER_PID" 2>/dev/null || true
        wait "$FAILURE_WATCHER_PID" 2>/dev/null || true
    fi
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
        summarize_stage2_failure_log /tmp/safe_rebuild_stage2.log
        if [ "${SEEN_STOP_AFTER_FROZEN_STAGE2_FAILURE:-0}" = "1" ] ||
           [ "${SEEN_STAGE2_FAIL_FAST:-0}" = "1" ]; then
            echo -e "${RED}Stopping after frozen Stage2 failure as requested.${NC}"
            echo "Set SEEN_STAGE2_FAIL_FAST=0 to allow direct IR recovery."
            rm -rf "$SNAPSHOT_DIR"
            exit "$COMPILE_EXIT"
        fi
        if [ "$IR_RECOVERY_DISABLED" = "1" ]; then
            echo -e "${RED}ERROR: Stage2 failed and SEEN_DISABLE_IR_RECOVERY=1 forbids direct IR recovery.${NC}"
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
            RECOVERY_LOG="/tmp/seen_stage2_recovery_$$.log"
            set +e
            run_guarded_command "Stage2 IR recovery" "$RECOVERY_TIMEOUT_SECS" "$OPT_VMEM_KB" \
                bash "$SCRIPT_DIR/recovery_opt.sh" "$OPT_WRAPPER_DIR" "$SCRIPT_DIR" "$LL_RECOVERY_SOURCE_DIR" 2>&1 | tee "$RECOVERY_LOG"
            RECOVERY_EXIT=${PIPESTATUS[0]}
            set -e
            RECOVERY_OUTPUT=$(cat "$RECOVERY_LOG" 2>/dev/null || true)
            rm -f "$RECOVERY_LOG"

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
                    RETRY_COMPILE_ENV=(env "PATH=$OPT_WRAPPER_DIR:$PATH" "SEEN_COMPILER_SOURCE_ROOT=$BOOTSTRAP_SOURCE_ROOT")
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
                        env SEEN_COMPILER_SOURCE_ROOT="$BOOTSTRAP_SOURCE_ROOT" \
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

            if run_guarded_command "Stage2 recovery link" 0 "" clang -O1 -fuse-ld=lld \
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

    S3_MARKER=$(mktemp -d /tmp/seen_stage3_marker.XXXXXX 2>/dev/null || true)
    if run_guarded_command_to_log_with_failure_watch "S2->S3" 1800 "$MAIN_COMPILER_VMEM_KB" /tmp/safe_rebuild_stage3.log \
        "$STAGE2" compile "$COMPILER_SOURCE" "$STAGE3" --fast --no-cache --no-fork $RELEASE_TARGET_CPU_FLAG \
        ; then
        rm -rf "$S3_MARKER"
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
        EXPECTED_STAGE3_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage3.log)
        if [ "$IR_RECOVERY_DISABLED" = "1" ]; then
            rm -rf "$S3_MARKER"
            echo -e "${RED}ERROR: Stage3 failed and SEEN_DISABLE_IR_RECOVERY=1 forbids direct IR recovery.${NC}"
            exit "$S3_EXIT"
        fi
        if recover_complete_ll_set_to_compiler "$EXPECTED_STAGE3_MODULES" "$S3_MARKER" "$STAGE3_RECOVERY" "Stage3"; then
            rm -rf "$S3_MARKER"
            echo ""
            echo "Stage3 recovery smoke: checking hello-world..."
            if smoke_test_compiler "$STAGE3_RECOVERY" "Recovered stage3" "stage3_recovery"; then
                echo -e "${GREEN}Using recovered stage3 as production compiler.${NC}"
                VERIFIED="$STAGE3_RECOVERY"
            else
                echo -e "${YELLOW}Recovered stage3 failed hello-world smoke.${NC}"
                if [ "${SEEN_REQUIRE_STAGE3:-0}" = "1" ]; then
                    echo -e "${RED}ERROR: SEEN_REQUIRE_STAGE3=1 and recovered Stage3 smoke failed.${NC}"
                    exit 1
                fi
                echo -e "${GREEN}Using Stage2 as production compiler (verified via frozen bootstrap).${NC}"
                VERIFIED="$STAGE2"
            fi
        else
            rm -rf "$S3_MARKER"
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
cp "$PACKAGE_CLIENT_BUILD_OUTPUT" compiler_seen/target/seen-pkg
chmod +x compiler_seen/target/seen-pkg
cp "$STAGE2" stage2_head 2>/dev/null || true
[ -f "$STAGE3" ] && cp "$STAGE3" stage3_head 2>/dev/null || true
rm -f stage3_recovery_head 2>/dev/null || true
[ -f "$STAGE3_RECOVERY" ] && cp "$STAGE3_RECOVERY" stage3_recovery_head 2>/dev/null || true

# Also install to target/release/seen (README install path)
mkdir -p target/release
cp compiler_seen/target/seen target/release/seen
chmod +x target/release/seen
cp "$PACKAGE_CLIENT_BUILD_OUTPUT" target/release/seen-pkg
chmod +x target/release/seen-pkg
if declare -F seen_build_write_full_release_stamp >/dev/null 2>&1; then
    seen_build_write_full_release_stamp "$REPO_ROOT" "$REPO_ROOT/compiler_seen/target/seen"
fi

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
