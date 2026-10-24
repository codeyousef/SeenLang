#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-select-guard-safety
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/select-guard-safety.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=select-guard-safety
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    for name in effects memory; do
        compile_case "$profile" "$name" "$ROOT_DIR/tests/codegen/test_select_guarded_$name.seen"
        rg -Fq 'PASS:' "$TMP_DIR/$name-$profile.log"
    done
done
python3 - "$TMP_DIR" <<'PY'
from pathlib import Path
import re
import sys
root = Path(sys.argv[1])
for name in ("effects", "memory"):
    source = "\n".join(p.read_text() for p in (root / f"{name}-fast-ir").glob("*.ll"))
    def body(function):
        match = re.search(r"^define[^\n]*@" + function + r"\([^\n]*\{\n(.*?)^}", source, re.M | re.S)
        assert match, f"missing function {function}"
        return match.group(1)
    for function in ("single", "paired"):
        assert "branchless" not in body(function), f"unsafe speculation in {name}/{function}"
    if name == "effects":
        assert "branchless max/min" in body("scalarMax"), "safe scalar selection was lost"
PY
echo 'PASS: branch guards retain memory safety and effect ordering across build profiles'
