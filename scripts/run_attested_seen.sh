#!/usr/bin/env bash
# Invoke one explicitly selected Seen compiler under the verified serializer.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
CAPPED_ENTRY="$SCRIPT_DIR/run_capped_regression.sh"

fail() {
    echo "RESOURCE STOP: attested Seen invocation: $*" >&2
    exit 126
}

USAGE_ONLY_INVALID_WORKER=0
if [ "${1:-}" = "--usage-only-invalid-worker" ]; then
    USAGE_ONLY_INVALID_WORKER=1
    shift
fi
[ "$#" -ge 1 ] ||
    fail "usage: run_attested_seen.sh [--usage-only-invalid-worker] <compiler> [args...]"
COMPILER=$1
shift
ARGS=("$@")
SCOPE=${SEEN_CAPPED_REGRESSION_SCOPE:-}
[ -n "$SCOPE" ] || fail "capped-regression scope is missing"

if [ "$USAGE_ONLY_INVALID_WORKER" = "1" ]; then
    [ "${#ARGS[@]}" -eq 4 ] && [ "${ARGS[0]:-}" = "compile" ] ||
        fail "invalid-worker usage probe must be one compile option and value"
    case "${ARGS[1]:-}" in
        ''|-*) fail "invalid-worker usage probe requires a missing source operand" ;;
    esac
    [ ! -e "${ARGS[1]}" ] && [ ! -L "${ARGS[1]}" ] ||
        fail "invalid-worker usage probe source must not exist"
    case "${ARGS[2]:-}" in
        --jobs|--opt-jobs) ;;
        *) fail "invalid-worker usage probe accepts only a worker option" ;;
    esac
    case "${ARGS[3]:-}" in
        '') fail "invalid-worker usage probe requires a nonempty value" ;;
        *[!0-9]*) ;;
        *) fail "invalid-worker usage probe requires a nonnumeric value" ;;
    esac
fi

CAPABILITY=$(bash "$CAPPED_ENTRY" --classify-active "$SCOPE" \
    --compiler "$COMPILER") || fail "compiler containment verification failed"
SERIALIZER=${SEEN_FORK_SERIALIZER_SO:-}
[ -f "$SERIALIZER" ] && [ ! -L "$SERIALIZER" ] ||
    fail "attested serializer is unavailable"

has_jobs=0
has_opt_jobs=0
has_no_fork=0
has_help=0
index=0
while [ "$index" -lt "${#ARGS[@]}" ]; do
    argument=${ARGS[$index]}
    case "$argument" in
        --)
            break
            ;;
        -h|--help)
            has_help=1
            ;;
        --jobs)
            [ "$has_jobs" = "0" ] || fail "duplicate --jobs option"
            [ $((index + 1)) -lt "${#ARGS[@]}" ] || fail "--jobs requires a value"
            if [ "${ARGS[$((index + 1))]}" != "1" ]; then
                [ "$USAGE_ONLY_INVALID_WORKER" = "1" ] || fail "--jobs must be 1"
            fi
            has_jobs=1
            index=$((index + 2))
            continue
            ;;
        --jobs=*)
            [ "$has_jobs" = "0" ] || fail "duplicate --jobs option"
            [ "${argument#--jobs=}" = "1" ] || fail "--jobs must be 1"
            has_jobs=1
            ;;
        --opt-jobs)
            [ "$has_opt_jobs" = "0" ] || fail "duplicate --opt-jobs option"
            [ $((index + 1)) -lt "${#ARGS[@]}" ] || fail "--opt-jobs requires a value"
            if [ "${ARGS[$((index + 1))]}" != "1" ]; then
                [ "$USAGE_ONLY_INVALID_WORKER" = "1" ] || fail "--opt-jobs must be 1"
            fi
            has_opt_jobs=1
            index=$((index + 2))
            continue
            ;;
        --opt-jobs=*)
            [ "$has_opt_jobs" = "0" ] || fail "duplicate --opt-jobs option"
            [ "${argument#--opt-jobs=}" = "1" ] || fail "--opt-jobs must be 1"
            has_opt_jobs=1
            ;;
        --no-fork)
            [ "$has_no_fork" = "0" ] || fail "duplicate --no-fork option"
            has_no_fork=1
            ;;
    esac
    index=$((index + 1))
