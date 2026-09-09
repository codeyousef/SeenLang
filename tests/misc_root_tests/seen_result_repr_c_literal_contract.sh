#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-result-repr-c-literal
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE="$ROOT_DIR/tests/fixtures/fel-1573/result_literal.seen"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/result-repr-c-literal.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/result-repr-c-literal.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved Result repr(C) artifacts: $WORK_DIR" >&2
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

for profile in fast release; do
    output="$WORK_DIR/result-literal-$profile"
    ir_dir="$WORK_DIR/ir-$profile"
    log="$WORK_DIR/result-literal-$profile.log"
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1
        --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$FIXTURE" >"$log" 2>&1
    run_compiler compile "$FIXTURE" "$output" "${flags[@]}" >>"$log" 2>&1
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1
done

RESULT_IR="$WORK_DIR/result-literal.ll"
find "$WORK_DIR/ir-release" -maxdepth 1 -type f -name '*.ll' -print0 |
    sort -z | xargs -0 cat >"$RESULT_IR"
grep -Eq 'load %BFloat16, ptr .*repr\(C\) literal call argument' "$RESULT_IR"
grep -Eq 'store %BFloat16 %[^,]+, ptr %' "$RESULT_IR"
grep -Eq '= ptrtoint ptr %[^ ]+ to i64' "$RESULT_IR"
grep -Eq '= (tail )?call i64 @Ok\(i64 %' "$RESULT_IR"
if grep -Eq '@Ok\(%BFloat16 ' "$RESULT_IR"; then
    echo "FAIL: Result constructor call bypassed the erased payload ABI" >&2
    exit 1
fi

echo "PASS: FEL-1573 repr(C) Result literal contract"
