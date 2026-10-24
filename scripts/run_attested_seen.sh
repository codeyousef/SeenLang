#!/usr/bin/env bash
# Invoke one explicitly selected Seen compiler under the verified serializer.

set -euo pipefail
unset SEEN_LLVM_FAILURE_PROBE_STAGE

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
CAPPED_ENTRY="$SCRIPT_DIR/run_capped_regression.sh"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"

fail() {
    echo "RESOURCE STOP: attested Seen invocation: $*" >&2
    exit 126
}

LLVM_FAILURE_PROBE_STAGE=""
if [ "${1:-}" = "--llvm-failure-probe" ]; then
    case "${2:-}" in opt|object) LLVM_FAILURE_PROBE_STAGE=$2 ;; *) fail 'invalid LLVM failure-probe stage' ;; esac
    shift 2
fi
DETERMINISTIC_ENVIRONMENT=0
if [ "${1:-}" = "--deterministic-environment" ]; then
    DETERMINISTIC_ENVIRONMENT=1
    shift
fi
USAGE_ONLY_INVALID_WORKER=0
if [ "${1:-}" = "--usage-only-invalid-worker" ]; then
    USAGE_ONLY_INVALID_WORKER=1
    shift
fi
CONSUMER_CONTEXT=""
if [ "${1:-}" = "--consumer-context" ]; then
    [ "$#" -ge 3 ] && [ -n "$2" ] || fail 'consumer context requires a project and compiler'
    CONSUMER_CONTEXT=$2
    shift 2
fi
[ "$#" -ge 1 ] ||
    fail "usage: run_attested_seen.sh [--consumer-context project] [--deterministic-environment] [--usage-only-invalid-worker] [--llvm-failure-probe opt|object] <compiler> [args...]"
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

PROBE_ENV=()
CONSUMER_ENV=()
CONSUMER_UNSET_ENV=()
if [ -n "$CONSUMER_CONTEXT" ]; then
    [ "$DETERMINISTIC_ENVIRONMENT" = 0 ] && [ "$USAGE_ONLY_INVALID_WORKER" = 0 ] &&
        [ -z "$LLVM_FAILURE_PROBE_STAGE" ] || fail 'consumer context cannot combine invocation modes'
    # First verify the original namespace, cgroup and exact compiler binding
    # above. Only the executed compiler receives these narrower cache inputs;
    # do not fabricate or relocate serializer/namespace evidence.
    case "$CONSUMER_CONTEXT" in
        "${SEEN_ARTIFACT_ROOT:?}"/*) ;;
        *) fail 'consumer project must stay inside the attested artifact root' ;;
    esac
    case "$CONSUMER_CONTEXT" in *$'\n'*|*$'\r'*) fail 'consumer project contains control characters' ;; esac
    [ -d "$CONSUMER_CONTEXT" ] && [ ! -L "$CONSUMER_CONTEXT" ] &&
        [ "$(realpath -e -- "$CONSUMER_CONTEXT")" = "$CONSUMER_CONTEXT" ] ||
        fail 'consumer project must be an existing canonical directory'
    case "${ARGS[0]:-}" in check|compile) ;; *) fail 'consumer context supports only check and compile' ;; esac
    source=${ARGS[1]:-}
    case "$source" in "$CONSUMER_CONTEXT"/*) ;; *) fail 'consumer source escapes its project' ;; esac
    case "$source" in *$'\n'*|*$'\r'*) fail 'consumer source contains control characters' ;; esac
    [ -f "$source" ] && [ ! -L "$source" ] &&
        [ "$(realpath -e -- "$source")" = "$source" ] || fail 'consumer source must be a canonical regular file'
    if [ "${ARGS[0]}" = compile ]; then
        output=${ARGS[2]:-}
        case "$output" in "$CONSUMER_CONTEXT"/*) ;; *) fail 'consumer output escapes its project' ;; esac
        case "$output" in *$'\n'*|*$'\r'*) fail 'consumer output contains control characters' ;; esac
        output_parent=${output%/*}
        [ -d "$output_parent" ] && [ ! -L "$output_parent" ] &&
            [ "$(realpath -e -- "$output_parent")" = "$output_parent" ] ||
            fail 'consumer output parent must be a canonical directory'
        case "${output##*/}" in ''|.|..) fail 'consumer output has an unsafe basename' ;; esac
        [ ! -L "$output" ] || fail 'consumer output must not be a symlink'
    fi
    CONSUMER_UNSET_ENV=(-u SEEN_COMPILER_SOURCE_ROOT -u SEEN_PACKAGE_CLIENT -u SEEN_DATA_PATH)
    CONSUMER_ENV=("SEEN_PROJECT_ROOT=$CONSUMER_CONTEXT"
        "SEEN_ARTIFACT_ROOT=$CONSUMER_CONTEXT/.seen/agent-tools")
