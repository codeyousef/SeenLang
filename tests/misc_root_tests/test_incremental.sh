#!/usr/bin/env bash
# Cold/warm/no-cache behavior in a disposable project, never the checkout cache.
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT/compiler_seen/target/seen}"
CAPPED="$ROOT/scripts/run_capped_regression.sh"
SCOPE=incremental-cli
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ] ||
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" != "$SCOPE" ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/incremental-cli.XXXXXX")"
cp -- "$ROOT/tests/fixtures/test-002a/incremental_cli.seen" "$WORK/main.seen"
cd -- "$WORK"

compile_and_run() {
    local mode=$1
    shift
    timeout --foreground --kill-after=5s 600s \
        bash "$SEEN_ATTESTED_COMPILER_RUNNER" "$COMPILER" compile \
        "$WORK/main.seen" "$WORK/$mode" --fast --no-fork "$@" \
        >"$WORK/$mode.compile.log" 2>&1
    timeout --foreground --kill-after=5s 30s "$WORK/$mode" \
        >"$WORK/$mode.run.log" 2>&1
    grep -Fxq 'Incremental CLI: 43' "$WORK/$mode.run.log"
}

compile_and_run cold
test -s .seen_cache/registry.hash
objects=(.seen_cache/*.o)
test -f "${objects[0]}"
if grep -Fq 'source cache hit' "$WORK/cold.compile.log"; then
    echo 'FAIL: cold project unexpectedly reused a source object' >&2
    exit 1
fi
compile_and_run warm
grep -Fq 'source cache hit' "$WORK/warm.compile.log"

# --no-cache must neither consume nor alter the already validated warm cache.
sha256sum .seen_cache/* >"$WORK/cache.before.sha256"
compile_and_run uncached --no-cache
if grep -Eq 'source cache hit|IR cache hit' "$WORK/uncached.compile.log"; then
    echo 'FAIL: --no-cache reused an object' >&2
    exit 1
fi
sha256sum .seen_cache/* >"$WORK/cache.after.sha256"
cmp -- "$WORK/cache.before.sha256" "$WORK/cache.after.sha256"
echo 'PASS: isolated incremental cold/warm/no-cache compilation and execution'
