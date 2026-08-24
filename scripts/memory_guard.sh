#!/usr/bin/env bash
# Run a command with process-tree memory safeguards.
#
# On Linux the guarded command runs in its own process group. Outside a verified
# cgroup, a process-table snapshot supplies the observer-only RSS/task guard.
# Inside a verified cgroup, memory.current is authoritative and the hot monitor
# uses only shell-builtin cgroup/proc reads; summed RSS is telemetry because
# forked copy-on-write mappings would otherwise be counted once per process.
# Resource-limit stops signal the whole group and the owning outer guard drains
# the complete transient scope.
#
# It also optionally applies a per-process virtual-memory ulimit to the child.

set -uo pipefail

RSS_LIMIT_KB="${SEEN_MEMORY_GUARD_RSS_KB:-}"
RESERVE_KB="${SEEN_MEMORY_GUARD_RESERVE_KB:-}"
VMEM_LIMIT_KB="${SEEN_MEMORY_GUARD_VMEM_KB:-}"
TIMEOUT_SECS="${SEEN_MEMORY_GUARD_TIMEOUT_SECS:-0}"
INTERVAL_SECS="${SEEN_MEMORY_GUARD_INTERVAL_SECS:-0.10}"
TASKS_MAX="${SEEN_MEMORY_GUARD_TASKS_MAX:-}"
REQUIRE_KERNEL_SCOPE="${SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE:-0}"
CGROUP_STOP_KB="${SEEN_MEMORY_GUARD_CGROUP_STOP_KB:-}"
KILL_ONLY="${SEEN_MEMORY_GUARD_KILL_ONLY:-0}"
LABEL="${SEEN_MEMORY_GUARD_LABEL:-command}"
METRICS_FILE="${SEEN_MEMORY_GUARD_METRICS_FILE:-}"
SUCCESS_METRICS_FILE="${SEEN_MEMORY_GUARD_SUCCESS_METRICS_FILE:-}"
REMOVE_EMPTY_TMPDIR="${SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR:-0}"
TEST_CGROUP_DIR="${SEEN_MEMORY_GUARD_TEST_CGROUP_DIR:-}"
TEST_TREE_RSS_KB="${SEEN_MEMORY_GUARD_TEST_TREE_RSS_KB:-}"
VERIFIED_CGROUP_DIR=""
VERIFIED_MEMORY_MAX_BYTES=""
VERIFIED_MEMORY_SWAP_MAX_BYTES=""
VERIFIED_MEMORY_OOM_GROUP=""
VERIFIED_PIDS_MAX=""
NATIVE_MEMORY_CURRENT_BYTES="unavailable"
NATIVE_MEMORY_PEAK_BYTES="unavailable"
NATIVE_MEMORY_EVENTS_LOW="unavailable"
NATIVE_MEMORY_EVENTS_HIGH="unavailable"
NATIVE_MEMORY_EVENTS_MAX="unavailable"
NATIVE_MEMORY_EVENTS_OOM="unavailable"
NATIVE_MEMORY_EVENTS_OOM_KILL="unavailable"
NATIVE_MEMORY_EVENTS_OOM_GROUP_KILL="unavailable"
NATIVE_MEMORY_EVENTS_LOCAL_LOW="unavailable"
NATIVE_MEMORY_EVENTS_LOCAL_HIGH="unavailable"
NATIVE_MEMORY_EVENTS_LOCAL_MAX="unavailable"
NATIVE_MEMORY_EVENTS_LOCAL_OOM="unavailable"
NATIVE_MEMORY_EVENTS_LOCAL_OOM_KILL="unavailable"
NATIVE_MEMORY_EVENTS_LOCAL_OOM_GROUP_KILL="unavailable"
NATIVE_PIDS_CURRENT="unavailable"
NATIVE_PIDS_PEAK="unavailable"
NATIVE_PIDS_EVENTS_MAX="unavailable"
NATIVE_PIDS_EVENTS_LOCAL_MAX="unavailable"
CGROUP_PROCESS_COUNT=0
CGROUP_SUMMED_RSS_KB=0
NATIVE_METRICS_FROZEN=0
METRICS_DIR=""

usage() {
    cat >&2 <<'EOF'
Usage: memory_guard.sh [options] -- command [args...]

Options:
  --rss-limit-kb N          Kernel MemoryMax request; observer-only RSS cap
                            outside a verified cgroup.
  --available-reserve-kb N  Kill when system MemAvailable falls below N KB.
  --vmem-limit-kb N         Apply ulimit -v N to the child process.
  --timeout-secs N          Kill if the command runs longer than N seconds.
  --interval-secs N         Poll interval in seconds (default: 0.10).
  --tasks-max N             In kernel-scope mode, cap total tasks in the command cgroup.
  --cgroup-stop-kb N        In kernel-scope mode, stop when cgroup memory.current reaches N KB.
  --kill-only               Stop the tree with SIGKILL only, without a SIGTERM grace period.
  --label TEXT              Human-readable label in guard messages.
  --metrics-file PATH       Write machine-readable peak-memory metrics.
  --success-metrics-file P  Atomically retain final metrics after success.
EOF
}

is_positive_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

is_nonnegative_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) return 0 ;;
    esac
}

is_positive_interval() {
    local value=${1:-}
    local fraction=""
    if is_positive_integer "$value"; then
        return 0
    fi
    case "$value" in
        0.*)
            fraction=${value#0.}
            case "$fraction" in
                ''|*[!0-9]*) return 1 ;;
                *[1-9]*) return 0 ;;
                *) return 1 ;;
            esac
            ;;
        *) return 1 ;;
    esac
}

format_kb() {
    local kb=$1
    if [ "$kb" -ge 1048576 ]; then
        local gb=$((kb / 1048576))
        local tenth=$((((kb % 1048576) * 10 + 524288) / 1048576))
        if [ "$tenth" -ge 10 ]; then
            gb=$((gb + 1))
            tenth=0
        fi
        printf "%d.%dGiB" "$gb" "$tenth"
    elif [ "$kb" -ge 1024 ]; then
        printf "%dMiB" "$((kb / 1024))"
    else
        printf "%dKiB" "$kb"
    fi
}

read_available_kb() {
    local available=""
    read_available_kb_into available
    [ -n "$available" ] && printf '%s\n' "$available"
}

read_available_kb_into() {
    local destination=$1
    local key=""
    local value=""
    local unit=""

    printf -v "$destination" '%s' ""
    if [ -r /proc/meminfo ]; then
        while IFS=' ' read -r key value unit; do
            if [ "$key" = "MemAvailable:" ] &&
                is_nonnegative_integer "$value"; then

                printf -v "$destination" '%s' "$value"
                return 0
            fi
        done < /proc/meminfo
    fi
}

detect_current_cgroup_dir() {
    local line
    local hierarchy
    local controllers
    local path

    [ -r /proc/self/cgroup ] || return 0
    while IFS=: read -r hierarchy controllers path; do
        if [ "$hierarchy" = "0" ] && [ -n "$path" ]; then
            if [ -d "/sys/fs/cgroup${path}" ]; then
                echo "/sys/fs/cgroup${path}"
                return 0
            fi
        elif [ "$controllers" = "memory" ] && [ -n "$path" ]; then
            if [ -d "/sys/fs/cgroup/memory${path}" ]; then
                echo "/sys/fs/cgroup/memory${path}"
                return 0
            fi
        fi
    done < /proc/self/cgroup
}

user_systemd_scope_available() {
    command -v systemd-run >/dev/null 2>&1 || return 1
    command -v systemctl >/dev/null 2>&1 || return 1
    systemctl --user show-environment >/dev/null 2>&1
}

