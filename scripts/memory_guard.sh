#!/usr/bin/env bash
# Run a command with process-tree memory safeguards.
#
# The guard stops the whole child process tree if either:
# - total child-tree RSS exceeds --rss-limit-kb, or
# - system MemAvailable drops below --available-reserve-kb.
#
# It also optionally applies a per-process virtual-memory ulimit to the child.

set -uo pipefail

RSS_LIMIT_KB="${SEEN_MEMORY_GUARD_RSS_KB:-}"
RESERVE_KB="${SEEN_MEMORY_GUARD_RESERVE_KB:-}"
VMEM_LIMIT_KB="${SEEN_MEMORY_GUARD_VMEM_KB:-}"
TIMEOUT_SECS="${SEEN_MEMORY_GUARD_TIMEOUT_SECS:-0}"
INTERVAL_SECS="${SEEN_MEMORY_GUARD_INTERVAL_SECS:-1}"
TASKS_MAX="${SEEN_MEMORY_GUARD_TASKS_MAX:-}"
REQUIRE_KERNEL_SCOPE="${SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE:-0}"
CGROUP_STOP_KB="${SEEN_MEMORY_GUARD_CGROUP_STOP_KB:-}"
LABEL="${SEEN_MEMORY_GUARD_LABEL:-command}"

usage() {
    cat >&2 <<'EOF'
Usage: memory_guard.sh [options] -- command [args...]

Options:
  --rss-limit-kb N          Kill when child process-tree RSS exceeds N KB.
  --available-reserve-kb N  Kill when system MemAvailable falls below N KB.
  --vmem-limit-kb N         Apply ulimit -v N to the child process.
  --timeout-secs N          Kill if the command runs longer than N seconds.
  --interval-secs N         Poll interval in seconds (default: 1).
  --tasks-max N             In kernel-scope mode, cap total tasks in the command cgroup.
  --cgroup-stop-kb N        In kernel-scope mode, stop when cgroup memory.current reaches N KB.
  --label TEXT              Human-readable label in guard messages.
EOF
}

is_positive_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

format_kb() {
    local kb=$1
    if [ "$kb" -ge 1048576 ]; then
        local gb=$((kb / 1048576))
        local tenth=$(((kb % 1048576) * 10 / 1048576))
        printf "%d.%dGiB" "$gb" "$tenth"
    elif [ "$kb" -ge 1024 ]; then
        printf "%dMiB" "$((kb / 1024))"
    else
        printf "%dKiB" "$kb"
    fi
}

read_available_kb() {
    if [ -r /proc/meminfo ]; then
        awk '/^MemAvailable:/ { print $2; exit }' /proc/meminfo 2>/dev/null
    fi
}

detect_cgroup_memory_current_file() {
    local line
    local hierarchy
    local controllers
    local path

    [ -r /proc/self/cgroup ] || return 0
    while IFS=: read -r hierarchy controllers path; do
        if [ "$hierarchy" = "0" ] && [ -n "$path" ]; then
            if [ -r "/sys/fs/cgroup${path}/memory.current" ]; then
                echo "/sys/fs/cgroup${path}/memory.current"
                return 0
            fi
        elif [ "$controllers" = "memory" ] && [ -n "$path" ]; then
            if [ -r "/sys/fs/cgroup/memory${path}/memory.usage_in_bytes" ]; then
                echo "/sys/fs/cgroup/memory${path}/memory.usage_in_bytes"
                return 0
            fi
        fi
    done < /proc/self/cgroup
}

read_cgroup_memory_kb() {
    local file=$1
    local bytes=""
    if [ -r "$file" ]; then
        IFS= read -r bytes < "$file" || true
    fi
    if is_positive_integer "$bytes"; then
        echo $(((bytes + 1023) / 1024))
    fi
}

children_of() {
    local pid=$1
    if command -v pgrep >/dev/null 2>&1; then
        pgrep -P "$pid" 2>/dev/null || true
        return
    fi
    ps -eo pid=,ppid= 2>/dev/null | awk -v ppid="$pid" '$2 == ppid { print $1 }'
}

collect_tree() {
    local root=$1
    kill -0 "$root" 2>/dev/null || return 0
    echo "$root"
    local child
    for child in $(children_of "$root"); do
        collect_tree "$child"
    done
}

pid_rss_kb() {
    local pid=$1
    local rss=""
    if [ -r "/proc/$pid/status" ]; then
        rss=$(awk '/^VmRSS:/ { print $2; exit }' "/proc/$pid/status" 2>/dev/null)
    fi
    if ! is_positive_integer "$rss"; then
        rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d '[:space:]' || true)
    fi
    if is_positive_integer "$rss"; then
        echo "$rss"
    else
        echo 0
    fi
}

tree_rss_kb() {
    local root=$1
    local total=0
    local pid
    for pid in $(collect_tree "$root" | sort -n | uniq); do
        total=$((total + $(pid_rss_kb "$pid")))
    done
    echo "$total"
}

kill_tree_with_signal() {
    local root=$1
    local signal=$2
    local pid
    for pid in $(collect_tree "$root" | sort -rn | uniq); do
        kill "-$signal" "$pid" 2>/dev/null || true
    done
}

