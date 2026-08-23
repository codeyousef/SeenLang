#!/usr/bin/env bash
# Enter or re-verify the aggregate hard-memory scope required by build-capable
# Seen verification scripts. This wrapper never treats environment markers as
# proof: memory_guard.sh reads MemoryMax, MemorySwapMax, memory.oom.group,
# TasksMax, and the transient unit ControlGroup before it starts the requested
# command.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
MEMORY_GUARD="$SCRIPT_DIR/memory_guard.sh"
ARTIFACT_ROOT_HELPER="$SCRIPT_DIR/artifact_root.sh"
SERIAL_AUXILIARY_HELPER="$SCRIPT_DIR/serial_auxiliary_env.sh"
LABEL="Seen build-capable command"
VERIFY_ONLY=0
TIMEOUT_SECS=0

usage() {
    cat >&2 <<'EOF'
Usage: run_in_hard_memory_scope.sh [--label TEXT] [--timeout-secs N] [--verify-only] -- command [args...]

Enters a verified Linux user-systemd scope capped at the smaller of 60% of
total memory and currently available memory minus a 10%-of-total system
reserve, with MemorySwapMax=0, memory.oom.group=1, and TasksMax=24.
--verify-only performs the same read-back but does not run a workload.
--timeout-secs adds a bounded wall-clock deadline.
EOF
}

is_positive_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --label)
            [ "$#" -ge 2 ] || {
                echo "ERROR: --label requires a value" >&2
                exit 2
            }
            LABEL=$2
            shift 2
            ;;
        --verify-only)
            VERIFY_ONLY=1
            shift
            ;;
        --timeout-secs)
            [ "$#" -ge 2 ] || {
                echo "ERROR: --timeout-secs requires a value" >&2
                exit 2
            }
            TIMEOUT_SECS=$2
            shift 2
            ;;
        --)
            shift
            break
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "ERROR: unknown hard-scope option: $1" >&2
            usage
            exit 2
            ;;
    esac
done

if [ "$VERIFY_ONLY" = "0" ] && [ "$#" -eq 0 ]; then
    echo "ERROR: missing command for hard memory scope" >&2
    usage
    exit 2
fi
case "$TIMEOUT_SECS" in
    ''|*[!0-9]*)
        echo "ERROR: --timeout-secs must be zero or a positive integer" >&2
        exit 2
        ;;
esac
if [ "$VERIFY_ONLY" = "1" ]; then
    if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ] ||
        [ -z "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" ]; then

        echo "ERROR: verify-only requires an inherited transient scope; refusing to create one for the probe" >&2
        exit 126
    fi
fi
if [ "$(uname -s)" != "Linux" ]; then
    echo "ERROR: no equally hard non-Linux memory scope is implemented" >&2
    exit 126
fi
if [ ! -x "$MEMORY_GUARD" ]; then
    echo "ERROR: required memory guard is missing: $MEMORY_GUARD" >&2
    exit 126
fi
if [ ! -f "$ARTIFACT_ROOT_HELPER" ]; then
    echo "ERROR: required artifact-root helper is missing: $ARTIFACT_ROOT_HELPER" >&2
    exit 126
fi
if [ ! -f "$SERIAL_AUXILIARY_HELPER" ]; then
    echo "ERROR: required serial-auxiliary helper is missing: $SERIAL_AUXILIARY_HELPER" >&2
    exit 126
fi
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_HELPER"
# shellcheck source=scripts/serial_auxiliary_env.sh
source "$SERIAL_AUXILIARY_HELPER"
seen_artifact_root_init "$REPO_ROOT" || exit 126

