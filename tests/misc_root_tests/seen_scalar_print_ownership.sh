#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-scalar-print-ownership
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR=$(mktemp -d "$SEEN_ARTIFACT_ROOT/scalar-print-ownership.XXXXXX")
FIXTURE_ARTIFACT_PREFIX=scalar-print-ownership
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_fixture "$profile" scalar-print "$ROOT_DIR/tests/fixtures/scalar_print_ownership.seen"
    run_fixture "$profile" scalar-print
    python3 - "$TMP_DIR/scalar-print-$profile.log" <<'PY'
import pathlib,sys
lines=pathlib.Path(sys.argv[1]).read_text().splitlines()
expected=[]
for i in range(8):
    expected.extend([f'{41+i} -17',f'1.25 {"true" if i%2==0 else "false"}','é retained caller-owned output'])
    expected.extend(['-128 -32768 -2147483648 -9223372036854775808',
                     '255 65535 4294967295 18446744073709551615','1.25 1.25'])
start=lines.index(expected[0])
assert lines[start:start+len(expected)]==expected, 'scalar cleanup changed observable formatting'
PY
done
echo 'PASS: scalar print formatting and temporary ownership under all four execution profiles'