user_systemd_manager_major() {
    local version=""
    local major=""

    # Query the running user manager rather than the systemctl client binary:
    # a newer client may be connected to an older manager that rejects scope
    # properties introduced after it was started.
    version=$(systemctl --user show --property=Version --value \
        2>/dev/null || true)
    case "$version" in
        [0-9]*) major=${version%%[!0-9]*} ;;
        *) return 1 ;;
    esac
    is_positive_integer "$major" || return 1
    printf '%s\n' "$major"
}

read_cgroup_value_into() {
    local file=$1
    local destination=$2
    local value="unavailable"

    if [ -r "$file" ]; then
        IFS= read -r value < "$file" || value="unavailable"
    fi
    if ! is_nonnegative_integer "$value"; then
        value="unavailable"
    fi
    printf -v "$destination" '%s' "$value"
}

read_cgroup_events_into() {
    local file=$1
    local prefix=$2
    local key=""
    local value=""

    printf -v "${prefix}_LOW" '%s' unavailable
    printf -v "${prefix}_HIGH" '%s' unavailable
    printf -v "${prefix}_MAX" '%s' unavailable
    printf -v "${prefix}_OOM" '%s' unavailable
    printf -v "${prefix}_OOM_KILL" '%s' unavailable
    printf -v "${prefix}_OOM_GROUP_KILL" '%s' unavailable
    [ -r "$file" ] || return 0

    while IFS=' ' read -r key value _; do
        is_nonnegative_integer "$value" || continue
        case "$key" in
            low) printf -v "${prefix}_LOW" '%s' "$value" ;;
            high) printf -v "${prefix}_HIGH" '%s' "$value" ;;
            max) printf -v "${prefix}_MAX" '%s' "$value" ;;
            oom) printf -v "${prefix}_OOM" '%s' "$value" ;;
            oom_kill) printf -v "${prefix}_OOM_KILL" '%s' "$value" ;;
            oom_group_kill)
                printf -v "${prefix}_OOM_GROUP_KILL" '%s' "$value"
                ;;
        esac
    done < "$file"
}

read_pids_events_into() {
    local file=$1
    local destination=$2
    local key=""
    local value=""

    printf -v "$destination" '%s' unavailable
    [ -r "$file" ] || return 0
    while IFS=' ' read -r key value _; do
        if [ "$key" = "max" ] && is_nonnegative_integer "$value"; then
            printf -v "$destination" '%s' "$value"
            return 0
        fi
    done < "$file"
}

capture_native_cgroup_metrics() {
    local cgroup_dir=${1:-}

    [ -n "$cgroup_dir" ] || return 0
    read_cgroup_value_into "$cgroup_dir/memory.current" \
        NATIVE_MEMORY_CURRENT_BYTES
    read_cgroup_value_into "$cgroup_dir/memory.peak" \
        NATIVE_MEMORY_PEAK_BYTES
    read_cgroup_events_into "$cgroup_dir/memory.events" NATIVE_MEMORY_EVENTS
    read_cgroup_events_into "$cgroup_dir/memory.events.local" \
        NATIVE_MEMORY_EVENTS_LOCAL
    read_cgroup_value_into "$cgroup_dir/pids.current" NATIVE_PIDS_CURRENT
    read_cgroup_value_into "$cgroup_dir/pids.peak" NATIVE_PIDS_PEAK
    read_pids_events_into "$cgroup_dir/pids.events" NATIVE_PIDS_EVENTS_MAX
    read_pids_events_into "$cgroup_dir/pids.events.local" \
        NATIVE_PIDS_EVENTS_LOCAL_MAX
}

# Sum per-process VmRSS for telemetry without starting ps/awk in a task-capped
# scope. Shared/COW pages remain deliberately double-counted; this value must
# never drive a stop when kernel cgroup accounting is available.
read_cgroup_process_snapshot() {
    local cgroup_dir=$1
    local pid=""
    local key=""
    local value=""
    local unit=""

    CGROUP_PROCESS_COUNT=0
    CGROUP_SUMMED_RSS_KB=0
    if is_nonnegative_integer "$TEST_TREE_RSS_KB"; then
        CGROUP_PROCESS_COUNT=1
        CGROUP_SUMMED_RSS_KB=$TEST_TREE_RSS_KB
        return 0
    fi
    [ -r "$cgroup_dir/cgroup.procs" ] || return 0
    while IFS= read -r pid; do
        is_positive_integer "$pid" || continue
        [ -r "/proc/$pid/status" ] || continue
        # The task may exit between the readability check and open. Open the
        # proc file explicitly so that expected race stays silent.
        if ! exec 3< "/proc/$pid/status" 2>/dev/null; then
            continue
        fi
        CGROUP_PROCESS_COUNT=$((CGROUP_PROCESS_COUNT + 1))
        while IFS=' ' read -r key value unit <&3; do
            if [ "$key" = "VmRSS:" ] && is_nonnegative_integer "$value"; then
                CGROUP_SUMMED_RSS_KB=$((CGROUP_SUMMED_RSS_KB + value))
                break
            fi
        done
        exec 3<&-
    done < "$cgroup_dir/cgroup.procs"
}

process_group_alive() {
    local pgid=${1:-}
    is_positive_integer "$pgid" || return 1
    [ "$pgid" -gt 1 ] || return 1
    kill -0 -- "-$pgid" 2>/dev/null
}

# Print "process_count rss_kb" from one process-table snapshot.  The fixed-point
# pass also catches live descendants which deliberately moved to another
# process group while their parent remains in the guarded tree.
process_group_snapshot() {
    local root=$1
    local pgid=$2
    ps -e -o pid=,ppid=,pgid=,rss= 2>/dev/null | awk -v root="$root" -v group="$pgid" '
        {
            count += 1
            pid[count] = $1
            parent[count] = $2
            process_group[count] = $3
            rss[count] = $4
            if ($1 == root || $3 == group) {
                included[$1] = 1
            }
        }
        END {
            changed = 1
            while (changed) {
                changed = 0
                for (i = 1; i <= count; i += 1) {
                    if (!included[pid[i]] && included[parent[i]]) {
                        included[pid[i]] = 1
                        changed = 1
                    }
                }
            }
            processes = 0
            total_rss = 0
            for (i = 1; i <= count; i += 1) {
                if (included[pid[i]]) {
                    processes += 1
                    total_rss += rss[i]
                }
            }
            printf "%d %d\n", processes, total_rss
        }
    '
}

# Find descendants that escaped the original process group. The token is
# exported only to the guarded command, so unrelated processes in an enclosing
# aggregate cgroup are excluded even when this is a nested command guard.
token_process_snapshot() {
    local token=$1
    local cgroup_dir=""
    local pid=""
    local candidates=""

    cgroup_dir=$(detect_current_cgroup_dir || true)
    if [ -n "$cgroup_dir" ] && [ -r "$cgroup_dir/cgroup.procs" ]; then
        while IFS= read -r pid; do
            token_process_entry "$token" "$pid"
        done < "$cgroup_dir/cgroup.procs"
        return 0
    fi
    if [ -d /proc ]; then
        candidates=$(ps -e -o pid= 2>/dev/null || true)
    fi
    for pid in $candidates; do
        token_process_entry "$token" "$pid"
    done
}

token_process_entry() {
    local token=$1
    local pid=$2
    local pgid=0
    local process_tokens=""
    local environment_entry=""
    local stat_line=""
    local stat_fields=""
    local process_state=""
    local parent_pid=""

    is_positive_integer "$pid" || return 0
    [ -r "/proc/$pid/environ" ] || return 0
    while IFS= read -r -d '' environment_entry; do
        case "$environment_entry" in
            SEEN_MEMORY_GUARD_PROCESS_TOKENS=*)
                process_tokens=${environment_entry#*=}
                break
                ;;
        esac
    done 2>/dev/null < "/proc/$pid/environ" || process_tokens=""
    case ":$process_tokens:" in
        *":$token:"*) ;;
        *) return 0 ;;
    esac

    if [ -r "/proc/$pid/stat" ]; then
        IFS= read -r stat_line < "/proc/$pid/stat" || stat_line=""
        stat_fields=${stat_line##*) }
        read -r process_state parent_pid pgid _ <<< "$stat_fields"
        is_positive_integer "$pgid" || pgid=0
    fi
    printf '%s %s\n' "$pid" "$pgid"
}

