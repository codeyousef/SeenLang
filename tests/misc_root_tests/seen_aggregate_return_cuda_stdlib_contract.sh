#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-aggregate-return-cuda-stdlib
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE_ROOT="$ROOT_DIR/tests/fixtures/fel-1570"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/aggregate-return-cuda.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/aggregate-return-cuda.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved aggregate/CUDA artifacts: $WORK_DIR" >&2
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
    local output="$WORK_DIR/${source_name%.seen}-$profile"
    local ir_dir="$WORK_DIR/ir-${source_name%.seen}-$profile"
    local log="$WORK_DIR/${source_name%.seen}-$profile.log"
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
    compile_case direct_aggregate.seen "$profile"
    compile_case cross_module_aggregate.seen "$profile"
    compile_case cuda_stdlib_surface.seen "$profile"
done

DIRECT_IR="$WORK_DIR/direct.ll"
find "$WORK_DIR/ir-direct_aggregate-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$DIRECT_IR"
grep -Eq 'define %BufferView @makeView\(' "$DIRECT_IR"
grep -Eq 'call %BufferView @makeView\(' "$DIRECT_IR"
grep -Eq 'load %BufferView, ptr ' "$DIRECT_IR"
grep -Eq 'ret %BufferView ' "$DIRECT_IR"
if grep -Eq 'call i64 @makeView\(' "$DIRECT_IR"; then
    echo "FAIL: same-module aggregate return used the erased i64 ABI" >&2
    exit 1
fi

CROSS_IR="$WORK_DIR/cross.ll"
find "$WORK_DIR/ir-cross_module_aggregate-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$CROSS_IR"
grep -Eq 'define %CrossModuleBufferView @makeCrossModuleView\(' "$CROSS_IR"
grep -Eq 'call %CrossModuleBufferView @makeCrossModuleView\(' "$CROSS_IR"
grep -Eq 'load %CrossModuleBufferView, ptr ' "$CROSS_IR"
grep -Eq 'ret %CrossModuleBufferView ' "$CROSS_IR"
if grep -Eq 'call i64 @makeCrossModuleView\(' "$CROSS_IR"; then
    echo "FAIL: cross-module aggregate return used the erased i64 ABI" >&2
    exit 1
fi

CUDA_IR="$WORK_DIR/cuda.ll"
find "$WORK_DIR/ir-cuda_stdlib_surface-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$CUDA_IR"
grep -Eq 'call %SeenString @seen_cstr_to_str\(ptr ' "$CUDA_IR"
if grep -Eq '(call|declare) %SeenString @String_fromCString\(' "$CUDA_IR"; then
    echo "FAIL: CUDA error conversion emitted an undefined static-method symbol" >&2
    exit 1
fi
if grep -Eq '(call|declare) % @String_fromCString\(' "$CUDA_IR"; then
    echo "FAIL: CUDA error conversion emitted an empty String ABI" >&2
    exit 1
fi

echo "PASS: FEL-1570/FEL-1576 aggregate return and packaged CUDA executable contract"
