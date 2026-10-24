#!/usr/bin/env bash
# Sanitizer CLI regression through the verified native-test path.
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT/compiler_seen/target/seen}"
CAPPED="$ROOT/scripts/run_capped_regression.sh"
SCOPE=sanitizer-cli
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/sanitizer-cli.XXXXXX")"
SOURCE="$ROOT/tests/fixtures/test-002a/sanitizer_cli.seen"

for sanitizer in address undefined; do
    binary="$WORK/$sanitizer"
    compile_log="$WORK/$sanitizer.compile.log"
    run_log="$WORK/$sanitizer.run.log"
    if ! timeout --foreground --kill-after=5s 600s \
        bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" compile \
        "$SOURCE" "$binary" --fast --no-cache --no-fork \
        "--sanitize=$sanitizer" >"$compile_log" 2>&1; then
        tail -c 32768 "$compile_log" >&2
        exit 1
    fi
    grep -Fq "  Sanitizer: $sanitizer" "$compile_log"
    status=0
    if [ "$sanitizer" = address ]; then
        timeout --foreground --kill-after=5s 120s \
            bash "$ROOT/scripts/run_asan_in_hard_memory_scope.sh" \
            --target-root "$WORK" --compile-log "$compile_log" -- "$binary" \
            >"$run_log" 2>&1 || status=$?
    else
        timeout --foreground --kill-after=5s 120s \
            env UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
            "$binary" >"$run_log" 2>&1 || status=$?
    fi
    if [ "$status" -ne 0 ]; then
        tail -c 32768 "$run_log" >&2
        exit "$status"
    fi
    grep -Fxq 'Sanitizer test: 43' "$run_log"
    echo "PASS: sanitizer CLI $sanitizer compilation and execution"
done
echo 'PASS: sanitizer CLI flags with leak detection and verified containment'
