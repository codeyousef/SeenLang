#!/usr/bin/env bash
# Performance bisect is fail-closed until a current, scope-owned driver can
# build historical revisions without executing their orchestration scripts.

set -euo pipefail

if [ "$#" -ne 4 ]; then
    echo "Usage: $0 <benchmark_file> <threshold_ms> <good_commit> <bad_commit>" >&2
    exit 2
fi

BENCH_FILE=$1
THRESHOLD_MS=$2
GOOD_COMMIT=$3
BAD_COMMIT=$4
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
SELF_PATH="$SCRIPT_DIR/$(basename -- "${BASH_SOURCE[0]}")"
ARTIFACT_ROOT_HELPER="$SCRIPT_DIR/artifact_root.sh"
ARTIFACT_WRAPPER="$SCRIPT_DIR/run_with_project_artifacts.sh"
HARD_SCOPE_WRAPPER="$SCRIPT_DIR/run_in_hard_memory_scope.sh"
ORIGINAL_ARGS=("$@")

case "$BENCH_FILE" in
    ''|*[!A-Za-z0-9._-]*)
        echo "ERROR: benchmark name contains unsafe characters" >&2
        exit 2
        ;;
esac
if [[ ! "$THRESHOLD_MS" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    echo "ERROR: threshold must be a non-negative integer or decimal" >&2
    exit 2
fi
for revision in "$GOOD_COMMIT" "$BAD_COMMIT"; do
    case "$revision" in
        ''|*[!A-Za-z0-9._~^/-]*)
            echo "ERROR: revision contains unsafe characters: $revision" >&2
            exit 2
            ;;
    esac
done

is_positive_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

derive_main_kb() {
    local total=$1 available=$2 cap available_cap
    cap=$((total / 4))
    available_cap=$((available / 2))
    [ "$available_cap" -ge "$cap" ] || cap=$available_cap
    [ "$cap" -le 4194304 ] || cap=4194304
    [ "$cap" -gt 0 ] || cap=1
    printf '%s\n' "$cap"
}

derive_opt_kb() {
    local total=$1 main=$2 cap half_main
    cap=$((total / 10))
    half_main=$((main / 2))
    [ "$half_main" -ge "$cap" ] || cap=$half_main
    [ "$cap" -le 2097152 ] || cap=2097152
    [ "$cap" -gt 0 ] || cap=1
    printf '%s\n' "$cap"
}

[ -f "$ARTIFACT_ROOT_HELPER" ] || {
    echo "ERROR: missing artifact-root helper: $ARTIFACT_ROOT_HELPER" >&2
    exit 1
}
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_HELPER"

total_kb=$(awk '/^MemTotal:/ {print $2; exit}' /proc/meminfo 2>/dev/null || true)
available_kb=$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo 2>/dev/null || true)
if ! is_positive_integer "$total_kb" || ! is_positive_integer "$available_kb"; then
    echo "ERROR: could not derive performance-bisect memory caps" >&2
    exit 1
fi
main_kb=${SEEN_MAIN_VMEM_KB:-$(derive_main_kb "$total_kb" "$available_kb")}
opt_kb=${SEEN_OPT_VMEM_KB:-$(derive_opt_kb "$total_kb" "$main_kb")}
memory_limit_bytes=${SEEN_MEMORY_LIMIT_BYTES:-$((main_kb * 1024))}
if ! is_positive_integer "$main_kb" || [ "$main_kb" -gt 4194304 ] ||
    ! is_positive_integer "$opt_kb" || [ "$opt_kb" -gt 2097152 ] ||
    [ "$opt_kb" -gt "$main_kb" ] ||
    ! is_positive_integer "$memory_limit_bytes" ||
    [ "$memory_limit_bytes" -gt "$((main_kb * 1024))" ]; then

    echo "ERROR: inconsistent performance-bisect memory caps" >&2
    exit 1
fi
export SEEN_LOW_MEMORY=1 SEEN_JOBS=1 SEEN_OPT_JOBS=1
export SEEN_MAIN_VMEM_KB="$main_kb" SEEN_OPT_VMEM_KB="$opt_kb"
export SEEN_MEMORY_LIMIT_BYTES="$memory_limit_bytes"

seen_artifact_root_init "$ROOT_DIR" || exit 1
if [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" != "1" ] ||
    [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ]; then

    [ -x "$ARTIFACT_WRAPPER" ] || {
        echo "ERROR: missing project-artifact wrapper: $ARTIFACT_WRAPPER" >&2
        exit 1
    }
    exec "$ARTIFACT_WRAPPER" perf-bisect -- "$SELF_PATH" "${ORIGINAL_ARGS[@]}"
fi

seen_artifact_root_init "$ROOT_DIR" || exit 1
if [ "$(uname -s)" = "Linux" ]; then
    namespace_tmp_identity=$(stat -c '%d:%i' /tmp 2>/dev/null || true)
    artifact_root_identity=$(stat -c '%d:%i' "$SEEN_ARTIFACT_ROOT" 2>/dev/null || true)
    if [ -z "$namespace_tmp_identity" ] ||
        [ "$namespace_tmp_identity" != "$artifact_root_identity" ]; then

        echo "ERROR: performance-bisect artifact namespace validation failed" >&2
        exit 1
    fi
fi
cd -- "$ROOT_DIR"

if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then

    [ -x "$HARD_SCOPE_WRAPPER" ] || {
        echo "ERROR: missing hard-memory-scope wrapper: $HARD_SCOPE_WRAPPER" >&2
        exit 1
    }
    exec "$HARD_SCOPE_WRAPPER" --label "Seen performance bisect" -- \
        "$SELF_PATH" "${ORIGINAL_ARGS[@]}"
fi
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
export SEEN_HARD_MEMORY_SCOPE_ACTIVE
"$HARD_SCOPE_WRAPPER" --label "Seen performance bisect read-back" --verify-only --

if ! ulimit -S -v "$SEEN_MAIN_VMEM_KB" 2>/dev/null; then
    echo "RESOURCE STOP: could not apply performance-bisect main cap" >&2
    exit 126
fi
active_main_kb=$(ulimit -S -v 2>/dev/null || true)
if ! is_positive_integer "$active_main_kb" ||
    [ "$active_main_kb" -gt "$SEEN_MAIN_VMEM_KB" ]; then

    echo "RESOURCE STOP: performance-bisect main cap read-back failed" >&2
    exit 126
fi

echo "ERROR: historical performance builds are disabled until a current, scope-owned bisect driver can verify each checked-out compiler without executing historical build orchestration." >&2
echo "No revision was checked out and no compiler or benchmark was executed." >&2
exit 126