# Once the outer guarded command has exited, the scope-owning supervisor is
# the only process that can safely decide whether the aggregate cgroup is
# empty. This catches descendants which both changed process groups and
# deliberately scrubbed the inherited process token. Nested guards must not
# use this check because their cgroup also contains the parent rebuild.
owner_scope_remaining_processes() {
    local supervisor_pid=$1
    local monitor_pid=$2
    local root_pid=$3
    local pid=""

    OWNER_SCOPE_REMAINING=""
    [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" = "1" ] || return 0
    [ -n "$VERIFIED_CGROUP_DIR" ] || return 1
    [ -r "$VERIFIED_CGROUP_DIR/cgroup.procs" ] || return 1

    while IFS= read -r pid; do
        is_positive_integer "$pid" || continue
        case "$pid" in
            "$supervisor_pid"|"$monitor_pid"|"$root_pid") continue ;;
        esac
        [ -d "/proc/$pid" ] || continue
        OWNER_SCOPE_REMAINING="${OWNER_SCOPE_REMAINING:+$OWNER_SCOPE_REMAINING }$pid"
    done < "$VERIFIED_CGROUP_DIR/cgroup.procs"
}

stop_token_processes() {
    local token=$1
    local attempt=0
    local snapshot=""
    local pid=""
    local pgid=""

    while [ "$attempt" -lt 20 ]; do
        snapshot=$(token_process_snapshot "$token")
        [ -n "$snapshot" ] || return 0
        while read -r pid pgid; do
            is_positive_integer "$pid" || continue
            if is_positive_integer "$pgid" && [ "$pgid" -gt 1 ]; then
                signal_process_group "$pgid" KILL
            fi
            kill -KILL "$pid" 2>/dev/null || true
        done <<< "$snapshot"
        sleep 0.01
        attempt=$((attempt + 1))
    done
    return 1
}

signal_process_group() {
    local pgid=$1
    local signal=$2
    if is_positive_integer "$pgid" && [ "$pgid" -gt 1 ]; then
        kill "-$signal" -- "-$pgid" 2>/dev/null || true
    fi
}

stop_process_group() {
    local pgid=$1
    local urgent=${2:-0}
    if [ "$urgent" = "1" ] || [ "$KILL_ONLY" = "1" ]; then
        signal_process_group "$pgid" KILL
        return
    fi
    signal_process_group "$pgid" TERM
    sleep 1
    signal_process_group "$pgid" KILL
}

validated_scope_unit() {
    local unit=${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}
    local stem=""
    local unit_uid=""
    local unit_pid=""
    local current_uid=""

    case "$unit" in
        seen-memory-guard-*-*.scope) ;;
        *) return 1 ;;
    esac
    stem=${unit#seen-memory-guard-}
    stem=${stem%.scope}
    unit_uid=${stem%%-*}
    unit_pid=${stem#*-}
    case "$unit_uid" in
        ''|*[!0-9]*) return 1 ;;
    esac
    is_positive_integer "$unit_pid" || return 1
    current_uid=$(id -u 2>/dev/null || true)
    [ "$unit_uid" = "$current_uid" ] || return 1
    printf '%s\n' "$unit"
}

# A nested command guard deliberately creates another process group. Killing
# only the outer leader group could therefore leave compiler grandchildren
# alive in the transient scope. Resource-limit stops ask the user manager to
# kill every process in the verified scope as the final containment action.
stop_kernel_scope() {
    local scope_unit=""

    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] || return 0
    [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" = "1" ] || return 0
    scope_unit=$(validated_scope_unit || true)
    [ -n "$scope_unit" ] || return 1
    [ -n "$VERIFIED_CGROUP_DIR" ] || return 1
    [ "${VERIFIED_CGROUP_DIR##*/}" = "$scope_unit" ] || return 1
    command -v systemctl >/dev/null 2>&1 || return 1

    systemctl --user kill --signal=KILL --kill-whom=all "$scope_unit" \
        >/dev/null 2>&1 || return 1
}

stop_guarded_tree() {
    local pgid=$1
    local urgent=${2:-0}

    stop_process_group "$pgid" "$urgent"
    if [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" = "1" ] &&
        ! stop_kernel_scope; then

        echo "memory_guard[$LABEL]: failed to request cgroup-wide termination" >&2
    fi
}

write_metrics_file() {
    local state=$1
    local peak_rss=${2:-0}
    local peak_cgroup=${3:-0}
    local current_rss=${4:-0}
    local peak_tasks=${5:-0}
    local command_status=${6:-}
    local temporary=""
    local metrics_cgroup_dir=""
    local native_peak_kb=0
    local recorded_at=""

    [ -n "$METRICS_FILE" ] || return 0
    if [ -n "$TEST_CGROUP_DIR" ]; then
        metrics_cgroup_dir=$TEST_CGROUP_DIR
    elif [ -n "$VERIFIED_CGROUP_DIR" ]; then
        metrics_cgroup_dir=$VERIFIED_CGROUP_DIR
    fi
    if [ "$NATIVE_METRICS_FROZEN" != "1" ]; then
        capture_native_cgroup_metrics "$metrics_cgroup_dir"
    fi
    if is_nonnegative_integer "$NATIVE_MEMORY_PEAK_BYTES"; then
        native_peak_kb=$(((NATIVE_MEMORY_PEAK_BYTES + 1023) / 1024))
        if [ "$native_peak_kb" -gt "$peak_cgroup" ]; then
            peak_cgroup=$native_peak_kb
        fi
    fi
    TZ=UTC printf -v recorded_at '%(%Y-%m-%dT%H:%M:%SZ)T' -1
    temporary="$METRICS_FILE.tmp.$$"
    {
        printf 'metrics_version=4\n'
        printf 'label=%s\n' "$LABEL"
        printf 'state=%s\n' "$state"
        printf 'peak_rss_kb=%s\n' "$peak_rss"
        printf 'peak_cgroup_kb=%s\n' "$peak_cgroup"
        printf 'last_rss_kb=%s\n' "$current_rss"
        printf 'peak_tasks=%s\n' "$peak_tasks"
        printf 'rss_limit_kb=%s\n' "${RSS_LIMIT_KB:-0}"
        printf 'cgroup_stop_kb=%s\n' "${CGROUP_STOP_KB:-0}"
        printf 'vmem_limit_kb=%s\n' "${VMEM_LIMIT_KB:-0}"
        printf 'available_reserve_kb=%s\n' "${RESERVE_KB:-0}"
        printf 'timeout_secs=%s\n' "$TIMEOUT_SECS"
        printf 'tasks_max=%s\n' "${TASKS_MAX:-0}"
        printf 'verified_cgroup_path=%s\n' "${VERIFIED_CGROUP_DIR:-unverified}"
        printf 'verified_memory_max_bytes=%s\n' "${VERIFIED_MEMORY_MAX_BYTES:-unverified}"
        printf 'verified_memory_swap_max_bytes=%s\n' "${VERIFIED_MEMORY_SWAP_MAX_BYTES:-unverified}"
        printf 'verified_pids_max=%s\n' "${VERIFIED_PIDS_MAX:-unverified}"
        printf 'cgroup_memory_current_bytes=%s\n' "$NATIVE_MEMORY_CURRENT_BYTES"
        printf 'cgroup_memory_peak_bytes=%s\n' "$NATIVE_MEMORY_PEAK_BYTES"
        printf 'cgroup_memory_events_low=%s\n' "$NATIVE_MEMORY_EVENTS_LOW"
        printf 'cgroup_memory_events_high=%s\n' "$NATIVE_MEMORY_EVENTS_HIGH"
        printf 'cgroup_memory_events_max=%s\n' "$NATIVE_MEMORY_EVENTS_MAX"
        printf 'cgroup_memory_events_oom=%s\n' "$NATIVE_MEMORY_EVENTS_OOM"
        printf 'cgroup_memory_events_oom_kill=%s\n' "$NATIVE_MEMORY_EVENTS_OOM_KILL"
        printf 'cgroup_memory_events_oom_group_kill=%s\n' "$NATIVE_MEMORY_EVENTS_OOM_GROUP_KILL"
        printf 'cgroup_memory_events_local_low=%s\n' "$NATIVE_MEMORY_EVENTS_LOCAL_LOW"
        printf 'cgroup_memory_events_local_high=%s\n' "$NATIVE_MEMORY_EVENTS_LOCAL_HIGH"
        printf 'cgroup_memory_events_local_max=%s\n' "$NATIVE_MEMORY_EVENTS_LOCAL_MAX"
        printf 'cgroup_memory_events_local_oom=%s\n' "$NATIVE_MEMORY_EVENTS_LOCAL_OOM"
        printf 'cgroup_memory_events_local_oom_kill=%s\n' "$NATIVE_MEMORY_EVENTS_LOCAL_OOM_KILL"
        printf 'cgroup_memory_events_local_oom_group_kill=%s\n' "$NATIVE_MEMORY_EVENTS_LOCAL_OOM_GROUP_KILL"
        printf 'cgroup_pids_current=%s\n' "$NATIVE_PIDS_CURRENT"
        printf 'cgroup_pids_peak=%s\n' "$NATIVE_PIDS_PEAK"
        printf 'cgroup_pids_events_max=%s\n' "$NATIVE_PIDS_EVENTS_MAX"
        printf 'cgroup_pids_events_local_max=%s\n' "$NATIVE_PIDS_EVENTS_LOCAL_MAX"
        if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ]; then
            printf 'enforcement=kernel_cgroup\n'
            printf 'summed_rss_role=telemetry_only\n'
        else
            printf 'enforcement=process_group_observer\n'
            printf 'summed_rss_role=observer_limit\n'
        fi
        printf 'recorded_at=%s\n' "$recorded_at"
        if [ -n "$command_status" ]; then
            printf 'command_status=%s\n' "$command_status"
        fi
    } > "$temporary" 2>/dev/null &&
        mv -f -- "$temporary" "$METRICS_FILE" 2>/dev/null || true
}

