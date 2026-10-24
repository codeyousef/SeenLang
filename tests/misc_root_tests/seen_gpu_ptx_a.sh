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
    compile_fixture "$profile" text "$ROOT_DIR/tests/gpu_artifacts/text_layout_contract.seen"
    for pair in layout:cuda_ptx_lowering/layout_happy splat:gpu_artifacts/vector_splat; do
        name=${pair%%:*}; source="$ROOT_DIR/tests/${pair#*:}.seen"
        cp -- "$TMP_DIR/text-$profile" "$TMP_DIR/$name-$profile"
        cp -- "$TMP_DIR/text-$profile.log" "$TMP_DIR/$name-$profile.log"
        cp -- "$TMP_DIR/text-$profile.compile.log" "$TMP_DIR/$name-$profile.compile.log"
        cp -- "$TMP_DIR/text-$profile.compile.status" "$TMP_DIR/$name-$profile.compile.status"
        run_fixture "$profile" "$name" cuda "$source"
        rg -Fq 'PASS: PTX target coverage deterministic artifacts reflection and repeated cleanup' "$TMP_DIR/$name-$profile.log"
    done
done
python3 "$ROOT_DIR/scripts/check_gpu_ptx_artifacts.py" "$TMP_DIR" layout splat
hardware_args=()
if [ "${SEEN_REQUIRE_PTX_HARDWARE:-0}" = 1 ]; then hardware_args+=(--require-device); fi
timeout --foreground --kill-after=5s 120s python3 "$ROOT_DIR/scripts/check_gpu_ptx_hardware.py" "$TMP_DIR" --kind layout --compiler "$COMPILER" "${hardware_args[@]}" > "$TMP_DIR/hardware-evidence.json"
cp "$TMP_DIR/hardware-evidence.json" "$SEEN_ARTIFACT_ROOT/ptx-a-hardware-evidence.json"
cat "$TMP_DIR/hardware-evidence.json"
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
compile_case fast rejection "$ROOT_DIR/tests/gpu_artifacts/text_rejection_contract.seen" cuda
rg -Fq 'PASS: PTX 60s seed=1091 malformed IR target capability and shared limits restore allocation baseline' "$TMP_DIR/rejection-fast.log"
echo 'PASS: CUDA PTX entries layout helper control and official offline assembly'
