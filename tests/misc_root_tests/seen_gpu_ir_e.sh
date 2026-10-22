#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ir-e
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ir-e.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ir-e
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
test ! -e "$ROOT_DIR/compiler_seen/src/codegen/glsl_gen.seen"
for profile in fast release undefined address; do
    compile_case "$profile" nested-inline "$ROOT_DIR/tests/compiler_regressions/nested_inline_field_assignment.seen"
    rg -Fq 'PASS: nested inline fields update original storage preserve copies and neighbors and evaluate receivers once' "$TMP_DIR/nested-inline-$profile.log"
    compile_case "$profile" rejection "$ROOT_DIR/tests/typed_gpu_compiler/ir_rejection_contract.seen"
    rg -Fq 'PASS: GPU-001E kernel names cannot replace parsed body behavior' "$TMP_DIR/rejection-$profile.log"
    rg -Fq 'PASS: GPU-001E stable rejection goldens unsupported stages and CPU marker isolation' "$TMP_DIR/rejection-$profile.log"
done
python3 - "$ROOT_DIR" "$TMP_DIR" <<'PY'
import json,pathlib,sys
root=pathlib.Path(sys.argv[1]); output=pathlib.Path(sys.argv[2])
expected=json.loads((root/'tests/typed_gpu_compiler/rejection_goldens.json').read_text())
for profile in ('fast','release','undefined','address'):
    records=[json.loads(line) for line in (output/f'rejection-{profile}.log').read_text().splitlines() if line.startswith('{"schema":"seen-gpu-rejection-golden-v1"')]
    assert records==expected, (profile,records)
print('PASS: exact rejection codes messages cardinality and locations match goldens in four profiles')
PY
for specification in graphics:E_GPU_SIGNATURE host:E_GPU_UNSUPPORTED; do
    name=${specification%:*}; code=${specification#*:}; status=0
    run_compiler gpu reflect "$ROOT_DIR/tests/typed_gpu_compiler/rejection_$name.seen" --output "$TMP_DIR/$name.json" > "$TMP_DIR/$name.log" 2>&1 || status=$?
    test "$status" -eq 1
    rg -Fq "$code" "$TMP_DIR/$name.log"
    test ! -e "$TMP_DIR/$name.json"
done
status=0
run_compiler compile "$ROOT_DIR/tests/typed_gpu_compiler/reflection_happy.seen" "$TMP_DIR/legacy" --emit-glsl --fast --frozen > "$TMP_DIR/legacy.log" 2>&1 || status=$?
test "$status" -eq 1
rg -Fq W_GPU_LEGACY_GLSL "$TMP_DIR/legacy.log"
rg -Fq E_GPU_BACKEND_UNAVAILABLE "$TMP_DIR/legacy.log"
test ! -e "$TMP_DIR/legacy"
if find "$TMP_DIR" -type f \( -name '*.spv' -o -name '*.glsl' -o -name '*.cu' -o -name '*.metal' \) -print -quit | rg -q .; then
    echo 'rejected GPU source left a backend artifact' >&2; exit 1
fi
compile_case fast fuzz "$ROOT_DIR/tests/typed_gpu_compiler/ir_rejection_fuzz.seen"
rg -Fq 'PASS: GPU-001E rejection fuzz 60s seed=1087 preserved diagnostics source positions and allocation baseline' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: GPU rejection goldens and unsupported-body isolation'