retain_success_metrics() {
    local source_file=$METRICS_FILE
    local destination=$SUCCESS_METRICS_FILE
    local destination_dir=""
    local temporary=""

    [ -n "$destination" ] || return 0
    [ -n "$source_file" ] && [ -f "$source_file" ] || return 1
    destination_dir=$(dirname -- "$destination")
    mkdir -p -- "$destination_dir" 2>/dev/null || return 1
    temporary="${destination}.tmp.$$"
    cp -- "$source_file" "$temporary" 2>/dev/null || return 1
    mv -f -- "$temporary" "$destination" 2>/dev/null || return 1
}

append_metrics_status() {
    local command_status=$1
    local line=""
    local temporary=""

    [ -n "$METRICS_FILE" ] || return 0
    if [ ! -f "$METRICS_FILE" ]; then
        write_metrics_file "unobserved" 0 0 0 0 "$command_status"
        return 0
    fi
    temporary="$METRICS_FILE.tmp.$$"
    {
        while IFS= read -r line || [ -n "$line" ]; do
            case "$line" in
                command_status=*) continue ;;
            esac
            printf '%s\n' "$line"
        done < "$METRICS_FILE"
        printf 'command_status=%s\n' "$command_status"
    } > "$temporary" 2>/dev/null &&
        mv -f -- "$temporary" "$METRICS_FILE" 2>/dev/null || true
}

require_option_value() {
    local option=$1
    local count=$2
    if [ "$count" -lt 2 ]; then
        echo "memory_guard: $option requires a value" >&2
        exit 2
    fi
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --rss-limit-kb)
            require_option_value "$1" "$#"
            RSS_LIMIT_KB="${2:-}"
            shift 2
            ;;
        --available-reserve-kb)
            require_option_value "$1" "$#"
            RESERVE_KB="${2:-}"
            shift 2
            ;;
        --vmem-limit-kb)
            require_option_value "$1" "$#"
            VMEM_LIMIT_KB="${2:-}"
            shift 2
            ;;
        --timeout-secs)
            require_option_value "$1" "$#"
            TIMEOUT_SECS="${2:-}"
            shift 2
            ;;
        --interval-secs)
            require_option_value "$1" "$#"
            INTERVAL_SECS="${2:-}"
            shift 2
            ;;
        --tasks-max)
            require_option_value "$1" "$#"
            TASKS_MAX="${2:-}"
            shift 2
            ;;
        --cgroup-stop-kb)
            require_option_value "$1" "$#"
            CGROUP_STOP_KB="${2:-}"
            shift 2
            ;;
        --kill-only)
            KILL_ONLY=1
            shift
            ;;
        --label)
            require_option_value "$1" "$#"
            LABEL="${2:-command}"
            shift 2
            ;;
        --metrics-file)
            require_option_value "$1" "$#"
            METRICS_FILE="${2:-}"
            shift 2
            ;;
        --success-metrics-file)
            require_option_value "$1" "$#"
            SUCCESS_METRICS_FILE="${2:-}"
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
if ! is_positive_interval "$INTERVAL_SECS"; then
    echo "memory_guard: --interval-secs must be a positive number" >&2
    exit 2
fi
if [ -n "$TASKS_MAX" ] && ! is_positive_integer "$TASKS_MAX"; then
    echo "memory_guard: --tasks-max must be a positive integer" >&2
    exit 2
fi
if [ "$REQUIRE_KERNEL_SCOPE" = "1" ] &&
    { [ -z "$TASKS_MAX" ] || [ "$TASKS_MAX" -gt 24 ]; }; then

    echo "memory_guard: a required kernel scope needs --tasks-max between 1 and 24" >&2
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
if [ "$KILL_ONLY" != "0" ] && [ "$KILL_ONLY" != "1" ]; then
    echo "memory_guard: --kill-only must be unset, 0, or 1" >&2
    exit 2
fi
if [ "$REQUIRE_KERNEL_SCOPE" != "0" ] && [ "$REQUIRE_KERNEL_SCOPE" != "1" ]; then
    echo "memory_guard: SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE must be 0 or 1" >&2
    exit 2
fi
if [ "${SEEN_MEMORY_GUARD_KERNEL_SCOPE:-1}" != "0" ] &&
    [ "${SEEN_MEMORY_GUARD_KERNEL_SCOPE:-1}" != "1" ]; then
    echo "memory_guard: SEEN_MEMORY_GUARD_KERNEL_SCOPE must be 0 or 1" >&2
    exit 2
