#!/usr/bin/env bash
# Exercise actual LLVM driver failures without replacing bounded tools.
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED="$ROOT/scripts/run_capped_regression.sh"
COMPILER="${COMPILER:-$ROOT/compiler_seen/target/seen}"
SCOPE=seen-llvm-failure-probes
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/llvm-failure-probes.XXXXXX")"
SOURCE="$ROOT/tests/fixtures/test-002a/failure-probe/main.seen"
RUNNER="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
sha256sum "$(command -v opt)" "$(command -v llc)" >"$WORK/tools.before"

expect_rejection() {
    local label=$1
    shift
    local status=0
    timeout --foreground --kill-after=5s 60s bash "$RUNNER" "$@" \
        >"$WORK/$label.log" 2>&1 || status=$?
    [ "$status" = 126 ]
    grep -Fq 'RESOURCE STOP: attested Seen invocation:' "$WORK/$label.log"
    ! grep -Fq 'Build succeeded' "$WORK/$label.log"
}

for mode in fast release; do
    FLAGS=()
    [ "$mode" != fast ] || FLAGS=(--fast)
    # An ambient selector must not affect an ordinary compilation.
    SEEN_LLVM_FAILURE_PROBE_STAGE=opt timeout --foreground --kill-after=5s 300s \
        bash "$RUNNER" "$COMPILER" compile "$SOURCE" "$WORK/$mode" \
        "${FLAGS[@]}" --no-cache --no-fork >"$WORK/$mode.log" 2>&1
    timeout --foreground --kill-after=5s 30s "$WORK/$mode" >"$WORK/$mode.run" 2>&1
    grep -Fxq 'PASS: native compiler failure probe baseline' "$WORK/$mode.run"
    for stage in opt object; do
        status=0
        output="$WORK/$mode-$stage"
        timeout --foreground --kill-after=5s 300s bash "$RUNNER" \
            --llvm-failure-probe "$stage" "$COMPILER" compile "$SOURCE" \
            "$output" "${FLAGS[@]}" --no-cache --no-fork \
            >"$output.log" 2>&1 || status=$?
        [ "$status" = 1 ]
        [ ! -e "$output" ]
        grep -Fq "test.002a.injected.$stage:" "$output.log"
        if [ "$stage" = opt ]; then
            grep -Fq 'optimizer rejected unmodified IR for module 1:' "$output.log"
            grep -Fq 'Pass 2b status: opt:86' "$output.log"
        else
            grep -Fq 'object emission failed for unmodified IR module 1:' "$output.log"
        fi
        ! grep -Eq 'Build succeeded|RESOURCE STOP|\[3\] Linking' "$output.log"
        echo "PASS: $mode $stage failure propagates without executable"
    done
done
expect_rejection unknown --llvm-failure-probe bogus "$COMPILER"
expect_rejection source --llvm-failure-probe opt "$COMPILER" compile \
    "$ROOT/tests/fixtures/test-002a/failure-probe/helper.seen" "$WORK/wrong" --no-cache --no-fork
expect_rejection stale --llvm-failure-probe opt "$COMPILER" compile \
    "$SOURCE" "$WORK/fast" --no-cache --no-fork
expect_rejection arguments --llvm-failure-probe opt "$COMPILER" compile \
    "$SOURCE" "$WORK/wrong" --no-fork
expect_rejection output --llvm-failure-probe opt "$COMPILER" compile \
    "$SOURCE" "$SEEN_ARTIFACT_ROOT/outside-probe" --no-cache --no-fork
status=0
timeout --foreground --kill-after=5s 60s bash "$CAPPED" \
    seen-llvm-wrong-scope --compiler "$COMPILER" -- bash "$RUNNER" \
    --llvm-failure-probe opt "$COMPILER" compile "$SOURCE" \
    "$WORK/wrong-scope" --no-cache --no-fork >"$WORK/scope.log" 2>&1 || status=$?
[ "$status" = 126 ]
grep -Fq 'LLVM failure probes require the reserved scope' "$WORK/scope.log"
test ! -e "$WORK/wrong-scope"
sha256sum "$(command -v opt)" "$(command -v llc)" >"$WORK/tools.after"
cmp "$WORK/tools.before" "$WORK/tools.after"
echo 'PASS: real LLVM failure probes retain canonical tools and reject invalid requests'
