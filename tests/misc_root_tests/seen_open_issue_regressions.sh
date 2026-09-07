#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-open-issue-regressions

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/open-issues.XXXXXX")"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        rm -rf -- "$WORK_DIR"
    else
        echo "Preserved open-issue regression artifacts: $WORK_DIR" >&2
    fi
    return "$status"
}
trap cleanup EXIT

run_seen() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }

INVALID="$ROOT_DIR/tests/fixtures/fel-1567/invalid_branch_move.seen"
if run_seen check "$INVALID" >"$WORK_DIR/invalid-check.log" 2>&1; then
    echo "FAIL: FEL-1567 use-after-move passed seen check" >&2
    exit 1
fi
grep -Fq E_OWNERSHIP_POSSIBLY_MOVED "$WORK_DIR/invalid-check.log"
if run_seen compile "$INVALID" "$WORK_DIR/invalid" --release --lto thin \
    --no-cache --no-fork --target-cpu x86-64 \
    >"$WORK_DIR/invalid-compile.log" 2>&1; then
    echo "FAIL: FEL-1567 use-after-move compiled" >&2
    exit 1
fi
grep -Fq E_OWNERSHIP_POSSIBLY_MOVED "$WORK_DIR/invalid-compile.log"
if grep -Eq '\[Pass 2|optimizer rejected|instruction expected to be numbered' \
    "$WORK_DIR/invalid-compile.log"; then
    echo "FAIL: FEL-1567 reached LLVM after ownership rejection" >&2
    exit 1
fi

for fixture in valid_borrow valid_early_return; do
    run_seen check "$ROOT_DIR/tests/fixtures/fel-1567/$fixture.seen" \
        >"$WORK_DIR/$fixture.log" 2>&1
    run_seen compile "$ROOT_DIR/tests/fixtures/fel-1567/$fixture.seen" \
        "$WORK_DIR/$fixture" --release --lto thin --no-cache --no-fork \
        --target-cpu x86-64 \
        >>"$WORK_DIR/$fixture.log" 2>&1
    timeout --foreground --kill-after=5s 60s "$WORK_DIR/$fixture" \
        >>"$WORK_DIR/$fixture.log" 2>&1
    grep -Fq 'PASS: FEL-1567' "$WORK_DIR/$fixture.log"
done

printf 'Z' > "$WORK_DIR/one-byte.bin"
run_seen compile "$ROOT_DIR/seen_std/tests/memory/uint8_pointer_deref.seen" \
    "$WORK_DIR/uint8-deref" --release --lto thin --no-cache --no-fork \
    --emit-module-ir-dir "$WORK_DIR/uint8-ir" --target-cpu x86-64 \
    >"$WORK_DIR/uint8.log" 2>&1
timeout --foreground --kill-after=5s 60s "$WORK_DIR/uint8-deref" \
    "$WORK_DIR/one-byte.bin" >>"$WORK_DIR/uint8.log" 2>&1
grep -Fq 'PASS: FEL-1559' "$WORK_DIR/uint8.log"
grep -R -Eq 'load i8, ptr .*; \*ptr dereference' "$WORK_DIR/uint8-ir"

run_seen compile "$ROOT_DIR/seen_std/tests/json/strict_error_cleanup.seen" \
    "$WORK_DIR/json-cleanup" --release --lto thin --no-cache --no-fork \
    --target-cpu x86-64 \
    >"$WORK_DIR/json-cleanup.log" 2>&1
timeout --foreground --kill-after=5s 120s \
    env SEEN_MEMORY_LIMIT_BYTES=16777216 "$WORK_DIR/json-cleanup" \
    >>"$WORK_DIR/json-cleanup.log" 2>&1
grep -Fq 'PASS: FEL-1561' "$WORK_DIR/json-cleanup.log"
grep -Fq 'destroyJsonParseResult(parsed, false)' \
    "$ROOT_DIR/seen_std/src/json/strict.seen"

echo "PASS: FEL-1559/FEL-1561/FEL-1567 release regressions"
