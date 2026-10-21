#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ptx-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ptx-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ptx-a
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
python3 "$ROOT_DIR/scripts/check_gpu_ptx_toolchain.py" > "$TMP_DIR/toolchain.json"
for profile in fast release undefined address; do
    compile_case "$profile" layout "$ROOT_DIR/tests/cuda_ptx_lowering/layout_contract.seen" \
        "$ROOT_DIR/tests/cuda_ptx_lowering/layout_happy.seen"
    rg -Fq 'PASS: PTX seven targets deterministic artifacts reflection and repeated cleanup' "$TMP_DIR/layout-$profile.log"
done
python3 "$ROOT_DIR/scripts/check_gpu_ptx_artifacts.py" "$TMP_DIR" layout
for target in sm_70 sm_75 sm_80 sm_86 sm_89 sm_90 sm_100; do
    run_compiler gpu emit "$ROOT_DIR/tests/spir_v_vulkan_lowering/return_control.seen" \
        --gpu-backend cuda --gpu-target "$target" --output "$TMP_DIR/returns-$target.ptx" \
        > "$TMP_DIR/returns-$target.log" 2>&1
    python3 - "$ROOT_DIR/scripts" "$target" "$TMP_DIR/returns-$target.ptx" <<'PY'
import pathlib,sys
sys.path.insert(0,sys.argv[1])
from check_gpu_ptx_toolchain import invoke,load_profile,require
path=pathlib.Path(sys.argv[3])
status,output,diagnostic=invoke(load_profile(),'ptxas',['-arch='+sys.argv[2],str(path),'-o',str(path.with_suffix('.cubin'))])
require(status==0 and not output and not diagnostic,'helper-return assembly failed: '+diagnostic)
PY
done
status=0
run_compiler gpu emit "$ROOT_DIR/tests/typed_gpu_compiler/rejection_host.seen" \
    --gpu-backend cuda --gpu-target sm_80 --output "$TMP_DIR/rejected.ptx" \
    > "$TMP_DIR/rejected.log" 2>&1 || status=$?
test "$status" -eq 1
rg -Fq E_GPU_UNSUPPORTED "$TMP_DIR/rejected.log"
test ! -e "$TMP_DIR/rejected.ptx"
compile_case fast rejection "$ROOT_DIR/tests/cuda_ptx_lowering/rejection_contract.seen"
rg -Fq 'PASS: PTX 60s seed=1091 malformed IR target capability and shared limits restore allocation baseline' "$TMP_DIR/rejection-fast.log"
echo 'PASS: CUDA PTX entries layout helper control and official offline assembly'
