#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-amdgpu-b-numeric
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-amdgpu-b-numeric.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-amdgpu-b-numeric
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_toolchain.py" > "$TMP_DIR/toolchain.json"
for profile in fast release undefined address; do
    compile_case "$profile" numeric "$ROOT_DIR/tests/gpu_scalar_words/amdgpu_numeric_contract.seen"
    rg -Fq 'PASS: emitted AMDGPU scalar cases share fixed IEEE packed goldens and restore allocation baseline' "$TMP_DIR/numeric-$profile.log"
    compile_case "$profile" specialization "$ROOT_DIR/tests/gpu_artifacts/specialization_contract.seen" hip
    rg -Fq 'PASS: AMDGPU bounded specialization overrides preserve source and independent clone ownership' "$TMP_DIR/specialization-$profile.log"
done
python3 "$ROOT_DIR/scripts/check_gpu_amdgpu_numerics.py" "$TMP_DIR/numeric-fast.log"
python3 - "$ROOT_DIR/scripts" "$TMP_DIR" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from check_gpu_amdgpu_numerics import cases_from_log
root = Path(sys.argv[2]); baseline = cases_from_log(root / 'numeric-fast.log')
for profile in ['release', 'undefined', 'address']:
    assert cases_from_log(root / ('numeric-' + profile + '.log')) == baseline, 'numeric LLVM differs across profiles'
print('PASS: AMDGPU numeric LLVM and fixed corpus identity across four compiler profiles')
PY
compile_case fast fuzz "$ROOT_DIR/tests/gpu_scalar_words/packed_fuzz.seen" hip "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
rg -Fq 'PASS: packed capability fuzz 60s seed=1089 preserved typed rejection and artifact ownership' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: AMDGPU numeric goldens specialization packed fuzz and ownership; device execution unverified'
