#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-repr-c-aggregate-argument
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE_ROOT="$ROOT_DIR/tests/fixtures/fel-1572"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/repr-c-argument.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/repr-c-argument.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved repr(C) argument artifacts: $WORK_DIR" >&2
            fi
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT

run_compiler() {
    bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

compile_case() {
    local source_name=$1
    local profile=$2
    local stem=${source_name%.seen}
    local output="$WORK_DIR/$stem-$profile"
    local ir_dir="$WORK_DIR/ir-$stem-$profile"
    local log="$WORK_DIR/$stem-$profile.log"
    local flags=(--no-cache --frozen --target-cpu=x86-64
        --jobs 1 --opt-jobs 1 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$FIXTURE_ROOT/$source_name" >"$log" 2>&1
    run_compiler compile "$FIXTURE_ROOT/$source_name" "$output" \
        "${flags[@]}" >>"$log" 2>&1
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1
}

for profile in fast release; do
    compile_case aggregate_arguments.seen "$profile"
    compile_case qwen_bfloat16_argument.seen "$profile"
done

IR="$WORK_DIR/aggregate-arguments.ll"
find "$WORK_DIR/ir-aggregate_arguments-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$IR"
grep -Fq 'repr(C) literal call argument' "$IR"
grep -Eq 'load %LocalBits, ptr .* ; repr\(C\) literal call argument' "$IR"
grep -Eq 'load %CrossModuleBits, ptr .* ; repr\(C\) literal call argument' "$IR"
grep -Eq 'call i64 @readLocalBits\(%LocalBits ' "$IR"
grep -Eq 'call i64 @readCrossModuleBits\(%CrossModuleBits ' "$IR"
if grep -Eq 'call i64 @read(Local|CrossModule)Bits\(ptr ' "$IR"; then
    echo "FAIL: repr(C) aggregate argument used a pointer ABI" >&2
    exit 1
fi

QWEN_IR="$WORK_DIR/qwen.ll"
find "$WORK_DIR/ir-qwen_bfloat16_argument-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$QWEN_IR"
grep -Eq 'load %BFloat16, ptr .* ; repr\(C\) literal call argument' "$QWEN_IR"
grep -Eq 'call i64 @readBits\(%BFloat16 ' "$QWEN_IR"
if grep -Eq 'call i64 @readBits\(ptr ' "$QWEN_IR"; then
    echo "FAIL: Qwen BFloat16 argument used a pointer ABI" >&2
    exit 1
fi

echo "PASS: FEL-1572 repr(C) literal/local/Result call argument contract"
