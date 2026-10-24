#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-generic-reprc-abi
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/generic-reprc-abi.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=generic-reprc-abi
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for name in fields methods plain; do
    source="$ROOT_DIR/tests/fixtures/reprc-generic-abi/$name.seen"
    run_compiler check "$source" > "$TMP_DIR/$name-check.log" 2>&1
    for profile in fast release undefined address; do
        compile_case "$profile" "$name" "$source"
        rg -Fq 'PASS:' "$TMP_DIR/$name-$profile.log"
    done
done
python3 - "$TMP_DIR" <<'PY'
from pathlib import Path
import re
import sys
root = Path(sys.argv[1])
for profile in ('fast', 'release', 'undefined', 'address'):
    modules = [p.read_text() for p in (root / f'methods-{profile}-ir').glob('*.ll')]
    definitions = '\n'.join(modules)
    for method, aggregate in [('GenericSpanProbe_lengthOf', 'Span'),
                              ('GenericSpanProbe_mutableLengthOf', 'MutableSpan')]:
        assert re.search(r'define[^\n]*@' + method + r'\(ptr[^,]*, %' + aggregate + r' ', definitions), method
        assert re.search(r'call[^\n]*@' + method + r'\(ptr[^,]*, %' + aggregate + r' ', definitions), method
        assert re.search(r'store %' + aggregate + r' %value.arg, ptr ', definitions), method
        assert not re.search(r'load %' + aggregate + r', ptr %value.arg', definitions), method
    fields = '\n'.join(p.read_text() for p in (root / f'fields-{profile}-ir').glob('*.ll'))
    assert 'extractvalue %Span ' in fields
    assert 'extractvalue %MutableSpan ' in fields
print('PASS: concrete aggregate method call/definition and local field IR contracts')
PY
echo 'PASS: generic and plain repr(C) value storage and method ABI'