fi
if [ -n "$LLVM_FAILURE_PROBE_STAGE" ]; then
    [ "$DETERMINISTIC_ENVIRONMENT" = 0 ] && [ "$USAGE_ONLY_INVALID_WORKER" = 0 ] ||
        fail 'LLVM failure probes cannot combine invocation modes'
    [ "$SCOPE" = seen-llvm-failure-probes ] ||
        fail 'LLVM failure probes require the reserved scope'
    [ "${ARGS[0]:-}" = compile ] &&
        [ "${ARGS[1]:-}" = "$REPO_ROOT/tests/fixtures/test-002a/failure-probe/main.seen" ] ||
        fail 'LLVM failure probes require the fixed source fixture'
    case "${#ARGS[@]}" in
        5)
            [ "${ARGS[3]}" = --no-cache ] && [ "${ARGS[4]}" = --no-fork ] ||
                fail 'LLVM failure probes require uncached serialized arguments'
            ;;
        6)
            [ "${ARGS[3]}" = --fast ] && [ "${ARGS[4]}" = --no-cache ] &&
                [ "${ARGS[5]}" = --no-fork ] ||
                fail 'LLVM failure probes require uncached serialized arguments'
            ;;
        *) fail 'unexpected LLVM failure-probe arguments' ;;
    esac
    output=${ARGS[2]}
    output_parent=$(cd -P -- "${output%/*}" 2>/dev/null && pwd -P) ||
        fail 'LLVM failure-probe output parent is unavailable'
    case "$output_parent" in
        "$SEEN_ARTIFACT_ROOT"/llvm-failure-probes.*) ;;
        *) fail 'LLVM failure-probe output escapes its private work root' ;;
    esac
    [ "$output_parent/${output##*/}" = "$output" ] ||
        fail 'LLVM failure-probe output is not canonical'
    [ ! -e "$output" ] && [ ! -L "$output" ] ||
        fail 'LLVM failure-probe output already exists'
    PROBE_ENV=("SEEN_LLVM_FAILURE_PROBE_STAGE=$LLVM_FAILURE_PROBE_STAGE")
fi

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

if [ "$DETERMINISTIC_ENVIRONMENT" = "1" ]; then
    [ "${SEEN_DETERMINISTIC:-}" = "1" ] ||
        fail "deterministic environment requires SEEN_DETERMINISTIC=1"
    [ -n "${SOURCE_DATE_EPOCH:-}" ] &&
        [ -n "${SEEN_DETERMINISTIC_SEED:-}" ] &&
        [ -n "${SEEN_HASH_SEED:-}" ] ||
        fail "deterministic environment inputs are incomplete"
    ARTIFACT_ROOT=${SEEN_ARTIFACT_ROOT:-}
    [ -n "$ARTIFACT_ROOT" ] && [ -d "$ARTIFACT_ROOT" ] &&
        [ ! -L "$ARTIFACT_ROOT" ] ||
        fail "deterministic environment requires a validated artifact root"
    case "$ARTIFACT_ROOT" in
        /*) ;;
        *) fail "deterministic artifact root must be absolute" ;;
    esac
    DETERMINISTIC_CACHE="$ARTIFACT_ROOT/deterministic-cache"
    mkdir -p -- "$DETERMINISTIC_CACHE" ||
        fail "could not create the deterministic package cache"
    [ -d "$DETERMINISTIC_CACHE" ] && [ ! -L "$DETERMINISTIC_CACHE" ] ||
        fail "deterministic package cache is unavailable or unsafe"
    OPTIONAL_ENV=()
    for variable in SEEN_COMPILER_SOURCE_ROOT SEEN_PACKAGE_CLIENT \
        SEEN_DATA_PATH TMPDIR SEEN_ARTIFACT_ROOT SEEN_PROJECT_ROOT \
        SEEN_PROJECT_ARTIFACT_WRAPPER \
        SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE \
        RIPGREP_CONFIG_PATH RAYON_NUM_THREADS OMP_NUM_THREADS \
        OPENBLAS_NUM_THREADS MKL_NUM_THREADS NUMEXPR_NUM_THREADS \
        VECLIB_MAXIMUM_THREADS BLIS_NUM_THREADS GOMAXPROCS \
        RUST_TEST_THREADS CARGO_BUILD_JOBS RPM_BUILD_NCPUS \
        SEEN_LLD_THREADS SEEN_THINLTO_JOBS; do

        if [ -n "${!variable:-}" ]; then
            OPTIONAL_ENV+=("$variable=${!variable}")
        fi
    done
    exec env -i \
        LD_PRELOAD="$SERIALIZER" \
        SEEN_FORK_SERIALIZER_TARGET="$COMPILER" \
        SEEN_LOW_MEMORY=1 SEEN_JOBS=1 SEEN_OPT_JOBS=1 \
        SEEN_MAIN_VMEM_KB="$SEEN_MAIN_VMEM_KB" \
        SEEN_OPT_VMEM_KB="$SEEN_OPT_VMEM_KB" \
        SEEN_MEMORY_LIMIT_BYTES="$SEEN_MEMORY_LIMIT_BYTES" \
        SEEN_BOUNDED_TOOLCHAIN_DIR="$SEEN_BOUNDED_TOOLCHAIN_DIR" \
        SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" \
        SEEN_DETERMINISTIC_SEED="$SEEN_DETERMINISTIC_SEED" \
        SEEN_HASH_SEED="$SEEN_HASH_SEED" \
        XDG_CACHE_HOME="$DETERMINISTIC_CACHE" \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC PATH="$EXEC_PATH" \
        "${OPTIONAL_ENV[@]}" "$COMPILER" "${ARGS[@]}"
fi

exec env -u SEEN_FORK_SERIALIZER_ROOT_PID -u SEEN_LLVM_FAILURE_PROBE_STAGE \
    "${CONSUMER_UNSET_ENV[@]}" \
    LD_PRELOAD="$SERIALIZER" \
    SEEN_FORK_SERIALIZER_TARGET="$COMPILER" \
    SEEN_LOW_MEMORY=1 \
    SEEN_JOBS=1 \
    SEEN_OPT_JOBS=1 \
    SEEN_MAIN_VMEM_KB="$SEEN_MAIN_VMEM_KB" \
    SEEN_OPT_VMEM_KB="$SEEN_OPT_VMEM_KB" \
    SEEN_MEMORY_LIMIT_BYTES="$SEEN_MEMORY_LIMIT_BYTES" \
    PATH="$EXEC_PATH" \
    "${PROBE_ENV[@]}" "${CONSUMER_ENV[@]}" "$COMPILER" "${ARGS[@]}"
