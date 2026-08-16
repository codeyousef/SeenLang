#!/usr/bin/env bash
# E2E Multi-Language Test Runner for Seen Compiler
# Tests all keywords and stdlib across 6 languages (en, ar, es, ja, ru, zh)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd -P)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd -P)"
WRAPPER="$ROOT_DIR/scripts/run_with_project_artifacts.sh"
HARD_SCOPE_WRAPPER="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"
ARTIFACT_ROOT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
BUILDER_CAPABILITY="$ROOT_DIR/scripts/rebuild_builder_capability.sh"
BUILDER_APPLICABILITY="$ROOT_DIR/scripts/rebuild_builder_applicability.sh"
SERIALIZER_VERIFY="$ROOT_DIR/scripts/verify_fork_serializer.sh"
BOUNDED_TOOLCHAIN_PREPARE="$ROOT_DIR/scripts/prepare_bounded_toolchain.sh"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
BUILD_CMD="compile"
LANGS="en ar es ja ru zh"
PASS=0
FAIL=0
SKIP=0
ERRORS=""

is_positive_integer() {
    case "$1" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

if ! is_positive_integer "${SEEN_MAIN_VMEM_KB:-}"; then
    echo "ERROR: SEEN_MAIN_VMEM_KB must be an explicit positive cap" >&2
    exit 2
fi
if ! is_positive_integer "${SEEN_OPT_VMEM_KB:-}"; then
    echo "ERROR: SEEN_OPT_VMEM_KB must be an explicit positive cap" >&2
    exit 2
fi
if ! is_positive_integer "${SEEN_MEMORY_LIMIT_BYTES:-}"; then
    echo "ERROR: SEEN_MEMORY_LIMIT_BYTES must be an explicit positive cap" >&2
    exit 2
fi

case "$COMPILER" in
    /*) ;;
    *) COMPILER="$PWD/$COMPILER" ;;
esac
if [ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" != "1" ] ||
    [ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" != "1" ]; then
    [ -x "$WRAPPER" ] || {
        echo "ERROR: missing project artifact wrapper: $WRAPPER" >&2
        exit 2
    }
    exec "$WRAPPER" multilingual-e2e -- \
        env \
            COMPILER="$COMPILER" \
            SEEN_LOW_MEMORY=1 \
            SEEN_MAIN_VMEM_KB="$SEEN_MAIN_VMEM_KB" \
            SEEN_OPT_VMEM_KB="$SEEN_OPT_VMEM_KB" \
            SEEN_MEMORY_LIMIT_BYTES="$SEEN_MEMORY_LIMIT_BYTES" \
            "$0"
fi

if [ ! -f "$ARTIFACT_ROOT_HELPER" ]; then
    echo "ERROR: missing artifact-root helper: $ARTIFACT_ROOT_HELPER" >&2
    exit 2
fi
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_HELPER"
seen_artifact_root_init "$ROOT_DIR" || {
    echo "ERROR: multilingual E2E artifact root validation failed" >&2
    exit 2
}
if [ "$(uname -s)" = "Linux" ]; then
    namespace_tmp_identity="$(stat -c '%d:%i' /tmp 2>/dev/null || true)"
    artifact_root_identity="$(stat -c '%d:%i' "$SEEN_ARTIFACT_ROOT" \
        2>/dev/null || true)"
    if [ -z "$namespace_tmp_identity" ] ||
        [ "$namespace_tmp_identity" != "$artifact_root_identity" ]; then

        echo "ERROR: multilingual E2E artifact namespace validation failed" >&2
        exit 2
    fi
fi
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != "1" ] &&
    [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != "1" ]; then
    [ -x "$HARD_SCOPE_WRAPPER" ] || {
        echo "ERROR: missing hard-memory-scope wrapper: $HARD_SCOPE_WRAPPER" >&2
        exit 2
    }
    exec "$HARD_SCOPE_WRAPPER" --label "Seen multilingual E2E" -- "$0"
fi
SEEN_HARD_MEMORY_SCOPE_ACTIVE=1
export SEEN_HARD_MEMORY_SCOPE_ACTIVE
"$HARD_SCOPE_WRAPPER" --label "Seen multilingual E2E read-back" \
    --verify-only --
if ! ulimit -S -v "$SEEN_MAIN_VMEM_KB" 2>/dev/null; then
    echo "RESOURCE STOP: could not apply multilingual E2E main cap" >&2
    exit 126
fi
active_main_vmem=$(ulimit -S -v 2>/dev/null || true)
if ! is_positive_integer "$active_main_vmem" ||
    [ "$active_main_vmem" -gt "$SEEN_MAIN_VMEM_KB" ]; then

    echo "RESOURCE STOP: multilingual E2E main cap read-back failed" >&2
    exit 126
fi
RUN_ROOT="$(mktemp -d "$SEEN_ARTIFACT_ROOT/multilingual-e2e.XXXXXX")"
cleanup() {
    local status=$?
    case "$RUN_ROOT" in
        "$SEEN_ARTIFACT_ROOT"/multilingual-e2e.*)
            if [ -d "$RUN_ROOT" ] && [ ! -L "$RUN_ROOT" ]; then
                rm -rf -- "$RUN_ROOT"
            fi
            ;;
        *)
            echo "ERROR: refusing to clean unexpected E2E path: $RUN_ROOT" >&2
            status=1
            ;;
    esac
    return "$status"
}
trap cleanup EXIT
export SEEN_DATA_PATH="$ROOT_DIR/languages"

if [ ! -x "$COMPILER" ]; then
    echo "ERROR: Compiler not found at $COMPILER"
    exit 2
fi
FORK_SERIALIZER_SO=${SEEN_FORK_SERIALIZER_SO:-}
FORK_SERIALIZER_ATTESTATION=${SEEN_FORK_SERIALIZER_ATTESTATION:-}
if ! bash "$SERIALIZER_VERIFY" "$FORK_SERIALIZER_SO" \
    "$FORK_SERIALIZER_ATTESTATION" "$SEEN_ARTIFACT_ROOT" \
    "${SEEN_MEMORY_GUARD_SCOPE_UNIT:-}" >/dev/null; then

    echo "ERROR: multilingual E2E requires the scope-attested fork serializer produced by safe_rebuild" >&2
    exit 126
fi
if ! SEEN_MEMORY_GUARD_IN_SCOPE=1 bash "$BUILDER_APPLICABILITY" \
    "$COMPILER" "$FORK_SERIALIZER_SO" >/dev/null; then

    echo "ERROR: multilingual E2E compiler is not serializer-applicable" >&2
    exit 126
fi
BOUNDED_TOOLCHAIN_DIR=$(bash "$BOUNDED_TOOLCHAIN_PREPARE" "$SEEN_ARTIFACT_ROOT") ||
    exit 126
PATH="$BOUNDED_TOOLCHAIN_DIR:$PATH"
export PATH SEEN_BOUNDED_TOOLCHAIN_DIR="$BOUNDED_TOOLCHAIN_DIR"
compiler_capability_status=0
compiler_capability=$(env -u LD_PRELOAD -u SEEN_FORK_SERIALIZER_TARGET \
    -u SEEN_FORK_SERIALIZER_ROOT_PID \
    bash "$BUILDER_CAPABILITY" "$COMPILER" 2>/dev/null) ||
    compiler_capability_status=$?
if [ "$compiler_capability_status" -ne 0 ]; then
    echo "ERROR: multilingual E2E compiler schema probe failed with status $compiler_capability_status" >&2
    exit 126
fi
case "$compiler_capability" in
    advertised-jobs) COMPILER_WORKER_FLAGS=(--jobs 1 --opt-jobs 1) ;;
    advertised-no-fork) COMPILER_WORKER_FLAGS=(--no-fork) ;;
    serializer-required) COMPILER_WORKER_FLAGS=() ;;
    *) echo "ERROR: multilingual E2E compiler schema probe failed" >&2; exit 126 ;;
esac

abort_on_resource_failure() {
    local status=$1
    local log_file=$2
    local label=$3
    case "$status" in
        124|125|126|137|143)
            echo "RESOURCE STOP: $label stopped with status $status" >&2
            exit "$status"
            ;;
    esac
    if [ "$status" -ne 0 ] && grep -Eiq \
        '(^|[^[:alnum:]_])(resource stop:|out of memory|cannot allocate memory|could not allocate memory|memory allocation (failed|failure)|allocation failure|std::bad_alloc|bad_alloc|resource temporarily unavailable|cannot fork|can.t fork|fork: retry|fork (failed|failure)|pthread_create([^[:alnum:]_].*)?(failed|failure)|failed to create (a )?thread|can.t create (a )?thread|cannot create (a )?thread|thread creation (failed|failure))([^[:alnum:]_]|$)' \
        "$log_file" 2>/dev/null; then

        echo "RESOURCE STOP: $label reported a resource failure" >&2
        exit 126
    fi
}

echo "=== Seen E2E Multi-Language Test Suite ==="
echo "Compiler: $COMPILER"
echo "Languages: $LANGS"
echo ""

for lang in $LANGS; do
    echo "--- Language: $lang ---"
    for test_file in "$SCRIPT_DIR/$lang"/*.seen; do
        [ -f "$test_file" ] || continue
        name=$(basename "$test_file" .seen)
        binary="$RUN_ROOT/seen_e2e_${lang}_${name}"
        compile_log="$RUN_ROOT/seen_e2e_${lang}_${name}.compile.log"
        run_log="$RUN_ROOT/seen_e2e_${lang}_${name}.run.log"
        compile_status=0

        # Check if compile-only (header comment contains "COMPILE-ONLY")
        if head -3 "$test_file" | grep -q "COMPILE-ONLY"; then
            timeout 120 env -u SEEN_FORK_SERIALIZER_ROOT_PID \
                LD_PRELOAD="$FORK_SERIALIZER_SO" \
                SEEN_FORK_SERIALIZER_TARGET="$COMPILER" \
                "$COMPILER" "$BUILD_CMD" "$test_file" "$binary" \
                --fast --no-cache "${COMPILER_WORKER_FLAGS[@]}" \
                --language "$lang" >"$compile_log" 2>&1 || compile_status=$?
            abort_on_resource_failure "$compile_status" "$compile_log" \
                "multilingual compile $lang/$name"
            if [ "$compile_status" -eq 0 ]; then
                echo "  PASS [compile] $name"
                PASS=$((PASS+1))
            else
                echo "  FAIL [compile] $name"
                FAIL=$((FAIL+1))
                ERRORS="$ERRORS\n  FAIL [compile] $lang/$name"
            fi
        else
            # Runtime test: compile then run
            timeout 120 env -u SEEN_FORK_SERIALIZER_ROOT_PID \
                LD_PRELOAD="$FORK_SERIALIZER_SO" \
                SEEN_FORK_SERIALIZER_TARGET="$COMPILER" \
                "$COMPILER" "$BUILD_CMD" "$test_file" "$binary" \
                --fast --no-cache "${COMPILER_WORKER_FLAGS[@]}" \
                --language "$lang" >"$compile_log" 2>&1 || compile_status=$?
            abort_on_resource_failure "$compile_status" "$compile_log" \
                "multilingual compile $lang/$name"
            if [ "$compile_status" -eq 0 ]; then
                run_status=0
                timeout 30 "$binary" >"$run_log" 2>&1 || run_status=$?
                abort_on_resource_failure "$run_status" "$run_log" \
                    "multilingual runtime $lang/$name"
                if [ "$run_status" -eq 0 ]; then
                    echo "  PASS [runtime] $name"
                    PASS=$((PASS+1))
                else
                    echo "  FAIL [runtime] $name"
                    FAIL=$((FAIL+1))
                    ERRORS="$ERRORS\n  FAIL [runtime] $lang/$name"
                fi
            else
                echo "  FAIL [compile] $name"
                FAIL=$((FAIL+1))
                ERRORS="$ERRORS\n  FAIL [compile] $lang/$name"
            fi
        fi
    done
    echo ""
done

echo "==========================================="
echo "TOTAL: $PASS passed, $FAIL failed, $SKIP skipped"
echo "==========================================="

if [ $FAIL -gt 0 ]; then
    echo ""
    echo "Failed tests:"
    echo -e "$ERRORS"
    exit 1
fi

exit 0
