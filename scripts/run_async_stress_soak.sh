#!/usr/bin/env bash
# Manual persistent-worker scenario. Never default-discovered by seen-test.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-async-stress-soak
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
seconds=86400
device_class=rtx4090
while [ "$#" -gt 0 ]; do
    case "$1" in
        --seconds)
            [ "$#" -ge 2 ] || { echo 'async.invalid: --seconds requires a value' >&2; exit 2; }
            seconds=$2; shift 2 ;;
        --device-class)
            [ "$#" -ge 2 ] || { echo 'async.invalid: --device-class requires a value' >&2; exit 2; }
            device_class=$2; shift 2 ;;
        *) echo 'async.invalid: expected --seconds N --device-class rtx4090|software-reference' >&2; exit 2 ;;
    esac
done
# Bound decimal parsing before shell arithmetic, compiler entry or any artifacts.
[[ "$seconds" =~ ^[0-9]{1,5}$ ]] || { echo 'async.invalid: seconds must be decimal 1..86400' >&2; exit 2; }
seconds=$((10#$seconds))
[ "$seconds" -ge 1 ] && [ "$seconds" -le 86400 ] || { echo 'async.limit: seconds must be 1..86400' >&2; exit 2; }
case "$device_class" in
    rtx4090|software-reference) ;;
    *) echo 'async.invalid: unsupported device class' >&2; exit 2 ;;
esac
if [ "$device_class" = rtx4090 ]; then
    # Reject rather than sanitize inherited interposition into hardware evidence.
    for setting in LD_PRELOAD LD_AUDIT SEEN_VULKAN_FIXTURE_FAULT VK_INSTANCE_LAYERS VK_LAYER_PATH VK_ADD_LAYER_PATH VK_LOADER_LAYERS_ENABLE; do
        [ -z "${!setting:-}" ] || { echo "ERROR: hardware soak forbids $setting interposition" >&2; exit 126; }
    done
