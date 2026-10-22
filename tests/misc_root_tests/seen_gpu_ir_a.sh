#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ir-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ir-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ir-a
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
expect_rejected() {
    local name=$1 diagnostic=$2 status=0
    shift 2
    run_compiler "$@" >"$TMP_DIR/$name.log" 2>&1 || status=$?
    [ "$status" -eq 1 ] || { echo "FAIL: $name returned $status instead of a typed rejection" >&2; exit 1; }
    rg -Fq "$diagnostic" "$TMP_DIR/$name.log"
}
for profile in fast release undefined address; do
    compile_case "$profile" contract "$ROOT_DIR/compiler_seen/tests/gpu_frontend_contract.seen"
    for name in 'closed types address spaces literal bounds and checked workgroups' \
        'actual parsed bodies enforce access typing scope and host isolation' \
        'bounded diagnostics traversal and validator cleanup'; do
        rg -Fq "PASS: GPU-001A $name" "$TMP_DIR/contract-$profile.log"
    done
    SEEN_TRACE_BUILD="$TMP_DIR/cpu-$profile.trace.jsonl" \
        compile_case "$profile" cpu "$ROOT_DIR/tests/typed_gpu_compiler/cpu_marker_literals.seen"
    rg -Fq 'PASS: GPU-001A CPU marker literals and non-extern names remain CPU-only' "$TMP_DIR/cpu-$profile.log"
    python3 - "$TMP_DIR/cpu-$profile.trace.jsonl" <<'PY'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
assert path.is_file() and 0 < path.stat().st_size <= 1048576
events = [json.loads(line) for line in path.read_text().splitlines()]
assert events and all(not (event['phase'] == 'runtime object cache' and event['detail'].startswith('seen_gpu')) for event in events)
PY
done
run_compiler check "$ROOT_DIR/tests/typed_gpu_compiler/frontend_happy.seen" --frozen >"$TMP_DIR/happy.log" 2>&1
for specification in readonly_write:E_GPU_ACCESS writeonly_read:E_GPU_ACCESS host_call:E_GPU_UNSUPPORTED \
    bad_workgroup:E_GPU_WORKGROUP bad_width:E_GPU_RESOURCE class_method:E_GPU_SIGNATURE; do
    fixture=${specification%:*}; diagnostic=${specification#*:}
    expect_rejected "$fixture" "$diagnostic" check "$ROOT_DIR/tests/typed_gpu_compiler/frontend_$fixture.seen" --frozen
done
python3 - "$TMP_DIR/too-many-kernels.seen" <<'PY'
import sys
from pathlib import Path
Path(sys.argv[1]).write_text(''.join(f'@compute(64) fun kernel_{i}() {{ }}\n' for i in range(257)) + 'fun main() r: Int { return 0 }\n')
PY
expect_rejected kernel-limit E_GPU_LIMIT check "$TMP_DIR/too-many-kernels.seen" --frozen
expect_rejected no-backend E_GPU_BACKEND_UNAVAILABLE compile \
    "$ROOT_DIR/tests/typed_gpu_compiler/frontend_happy.seen" "$TMP_DIR/no-backend" --fast --no-cache --frozen
[ ! -e "$TMP_DIR/no-backend" ]
expect_rejected spaced-backend E_GPU_BACKEND_UNAVAILABLE compile \
    "$ROOT_DIR/tests/typed_gpu_compiler/frontend_spaced_annotation.seen" "$TMP_DIR/spaced-backend" --fast --no-cache --frozen
[ ! -e "$TMP_DIR/spaced-backend" ]
for stage in vertex fragment; do
    expect_rejected "$stage-stage" E_GPU_SIGNATURE compile \
        "$ROOT_DIR/tests/typed_gpu_compiler/frontend_${stage}_unsupported.seen" "$TMP_DIR/$stage-stage" --fast --no-cache --frozen
    [ ! -e "$TMP_DIR/$stage-stage" ]
done
expect_rejected legacy-glsl E_GPU_BACKEND_UNAVAILABLE compile \
    "$ROOT_DIR/tests/typed_gpu_compiler/frontend_happy.seen" "$TMP_DIR/legacy-glsl" --emit-glsl --fast --no-cache --frozen
[ ! -e "$TMP_DIR/legacy-glsl" ] && [ ! -e "$TMP_DIR/legacy-glsl.shaders" ]
compile_case fast fuzz "$ROOT_DIR/tests/typed_gpu_compiler/frontend_metadata_fuzz.seen"
rg -Fq 'PASS: GPU-001A metadata fuzz 60s seed=1411 type literal geometry and allocation bounds' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: typed compute frontend and fail-closed GPU backend boundary'
