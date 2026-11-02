#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-p0-accel-gate-b
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "${SEEN_ARTIFACT_ROOT:?}/p0-accel-b.XXXXXX")"
# Keep the complete child artifacts as well as this aggregate's raw logs/statuses.
# No child scope or serializer attestation is forged; each entry rebinds itself.
export SEEN_KEEP_TMP=1
ulimit -c 0
ulimit -S -f 65536
printf '[]\n' > "$TMP_DIR/checks.json"
record() {
    local name=$1 status=$2 execution=$3 classification=$4 required=$5 detail=$6
    shift 6
    python3 - "$TMP_DIR/checks.json" "$name" "$status" "$execution" "$classification" "$required" "$detail" "$@" <<'PY'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
rows = json.loads(path.read_text())
rows.append({'name': sys.argv[2], 'exit_status': int(sys.argv[3]),
             'execution': sys.argv[4], 'classification': sys.argv[5],
             'required': sys.argv[6] == 'true', 'logs': sys.argv[8:],
             'details': {'evidence_boundary': sys.argv[7]}})
path.write_text(json.dumps(rows, indent=2) + '\n')
PY
}
finish() {
    local status=$? report_status=0
    trap - EXIT
    if [ "$status" -ne 0 ]; then
        printf 'Harness exit status: %s\n' "$status" > "$TMP_DIR/harness-exit.log"
        record harness-exit "$status" none unsupported true \
            'Aggregate infrastructure failure; preceding child results remain available' harness-exit.log || :
    fi
    python3 "$ROOT_DIR/scripts/accelerator_gate_evidence.py" --phase b \
        --compiler "$COMPILER" --root "$ROOT_DIR" --artifacts "$TMP_DIR" \
        --checks "$TMP_DIR/checks.json" --output "$TMP_DIR/accelerator-b-evidence.json" || report_status=$?
    printf 'Emitter corpus evidence retained: %s\n' "$TMP_DIR" >&2
    if [ "$status" -ne 0 ]; then exit "$status"; fi
    exit "$report_status"
}
trap finish EXIT
children=(
    seen_gpu_ir_a seen_gpu_ir_b seen_gpu_ir_c
    seen_gpu_spirv_a seen_gpu_spirv_b seen_gpu_spirv_c
    seen_gpu_ptx_a seen_gpu_ptx_b seen_gpu_ptx_b_numeric seen_gpu_ptx_c
    seen_gpu_amdgpu_a seen_gpu_amdgpu_b seen_gpu_amdgpu_b_numeric seen_gpu_amdgpu_c
    seen_gpu_metal_a seen_gpu_metal_b seen_gpu_metal_b_numeric seen_gpu_metal_c
)
for child in "${children[@]}"; do
    status=0
    timeout --foreground --kill-after=5s 3600s env SEEN_CAPPED_REGRESSION_ACTIVE=0 \
        COMPILER="$COMPILER" bash "$ROOT_DIR/tests/misc_root_tests/$child.sh" \
        > "$TMP_DIR/$child.log" 2>&1 || status=$?
    printf '%s\n' "$status" > "$TMP_DIR/$child.status"
    # A child may run official tools and (PTX A/B) optional hardware. Its overall
    # success is not promoted into hardware certification; those observations
    # are recorded separately below, including unavailable/unreached execution.
    if [ "$child" = seen_gpu_metal_c ] && [ "$status" -eq 126 ]; then
        # The required native CPU receipt is independently checked below.
        record "$child" "$status" none unsupported false \
            'Apple SDK unavailable; accepting this status still requires the complete native CPU integrity receipt' \
            "$child.log" "$child.status"
    else
        record "$child" "$status" native-cpu compile-only true \
            'Full child gate, including native profiles, independent goldens, typed rejections and its required artifact/SDK checks; not a blanket GPU-execution claim' \
            "$child.log" "$child.status"
    fi