stop_tree() {
    local root=$1
    kill_tree_with_signal "$root" TERM
    sleep 2
    kill_tree_with_signal "$root" KILL
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --rss-limit-kb)
            RSS_LIMIT_KB="${2:-}"
            shift 2
            ;;
        --available-reserve-kb)
            RESERVE_KB="${2:-}"
            shift 2
            ;;
        --vmem-limit-kb)
            VMEM_LIMIT_KB="${2:-}"
            shift 2
            ;;
        --timeout-secs)
            TIMEOUT_SECS="${2:-}"
            shift 2
            ;;
        --interval-secs)
            INTERVAL_SECS="${2:-}"
            shift 2
            ;;
        --tasks-max)
            TASKS_MAX="${2:-}"
            shift 2
            ;;
        --cgroup-stop-kb)
            CGROUP_STOP_KB="${2:-}"
            shift 2
            ;;
        --label)
            LABEL="${2:-command}"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        --)
            shift
            break
            ;;
        *)
            echo "memory_guard: unknown option: $1" >&2
            usage
            exit 2
            ;;
    esac
done

if [ "$#" -eq 0 ]; then
    echo "memory_guard: missing command" >&2
    usage
    exit 2
fi

if [ -n "$RSS_LIMIT_KB" ] && ! is_positive_integer "$RSS_LIMIT_KB"; then
    echo "memory_guard: --rss-limit-kb must be a positive integer" >&2
    exit 2
fi
if [ -n "$RESERVE_KB" ] && ! is_positive_integer "$RESERVE_KB"; then
    echo "memory_guard: --available-reserve-kb must be a positive integer" >&2
    exit 2
fi
if [ -n "$VMEM_LIMIT_KB" ] && ! is_positive_integer "$VMEM_LIMIT_KB"; then
    echo "memory_guard: --vmem-limit-kb must be a positive integer" >&2
    exit 2
fi
if ! is_positive_integer "$INTERVAL_SECS"; then
    echo "memory_guard: --interval-secs must be a positive integer" >&2
    exit 2
fi
if [ -n "$TASKS_MAX" ] && ! is_positive_integer "$TASKS_MAX"; then
    echo "memory_guard: --tasks-max must be a positive integer" >&2
    exit 2
fi
if [ -n "$CGROUP_STOP_KB" ] && ! is_positive_integer "$CGROUP_STOP_KB"; then
    echo "memory_guard: --cgroup-stop-kb must be a positive integer" >&2
    exit 2
fi
if [ "$TIMEOUT_SECS" != "0" ] && ! is_positive_integer "$TIMEOUT_SECS"; then
    echo "memory_guard: --timeout-secs must be 0 or a positive integer" >&2
    exit 2
fi

if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ] &&
    [ "$REQUIRE_KERNEL_SCOPE" = "1" ]; then
    if [ "${SEEN_MEMORY_GUARD_KERNEL_SCOPE:-1}" = "0" ]; then
        echo "memory_guard[$LABEL]: refusing to run; kernel memory scope is required but disabled" >&2
        exit 126
    fi
    if [ -z "$RSS_LIMIT_KB" ]; then
        echo "memory_guard[$LABEL]: refusing to run; kernel memory scope requires --rss-limit-kb" >&2
        exit 126
    fi
    if ! command -v systemd-run >/dev/null 2>&1; then
        echo "memory_guard[$LABEL]: refusing to run; systemd-run is unavailable for kernel memory scope" >&2
        exit 126
    fi
fi

if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_KERNEL_SCOPE:-1}" != "0" ] &&
    [ -n "$RSS_LIMIT_KB" ] &&
    command -v systemd-run >/dev/null 2>&1; then

    unit_name="seen-memory-guard-$USER-$$"
    scoped_args=("$0")
    if [ -n "$RSS_LIMIT_KB" ]; then
        scoped_args+=(--rss-limit-kb "$RSS_LIMIT_KB")
    fi
    if [ -n "$RESERVE_KB" ]; then
        scoped_args+=(--available-reserve-kb "$RESERVE_KB")
    fi
    if [ -n "$VMEM_LIMIT_KB" ]; then
        scoped_args+=(--vmem-limit-kb "$VMEM_LIMIT_KB")
    fi
    if [ "$TIMEOUT_SECS" != "0" ]; then
        scoped_args+=(--timeout-secs "$TIMEOUT_SECS")
    fi
    if [ -n "$TASKS_MAX" ]; then
        scoped_args+=(--tasks-max "$TASKS_MAX")
    fi
    if [ -n "$CGROUP_STOP_KB" ]; then
        scoped_args+=(--cgroup-stop-kb "$CGROUP_STOP_KB")
    fi
    scoped_args+=(--interval-secs "$INTERVAL_SECS" --label "$LABEL" -- "$@")

    high_kb=$((RSS_LIMIT_KB * 90 / 100))
    if [ "$high_kb" -lt 1 ]; then
        high_kb=1
    fi
    tasks_max="${TASKS_MAX:-512}"

    exec systemd-run --user --wait --collect -P -d \
        --unit="$unit_name" \
        -p "MemoryMax=${RSS_LIMIT_KB}K" \
        -p "MemoryHigh=${high_kb}K" \
        -p "MemorySwapMax=0" \
        -p "TasksMax=${tasks_max}" \
        -E SEEN_MEMORY_GUARD_IN_SCOPE=1 \
        -E SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE="$REQUIRE_KERNEL_SCOPE" \
        "${scoped_args[@]}"
