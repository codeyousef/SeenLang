#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-repr-c-aggregate-array-push
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE_ROOT="$ROOT_DIR/tests/fixtures/fel-1585"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/aggregate-array-push.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/aggregate-array-push.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved aggregate Array.push artifacts: $WORK_DIR" >&2
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
    local fixture=$1
    local profile=$2
    local source_file="$FIXTURE_ROOT/$fixture.seen"
    local output="$WORK_DIR/$fixture-$profile"
    local ir_dir="$WORK_DIR/ir-$fixture-$profile"
    local log="$WORK_DIR/$fixture-$profile.log"
    local flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1
        --opt-jobs 1 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$source_file" >"$log" 2>&1
    run_compiler compile "$source_file" "$output" "${flags[@]}" \
        >>"$log" 2>&1
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1
}

for profile in fast release; do
    compile_case same_module "$profile"
    compile_case cross_module "$profile"
done

SAME_IR="$WORK_DIR/same-module.ll"
find "$WORK_DIR/ir-same_module-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$SAME_IR"
grep -Eq '%BufferView = type \{ i64, i64 \}' "$SAME_IR"
grep -Eq '= alloca %BufferView' "$SAME_IR"
grep -Eq 'store %BufferView %[^,]+, ptr %[0-9]+' "$SAME_IR"
grep -Eq 'call i64 @Array_push\(ptr [^,]+, ptr %[0-9]+\)' "$SAME_IR"
grep -Eq 'call ptr @llvm\.stacksave\(\)' "$SAME_IR"
grep -Eq 'call void @llvm\.stackrestore\(ptr %[0-9]+\)' "$SAME_IR"
grep -Eq 'call void @seen_arr_push_ptr\(ptr [^,]+, ptr %[0-9]+\)' "$SAME_IR"

CROSS_IR="$WORK_DIR/cross-module.ll"
find "$WORK_DIR/ir-cross_module-release" -maxdepth 1 -type f \
    -name '*.ll' -print0 | sort -z | xargs -0 cat >"$CROSS_IR"
grep -Eq '%BufferView = type \{ i64, i64 \}' "$CROSS_IR"
grep -Eq '%ViewRange = type \{ i32, i32 \}' "$CROSS_IR"
grep -Eq '%NestedView = type \{ %BufferView, %ViewRange, i32 \}' "$CROSS_IR"
grep -Eq '= alloca %BufferView' "$CROSS_IR"
grep -Eq 'store %BufferView %[^,]+, ptr %[0-9]+' "$CROSS_IR"
grep -Eq '= alloca %NestedView' "$CROSS_IR"
grep -Eq 'store %NestedView %[^,]+, ptr %[0-9]+' "$CROSS_IR"
grep -Eq 'call i64 @Array_push\(ptr [^,]+, ptr %[0-9]+\)' "$CROSS_IR"
if grep -Eq 'call void @seen_arr_push_ptr\(' "$CROSS_IR"; then
    echo "FAIL: repr(C) aggregate push used the pointer-array helper" >&2
    exit 1
fi

echo "PASS: FEL-1585 repr(C) aggregate Array.push contract"