done
# Extract only actual structured PTX hardware observations from this run's raw
# child logs, never a stale root-level hardware-evidence.json or PASS wording.
python3 - "$TMP_DIR" <<'PY'
import json, sys
from pathlib import Path
root = Path(sys.argv[1])
path = root / 'checks.json'
rows = json.loads(path.read_text())
for child in ('seen_gpu_ptx_a', 'seen_gpu_ptx_b'):
    raw = root / (child + '.log')
    if raw.stat().st_size > 64 * 1024**2:
        raise ValueError('child raw log exceeds its evidence ceiling')
    observations = []
    for line in raw.read_text(errors='replace').splitlines():
        if not line.startswith('{'):
            continue
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict) and value.get('schema') == 'seen-ptx-hardware-evidence-v1':
            observations.append(value)
    if len(observations) > 1:
        raise ValueError('duplicate child hardware observation')
    value = observations[0] if observations else None
    executed = value is not None and value.get('gpu_execution_verified') is True
    if executed and (value.get('classification') != 'experimental-hardware' or
                     value.get('fallback_count') != 0 or not value.get('profiles')):
        raise ValueError('invalid child hardware observation')
    name = child + '-physical-hardware'
    evidence = name + '.json'
    (root / evidence).write_text(json.dumps({'observed': value,
        'state': 'executed' if executed else ('unexecuted' if value else 'not-observed'),
        'note': 'Only the child hardware observation proves execution; child exit zero alone does not.'}, indent=2) + '\n')
    rows.append({'name': name, 'exit_status': 0 if executed else 125,
                 'classification': 'experimental-hardware' if executed else 'unsupported',
                 'execution': 'native-gpu' if executed else 'none', 'required': False,
                 'logs': [evidence, child + '.log', child + '.status'],
                 'details': {'child': child, 'production_certified': False}})
for backend in ('spirv', 'hip', 'metal'):
    name = backend + '-physical-hardware'
    evidence = name + '.json'
    (root / evidence).write_text(json.dumps({'backend': backend, 'state': 'unexecuted',
        'gpu_execution_verified': False,
        'reason': 'These emitter children provide no physical-device execution for this backend.'}, indent=2) + '\n')
    rows.append({'name': name, 'exit_status': 125, 'classification': 'unsupported',
                 'execution': 'none', 'required': False, 'logs': [evidence],
                 'details': {'production_certified': False}})
path.write_text(json.dumps(rows, indent=2) + '\n')
PY
status=0
python3 "$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/metal_child_evidence.py" "$TMP_DIR" \
    > "$TMP_DIR/metal-certificate-observation.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$TMP_DIR/metal-certificate-observation.status"
record metal-certificate-observation "$status" native-cpu compile-only true \
    'Bind the exact Metal-C receipt to four retained native profile witnesses; SDK unsupported does not imply CPU success' \
    metal-certificate-observation.log metal-certificate-observation.status
SHARED="$TMP_DIR/shared-foundation"
mkdir -- "$SHARED"
SOURCE="$ROOT_DIR/tests/production_gpu_runtime/vulkan_foundation_kernel.seen"
cp -- "$SOURCE" "$SHARED/source.seen"
sha256sum -- "$SHARED/source.seen" > "$SHARED/source.sha256"
for specification in spirv:vulkan1.2:spv cuda:sm_89:ptx hip:gfx90a:ll metal:macos13:metal; do
    IFS=: read -r backend target extension <<< "$specification"
    status=0
    timeout --foreground --kill-after=5s 300s bash "$ATTESTED_SEEN" "$COMPILER" \
        gpu emit "$SHARED/source.seen" --gpu-backend "$backend" --gpu-target "$target" \
        --gpu-entry foundation --gpu-fallback error --module-id p0-accel-foundation \
        --artifact-info --output "$SHARED/$backend.$extension" --reflection "$SHARED/$backend.json" \
        > "$SHARED/$backend.log" 2>&1 || status=$?
    printf '%s\n' "$status" > "$SHARED/$backend.status"
    record "shared-foundation-$backend" "$status" native-cpu compile-only true \
        'Actual selected foundation emission from identical retained source and module identity; no official validation or device execution claimed by this additional row' \
        "shared-foundation/$backend.log" "shared-foundation/$backend.status" \
        shared-foundation/source.seen shared-foundation/source.sha256
done
status=0
python3 "$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/logical_reflection.py" \
    --root "$ROOT_DIR" --artifacts "$SHARED" > "$TMP_DIR/shared-logical-reflection.log" 2>&1 || status=$?
logs=(shared-logical-reflection.log shared-foundation/source.seen shared-foundation/source.sha256)
if [ -f "$SHARED/logical-reflection.json" ]; then logs+=(shared-foundation/logical-reflection.json); fi
# Hash all actual emitted bytes/reflections into the parent report. Missing files
# remain failures of their emission/comparison rows, not manufactured artifacts.
for specification in spirv:spv cuda:ptx hip:ll metal:metal; do
    IFS=: read -r backend extension <<< "$specification"
    for file in "$backend.$extension" "$backend.json"; do
        if [ -f "$SHARED/$file" ]; then logs+=("shared-foundation/$file"); fi
    done
done
record shared-foundation-logical-reflection "$status" native-cpu compile-only true \
    'Logical entry/workgroup/user-binding parity only; canonical IR digest equality is justified by identical backend-independent lowering inputs, never by equal physical ABI' "${logs[@]}"
