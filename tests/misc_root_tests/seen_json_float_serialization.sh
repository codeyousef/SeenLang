#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-json-float-serialization
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/json-float-serialization.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=json-float-serialization
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
ulimit -c 0
for profile in fast release undefined address; do
    compile_fixture "$profile" json-float "$ROOT_DIR/tests/fixtures/json_float_serialization.seen"
    run_fixture "$profile" json-float all
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/json-float-$profile-all.log"
    for mode in reject-builder reject-value reject-pretty; do
        for kind in nan inf -inf; do
            status=0
            run_fixture "$profile" json-float "$mode" "$kind" || status=$?
            cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/json-float-$profile-$mode-$kind.log"
            [ "$status" = 134 ] || { echo "FAIL: nonfinite JSON input did not hit its precondition ($profile $mode $kind status=$status)" >&2; exit 1; }
            python3 - "$TMP_DIR/00-active.log" <<'PY'
import pathlib,sys
text=pathlib.Path(sys.argv[1]).read_text()
for marker in ('ERROR: AddressSanitizer:', 'ERROR: LeakSanitizer:', 'runtime error:', 'SUMMARY: UndefinedBehaviorSanitizer:'):
    assert marker not in text, 'nonfinite rejection hit a sanitizer defect instead of the finite-number precondition'
PY
        done
    done
done
echo 'PASS: exact finite JSON numeric roundtrips ownership and explicit nonfinite rejection under four profiles'
