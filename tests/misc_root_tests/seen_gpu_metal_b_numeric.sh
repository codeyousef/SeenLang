#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-metal-b-numeric
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-metal-b-numeric.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-metal-b-numeric
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" numeric "$ROOT_DIR/tests/gpu_scalar_words/metal_numeric_contract.seen"
    rg -Fq 'PASS: emitted Metal scalar cases share fixed IEEE packed goldens and restore allocation baseline' "$TMP_DIR/numeric-$profile.log"
    compile_case "$profile" specialization "$ROOT_DIR/tests/gpu_artifacts/specialization_contract.seen" metal
    rg -Fq 'PASS: Metal bounded specialization overrides preserve source and independent clone ownership' "$TMP_DIR/specialization-$profile.log"
done
python3 "$ROOT_DIR/scripts/check_gpu_metal_numerics.py" "$TMP_DIR/numeric-fast.log"
python3 - "$ROOT_DIR/scripts" "$TMP_DIR" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, sys.argv[1])
from gpu_numeric_cases import read_numeric_cases
root = Path(sys.argv[2]); baseline = read_numeric_cases(root / 'numeric-fast.log', 'seen-metal-numeric-case-v1')
for profile in ('release', 'undefined', 'address'):
    assert read_numeric_cases(root / f'numeric-{profile}.log', 'seen-metal-numeric-case-v1') == baseline, 'Metal numeric source differs across profiles'
print('PASS: Metal numeric source and shared fixed corpus identity across four compiler profiles')
PY
echo 'PASS: Metal precision specialization and ownership; SDK and GPU execution unverified'
