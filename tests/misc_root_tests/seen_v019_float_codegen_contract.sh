#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-v019-float-codegen
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE_ROOT="$ROOT_DIR/tests/fixtures/fel-1544-1549"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/v019-float.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/v019-float.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved float-codegen artifacts: $WORK_DIR" >&2
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

compile_and_run() {
    local profile=$1
    local output="$WORK_DIR/float-$profile"
    local ir_dir="$WORK_DIR/ir-$profile"
    local log="$WORK_DIR/$profile.log"
    local flags=(--no-cache --frozen --target-cpu=x86-64
        --jobs 1 --opt-jobs 1 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$FIXTURE_ROOT/main.seen" >"$log" 2>&1
    run_compiler compile "$FIXTURE_ROOT/main.seen" "$output" \
        "${flags[@]}" >>"$log" 2>&1
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1
    for issue in 1544 1545 1546 1549; do
        grep -Fq "PASS: FEL-$issue" "$log"
    done
}

compile_and_run fast
compile_and_run release

IR_TEXT="$WORK_DIR/all.ll"
find "$WORK_DIR/ir-release" -maxdepth 1 -type f -name '*.ll' -print0 |
    sort -z | xargs -0 cat >"$IR_TEXT"
grep -Eq 'fpext float .* to double' "$IR_TEXT"
grep -Eq 'fmul float ' "$IR_TEXT"
grep -Eq 'uitofp i64 .* to double' "$IR_TEXT"
[ "$(grep -Ec 'fptosi double .* to i64' "$IR_TEXT")" -ge 4 ]
if grep -Eq 'icmp [a-z]+ i64 %[0-9]+.*defined with type.*double|%SeenString 0' \
    "$IR_TEXT"; then
    echo "FAIL: float/cast regression emitted a forbidden LLVM shape" >&2
    exit 1
fi

echo "PASS: FEL-1544/1545/1546/1549 float codegen contract"
