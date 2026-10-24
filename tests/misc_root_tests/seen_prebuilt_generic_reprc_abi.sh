#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-prebuilt-generic-reprc-abi
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/prebuilt-generic-reprc.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=prebuilt-generic-reprc
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
cp -R "$ROOT_DIR/tests/fixtures/reprc-generic-abi/prebuilt/producer" "$TMP_DIR/producer"
cp -R "$ROOT_DIR/tests/fixtures/reprc-generic-abi/prebuilt/consumer" "$TMP_DIR/consumer"
run_compiler pkg prebuild "$TMP_DIR/producer" "$TMP_DIR/artifact" > "$TMP_DIR/producer.log" 2>&1
python3 - "$TMP_DIR/artifact" <<'PY'
import hashlib
from pathlib import Path
import sys
import tomllib
root = Path(sys.argv[1])
metadata = tomllib.loads((root / 'Seen.pkg.toml').read_text())
assert metadata['layout_abi'] == 'seen-layout-abi-v3'
raw = (root / metadata['object_manifest']).read_bytes()
assert hashlib.sha256(raw).hexdigest() == metadata['object_manifest_sha256']
rows = raw.decode().splitlines()
assert rows[2] == 'layout_abi\tseen-layout-abi-v3'
for row in rows[4:]:
    object_path, source_path, digest = row.split('\t')
    assert not Path(object_path).is_absolute() and '..' not in Path(object_path).parts
    assert not Path(source_path).is_absolute() and '..' not in Path(source_path).parts
    assert hashlib.sha256((root / object_path).read_bytes()).hexdigest() == digest
PY
run_compiler pkg fetch "$TMP_DIR/consumer/Seen.toml" --quiet > "$TMP_DIR/fetch.log" 2>&1
[ -f "$TMP_DIR/consumer/Seen.lock" ]
[ ! -L "$TMP_DIR/consumer/Seen.lock" ]
run_compiler check "$TMP_DIR/consumer/src/main.seen" > "$TMP_DIR/check.log" 2>&1
for profile in fast release undefined address; do
    compile_case "$profile" consumer "$TMP_DIR/consumer/src/main.seen"
    rg -Fq 'PASS: prebuilt generic record method ABI and cleanup' "$TMP_DIR/consumer-$profile.log"
done
echo 'PASS: current prebuilt aggregate method ABI and hash-bearing manifests'
