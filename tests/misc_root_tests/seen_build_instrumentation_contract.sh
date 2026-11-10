#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT_DIR/tests/fixtures/core-rel-002"
CHECKER="$ROOT_DIR/scripts/check_build_instrumentation.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_build_instrumentation.py"

fail() { echo "FAIL: build instrumentation contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECKER" "$BENCHMARK" \
    "$ROOT_DIR/tests/misc_root_tests/seen_build_instrumentation_unit.py" ||
    fail "Python syntax"
python3 "$ROOT_DIR/tests/misc_root_tests/seen_build_instrumentation_unit.py" \
    >/dev/null || fail "unit matrix"

happy=$(python3 "$CHECKER" --evidence "$FIXTURES/happy/evidence.json") ||
    fail "CORE-REL-002_happy"
[ "$happy" = "$(tr -d '\n' <"$FIXTURES/happy/evidence.json")" ] ||
    fail "happy evidence is not canonical"

if python3 "$CHECKER" --evidence "$FIXTURES/invalid/evidence.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/core-rel-002-invalid.err"; then
    fail "CORE-REL-002_invalid was accepted"
fi
grep -Fq core.rel.002.invalid "$ROOT_DIR/.seen/core-rel-002-invalid.err" ||
    fail "invalid code"
if python3 "$CHECKER" --evidence "$FIXTURES/limit/evidence.json" \
    --max-bytes 32 >/dev/null 2>"$ROOT_DIR/.seen/core-rel-002-limit.err"; then
    fail "CORE-REL-002_limit was accepted"
fi
grep -Fq core.rel.002.limit "$ROOT_DIR/.seen/core-rel-002-limit.err" ||
    fail "limit code"
cancel_status=0
python3 "$CHECKER" --evidence "$FIXTURES/cancel/evidence.json" \
    --test-cancel-after-read >/dev/null 2>"$ROOT_DIR/.seen/core-rel-002-cancel.err" ||
    cancel_status=$?
[ "$cancel_status" -eq 130 ] || fail "CORE-REL-002_cancel status"

python3 "$CHECKER" --evidence "$FIXTURES/happy/evidence.json" \
    --fuzz-seconds "${SEEN_CORE_REL_002_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/core-rel-002-fuzz.err" || fail "seed-1101 fuzz"
python3 "$BENCHMARK" "$FIXTURES/happy/evidence.json" \
    "$FIXTURES/happy/benchmark.json" >/dev/null || fail "5/30/5 benchmark"

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" \
    "$ROOT_DIR/docs/architecture/native-boundaries.json" >/dev/null || fail "ledger"
echo "PASS: debug coverage and sanitizer build contract"