fi
if [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" != "0" ] &&
    [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" != "1" ]; then
    echo "memory_guard: SEEN_MEMORY_GUARD_SCOPE_OWNER must be 0 or 1" >&2
    exit 2
fi
if [ "$REMOVE_EMPTY_TMPDIR" != "0" ] && [ "$REMOVE_EMPTY_TMPDIR" != "1" ]; then
    echo "memory_guard: SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR must be 0 or 1" >&2
    exit 2
fi
if { [ -n "$TEST_CGROUP_DIR" ] || [ -n "$TEST_TREE_RSS_KB" ]; } &&
    [ "${SEEN_MEMORY_GUARD_TEST_HOOKS:-0}" != "1" ]; then

    echo "memory_guard[$LABEL]: refusing ungated cgroup accounting test hook" >&2
    exit 126
fi
if [ -n "$TEST_TREE_RSS_KB" ] &&
    ! is_nonnegative_integer "$TEST_TREE_RSS_KB"; then

    echo "memory_guard[$LABEL]: refusing invalid test-only tree RSS value" >&2
    exit 126
fi
if [ -n "$SUCCESS_METRICS_FILE" ] && [ -z "$METRICS_FILE" ]; then
    echo "memory_guard: --success-metrics-file requires --metrics-file" >&2
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
    if ! user_systemd_scope_available; then
        echo "memory_guard[$LABEL]: refusing to run; systemd-run is unavailable for kernel memory scope" >&2
        exit 126
    fi
fi

verify_kernel_scope_limits() {
    local cgroup_dir=""
    local memory_max=""
    local memory_swap_max=""
    local memory_oom_group=""
    local pids_max=""
    local expected_memory_bytes=0
    local scope_unit=""
    local unit_control_group=""
    local unit_cgroup_dir=""

    cgroup_dir=$(detect_current_cgroup_dir || true)
    if [ -z "$cgroup_dir" ]; then
        echo "memory_guard[$LABEL]: refusing to run; cannot resolve the transient cgroup" >&2
        return 1
    fi
    if [ ! -r "$cgroup_dir/memory.max" ] ||
        [ ! -r "$cgroup_dir/memory.swap.max" ] ||
        [ ! -r "$cgroup_dir/pids.max" ]; then
        echo "memory_guard[$LABEL]: refusing to run; transient cgroup lacks MemoryMax, MemorySwapMax, or TasksMax" >&2
        return 1
    fi

    IFS= read -r memory_max < "$cgroup_dir/memory.max" || true
    IFS= read -r memory_swap_max < "$cgroup_dir/memory.swap.max" || true
    IFS= read -r pids_max < "$cgroup_dir/pids.max" || true
    expected_memory_bytes=$((RSS_LIMIT_KB * 1024))
    if ! is_positive_integer "$memory_max" || [ "$memory_max" -gt "$expected_memory_bytes" ]; then
        echo "memory_guard[$LABEL]: refusing to run; transient cgroup MemoryMax is not enforcing the requested cap" >&2
        return 1
    fi
    if [ "$memory_swap_max" != "0" ]; then
        echo "memory_guard[$LABEL]: refusing to run; transient cgroup MemorySwapMax is not zero" >&2
        return 1
    fi
    if [ -n "$TASKS_MAX" ] &&
        { ! is_positive_integer "$pids_max" || [ "$pids_max" -gt "$TASKS_MAX" ]; }; then
        echo "memory_guard[$LABEL]: refusing to run; transient cgroup TasksMax is not enforcing the requested cap" >&2
        return 1
    fi

    scope_unit=$(validated_scope_unit || true)
    if [ -z "$scope_unit" ]; then
        echo "memory_guard[$LABEL]: refusing to run; transient scope unit identity is missing or invalid" >&2
        return 1
    fi
    if ! command -v systemctl >/dev/null 2>&1; then
        echo "memory_guard[$LABEL]: refusing to run; systemctl is required to verify cgroup ownership" >&2
        return 1
    fi
    unit_control_group=$(systemctl --user show "$scope_unit" \
        --property=ControlGroup --value 2>/dev/null || true)
    if [ -z "$unit_control_group" ] || [ "$unit_control_group" = "/" ]; then
        echo "memory_guard[$LABEL]: refusing to run; cannot read the transient scope ControlGroup" >&2
        return 1
    fi
    unit_cgroup_dir="/sys/fs/cgroup${unit_control_group}"
    if [ "$unit_cgroup_dir" != "$cgroup_dir" ]; then
        echo "memory_guard[$LABEL]: refusing to run; current cgroup does not match the transient scope unit" >&2
        return 1
    fi

    if [ ! -r "$cgroup_dir/memory.oom.group" ]; then
        echo "memory_guard[$LABEL]: refusing to run; transient cgroup lacks group-wide OOM control" >&2
        return 1
    fi
    IFS= read -r memory_oom_group < "$cgroup_dir/memory.oom.group" || true
    if [ "$memory_oom_group" != "1" ]; then
        if [ ! -w "$cgroup_dir/memory.oom.group" ] ||
            ! printf '1\n' > "$cgroup_dir/memory.oom.group"; then

            echo "memory_guard[$LABEL]: refusing to run; cannot enable group-wide OOM enforcement" >&2
            return 1
        fi
        IFS= read -r memory_oom_group < "$cgroup_dir/memory.oom.group" || true
    fi
    if [ "$memory_oom_group" != "1" ]; then
        echo "memory_guard[$LABEL]: refusing to run; group-wide OOM enforcement failed read-back" >&2
        return 1
    fi

    VERIFIED_CGROUP_DIR=$cgroup_dir
    VERIFIED_MEMORY_MAX_BYTES=$memory_max
    VERIFIED_MEMORY_SWAP_MAX_BYTES=$memory_swap_max
    VERIFIED_MEMORY_OOM_GROUP=$memory_oom_group
    VERIFIED_PIDS_MAX=$pids_max
    echo "memory_guard[$LABEL]: verified cgroup=$VERIFIED_CGROUP_DIR memory.max=$VERIFIED_MEMORY_MAX_BYTES memory.swap.max=$VERIFIED_MEMORY_SWAP_MAX_BYTES memory.oom.group=$VERIFIED_MEMORY_OOM_GROUP pids.max=$VERIFIED_PIDS_MAX" >&2
    return 0
}

if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_KERNEL_SCOPE:-1}" != "0" ] &&
    [ -n "$RSS_LIMIT_KB" ] &&
    user_systemd_scope_available; then

    guard_uid=$(id -u 2>/dev/null || echo 0)
    unit_name="seen-memory-guard-${guard_uid}-$$"
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
    if [ "$KILL_ONLY" = "1" ]; then
        scoped_args+=(--kill-only)
    fi
    if [ -n "$METRICS_FILE" ]; then
        scoped_args+=(--metrics-file "$METRICS_FILE")
    fi
    if [ -n "$SUCCESS_METRICS_FILE" ]; then
        scoped_args+=(--success-metrics-file "$SUCCESS_METRICS_FILE")
    fi
    scoped_args+=(--interval-secs "$INTERVAL_SECS" --label "$LABEL" -- "$@")

    high_kb=$((RSS_LIMIT_KB * 99 / 100))
    if [ "$high_kb" -lt 1 ]; then
        high_kb=1
    fi
    tasks_max="${TASKS_MAX:-24}"
    scope_properties=(
        -p "MemoryMax=${RSS_LIMIT_KB}K"
        -p "MemoryHigh=${high_kb}K"
        -p "MemorySwapMax=0"
        -p "TasksMax=${tasks_max}"
    )
    manager_major=$(user_systemd_manager_major || true)
    if is_positive_integer "$manager_major" &&
        [ "$manager_major" -ge 253 ]; then

        scope_properties+=(-p "OOMPolicy=kill")
    fi

    # A transient scope executes the command as a child of systemd-run.  Unlike
    # a transient service, it preserves the caller's mount namespace, including
    # safe_rebuild's project-local /tmp mapping. User managers at version 253 or
    # newer accept OOMPolicy=kill for scopes, allowing systemd to set
    # memory.oom.group before the delegated process starts. Older or unknown
    # managers omit that version-specific property. Every version still uses
    # verify_kernel_scope_limits to write the kernel control directly when
    # needed, read it back, and refuse execution unless it reports exactly 1.
    exec systemd-run --user --scope --quiet \
        --unit="$unit_name" \
        "${scope_properties[@]}" \
        env \
            SEEN_MEMORY_GUARD_IN_SCOPE=1 \
            SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE="$REQUIRE_KERNEL_SCOPE" \
            SEEN_MEMORY_GUARD_SCOPE_UNIT="${unit_name}.scope" \
            SEEN_MEMORY_GUARD_SCOPE_OWNER=1 \
            SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR="$REMOVE_EMPTY_TMPDIR" \
            "${scoped_args[@]}"
fi

if [ "$REQUIRE_KERNEL_SCOPE" = "1" ]; then
    if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then
        echo "memory_guard[$LABEL]: refusing to run outside the required kernel memory scope" >&2
        exit 126
    fi
    if ! verify_kernel_scope_limits; then
        exit 126
    fi
fi

if [ -n "$RESERVE_KB" ]; then
    initial_available=$(read_available_kb || true)
    if is_positive_integer "$initial_available" && [ "$initial_available" -lt "$RESERVE_KB" ]; then
        echo "memory_guard[$LABEL]: refusing to start; MemAvailable $(format_kb "$initial_available") is below reserve $(format_kb "$RESERVE_KB")" >&2
        exit 125
    fi
fi

guard_tmpdir=${TMPDIR:-}
if [ -z "$guard_tmpdir" ]; then
    echo "memory_guard[$LABEL]: refusing to run; TMPDIR must be explicitly project-local" >&2
    exit 126
fi
case "$guard_tmpdir" in
    /*) ;;
    *)
        echo "memory_guard[$LABEL]: refusing to run; TMPDIR must be absolute" >&2
        exit 126
        ;;
esac
if [ ! -d "$guard_tmpdir" ] || [ -L "$guard_tmpdir" ] || [ ! -w "$guard_tmpdir" ]; then
    echo "memory_guard[$LABEL]: refusing to run; TMPDIR is not a safe writable directory" >&2
    exit 126
fi
guard_tmpdir_physical=$(cd -P -- "$guard_tmpdir" 2>/dev/null && pwd -P || true)
if [ "$guard_tmpdir_physical" != "$guard_tmpdir" ]; then
    echo "memory_guard[$LABEL]: refusing to run; TMPDIR traverses a symbolic link or is not canonical" >&2
    exit 126
fi

prepare_metrics_parent() {
    local metrics_path=$1
    local parent=""

    [ -n "$metrics_path" ] || return 0
    case "$metrics_path" in
        */*) parent=${metrics_path%/*} ;;
        *) parent=. ;;
    esac
    [ -n "$parent" ] || parent=/
    if [ -L "$parent" ]; then
        echo "memory_guard[$LABEL]: refusing symbolic-link metrics directory: $parent" >&2
        return 1
    fi
    mkdir -p -- "$parent" 2>/dev/null || return 1
    [ -d "$parent" ] && [ ! -L "$parent" ] && [ -w "$parent" ] || return 1
    if [ "$metrics_path" = "$METRICS_FILE" ]; then
        METRICS_DIR=$parent
    fi
}

if ! prepare_metrics_parent "$METRICS_FILE" ||
    ! prepare_metrics_parent "$SUCCESS_METRICS_FILE"; then

    echo "memory_guard[$LABEL]: refusing unsafe or unwritable metrics destination" >&2
    exit 126
fi

cleanup_guard_tmpdir() {
    if [ "$REMOVE_EMPTY_TMPDIR" = "1" ]; then
        rmdir -- "$guard_tmpdir" 2>/dev/null || true
    fi
}
reason_file=$(mktemp "$guard_tmpdir/seen_memory_guard_reason.XXXXXX") || exit 126
ready_file=$(mktemp "$guard_tmpdir/seen_memory_guard_ready.XXXXXX") || exit 126
rm -f "$reason_file" "$ready_file"

TRUSTED_SETSID=""
for setsid_candidate in /usr/bin/setsid /bin/setsid; do
    if [ -f "$setsid_candidate" ] && [ -x "$setsid_candidate" ] &&
        [ ! -L "$setsid_candidate" ]; then

        TRUSTED_SETSID=$setsid_candidate
        break
    fi
done
if [ -z "$TRUSTED_SETSID" ]; then
    echo "memory_guard[$LABEL]: refusing to run; no trusted absolute setsid executable is available" >&2
    exit 126
fi
GUARD_PROCESS_TOKEN="seen-memory-guard-$$-${RANDOM}-${SECONDS}"
export GUARD_PROCESS_TOKEN

TEST_READY_DELAY_SECS=${SEEN_MEMORY_GUARD_TEST_READY_DELAY_SECS:-}
TEST_READY_PID_FILE=${SEEN_MEMORY_GUARD_TEST_READY_PID_FILE:-}
if [ -n "$TEST_READY_DELAY_SECS" ] || [ -n "$TEST_READY_PID_FILE" ]; then
    if [ "${SEEN_MEMORY_GUARD_TEST_HOOKS:-0}" != "1" ] ||
        ! is_positive_integer "$TEST_READY_DELAY_SECS"; then

        echo "memory_guard[$LABEL]: refusing invalid test-only ready-delay hook" >&2
        exit 126
    fi
    case "$TEST_READY_PID_FILE" in
        "$guard_tmpdir"/seen_memory_guard_test_ready_pid.*) ;;
        *)
            echo "memory_guard[$LABEL]: refusing unsafe test-only ready PID path" >&2
            exit 126
            ;;
    esac
    if [ -e "$TEST_READY_PID_FILE" ] || [ -L "$TEST_READY_PID_FILE" ] ||
        [ "$(dirname -- "$TEST_READY_PID_FILE")" != "$guard_tmpdir" ]; then

        echo "memory_guard[$LABEL]: refusing existing or escaped test-only ready PID path" >&2
        exit 126
    fi
fi
if [ -n "$TEST_CGROUP_DIR" ]; then
    case "$TEST_CGROUP_DIR" in
        "$guard_tmpdir"/seen_memory_guard_test_cgroup.*) ;;
        *)
            echo "memory_guard[$LABEL]: refusing unsafe test-only cgroup directory" >&2
            exit 126
            ;;
    esac
    if [ ! -d "$TEST_CGROUP_DIR" ] || [ -L "$TEST_CGROUP_DIR" ] ||
        [ "${TEST_CGROUP_DIR%/*}" != "$guard_tmpdir" ]; then

        echo "memory_guard[$LABEL]: refusing unsafe test-only cgroup directory" >&2
        exit 126
    fi
    test_cgroup_physical=$(cd -P -- "$TEST_CGROUP_DIR" 2>/dev/null && pwd -P || true)
    if [ "$test_cgroup_physical" != "$TEST_CGROUP_DIR" ]; then
        echo "memory_guard[$LABEL]: refusing noncanonical test-only cgroup directory" >&2
        exit 126
    fi
    for test_cgroup_file in \
        memory.current memory.peak memory.events memory.events.local \
        pids.current pids.peak pids.events pids.events.local cgroup.procs; do

        if [ ! -f "$TEST_CGROUP_DIR/$test_cgroup_file" ] ||
            [ -L "$TEST_CGROUP_DIR/$test_cgroup_file" ] ||
            [ ! -r "$TEST_CGROUP_DIR/$test_cgroup_file" ]; then

            echo "memory_guard[$LABEL]: incomplete test-only cgroup accounting directory" >&2
            exit 126
        fi
    done
fi

(
    if [ -n "$VMEM_LIMIT_KB" ]; then
        if ! ulimit -S -v "$VMEM_LIMIT_KB" 2>/dev/null; then
            echo "memory_guard[$LABEL]: refusing to run; could not apply the virtual-memory cap" >&2
            exit 126
        fi
        active_vmem=$(ulimit -S -v 2>/dev/null || true)
        if ! is_positive_integer "$active_vmem" || [ "$active_vmem" -gt "$VMEM_LIMIT_KB" ]; then
            echo "memory_guard[$LABEL]: refusing to run; virtual-memory cap verification failed" >&2
            exit 126
        fi
    fi
    export SEEN_MEMORY_GUARD_SCOPE_OWNER=0
    if [ -n "${SEEN_MEMORY_GUARD_PROCESS_TOKENS:-}" ]; then
        export SEEN_MEMORY_GUARD_PROCESS_TOKENS="${SEEN_MEMORY_GUARD_PROCESS_TOKENS}:$GUARD_PROCESS_TOKEN"
    else
        export SEEN_MEMORY_GUARD_PROCESS_TOKENS="$GUARD_PROCESS_TOKEN"
    fi
    exec "$TRUSTED_SETSID" bash -c '
        ready_file=$1
        process_token=$2
        ready_delay=$3
        ready_pid_file=$4
        shift 4
        export SEEN_MEMORY_GUARD_SCOPE_OWNER=0
        case ":${SEEN_MEMORY_GUARD_PROCESS_TOKENS:-}:" in
            *":$process_token:"*) ;;
            *) exit 126 ;;
        esac
        if [ -n "$ready_delay" ]; then
            (set -C; printf "%s\n" "$$" > "$ready_pid_file") || exit 126
            sleep "$ready_delay" || exit 126
        fi
        printf "%s\n" "$$" > "$ready_file" || exit 126
        # This flag belongs to the current supervisor only. Letting the
        # guarded command inherit it allows a nested guard to rmdir a shared
        # project TMPDIR after an otherwise successful preflight.
        export SEEN_MEMORY_GUARD_REMOVE_EMPTY_TMPDIR=0
        exec "$@"
    ' bash "$ready_file" "$GUARD_PROCESS_TOKEN" \
        "$TEST_READY_DELAY_SECS" "$TEST_READY_PID_FILE" "$@"
) &
child_pid=$!
child_pgid=$child_pid
monitor_pid=""
guard_cleanup_armed=1

cleanup_guard_on_exit() {
    local status=$?
    if [ "${guard_cleanup_armed:-0}" = "1" ]; then
        stop_guarded_tree "$child_pgid" 1
        kill -KILL "$child_pid" 2>/dev/null || true
        stop_token_processes "$GUARD_PROCESS_TOKEN" || true
    fi
    cleanup_guard_tmpdir
    return "$status"
}

trap cleanup_guard_on_exit EXIT

terminate_guarded_command() {
    local status=${1:-143}
    echo "$status" > "$reason_file" 2>/dev/null || true
    stop_guarded_tree "$child_pgid" 1
    kill -KILL "$child_pid" 2>/dev/null || true
    stop_token_processes "$GUARD_PROCESS_TOKEN" || true
    guard_cleanup_armed=0
    if [ -n "${monitor_pid:-}" ]; then
        kill "$monitor_pid" 2>/dev/null || true
    fi
}

# Arm signal containment before the child can complete its ready handshake.
# Otherwise a fast child could expose a brief default-HUP window and outlive
# the supervisor.
trap 'terminate_guarded_command 143; exit 143' HUP TERM INT

ready_attempt=0
while [ ! -s "$ready_file" ] && kill -0 "$child_pid" 2>/dev/null && [ "$ready_attempt" -lt 100 ]; do
    sleep 0.01
    ready_attempt=$((ready_attempt + 1))
done
ready_pid=""
if [ -s "$ready_file" ]; then
    IFS= read -r ready_pid < "$ready_file" || true
fi
rm -f "$ready_file"
if [ "$ready_pid" != "$child_pid" ]; then
    echo "memory_guard[$LABEL]: refusing to continue; command did not enter its isolated process group" >&2
    write_metrics_file "startup_failure" 0 0 0 1 126
    kill -KILL "$child_pid" 2>/dev/null || true
    stop_guarded_tree "$child_pgid" 1
    stop_token_processes "$GUARD_PROCESS_TOKEN" || true
    wait "$child_pid" 2>/dev/null || true
    guard_cleanup_armed=0
    rm -f "$reason_file"
    cleanup_guard_tmpdir
    exit 126
fi

monitor_loop() {
    local root=$1
    local pgid=$2
    local started=$3
    local process_token=$4
    local supervisor_pid=$5
    local peak_rss=0
    local cgroup_dir=""
    local cgroup_memory_file=""
    local cgroup_stop_kb="$CGROUP_STOP_KB"
    local peak_cgroup_kb=0
    local peak_tasks=0
    local empty_observations=0
    local kernel_cgroup_mode=0
    local authoritative_cgroup_monitor=0
    local initial_pids_events_max="unavailable"

    # The monitor is a background subshell. It must not inherit the parent
    # guard's EXIT cleanup trap, or a normal monitor return can kill detached
    # work without recording the containment failure.
    trap - EXIT
    guard_cleanup_armed=0

    if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] && [ -n "$RSS_LIMIT_KB" ]; then
        if [ -n "$TEST_CGROUP_DIR" ]; then
            cgroup_dir=$TEST_CGROUP_DIR
        elif [ -n "$VERIFIED_CGROUP_DIR" ]; then
            cgroup_dir=$VERIFIED_CGROUP_DIR
        else
            cgroup_dir=$(detect_current_cgroup_dir || true)
        fi
        if [ -n "$cgroup_dir" ] && [ -r "$cgroup_dir/memory.current" ] &&
            [ -r "$cgroup_dir/cgroup.procs" ]; then

            kernel_cgroup_mode=1
            cgroup_memory_file="$cgroup_dir/memory.current"
            if [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" = "1" ] ||
                [ -n "$TEST_CGROUP_DIR" ]; then

                authoritative_cgroup_monitor=1
                capture_native_cgroup_metrics "$cgroup_dir"
                initial_pids_events_max=$NATIVE_PIDS_EVENTS_MAX
            fi
        fi
        if [ -z "$cgroup_stop_kb" ]; then
            cgroup_stop_kb=$((RSS_LIMIT_KB * 90 / 100))
            if [ "$cgroup_stop_kb" -lt 1 ]; then
                cgroup_stop_kb=1
            fi
        fi
    fi

    while :; do
        local snapshot=""
        local tasks=0
        local rss=0
        local escaped_snapshot=""
        local cgroup_kb=0

        if [ "$kernel_cgroup_mode" = "1" ] &&
            [ "$authoritative_cgroup_monitor" = "1" ]; then

            capture_native_cgroup_metrics "$cgroup_dir"
            read_cgroup_process_snapshot "$cgroup_dir"
            rss=$CGROUP_SUMMED_RSS_KB
            if is_nonnegative_integer "$NATIVE_PIDS_CURRENT"; then
                tasks=$NATIVE_PIDS_CURRENT
            else
                tasks=$CGROUP_PROCESS_COUNT
            fi
            if is_nonnegative_integer "$NATIVE_MEMORY_CURRENT_BYTES"; then
                cgroup_kb=$(((NATIVE_MEMORY_CURRENT_BYTES + 1023) / 1024))
            fi
            if is_nonnegative_integer "$NATIVE_MEMORY_PEAK_BYTES"; then
                local native_peak_kb=$(((NATIVE_MEMORY_PEAK_BYTES + 1023) / 1024))
                if [ "$native_peak_kb" -gt "$peak_cgroup_kb" ]; then
                    peak_cgroup_kb=$native_peak_kb
                fi
            fi
        elif [ "$kernel_cgroup_mode" != "1" ]; then
            snapshot=$(process_group_snapshot "$root" "$pgid")
            read -r tasks rss <<< "$snapshot"
            tasks=${tasks:-0}
            rss=${rss:-0}
        fi

        if ! process_group_alive "$pgid"; then
            if [ "$kernel_cgroup_mode" = "1" ] &&
                [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" = "1" ]; then

                if ! owner_scope_remaining_processes \
                    "$supervisor_pid" "$BASHPID" "$root"; then

                    echo "memory_guard[$LABEL]: refusing to return; could not prove the aggregate cgroup is empty" >&2
                    echo "125" > "$reason_file"
                    write_metrics_file "detached_descendants" "$peak_rss" "$peak_cgroup_kb" 0 "$peak_tasks" 125
                    stop_guarded_tree "$pgid" 1
                    return
                fi
                if [ -n "$OWNER_SCOPE_REMAINING" ]; then
                    echo "memory_guard[$LABEL]: stopping detached cgroup processes after the command exited: $OWNER_SCOPE_REMAINING" >&2
                    echo "125" > "$reason_file"
                    write_metrics_file "detached_descendants" "$peak_rss" "$peak_cgroup_kb" 0 "$peak_tasks" 125
                    stop_guarded_tree "$pgid" 1
                    return
                fi
            else
                escaped_snapshot=$(token_process_snapshot "$process_token")
                if [ -n "$escaped_snapshot" ]; then
                    echo "memory_guard[$LABEL]: stopping detached descendants which escaped the command process group" >&2
                    echo "125" > "$reason_file"
                    write_metrics_file "detached_descendants" "$peak_rss" "$peak_cgroup_kb" 0 "$peak_tasks" 125
                    stop_token_processes "$process_token" || true
                    stop_guarded_tree "$pgid" 1
                    return
                fi
            fi
            empty_observations=$((empty_observations + 1))
            if [ "$empty_observations" -ge 3 ]; then
                break
            fi
            sleep "$INTERVAL_SECS"
            continue
        fi
        empty_observations=0
        if [ "$tasks" -gt "$peak_tasks" ]; then
            peak_tasks=$tasks
        fi
        if [ "$rss" -gt "$peak_rss" ]; then
            peak_rss=$rss
        fi

        if [ "$kernel_cgroup_mode" != "1" ] &&
            [ -n "$TASKS_MAX" ] && [ "$tasks" -gt "$TASKS_MAX" ]; then

            echo "memory_guard[$LABEL]: stopping command; process count $tasks exceeded cap $TASKS_MAX" >&2
            echo "137" > "$reason_file"
            write_metrics_file "tasks_limit" "$peak_rss" "$peak_cgroup_kb" "$rss" "$peak_tasks" 137
            stop_guarded_tree "$pgid" 1
            return
        fi

        if [ "$kernel_cgroup_mode" != "1" ] &&
            [ -n "$RSS_LIMIT_KB" ] && [ "$rss" -ge "$RSS_LIMIT_KB" ]; then

            echo "memory_guard[$LABEL]: stopping command; RSS $(format_kb "$rss") exceeded cap $(format_kb "$RSS_LIMIT_KB") (peak $(format_kb "$peak_rss"))" >&2
            echo "137" > "$reason_file"
            write_metrics_file "rss_limit" "$peak_rss" "$peak_cgroup_kb" "$rss" "$peak_tasks" 137
            stop_guarded_tree "$pgid" 1
            return
        fi

        if [ "$kernel_cgroup_mode" = "1" ] &&
            { [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" = "1" ] ||
              [ -n "$TEST_CGROUP_DIR" ]; } &&
            [ -n "$cgroup_memory_file" ] && [ -n "$cgroup_stop_kb" ] &&
            [ "$cgroup_kb" -ge "$cgroup_stop_kb" ]; then

            if [ "$cgroup_kb" -gt "$peak_cgroup_kb" ]; then
                peak_cgroup_kb=$cgroup_kb
            fi
            echo "memory_guard[$LABEL]: stopping command; cgroup memory $(format_kb "$cgroup_kb") reached stop threshold $(format_kb "$cgroup_stop_kb") below hard cap $(format_kb "$RSS_LIMIT_KB") (summed RSS telemetry $(format_kb "$rss"), peak cgroup $(format_kb "$peak_cgroup_kb"))" >&2
            echo "137" > "$reason_file"
            write_metrics_file "cgroup_limit" "$peak_rss" "$peak_cgroup_kb" "$rss" "$peak_tasks" 137
            stop_guarded_tree "$pgid" 1
            return
        fi

        if [ "$kernel_cgroup_mode" = "1" ] &&
            [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" = "1" ] &&
            is_nonnegative_integer "$initial_pids_events_max" &&
            is_nonnegative_integer "$NATIVE_PIDS_EVENTS_MAX" &&
            [ "$NATIVE_PIDS_EVENTS_MAX" -gt "$initial_pids_events_max" ]; then

            echo "memory_guard[$LABEL]: stopping command; pids controller rejected task creation (events $initial_pids_events_max -> $NATIVE_PIDS_EVENTS_MAX, pids.current=$NATIVE_PIDS_CURRENT, cap=$TASKS_MAX)" >&2
            echo "137" > "$reason_file"
            # Preserve the event-time controller snapshot, then free the
            # workload's task slots before the single atomic metrics rename.
            # The owner performs the cgroup-wide drain immediately afterward.
            NATIVE_METRICS_FROZEN=1
            stop_process_group "$pgid" 1
            write_metrics_file "tasks_limit" "$peak_rss" "$peak_cgroup_kb" "$rss" "$peak_tasks" 137
            stop_guarded_tree "$pgid" 1
            return
        fi

        if [ -n "$RESERVE_KB" ] &&
            { [ "$kernel_cgroup_mode" != "1" ] ||
              [ "${SEEN_MEMORY_GUARD_SCOPE_OWNER:-0}" = "1" ]; }; then

            local available=""
            read_available_kb_into available
            if is_positive_integer "$available" && [ "$available" -lt "$RESERVE_KB" ]; then
                echo "memory_guard[$LABEL]: stopping command; MemAvailable $(format_kb "$available") fell below reserve $(format_kb "$RESERVE_KB") (tree RSS $(format_kb "$rss"), peak $(format_kb "$peak_rss"))" >&2
                echo "137" > "$reason_file"
                write_metrics_file "reserve_limit" "$peak_rss" "$peak_cgroup_kb" "$rss" "$peak_tasks" 137
                stop_guarded_tree "$pgid" 1
                return
            fi
        fi

        if [ "$TIMEOUT_SECS" != "0" ]; then
            local elapsed=$((SECONDS - started))
            if [ "$elapsed" -gt "$TIMEOUT_SECS" ]; then
                echo "memory_guard[$LABEL]: stopping command; timeout ${TIMEOUT_SECS}s exceeded (tree RSS $(format_kb "$rss"), peak $(format_kb "$peak_rss"))" >&2
                echo "124" > "$reason_file"
                write_metrics_file "timeout" "$peak_rss" "$peak_cgroup_kb" "$rss" "$peak_tasks" 124
                stop_guarded_tree "$pgid" 0
                return
            fi
        fi

        if ! kill -0 "$root" 2>/dev/null && process_group_alive "$pgid"; then
            echo "memory_guard[$LABEL]: stopping detached descendants after the command exited" >&2
            echo "125" > "$reason_file"
            write_metrics_file "detached_descendants" "$peak_rss" "$peak_cgroup_kb" "$rss" "$peak_tasks" 125
            stop_guarded_tree "$pgid" 1
            return
        fi

        sleep "$INTERVAL_SECS"
    done
    write_metrics_file "complete" "$peak_rss" "$peak_cgroup_kb" 0 "$peak_tasks"
}

guard_start=$SECONDS
guard_supervisor_pid=$$
monitor_loop "$child_pid" "$child_pgid" "$guard_start" \
    "$GUARD_PROCESS_TOKEN" "$guard_supervisor_pid" &
monitor_pid=$!

command_status=0
wait "$child_pid" || command_status=$?

trap - HUP TERM INT
wait "$monitor_pid" 2>/dev/null || true
guard_cleanup_armed=0

if [ -f "$reason_file" ]; then
    guarded_status=$(cat "$reason_file" 2>/dev/null || echo 137)
    rm -f "$reason_file"
    cleanup_guard_tmpdir
    append_metrics_status "$guarded_status"
    exit "$guarded_status"
fi

rm -f "$reason_file"
append_metrics_status "$command_status"
if [ "$command_status" -eq 0 ] && ! retain_success_metrics; then
    echo "memory_guard[$LABEL]: could not retain successful metrics" >&2
    exit 125
fi
cleanup_guard_tmpdir
exit "$command_status"
