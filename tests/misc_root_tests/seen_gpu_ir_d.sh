#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-ir-d
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-ir-d.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-ir-d
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
for profile in fast release undefined address; do
    compile_case "$profile" quotes "$ROOT_DIR/tests/compiler_regressions/escaped_literal_quotes.seen"
    rg -Fq 'PASS: decoded string literals preserve meaningful quote bytes' "$TMP_DIR/quotes-$profile.log"
    compile_case "$profile" bytes "$ROOT_DIR/tests/typed_gpu_compiler/ir_bytes_contract.seen"
    rg -Fq 'PASS: GPU-001D byte text round trip all octets empty rejection and ownership' "$TMP_DIR/bytes-$profile.log"
    compile_case "$profile" files "$ROOT_DIR/tests/typed_gpu_compiler/ir_file_contract.seen" "$ROOT_DIR/tests/typed_gpu_compiler/reflection_happy.seen"
    rg -Fq 'PASS: GPU bounded file success rejection and ownership' "$TMP_DIR/files-$profile.log"
    compile_case "$profile" options "$ROOT_DIR/tests/typed_gpu_compiler/ir_options_contract.seen"
    rg -Fq 'PASS: GPU-001D strict manifest vocabulary duplicate keys targets entries and cleanup' "$TMP_DIR/options-$profile.log"
    compile_case "$profile" reflection "$ROOT_DIR/tests/typed_gpu_compiler/ir_reflection_contract.seen"
    rg -Fq 'PASS: GPU-001D deterministic multi-entry reflection hashes reachable requirements and cleanup' "$TMP_DIR/reflection-$profile.log"
    compile_case "$profile" cli "$ROOT_DIR/tests/typed_gpu_compiler/ir_cli_contract.seen"
    rg -Fq 'PASS: GPU-001D strict GPU CLI aliases selection and source reflection' "$TMP_DIR/cli-$profile.log"
done
python3 - "$TMP_DIR" <<'PY'
import hashlib,json,pathlib,sys
root=pathlib.Path(sys.argv[1]); previous=None
for profile in ('fast','release','undefined','address'):
    lines=(root/f'reflection-{profile}.log').read_text().splitlines()
    ir=next(line for line in lines if line.startswith('{"schema":"seen-gpu-ir-v1"'))
    raw=next(line for line in lines if line.startswith('{"schema":"seen-gpu-reflection-v1"'))
    data=json.loads(raw)
    assert data['ir_sha256']==hashlib.sha256(ir.encode()).hexdigest()
    assert data['hardware_verified'] is False and data['gpu_execution_verified'] is False
    assert [entry['name'] for entry in data['entries']]==['first','second']
    for entry in data['entries']:
        keys=('workgroup','bindings','required_capabilities','shared_bytes','requires_fault_buffer','fault_binding')
        abi={key:entry[key] for key in keys};abi['specializations']=data['specializations']
        canonical=json.dumps(abi,separators=(',',':'),ensure_ascii=False)
        assert entry['abi_sha256']==hashlib.sha256(canonical.encode()).hexdigest()
    if previous is not None: assert raw==previous
    previous=raw
print('PASS: independently recomputed IR and ABI SHA-256 reflection hashes')
PY
SOURCE="$ROOT_DIR/tests/typed_gpu_compiler/reflection_happy.seen"
run_compiler gpu reflect "$SOURCE" --output "$TMP_DIR/first.json" --gpu-entry second --gpu-entry first
run_compiler gpu reflect "$SOURCE" --output "$TMP_DIR/second.json" --gpu-entry first --gpu-entry second
cmp "$TMP_DIR/first.json" "$TMP_DIR/second.json"
if run_compiler gpu reflect "$SOURCE" --output "$TMP_DIR/rejected.json" --gpu-target sm_80 > "$TMP_DIR/rejected.log" 2>&1; then
    echo 'GPU reflection accepted a target without a backend' >&2; exit 1
fi
rg -Fq E_GPU_OPTIONS "$TMP_DIR/rejected.log"
test ! -e "$TMP_DIR/rejected.json"
if run_compiler compile "$SOURCE" "$TMP_DIR/unavailable" --gpu-backend cuda --frozen > "$TMP_DIR/backend.log" 2>&1; then
    echo 'unavailable GPU backend produced an executable' >&2; exit 1
fi
rg -Fq E_GPU_BACKEND_UNAVAILABLE "$TMP_DIR/backend.log"
test ! -e "$TMP_DIR/unavailable"
compile_case fast fuzz "$ROOT_DIR/tests/typed_gpu_compiler/ir_reflection_fuzz.seen"
rg -Fq 'PASS: GPU-001D reflection metadata fuzz 60s seed=1086 checked malformed policy IR and ownership' "$TMP_DIR/fuzz-fast.log"
echo 'PASS: GPU multi-entry reflection manifest and strict CLI contract'
