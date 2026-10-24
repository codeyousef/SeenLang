#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT/compiler_seen/target/seen}"
SCOPE=test-instrumentation-soak
CAPPED="$ROOT/scripts/run_capped_regression.sh"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/test-002a-soak.XXXXXX")"
SOURCE="$ROOT/tests/fixtures/soak/test_002a_instrumentation.seen"

for profile in fast release undefined address coverage; do
    args=(--fast)
    case "$profile" in
        release) args=(--release --lto=thin --target-cpu=x86-64) ;;
        undefined) args+=(--sanitize undefined) ;;
        address) args+=(--sanitize address) ;;
        coverage) args+=(--coverage --sanitize undefined) ;;
    esac
    binary="$WORK/$profile"
    compile_log="$WORK/$profile.compile.log"
    run_log="$WORK/$profile.run.log"
    if ! timeout --foreground --kill-after=5s 600s \
        bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" compile \
        "$SOURCE" "$binary" --no-cache --no-fork "${args[@]}" \
        >"$compile_log" 2>&1; then
        tail -c 32768 "$compile_log" >&2
        exit 1
    fi
    status=0
    if [ "$profile" = address ]; then
        timeout --foreground --kill-after=5s 120s \
            bash "$ROOT/scripts/run_asan_in_hard_memory_scope.sh" \
            --target-root "$WORK" --compile-log "$compile_log" -- "$binary" \
            >"$run_log" 2>&1 || status=$?
    else
        timeout --foreground --kill-after=5s 120s \
            env UBSAN_OPTIONS=halt_on_error=1:abort_on_error=1 \
            LLVM_PROFILE_FILE="$WORK/coverage.profraw" "$binary" \
            >"$run_log" 2>&1 || status=$?
    fi
    if [ "$status" -ne 0 ]; then
        tail -c 32768 "$run_log" >&2
        exit "$status"
    fi
    grep -Fxq 'PASS: TEST-002A_soak' "$run_log"
    if [ "$profile" = coverage ]; then
        llvm-profdata merge -sparse --num-threads=1 "$WORK/coverage.profraw" \
            -o "$WORK/coverage.profdata"
        llvm-cov export --num-threads=1 "$binary" \
            -instr-profile "$WORK/coverage.profdata" >"$WORK/coverage.json"
        python3 "$ROOT/scripts/check_source_coverage_thresholds.py" \
            "$WORK/coverage.json" --source compiler_seen/src/testing/instrumentation.seen \
            >"$WORK/coverage-summary.json"
        cat "$WORK/coverage-summary.json"
    fi
    echo "PASS: TEST-002A $profile exact live baseline and bounded warm reserve"
done