fi
# The caller supplies a separate finite scope budget for the manual long run
# (safe_rebuild SEEN_TIER_TIMEOUT_SECS=87000); never raise CI/test defaults here.
if [ "$seconds" -gt 900 ]; then
    budget=${SEEN_TIER_TIMEOUT_SECS:-}
    [[ "$budget" =~ ^[0-9]{1,9}$ ]] || { echo 'RESOURCE STOP: long soak requires caller-supplied finite SEEN_TIER_TIMEOUT_SECS' >&2; exit 126; }
    budget=$((10#$budget))
    [ "$budget" -ge "$((seconds + 600))" ] || { echo 'RESOURCE STOP: caller scope budget must cover duration plus bounded preparation/drain' >&2; exit 126; }
fi
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" --seconds "$seconds" --device-class "$device_class"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/async-stress-soak.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=async-stress-soak
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
# Keep the actual bounded evidence even on success. This is one finite artifact
# directory, not per-iteration output, and does not change a resource policy.
cleanup() {
    local status=$?
    trap - EXIT
    echo "Async soak artifacts: $TMP_DIR" >&2
    if [ "$status" -ne 0 ]; then seen_report_fixture_failure_logs "$TMP_DIR"; fi
    exit "$status"
}
trap cleanup EXIT
# Reuse the gate's bounded source/compiler/scope provenance contract. Capture
# before staging/compilation; a changed source view must not label the binary.
python3 - "$ROOT_DIR" "$COMPILER" "$TMP_DIR" <<'PY'
import json, pathlib, sys
root, compiler, output = map(pathlib.Path, sys.argv[1:])
sys.path.insert(0, str(root / "scripts"))
from accelerator_gate_evidence import source_identity, containment, digest
record = {"schema": "seen-async-soak-build-v1", "source": source_identity(),
          "compiler": {"path": str(compiler), **digest(compiler, 64 * 1024**2)},
          "containment": containment(), "production_certified": False}
data = (json.dumps(record, indent=2) + "\n").encode()
if len(data) > 256 * 1024: raise SystemExit("soak build provenance exceeds 256 KiB")
(output / "build-inputs.json").write_bytes(data)
PY
if [ "$device_class" = rtx4090 ]; then
    export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/nvidia_icd.json
    export VK_ICD_FILENAMES="$VK_DRIVER_FILES"
    export __GL_SHADER_DISK_CACHE=0
    export __GL_SHADER_DISK_CACHE_PATH="$TMP_DIR/nvidia-cache"
    export VK_LOADER_LAYERS_DISABLE='~all~'
    [ -f "$VK_DRIVER_FILES" ] && [ ! -L "$VK_DRIVER_FILES" ] || {
        echo 'ERROR: designated NVIDIA hardware ICD is unavailable; no fallback' >&2; exit 126;
    }
else
    seen_prepare_vulkan_reference
    export VK_ICD_FILENAMES="$VK_DRIVER_FILES"
fi
# The owning manifest opts into Vulkan explicitly. Install the exact common
# async sources before offline lock preparation/frozen compilation, not after.
seen_prepare_vulkan_fixture_project
for support in stress_fixture_support cpu_stress_fixture_support io_stress_fixture_support; do
    cp -- "$ROOT_DIR/seen_std/tests/async/$support.seen" "$TMP_DIR/vulkan-native-project/"
done
run_compiler gpu emit "$ROOT_DIR/tests/production_gpu_runtime/vulkan_foundation_kernel.seen" \
    --gpu-backend spirv --gpu-target vulkan1.2 --gpu-entry foundation \
    --gpu-fallback error --module-id p0-accel-foundation --artifact-info \
    --output "$TMP_DIR/foundation.spv" --reflection "$TMP_DIR/foundation.json" \
    > "$TMP_DIR/foundation-emit.log" 2>&1
spirv-val --target-env vulkan1.2 "$TMP_DIR/foundation.spv" > "$TMP_DIR/foundation-validate.log" 2>&1
compile_vulkan_fixture release soak "$ROOT_DIR/tests/production_gpu_runtime/async_stress_soak_contract.seen"
# Successful compiler intermediates are not soak evidence. Remove only this
# invocation's owned IR tree; preserve all diagnostics on a failed compile.
if [ -e "$TMP_DIR/soak-release-ir" ]; then
    [ -d "$TMP_DIR/soak-release-ir" ] && [ ! -L "$TMP_DIR/soak-release-ir" ] || {
        echo 'ERROR: unsafe owned soak IR directory' >&2; exit 126;
    }
    rm -rf -- "$TMP_DIR/soak-release-ir"
fi
python3 - "$ROOT_DIR" "$COMPILER" "$TMP_DIR" <<'PY'
import json, pathlib, sys
root, compiler, output = map(pathlib.Path, sys.argv[1:])
sys.path.insert(0, str(root / "scripts"))
from accelerator_gate_evidence import source_identity, digest
record = json.loads((output / "build-inputs.json").read_bytes())
if record["source"] != source_identity() or record["compiler"] != {
        "path": str(compiler), **digest(compiler, 64 * 1024**2)}:
    raise SystemExit("soak source/compiler inputs changed during preparation")
record["native"] = digest(output / "soak-release", 64 * 1024**2)
record["shader"] = digest(output / "foundation.spv", 32 * 1024**2)
record["reflection"] = digest(output / "foundation.json", 4 * 1024**2)
record["profile"] = "release-thin-lto"
data = (json.dumps(record, indent=2) + "\n").encode()
if len(data) > 256 * 1024: raise SystemExit("soak build receipt exceeds 256 KiB")
(output / "build-receipt.json").write_bytes(data)
PY
# No shell sits between observer and native program. The observer owns exactly
# its new child session/group and preserves the real native return status.
status=0
python3 "$ROOT_DIR/scripts/monitor_async_stress_soak.py" \
    --seconds "$seconds" --device-class "$device_class" --output-dir "$TMP_DIR" --check-boundaries -- \
    "$TMP_DIR/soak-release" "$seconds" "$TMP_DIR/completion.bin" \
    "$TMP_DIR/foundation.spv" "$TMP_DIR/foundation.json" "$device_class" || status=$?
printf '%s\n' "$status" > "$TMP_DIR/soak-release.run.status"
[ "$status" -eq 0 ] || exit "$status"
echo 'PASS: component-only persistent CPU/I/O/GPU soak; not release, performance or TSan certification'
