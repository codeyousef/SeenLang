#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-metal-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-metal-b.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-metal-b
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" numeric "$ROOT_DIR/tests/gpu_scalar_words/metal_numeric_contract.seen"
    rg -Fq 'PASS: emitted Metal scalar cases share fixed IEEE packed goldens and restore allocation baseline' "$TMP_DIR/numeric-$profile.log"
    compile_case "$profile" specialization "$ROOT_DIR/tests/gpu_artifacts/specialization_contract.seen" metal
    rg -Fq 'PASS: Metal bounded specialization overrides preserve source and independent clone ownership' "$TMP_DIR/specialization-$profile.log"
    compile_case "$profile" synchronization "$ROOT_DIR/tests/metal_msl_metallib_lowering/synchronization_contract.seen"
    rg -Fq 'PASS: Metal per-target atomic ordering fences barriers typed rejections and repeated ownership' "$TMP_DIR/synchronization-$profile.log"
    compile_fixture "$profile" text "$ROOT_DIR/tests/gpu_artifacts/text_layout_contract.seen"
    for pair in packed:spir_v_vulkan_lowering/packed_happy atomic:metal_msl_metallib_lowering/atomic_relaxed edges:metal_msl_metallib_lowering/atomic_edges ordered:metal_msl_metallib_lowering/atomic_ordered fences:metal_msl_metallib_lowering/synchronization_happy; do
        name=${pair%%:*}; source="$ROOT_DIR/tests/${pair#*:}.seen"; arguments=()
        if [ "$name" = ordered ] || [ "$name" = fences ]; then arguments=(metal3.2); fi
        cp -- "$TMP_DIR/text-$profile" "$TMP_DIR/$name-$profile"
        cp -- "$TMP_DIR/text-$profile.log" "$TMP_DIR/$name-$profile.log"
        run_fixture "$profile" "$name" metal "$source" "${arguments[@]}"
        rg -Fq 'PASS: Metal target coverage deterministic artifacts reflection and repeated cleanup' "$TMP_DIR/$name-$profile.log"
    done
done
python3 "$ROOT_DIR/scripts/check_gpu_metal_artifacts.py" "$TMP_DIR" packed atomic edges ordered fences --msl32-only ordered fences > "$TMP_DIR/source-evidence.json"
python3 "$ROOT_DIR/scripts/check_gpu_metal_numerics.py" "$TMP_DIR/numeric-fast.log"
for target in macos13 macos14 macos15; do
    python3 "$ROOT_DIR/scripts/check_gpu_metal_atomics.py" "$TMP_DIR/atomic-fast-$target.metal"
    python3 "$ROOT_DIR/scripts/check_gpu_metal_atomics.py" "$TMP_DIR/edges-fast-$target.metal" --edges
done
python3 "$ROOT_DIR/scripts/check_gpu_metal_atomics.py" "$TMP_DIR/ordered-fast-macos15.metal" --ordered
python3 "$ROOT_DIR/scripts/check_gpu_metal_atomics.py" "$TMP_DIR/fences-fast-macos15.metal" --fences
python3 "$ROOT_DIR/tests/gpu_artifacts/metal_metadata_contract.py" --atomic "$TMP_DIR/atomic-fast-macos13.metal"
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
compile_case fast fuzz "$ROOT_DIR/tests/gpu_scalar_words/packed_fuzz.seen" metal "$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"
rg -Fq 'PASS: packed capability fuzz 60s seed=1089 preserved typed rejection and artifact ownership' "$TMP_DIR/fuzz-fast.log"
compile_case fast atomic-fuzz "$ROOT_DIR/tests/typed_gpu_compiler/ir_metal_atomic_fuzz.seen"
rg -Fq 'PASS: Metal atomic fuzz 60s seed=1098 rejects malformed semantics and unsupported targets without leaks' "$TMP_DIR/atomic-fuzz-fast.log"
echo 'PASS: Metal packed precision native atomics scoped synchronization specialization and ownership; SDK and GPU execution unverified'
