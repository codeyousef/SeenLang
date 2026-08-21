#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
C="$ROOT/scripts/check_test_reporters.py"; H="$ROOT/tests/fixtures/test-001e/happy"; N="$ROOT/compiler_seen/src/testing/reporters.seen"
fail(){ echo "FAIL: test reporters: $*" >&2; exit 1; }
python3 -m py_compile "$C" "$ROOT/tests/runner/test_reporters_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_reporters_unit.py" >/dev/null || fail units
for pair in 'human report.txt' 'json report.json' 'junit report.xml'; do set -- $pair; python3 "$C" --format "$1" --validate "$H/$2" --fuzz-seconds "${SEEN_TEST_001E_FUZZ_SECONDS:-1}" --seed 1101 --benchmark-limit-ms 10 >/dev/null 2>"$ROOT/.seen/test-001e-$1.err" || fail "$1"; grep -Fq seed=1101 "$ROOT/.seen/test-001e-$1.err" || fail seed; done
for s in TestReporterFormat TestReporterRequest TestReporterOutput renderTestRunHuman renderTestRunJson renderTestRunJunit renderTestReportChecked renderTestReport; do grep -Fq "$s" "$N" || fail "$s"; done
for c in test.001e.invalid test.001e.limit test.001e.cancelled; do grep -Fq "$c" "$N" || fail "$c"; done
for n in happy invalid limit cancel cleanup; do grep -Fq "TEST-001E_$n" "$ROOT/compiler_seen/tests/test_001e_reporters.seen" || fail "$n"; done
grep -Fq seen-test-report-v1 "$ROOT/docs/testing.md" || fail docs
grep -Fq seen-test-report-v1 "$ROOT/CHANGELOG.md" || fail changelog
grep -Fq seen-test-report-v1 "$ROOT/schemas/compatibility-manifest.schema.json" || fail compatibility
[ -z "$(jobs -pr)" ] || fail cleanup
echo "PASS: human JSON and JUnit reporter contract"
