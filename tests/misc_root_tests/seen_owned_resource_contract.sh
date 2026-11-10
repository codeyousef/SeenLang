#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_owned_resources.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_owned_resources.py"
FIXTURES="$ROOT_DIR/tests/fixtures/p0-own-001"
COMPILER="${SEEN_OWNERSHIP_COMPILER:-}"
fail(){ echo "FAIL: P0-OWN-001 contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" "$BENCHMARK" \
    "$ROOT_DIR/tests/runner/test_owned_resources_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_owned_resources_unit.py" >/dev/null || fail "unit matrix"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" |
    grep -Fq '"active":0' || fail "P0-OWN-001_happy"
if python3 "$CHECK" --validate "$FIXTURES/invalid/contract.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/p0-own-001-invalid.err"; then
    fail "P0-OWN-001_invalid"
fi
grep -Fq p0.own.001.use-after-move \
    "$ROOT_DIR/.seen/p0-own-001-invalid.err" || fail "invalid diagnostic"
if python3 "$CHECK" --validate "$FIXTURES/limit/contract.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/p0-own-001-limit.err"; then
    fail "P0-OWN-001_limit"
fi
grep -Fq p0.own.001.limit "$ROOT_DIR/.seen/p0-own-001-limit.err" ||
    fail "limit diagnostic"
status=0
python3 "$CHECK" --validate "$FIXTURES/cancel/contract.json" \
    --test-cancel-after-read >/dev/null \
    2>"$ROOT_DIR/.seen/p0-own-001-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "P0-OWN-001_cancel"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" \
    --fuzz-seconds "${SEEN_P0_OWN_001_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/p0-own-001-fuzz.err" || fail "fuzz"
python3 "$BENCHMARK" "$FIXTURES/happy/contract.json" \
    "$FIXTURES/happy/benchmark.json" >/dev/null || fail "5/30/5 benchmark"

if [ -n "$COMPILER" ]; then
    [ -x "$COMPILER" ] || fail "compiler is not executable: $COMPILER"
    for fixture in use_after_move conditional_move c_resource_use_after_move; do
        log="$ROOT_DIR/.seen/p0-own-001-$fixture.log"
        if "$COMPILER" check "$FIXTURES/invalid/$fixture.seen" >"$log" 2>&1; then
            fail "$fixture was accepted"
        fi
    done
    grep -Fq E_OWNERSHIP_USE_AFTER_MOVE \
        "$ROOT_DIR/.seen/p0-own-001-use_after_move.log" ||
        fail "deterministic use-after-move diagnostic"
    grep -Fq E_OWNERSHIP_POSSIBLY_MOVED \
        "$ROOT_DIR/.seen/p0-own-001-conditional_move.log" ||
        fail "control-flow ownership diagnostic"
    grep -Fq E_OWNERSHIP_USE_AFTER_MOVE \
        "$ROOT_DIR/.seen/p0-own-001-c_resource_use_after_move.log" ||
        fail "foreign-resource ownership diagnostic"
fi
echo "PASS: move-only owned-resource semantics"
