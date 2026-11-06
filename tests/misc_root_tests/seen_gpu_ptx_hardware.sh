#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ptx-hardware
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ptx-hardware.XXXXXX")"
python3 "$ROOT_DIR/scripts/check_gpu_ptx_toolchain.py" > "$TMP_DIR/toolchain.json"
for kind in layout capabilities packed cooperative bf16; do
    source="$ROOT_DIR/tests/cuda_ptx_lowering/${kind}_happy.seen"
    if [ "$kind" = packed ]; then source="$ROOT_DIR/tests/spir_v_vulkan_lowering/packed_happy.seen"; fi
    if [ "$kind" = bf16 ]; then source="$ROOT_DIR/tests/cuda_ptx_lowering/cooperative_bf16.seen"; fi
    timeout --foreground --kill-after=5s 120s bash "$ATTESTED_SEEN" "$COMPILER" gpu emit "$source" \
        --gpu-backend cuda --gpu-target sm_89 --output "$TMP_DIR/$kind.ptx" \
        --reflection "$TMP_DIR/$kind.reflection.json" --artifact-info > "$TMP_DIR/$kind-emit.log" 2>&1
    python3 - "$ROOT_DIR/scripts" "$TMP_DIR/$kind.ptx" <<'PY'
import pathlib, sys
sys.path.insert(0, sys.argv[1])
from check_gpu_ptx_toolchain import invoke, load_profile, require
path = pathlib.Path(sys.argv[2])
status, output, diagnostic = invoke(load_profile(), 'ptxas', ['-arch=sm_89', str(path), '-o', str(path.with_suffix('.cubin'))])
require(status == 0 and not output and not diagnostic, 'official SM89 assembly failed: ' + diagnostic)
PY
    timeout --foreground --kill-after=5s 120s python3 "$ROOT_DIR/scripts/check_gpu_ptx_hardware.py" "$TMP_DIR" \
        --kind "$kind" --compiler "$COMPILER" --require-device --cli-artifact > "$TMP_DIR/$kind-hardware-evidence.json"
    cat "$TMP_DIR/$kind-hardware-evidence.json"
done
echo "PASS: direct compiler CLI SM89 device execution; artifacts=$TMP_DIR"
