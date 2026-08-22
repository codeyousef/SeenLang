#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CHECK="$ROOT_DIR/scripts/check_secret_markers.py"
BENCHMARK="$ROOT_DIR/scripts/benchmark_secret_markers.py"
FIXTURES="$ROOT_DIR/tests/fixtures/p0-secret-001"
NATIVE="$ROOT_DIR/seen_std/src/core/secret.seen"
NATIVE_TEST="$ROOT_DIR/seen_std/tests/error/p0_secret_001_secret_markers.seen"
fail(){ echo "FAIL: P0-SECRET-001 contract: $*" >&2; exit 1; }

python3 -m py_compile "$CHECK" "$BENCHMARK" \
    "$ROOT_DIR/tests/runner/test_secret_markers_unit.py" || fail "Python syntax"
python3 "$ROOT_DIR/tests/runner/test_secret_markers_unit.py" >/dev/null ||
    fail "unit matrix"
rendered="$ROOT_DIR/.seen/p0-secret-001-rendered.json"
python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" >"$rendered" ||
    fail "P0-SECRET-001_happy"
grep -Fq '"value":"[redacted]"' "$rendered" || fail "secret redaction"
grep -Fq '"value":"west"' "$rendered" || fail "public value"
if grep -Fq 'never-print-this' "$rendered"; then fail "plaintext leaked"; fi

if python3 "$CHECK" --validate "$FIXTURES/invalid/contract.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/p0-secret-001-invalid.err"; then
    fail "P0-SECRET-001_invalid"
fi
grep -Fq p0.secret.001.invalid "$ROOT_DIR/.seen/p0-secret-001-invalid.err" ||
    fail "invalid diagnostic"
if grep -Fq 'different' "$ROOT_DIR/.seen/p0-secret-001-invalid.err"; then
    fail "invalid diagnostic leaked material"
fi

if python3 "$CHECK" --validate "$FIXTURES/limit/contract.json" \
    >/dev/null 2>"$ROOT_DIR/.seen/p0-secret-001-limit.err"; then
    fail "P0-SECRET-001_limit"
fi
grep -Fq p0.secret.001.limit "$ROOT_DIR/.seen/p0-secret-001-limit.err" ||
    fail "limit diagnostic"

status=0
python3 "$CHECK" --validate "$FIXTURES/cancel/contract.json" \
    --test-cancel-after-read >/dev/null \
    2>"$ROOT_DIR/.seen/p0-secret-001-cancel.err" || status=$?
[ "$status" -eq 130 ] || fail "P0-SECRET-001_cancel"

python3 "$CHECK" --validate "$FIXTURES/happy/contract.json" \
    --fuzz-seconds "${SEEN_P0_SECRET_001_FUZZ_SECONDS:-1}" --seed 1101 \
    >/dev/null 2>"$ROOT_DIR/.seen/p0-secret-001-fuzz.err" || fail "fuzz"
grep -Fq seed=1101 "$ROOT_DIR/.seen/p0-secret-001-fuzz.err" || fail "fuzz seed"
python3 "$BENCHMARK" "$FIXTURES/happy/contract.json" \
    "$FIXTURES/happy/benchmark.json" |
    grep -Fq 'warmups=5 samples=30 status=pass' || fail "5/30/5 benchmark"

for symbol in SecretString SecretBytes SecretRevealPolicy secretString \
    secretBytes SEEN_SECRET_MAX_BYTES; do
    grep -Fq "$symbol" "$NATIVE" || fail "native symbol $symbol"
done
for invariant in '@move' 'return SEEN_SECRET_REDACTED' \
    'this.material[index] = 0' 'SecretRevealPolicy.Allow'; do
    grep -Fq "$invariant" "$NATIVE" || fail "native invariant $invariant"
done
grep -Fq 'pipeTypeListContainsImpl(moveOnlyClasses, argType)' \
    "$ROOT_DIR/compiler_seen/src/codegen/ir_variable_gen.seen" ||
    fail "exact move-only type membership"
for name in P0-SECRET-001_happy P0-SECRET-001_invalid \
    P0-SECRET-001_limit P0-SECRET-001_cancel P0-SECRET-001_cleanup; do
    grep -Fq "$name" "$NATIVE_TEST" || fail "native case $name"
done
grep -Fq 'seen_std/tests/error/p0_secret_001_secret_markers.seen' \
    "$ROOT_DIR/scripts/seen_stage1_acceptance.sh" || fail "Stage1 wiring"
grep -Fq seen-secret-marker-v1 \
    "$ROOT_DIR/schemas/compatibility-manifest.schema.json" ||
    fail "compatibility binding"
grep -Fq 'Redaction-safe secret values' "$ROOT_DIR/docs/errors.md" ||
    fail "documentation"
grep -Fq seen-secret-marker-v1 "$ROOT_DIR/CHANGELOG.md" || fail "changelog"
[ -z "$(jobs -pr)" ] || fail "P0-SECRET-001_cleanup"
echo "PASS: redaction-safe secret value markers"
