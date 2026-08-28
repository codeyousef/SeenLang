#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
FIXTURES="$ROOT/tests/fixtures/core-004g"
CHECKER="$ROOT/scripts/check_program_reproducibility.py"
CERTIFIER="$ROOT/scripts/certify_two_builder_programs.py"
UNIT="$ROOT/tests/runner/test_program_reproducibility_unit.py"
mkdir -p "$ROOT/.seen"
WORK="$(mktemp -d "$ROOT/.seen/core-004g.XXXXXX")"

cleanup() {
  rm -rf -- "$WORK"
}
trap cleanup EXIT INT TERM

fail() {
  echo "FAIL: CORE-004G contract: $*" >&2
  exit 1
}

python3 -m py_compile "$CHECKER" "$CERTIFIER" "$UNIT" || fail syntax
python3 -m json.tool "$ROOT/schemas/program-reproducibility.schema.json" \
  >/dev/null || fail schema
python3 "$UNIT" >/dev/null || fail unit

python3 "$CHECKER" --evidence "$FIXTURES/happy/evidence.json" \
  >"$WORK/canonical.json" || fail CORE-004G_two_builder_happy
cmp -s "$FIXTURES/happy/evidence.json" "$WORK/canonical.json" || fail canonical

if python3 "$CHECKER" --evidence "$FIXTURES/invalid/evidence.json" \
  >"$WORK/invalid.out" 2>"$WORK/invalid.err"; then
  fail invalid
fi
grep -Fq core.004g.invalid "$WORK/invalid.err" || fail invalid-code

if python3 "$CHECKER" --evidence "$FIXTURES/limit/evidence.json" \
  >"$WORK/limit.out" 2>"$WORK/limit.err"; then
  fail limit
fi
grep -Fq core.004g.limit "$WORK/limit.err" || fail limit-code

status=0
python3 "$CHECKER" --evidence "$FIXTURES/happy/evidence.json" \
  --test-cancel-after-read >"$WORK/cancel.out" 2>"$WORK/cancel.err" || status=$?
[ "$status" -eq 130 ] || fail CORE-004G_cancel
grep -Fq core.004g.cancelled "$WORK/cancel.err" || fail cancel-code

python3 "$CHECKER" --evidence "$FIXTURES/happy/evidence.json" \
  --fuzz-seconds "${SEEN_CORE_004G_FUZZ_SECONDS:-1}" --seed 1101 \
  >"$WORK/fuzz.out" 2>"$WORK/fuzz.err" || fail fuzz
grep -Fq seed=1101 "$WORK/fuzz.err" || fail fuzz-seed

[ -f "$ROOT/compiler_seen/src/release/program_reproducibility.seen" ] || \
  fail native-policy
[ -f "$ROOT/compiler_seen/tests/reproducibility/core_004g_programs.seen" ] || \
  fail native-test
[ -z "$(find "$ROOT/.seen" -maxdepth 1 -name '.program-certification-*' -print -quit)" ] || \
  fail CORE-004G_atomic_evidence
[ -z "$(jobs -pr)" ] || fail cleanup

echo "PASS: CORE-004G two-builder program certification contract"
