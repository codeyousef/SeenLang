#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-vulkan-hardware
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
[ "${SEEN_REQUIRE_VULKAN_HARDWARE:-0}" = 1 ] || {
    echo 'ERROR: Vulkan hardware certification requires explicit SEEN_REQUIRE_VULKAN_HARDWARE=1' >&2
    exit 2
}
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-vulkan-hardware.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-vulkan-hardware
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
# Select the designated hardware ICD explicitly. Never inherit Mesa reference
# or fault-injection settings from another fixture.
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/nvidia_icd.json
export VK_ICD_FILENAMES="$VK_DRIVER_FILES"
export __GL_SHADER_DISK_CACHE=0
export __GL_SHADER_DISK_CACHE_PATH="$TMP_DIR/nvidia-cache"
unset SEEN_VULKAN_FIXTURE_FAULT VK_INSTANCE_LAYERS VK_LAYER_PATH
case "${LD_PRELOAD:-}" in
    *vulkan-fault*) echo 'ERROR: Vulkan fault interposition is forbidden for hardware evidence' >&2; exit 126 ;;
esac
[ -f "$VK_DRIVER_FILES" ] && [ ! -L "$VK_DRIVER_FILES" ]
for profile in fast release undefined address; do
    compile_case "$profile" resources "$ROOT_DIR/tests/production_gpu_runtime/vulkan_hardware_contract.seen"
    rg -Fq 'PASS: GPU-006A/B RTX 4090 Vulkan discovery context typed allocation accounting and cleanup' "$TMP_DIR/resources-$profile.log"
done
python3 - "$ROOT_DIR" "$COMPILER" "$TMP_DIR" <<'PY'
import hashlib, json, pathlib, subprocess, sys
root, compiler, output = map(pathlib.Path, sys.argv[1:])
cgroup = pathlib.Path('/sys/fs/cgroup') / pathlib.Path('/proc/self/cgroup').read_text().strip().split('::', 1)[1].lstrip('/')
evidence = {
    'schema': 'seen-vulkan-hardware-resources-v1',
    'classification': 'experimental-hardware',
    'hardware_resources_verified': True,
    'gpu_execution_verified': False,
    'scope': 'discovery-context-device-local-allocation-accounting-cleanup',
    'fallback_count': 0,
    'source_commit': subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip(),
    'compiler_sha256': hashlib.file_digest(compiler.open('rb'), 'sha256').hexdigest(),
    'fixture_sha256': hashlib.sha256((root / 'tests/production_gpu_runtime/vulkan_hardware_contract.seen').read_bytes()).hexdigest(),
    'profiles': {name: hashlib.sha256((output / f'resources-{name}.log').read_bytes()).hexdigest()
                 for name in ('fast', 'release', 'undefined', 'address')},
    'containment': {name: (cgroup / name).read_text().strip()
                    for name in ('memory.max', 'memory.swap.max', 'memory.oom.group', 'pids.max', 'memory.peak', 'pids.peak', 'memory.events', 'pids.events')},
}
payload = json.dumps(evidence, sort_keys=True) + '\n'
(output / 'hardware-evidence.json').write_text(payload)
print(payload, end='')
PY
cp "$TMP_DIR/hardware-evidence.json" "$SEEN_ARTIFACT_ROOT/vulkan-hardware-evidence.json"
echo 'PASS: designated Vulkan hardware resource lifecycle; queue and transfer certification remains separate'