done

INSERT=()
skip_worker_insert=$has_help
if [ "${ARGS[0]:-}" = "compile" ]; then
    case "${ARGS[1]:-}" in
        ''|-*) ;;
        *)
            if [ ! -e "${ARGS[1]}" ] && [ ! -L "${ARGS[1]}" ]; then
                # A missing first source operand cannot reach code generation;
                # preserve exact argv for strict CLI usage diagnostics.
                skip_worker_insert=1
            fi
            ;;
    esac
fi
if [ "${ARGS[0]:-}" = "compile" ] && [ "$skip_worker_insert" = "0" ] &&
    [ "$USAGE_ONLY_INVALID_WORKER" = "0" ]; then
    if [ "$has_no_fork" = "1" ] &&
        { [ "$has_jobs" = "1" ] || [ "$has_opt_jobs" = "1" ]; }; then

        fail "--no-fork conflicts with explicit worker options"
    fi
    case "$CAPABILITY" in
        advertised-jobs)
            if [ "$has_no_fork" = "0" ]; then
                [ "$has_jobs" = "1" ] || INSERT+=(--jobs 1)
                [ "$has_opt_jobs" = "1" ] || INSERT+=(--opt-jobs 1)
            fi
            ;;
        advertised-no-fork)
            [ "$has_no_fork" = "1" ] || INSERT+=(--no-fork)
            ;;
        serializer-required) ;;
        *) fail "unknown compiler capability: $CAPABILITY" ;;
    esac
fi

if [ "${#INSERT[@]}" -gt 0 ]; then
    separator=${#ARGS[@]}
    index=0
    while [ "$index" -lt "${#ARGS[@]}" ]; do
        if [ "${ARGS[$index]}" = "--" ]; then
            separator=$index
            break
        fi
        index=$((index + 1))
    done
    ARGS=("${ARGS[@]:0:$separator}" "${INSERT[@]}" "${ARGS[@]:$separator}")
fi

if ! ulimit -S -v "$SEEN_MAIN_VMEM_KB" 2>/dev/null; then
    fail "could not apply the main virtual-memory cap"
fi
active_main=$(ulimit -S -v 2>/dev/null || true)
case "$active_main" in
    ''|*[!0-9]*) fail "could not read back the main virtual-memory cap" ;;
esac
[ "$active_main" -le "$SEEN_MAIN_VMEM_KB" ] ||
    fail "active main memory cap exceeds the requested cap"
case "${PATH:-}" in
    "$SEEN_BOUNDED_TOOLCHAIN_DIR"|"$SEEN_BOUNDED_TOOLCHAIN_DIR":*) ;;
    *) fail "bounded toolchain is not the first PATH entry" ;;
esac
EXEC_PATH=${SEEN_ATTESTED_SEEN_PATH:-$PATH}
case "$EXEC_PATH" in
    "$SEEN_BOUNDED_TOOLCHAIN_DIR"|"$SEEN_BOUNDED_TOOLCHAIN_DIR":*) ;;
    *) fail "compiler PATH must keep the bounded toolchain first" ;;
esac

exec env -u SEEN_FORK_SERIALIZER_ROOT_PID \
    LD_PRELOAD="$SERIALIZER" \
    SEEN_FORK_SERIALIZER_TARGET="$COMPILER" \
    SEEN_LOW_MEMORY=1 \
    SEEN_JOBS=1 \
    SEEN_OPT_JOBS=1 \
    SEEN_MAIN_VMEM_KB="$SEEN_MAIN_VMEM_KB" \
    SEEN_OPT_VMEM_KB="$SEEN_OPT_VMEM_KB" \
    SEEN_MEMORY_LIMIT_BYTES="$SEEN_MEMORY_LIMIT_BYTES" \
    PATH="$EXEC_PATH" \
    "$COMPILER" "${ARGS[@]}"