fi

if [ -n "$RESERVE_KB" ]; then
    initial_available=$(read_available_kb || true)
    if is_positive_integer "$initial_available" && [ "$initial_available" -lt "$RESERVE_KB" ]; then
        echo "memory_guard[$LABEL]: refusing to start; MemAvailable $(format_kb "$initial_available") is below reserve $(format_kb "$RESERVE_KB")" >&2
        exit 125
    fi
fi

reason_file=$(mktemp /tmp/seen_memory_guard_reason.XXXXXX)
rm -f "$reason_file"

(
    if [ -n "$VMEM_LIMIT_KB" ]; then
        ulimit -v "$VMEM_LIMIT_KB" 2>/dev/null || true
    fi
    exec "$@"
) &
child_pid=$!

monitor_loop() {
    local root=$1
    local started=$2
    local peak_rss=0
    local cgroup_memory_file=""
    local cgroup_stop_kb="$CGROUP_STOP_KB"
    local peak_cgroup_kb=0

    if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] && [ -n "$RSS_LIMIT_KB" ]; then
        cgroup_memory_file=$(detect_cgroup_memory_current_file || true)
        if [ -z "$cgroup_stop_kb" ]; then
            cgroup_stop_kb=$((RSS_LIMIT_KB * 90 / 100))
            if [ "$cgroup_stop_kb" -lt 1 ]; then
                cgroup_stop_kb=1
            fi
        fi
    fi

    while kill -0 "$root" 2>/dev/null; do
        local rss
        rss=$(tree_rss_kb "$root")
        if [ "$rss" -gt "$peak_rss" ]; then
            peak_rss=$rss
        fi

        if [ -n "$RSS_LIMIT_KB" ] && [ "$rss" -gt "$RSS_LIMIT_KB" ]; then
            echo "memory_guard[$LABEL]: stopping command; RSS $(format_kb "$rss") exceeded cap $(format_kb "$RSS_LIMIT_KB") (peak $(format_kb "$peak_rss"))" >&2
            echo "137" > "$reason_file"
            stop_tree "$root"
            return
        fi

        if [ -n "$cgroup_memory_file" ] && [ -n "$cgroup_stop_kb" ]; then
            local cgroup_kb
            cgroup_kb=$(read_cgroup_memory_kb "$cgroup_memory_file" || true)
            if is_positive_integer "$cgroup_kb"; then
                if [ "$cgroup_kb" -gt "$peak_cgroup_kb" ]; then
                    peak_cgroup_kb=$cgroup_kb
                fi
                if [ "$cgroup_kb" -ge "$cgroup_stop_kb" ]; then
                    echo "memory_guard[$LABEL]: stopping command; cgroup memory $(format_kb "$cgroup_kb") reached stop threshold $(format_kb "$cgroup_stop_kb") below hard cap $(format_kb "$RSS_LIMIT_KB") (tree RSS $(format_kb "$rss"), peak cgroup $(format_kb "$peak_cgroup_kb"))" >&2
                    echo "137" > "$reason_file"
                    stop_tree "$root"
                    return
                fi
            fi
        fi

        if [ -n "$RESERVE_KB" ]; then
            local available
            available=$(read_available_kb || true)
            if is_positive_integer "$available" && [ "$available" -lt "$RESERVE_KB" ]; then
                echo "memory_guard[$LABEL]: stopping command; MemAvailable $(format_kb "$available") fell below reserve $(format_kb "$RESERVE_KB") (tree RSS $(format_kb "$rss"), peak $(format_kb "$peak_rss"))" >&2
                echo "137" > "$reason_file"
                stop_tree "$root"
                return
            fi
        fi

        if [ "$TIMEOUT_SECS" != "0" ]; then
            local elapsed=$((SECONDS - started))
            if [ "$elapsed" -gt "$TIMEOUT_SECS" ]; then
                echo "memory_guard[$LABEL]: stopping command; timeout ${TIMEOUT_SECS}s exceeded (tree RSS $(format_kb "$rss"), peak $(format_kb "$peak_rss"))" >&2
                echo "124" > "$reason_file"
                stop_tree "$root"
                return
            fi
        fi

        sleep "$INTERVAL_SECS"
    done
}

guard_start=$SECONDS
monitor_loop "$child_pid" "$guard_start" &
monitor_pid=$!

command_status=0
wait "$child_pid" || command_status=$?

kill "$monitor_pid" 2>/dev/null || true
wait "$monitor_pid" 2>/dev/null || true

if [ -f "$reason_file" ]; then
    guarded_status=$(cat "$reason_file" 2>/dev/null || echo 137)
    rm -f "$reason_file"
    exit "$guarded_status"
fi

rm -f "$reason_file"
exit "$command_status"
