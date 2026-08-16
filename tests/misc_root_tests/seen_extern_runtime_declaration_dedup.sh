#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen-dev}"
FIXTURE="$ROOT_DIR/tests/codegen/test_extern_runtime_declaration_dedup.seen"
MEMORY_FIXTURE="$ROOT_DIR/tests/codegen/test_extern_memory_runtime_declaration_dedup.seen"
ABI_FIXTURE="$ROOT_DIR/compiler_seen/tests/extern_declaration_dedup.seen"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-extern-runtime-declaration-dedup
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
ARTIFACT_PARENT="$SEEN_ARTIFACT_ROOT"
TEST_ROOT="$(mktemp -d "$ARTIFACT_PARENT/seen-extern-runtime-dedup.XXXXXX")"
COMPILER_LOG_DIR="$TEST_ROOT/compiler-logs"
mkdir -p -- "$COMPILER_LOG_DIR"
COMPILER_LOG_INDEX=0

cleanup() {
    local status=$?
    trap - EXIT
    if [ "$status" -ne 0 ]; then
        echo "extern declaration regression artifacts retained: $TEST_ROOT" >&2
        exit "$status"
    fi
    case "$TEST_ROOT" in
        "$ARTIFACT_PARENT"/seen-extern-runtime-dedup.*)
            [ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] &&
                [ "$(dirname -- "$TEST_ROOT")" = "$ARTIFACT_PARENT" ] || {
                    echo "refusing to remove unexpected test path: $TEST_ROOT" >&2
                    exit 1
                }
            if ! rm -rf -- "$TEST_ROOT"; then
                exit 1
            fi
            ;;
        *) echo "refusing to remove unexpected test path: $TEST_ROOT" >&2; exit 1 ;;
    esac
    exit 0
}
trap cleanup EXIT

run_compiler() {
    local log="$COMPILER_LOG_DIR/compile-$COMPILER_LOG_INDEX.log"
    local status=0
    COMPILER_LOG_INDEX=$((COMPILER_LOG_INDEX + 1))
    if env SEEN_DATA_PATH="$ROOT_DIR/languages" \
        bash "$ATTESTED_SEEN" "$COMPILER" "$@" >"$log" 2>&1; then
        return 0
    else
        status=$?
    fi
    echo "compiler command failed (status $status); log: $log" >&2
    tail -c 32768 -- "$log" >&2 || true
    return "$status"
}

[ -x "$COMPILER" ] || {
    echo "fresh compiler is not executable: $COMPILER" >&2
    exit 1
}
command -v llvm-as >/dev/null 2>&1 || {
    echo "llvm-as is required for the declaration-dedup regression" >&2
    exit 1
}

run_compiler compile "$FIXTURE" "$TEST_ROOT/program-ir-only" \
    --no-cache \
    --emit-module-ir-dir "$TEST_ROOT/ir" --stop-after-ir >/dev/null

IR_FILE="$(find "$TEST_ROOT/ir" -maxdepth 1 -type f -name '*.ll' -print -quit)"
[ -n "$IR_FILE" ] || {
    echo "compiler did not emit the focused LLVM IR fixture" >&2
    exit 1
}

DECLARATION_COUNT="$(grep -Ec '^declare .*@seen_arr_free\(' "$IR_FILE")"
[ "$DECLARATION_COUNT" -eq 1 ] || {
    echo "expected one seen_arr_free declaration, found $DECLARATION_COUNT" >&2
    exit 1
}
llvm-as "$IR_FILE" -o "$TEST_ROOT/program.bc"

run_compiler compile "$FIXTURE" "$TEST_ROOT/program" \
    --no-cache >/dev/null
"$TEST_ROOT/program"

run_compiler compile "$MEMORY_FIXTURE" "$TEST_ROOT/memory-ir-only" \
    --no-cache \
    --emit-module-ir-dir "$TEST_ROOT/memory-ir" --stop-after-ir >/dev/null

MEMORY_CALL_IR=""
while IFS= read -r memory_ir; do
    llvm-as "$memory_ir" -o "$TEST_ROOT/$(basename "$memory_ir").bc"
    memory_decl_count="$(grep -Ec \
        '^declare i64 @seen_stack_region_new\(i64\)' "$memory_ir")"
    if [ "$memory_decl_count" -gt 1 ]; then
        echo "duplicate seen_stack_region_new declaration in $memory_ir" >&2
        exit 1
    fi
    if grep -Eq 'call i64 @seen_stack_region_new\(i64 ' "$memory_ir"; then
        MEMORY_CALL_IR="$memory_ir"
        [ "$memory_decl_count" -eq 1 ] || {
            echo "stack-region call has no retained runtime declaration" >&2
            exit 1
        }
    fi
done < <(find "$TEST_ROOT/memory-ir" -maxdepth 1 -type f -name '*.ll' -print)
[ -n "$MEMORY_CALL_IR" ] || {
    echo "memory fixture did not emit a seen_stack_region_new call" >&2
    exit 1
}

run_compiler compile "$MEMORY_FIXTURE" "$TEST_ROOT/memory-program" \
    --no-cache >/dev/null
"$TEST_ROOT/memory-program"

run_compiler compile "$ABI_FIXTURE" "$TEST_ROOT/abi-program" \
    --no-cache >/dev/null
"$TEST_ROOT/abi-program"

echo "extern runtime declaration deduplication tests passed"