case "${TMPDIR:-}" in
    "$SEEN_ARTIFACT_ROOT"/*)
        tmp_relative=${TMPDIR#"$REPO_ROOT"/}
        ;;
    *)
        echo "ERROR: hard-scope TMPDIR must stay below the validated project artifact root" >&2
        exit 126
        ;;
esac
seen_artifact_assert_safe_relative_path "$tmp_relative" || exit 126
seen_artifact_assert_no_symlink_components "$REPO_ROOT" "$tmp_relative" || exit 126
if [ ! -d "$TMPDIR" ] || [ -L "$TMPDIR" ] || [ ! -w "$TMPDIR" ]; then
    echo "ERROR: hard-scope TMPDIR is not a safe project-local writable directory" >&2
    exit 126
fi
canonical_tmpdir=$(seen_artifact_canonical_dir "$TMPDIR" || true)
if [ "$canonical_tmpdir" != "$TMPDIR" ]; then
    echo "ERROR: hard-scope TMPDIR is not canonical" >&2
    exit 126
fi
if [ "$VERIFY_ONLY" = "1" ]; then
    seen_serial_auxiliary_verify "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT" || exit 126
else
    seen_serial_auxiliary_prepare "$REPO_ROOT" "$SEEN_ARTIFACT_ROOT" || exit 126
fi

total_kb=""
available_kb=""
if [ -r /proc/meminfo ]; then
    total_kb=$(awk '/^MemTotal:/ { print $2; exit }' /proc/meminfo)
    available_kb=$(awk '/^MemAvailable:/ { print $2; exit }' /proc/meminfo)
fi
if ! is_positive_integer "$total_kb" || ! is_positive_integer "$available_kb"; then
    echo "ERROR: cannot derive the required aggregate memory cap" >&2
    exit 126
fi

total_ceiling_kb=$((total_kb * 60 / 100))
derived_rss_kb=$total_ceiling_kb
system_reserve_kb=$((total_kb * 10 / 100))
available_cap_kb=$((available_kb - system_reserve_kb))
if [ "$available_cap_kb" -lt 1 ]; then
    available_cap_kb=$((available_kb / 2))
fi
# Scope creation must account for current availability. A verify-only child is
# re-reading an already-installed, entry-derived cgroup limit; MemAvailable can
# legitimately fall while that scope is running. Keep the read-back bounded by
# the stable total-memory ceiling while the owning guard continues enforcing
# the original reserve and exact cgroup limits.
if [ "$VERIFY_ONLY" = "0" ] && [ "$derived_rss_kb" -gt "$available_cap_kb" ]; then
    derived_rss_kb=$available_cap_kb
fi
if [ "$derived_rss_kb" -lt 1 ]; then
    echo "ERROR: derived aggregate memory cap is not positive" >&2
    exit 126
fi

rss_kb=${SEEN_MEMORY_GUARD_RSS_KB:-$derived_rss_kb}
tasks_max=${SEEN_MEMORY_GUARD_TASKS_MAX:-24}
if ! is_positive_integer "$rss_kb" || [ "$rss_kb" -gt "$derived_rss_kb" ]; then
    echo "ERROR: aggregate memory cap must be positive and no larger than the current-memory-derived cap ($derived_rss_kb KiB)" >&2
    exit 126
fi
if ! is_positive_integer "$tasks_max" || [ "$tasks_max" -gt 24 ]; then
    echo "ERROR: aggregate task cap must be a positive value no larger than 24" >&2
    exit 126
fi

main_vmem_kb=${SEEN_MAIN_VMEM_KB:-$rss_kb}
if ! is_positive_integer "$main_vmem_kb" || [ "$main_vmem_kb" -gt "$rss_kb" ]; then
    main_vmem_kb=$rss_kb
fi
opt_vmem_kb=${SEEN_OPT_VMEM_KB:-$((main_vmem_kb / 2))}
if ! is_positive_integer "$opt_vmem_kb" || [ "$opt_vmem_kb" -gt 2097152 ]; then
    opt_vmem_kb=2097152
fi
if [ "$opt_vmem_kb" -gt "$main_vmem_kb" ]; then
    opt_vmem_kb=$main_vmem_kb
fi
memory_limit_bytes=${SEEN_MEMORY_LIMIT_BYTES:-$((main_vmem_kb * 1024))}
if ! is_positive_integer "$memory_limit_bytes" ||
    [ "$memory_limit_bytes" -gt "$((main_vmem_kb * 1024))" ]; then

    echo "ERROR: compiler allocation budget must be positive and no larger than the aggregate cap" >&2
    exit 126
fi
if [ "$VERIFY_ONLY" = "1" ]; then
    if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] ||
        [ "${SEEN_LOW_MEMORY:-0}" != "1" ] ||
        [ "${SEEN_JOBS:-0}" != "1" ] || [ "${SEEN_OPT_JOBS:-0}" != "1" ]; then

        echo "ERROR: verify-only requires inherited serial low-memory execution settings" >&2
        exit 126
    fi
    if ! is_positive_integer "${SEEN_MAIN_VMEM_KB:-}" ||
        ! is_positive_integer "${SEEN_OPT_VMEM_KB:-}" ||
        ! is_positive_integer "${SEEN_MEMORY_LIMIT_BYTES:-}" ||
        [ "$SEEN_MAIN_VMEM_KB" -gt "$rss_kb" ] ||
        [ "$SEEN_OPT_VMEM_KB" -gt 2097152 ] ||
        [ "$SEEN_OPT_VMEM_KB" -gt "$SEEN_MAIN_VMEM_KB" ] ||
        [ "$SEEN_MEMORY_LIMIT_BYTES" -gt "$((SEEN_MAIN_VMEM_KB * 1024))" ]; then

        echo "ERROR: verify-only inherited inconsistent compiler memory budgets" >&2
        exit 126
    fi
fi
cgroup_stop_kb=${SEEN_MEMORY_GUARD_CGROUP_STOP_KB:-$rss_kb}
if ! is_positive_integer "$cgroup_stop_kb" || [ "$cgroup_stop_kb" -gt "$rss_kb" ]; then
    echo "ERROR: cgroup stop threshold must be positive and no larger than the aggregate cap" >&2
    exit 126
fi
reserve_kb=${SEEN_MEMORY_GUARD_RESERVE_KB:-$((total_kb / 10))}
half_available_for_reserve=$((available_kb / 2))
if [ "$reserve_kb" -gt "$half_available_for_reserve" ]; then
    reserve_kb=$half_available_for_reserve
fi
if ! is_positive_integer "$reserve_kb"; then
    echo "ERROR: available-memory reserve must be a positive value" >&2
    exit 126
fi

scope_command=(true)
if [ "$VERIFY_ONLY" = "0" ]; then
    scope_command=(env
        SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
        SEEN_LOW_MEMORY=1
        SEEN_JOBS=1
        SEEN_OPT_JOBS=1
        SEEN_MAIN_VMEM_KB="$main_vmem_kb"
        SEEN_OPT_VMEM_KB="$opt_vmem_kb"
        SEEN_MEMORY_LIMIT_BYTES="$memory_limit_bytes"
        SEEN_MEMORY_GUARD_RSS_KB="$rss_kb"
        SEEN_MEMORY_GUARD_TASKS_MAX="$tasks_max"
        SEEN_MEMORY_GUARD_CGROUP_STOP_KB="$cgroup_stop_kb"
        SEEN_MEMORY_GUARD_RESERVE_KB="$reserve_kb"
        RIPGREP_CONFIG_PATH="$RIPGREP_CONFIG_PATH"
        RAYON_NUM_THREADS=1
        OMP_NUM_THREADS=1
        OPENBLAS_NUM_THREADS=1
        MKL_NUM_THREADS=1
        NUMEXPR_NUM_THREADS=1
        VECLIB_MAXIMUM_THREADS=1
        BLIS_NUM_THREADS=1
        GOMAXPROCS=1
        RUST_TEST_THREADS=1
        CARGO_BUILD_JOBS=1
        "$@")
fi

unset SEEN_MEMORY_GUARD_METRICS_FILE SEEN_MEMORY_GUARD_SUCCESS_METRICS_FILE
exec env \
    SEEN_MEMORY_GUARD_KERNEL_SCOPE=1 \
    SEEN_MEMORY_GUARD_REQUIRE_KERNEL_SCOPE=1 \
    SEEN_MEMORY_GUARD_SCOPE_OWNER=0 \
    SEEN_MEMORY_GUARD_RSS_KB="$rss_kb" \
    SEEN_MEMORY_GUARD_TASKS_MAX="$tasks_max" \
    SEEN_MEMORY_GUARD_CGROUP_STOP_KB="$cgroup_stop_kb" \
    SEEN_MEMORY_GUARD_RESERVE_KB="$reserve_kb" \
    "$MEMORY_GUARD" \
        --label "$LABEL" \
        --rss-limit-kb "$rss_kb" \
        --available-reserve-kb "$reserve_kb" \
        --vmem-limit-kb "$main_vmem_kb" \
        --tasks-max "$tasks_max" \
        --cgroup-stop-kb "$cgroup_stop_kb" \
        --timeout-secs "$TIMEOUT_SECS" \
        --kill-only \
        -- "${scope_command[@]}"
