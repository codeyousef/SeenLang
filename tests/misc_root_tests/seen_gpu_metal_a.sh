#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-metal-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-metal-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-metal-a
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_fixture "$profile" text "$ROOT_DIR/tests/gpu_artifacts/text_layout_contract.seen"
    for pair in layout:cuda_ptx_lowering/layout_happy scalar:hip_amdgpu_lowering/scalar_differential edges:hip_amdgpu_lowering/layout_edges splat:gpu_artifacts/vector_splat semantic:metal_msl_metallib_lowering/semantic_edges; do
        name=${pair%%:*}; source="$ROOT_DIR/tests/${pair#*:}.seen"
        cp -- "$TMP_DIR/text-$profile" "$TMP_DIR/$name-$profile"
        cp -- "$TMP_DIR/text-$profile.log" "$TMP_DIR/$name-$profile.log"
        cp -- "$TMP_DIR/text-$profile.compile.log" "$TMP_DIR/$name-$profile.compile.log"
        cp -- "$TMP_DIR/text-$profile.compile.status" "$TMP_DIR/$name-$profile.compile.status"
        run_fixture "$profile" "$name" metal "$source"
        rg -Fq 'PASS: Metal target coverage deterministic artifacts reflection and repeated cleanup' "$TMP_DIR/$name-$profile.log"
    done
    compile_case "$profile" admission "$ROOT_DIR/tests/metal_msl_metallib_lowering/admission_contract.seen"
    rg -Fq 'PASS: Metal native buffer slots padded threadgroup limits selected-entry isolation default target and bounded ownership' "$TMP_DIR/admission-$profile.log"
done
python3 "$ROOT_DIR/scripts/check_gpu_metal_artifacts.py" "$TMP_DIR" layout scalar edges splat semantic > "$TMP_DIR/source-evidence.json"
python3 "$ROOT_DIR/tests/gpu_artifacts/metal_metadata_contract.py" "$TMP_DIR"
python3 "$ROOT_DIR/scripts/check_gpu_metal_scalar.py" "$TMP_DIR/scalar-fast-macos13.metal" "$TMP_DIR/edges-fast-macos13.metal" "$TMP_DIR/splat-fast-macos13.metal" --edges "$TMP_DIR/semantic-fast-macos13.metal"
compile_case fast rejection "$ROOT_DIR/tests/gpu_artifacts/text_rejection_contract.seen" metal
rg -Fq 'PASS: Metal 60s seed=1097 malformed IR target capability and padded shared limits restore allocation baseline' "$TMP_DIR/rejection-fast.log"
SOURCE="$ROOT_DIR/tests/cuda_ptx_lowering/layout_happy.seen"
for target in macos13 macos14 macos15; do
    run_compiler gpu emit "$SOURCE" --gpu-backend metal --gpu-target "$target" --module-id metal-layout-contract.seen \
        --output "$TMP_DIR/cli-$target.metal" --reflection "$TMP_DIR/cli-$target.json" \
        --artifact-info > "$TMP_DIR/cli-$target.log"
    cmp "$TMP_DIR/cli-$target.metal" "$TMP_DIR/layout-fast-$target.metal"
done
run_compiler gpu emit "$SOURCE" --gpu-backend metal --module-id metal-layout-contract.seen --output "$TMP_DIR/cli-default.metal" \
    --reflection "$TMP_DIR/cli-default.json" --artifact-info > "$TMP_DIR/cli-default.log"
cmp "$TMP_DIR/cli-default.metal" "$TMP_DIR/cli-macos13.metal"
cmp "$TMP_DIR/cli-default.json" "$TMP_DIR/cli-macos13.json"
run_compiler gpu emit "$SOURCE" --gpu-backend metal --module-id metal-layout-contract.seen --gpu-entry integers \
    --output "$TMP_DIR/cli-selected.metal" --reflection "$TMP_DIR/cli-selected.json" \
    --artifact-info > "$TMP_DIR/cli-selected.log"
python3 - "$ROOT_DIR" "$TMP_DIR" <<'PY'
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(sys.argv[1]) / 'scripts'))
from check_gpu_metal_artifacts import check_artifact
from gpu_certification_io import read_bounded, manifest_from_log
root = Path(sys.argv[2])
for name in ('macos13', 'macos14', 'macos15', 'default', 'selected'):
    prefix = root / ('cli-' + name)
    reflection = json.loads(read_bounded(prefix.with_suffix('.json'), 4194304))
    record = {'target': reflection['target'], 'text': read_bounded(prefix.with_suffix('.metal'), 33554432).decode('ascii')}
    check_artifact(record, reflection, manifest_from_log(prefix.with_suffix('.log')))
    if name == 'selected': assert [entry['name'] for entry in reflection['entries']] == ['integers']
    elif name != 'default':
        expected = json.loads(read_bounded(root / ('layout-fast-' + name + '.json'), 4194304))
        assert reflection == expected
parameters = ', '.join(f'b{i}: ReadBuffer<UInt32>' for i in range(30))
(root / 'too-many.seen').write_text('@compute(1) fun excessive(' + parameters + ') {}')
print('PASS: Metal CLI target coverage default identity selected entries and artifact snapshots')
PY
if run_compiler gpu reflect "$TMP_DIR/too-many.seen" --gpu-backend metal --output "$TMP_DIR/rejected.json" > "$TMP_DIR/layout.log" 2>&1; then
    echo 'Metal reflection accepted too many native buffer slots' >&2; exit 1
fi
rg -Fq E_GPU_METAL_LAYOUT "$TMP_DIR/layout.log"; test ! -e "$TMP_DIR/rejected.json"
if run_compiler gpu emit "$SOURCE" --gpu-backend metal --output "$TMP_DIR/rejected.spv" > "$TMP_DIR/extension.log" 2>&1; then
    echo 'Metal CLI accepted a mismatched artifact extension' >&2; exit 1
fi
rg -Fq E_GPU_CLI "$TMP_DIR/extension.log"; test ! -e "$TMP_DIR/rejected.spv"
if run_compiler compile "$SOURCE" "$TMP_DIR/unavailable" --gpu-backend metal --frozen > "$TMP_DIR/backend.log" 2>&1; then
    echo 'Metal compile silently produced a CPU executable' >&2; exit 1
fi
rg -Fq E_GPU_BACKEND_UNAVAILABLE "$TMP_DIR/backend.log"; test ! -e "$TMP_DIR/unavailable"
echo 'PASS: Metal MSL source reflected native buffer ABI CPU semantic witnesses and bounded ownership; Apple SDK and GPU execution unverified'
