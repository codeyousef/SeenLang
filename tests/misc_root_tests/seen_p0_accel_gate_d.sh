#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-p0-accel-gate-d
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { timeout --foreground --kill-after=5s 300s bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "${SEEN_ARTIFACT_ROOT:?}/p0-accel-d.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=p0-accel-d
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
# Only reuse the explicit-native compilation helper, never software ICD setup.
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
export SEEN_KEEP_TMP=1
ulimit -S -f 65536
CHECKER="$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/hardware_evidence.py"
printf '[]\n' > "$TMP_DIR/checks.json"
record() {
    local name=$1 status=$2 execution=$3 classification=$4 detail=$5
    shift 5
    python3 - "$TMP_DIR/checks.json" "$name" "$status" "$execution" "$classification" "$detail" "$@" <<'PY'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
rows = json.loads(path.read_text())
if any(row['name'] == sys.argv[2] for row in rows):
    raise ValueError('duplicate D check name')
rows.append({'name': sys.argv[2], 'exit_status': int(sys.argv[3]), 'execution': sys.argv[4],
             'classification': sys.argv[5], 'required': True, 'logs': sys.argv[7:],
             'details': {'evidence_boundary': sys.argv[6], 'production_certified': False}})
path.write_text(json.dumps(rows, indent=2) + '\n')
PY
}
finish() {
    local status=$? report_status=0
    trap - EXIT
    if [ "$status" -ne 0 ]; then
        printf 'Harness exit status: %s\n' "$status" > "$TMP_DIR/harness-exit.log"
        record harness-exit "$status" none unsupported \
            'Harness failure; all preceding raw hardware/status observations retained' harness-exit.log || :
    fi
    python3 "$ROOT_DIR/scripts/accelerator_gate_evidence.py" --phase d \
        --compiler "$COMPILER" --root "$ROOT_DIR" --artifacts "$TMP_DIR" \
        --checks "$TMP_DIR/checks.json" --output "$TMP_DIR/accelerator-d-evidence.json" || report_status=$?
    printf 'RTX4090 CUDA/Vulkan development evidence retained: %s\n' "$TMP_DIR" >&2
    if [ "$status" -ne 0 ]; then exit "$status"; fi
    exit "$report_status"
}
trap finish EXIT
# Every command gets its own immutable log/status, and a failed prerequisite
# does not discard the other backend or later sanitizer profiles.
capture() {
    local name=$1 execution=$2 classification=$3 detail=$4 status=0
    shift 4
    "$@" > "$TMP_DIR/$name.log" 2>&1 || status=$?
    printf '%s\n' "$status" > "$TMP_DIR/$name.status"
    record "$name" "$status" "$execution" "$classification" "$detail" "$name.log" "$name.status"
    return "$status"
}
capture nvidia-inventory native-cpu experimental-hardware \
    'Actual installed NVIDIA driver inventory; identity cross-checked against each native backend observation' \
    timeout --foreground --kill-after=5s 30s nvidia-smi \
    --query-gpu=name,uuid,driver_version --format=csv,noheader,nounits || :
capture nvidia-icd native-cpu experimental-hardware \
    'Only an installed NVIDIA Vulkan ICD is admissible; no Mesa/lvp fallback' \
    python3 "$CHECKER" --root "$ROOT_DIR" --artifacts "$TMP_DIR" select-icd || :
# Exact C SDK oracles are separate CPU ABI observations, never GPU passes.
cuda_include=()
for directory in "${CUDA_PATH:-/opt/cuda}/include" /usr/local/cuda/include; do
    if [ -f "$directory/cuda.h" ]; then cuda_include=(-I "$directory"); break; fi
done
for backend in cuda vulkan; do
    oracle="$ROOT_DIR/tests/production_gpu_runtime/vulkan_abi_oracle.c"
    includes=()
    if [ "$backend" = cuda ]; then
        oracle="$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/cuda_foundation/sdk_abi_oracle.c"
        includes=("${cuda_include[@]}")
    fi
    oracle_status=0
    capture "$backend-sdk-abi-build" native-cpu compile-only \
        'Actual installed SDK header ABI oracle with strict C warnings; not native GPU execution' \
        timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -Wall -Wextra -Werror \
        "${includes[@]}" "$oracle" -o "$TMP_DIR/$backend-sdk-abi-oracle" || oracle_status=$?
    if [ "$oracle_status" -eq 0 ]; then
        capture "$backend-sdk-abi-run" native-cpu compile-only \
            'Executed actual C SDK ABI oracle, independently of Seen declarations' \
            timeout --foreground --kill-after=5s 30s "$TMP_DIR/$backend-sdk-abi-oracle" || :
    fi
done
# SDK-only ASan leak baselines for the owner-approved scoped driver attribution.
# They link only the installed driver/loader; a failed build leaves the address
# rows under unchanged strict leak checking.
for backend in cuda vulkan; do
    control="$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/driver_leak_controls/vulkan_instance_control.c"
    includes=()
    library=-lvulkan
    if [ "$backend" = cuda ]; then
        control="$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/driver_leak_controls/cuda_init_control.c"
        includes=("${cuda_include[@]}")
        library=-lcuda
    fi
    capture "$backend-driver-leak-control-build" native-cpu compile-only \
        'SDK-only ASan driver leak baseline; same-run bound for scoped driver attribution, not GPU execution' \
        timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -fno-omit-frame-pointer -fsanitize=address \
        -Wall -Wextra -Werror "${includes[@]}" "$control" "$library" -o "$TMP_DIR/$backend-driver-leak-control" || :
done
# Compile the H tracing SDK interceptor with exactly H's warning, optimization,
# linking and ASan flags. It forwards real NVIDIA calls and records hot owners.
for variant in plain address; do
    flags=()
    destination="$TMP_DIR/vulkan-fault-driver.so"
    if [ "$variant" = address ]; then
        flags=(-fno-omit-frame-pointer -fsanitize=address)
        destination="$TMP_DIR/vulkan-fault-driver-address.so"
    fi
    capture "vulkan-trace-driver-$variant" native-cpu compile-only \
        'Exact Runtime-H trace driver build, real SDK forwarding and native hot-path interception' \
        timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -Wall -Wextra -Werror \
        -fPIC -shared "${flags[@]}" "$ROOT_DIR/tests/production_gpu_runtime/vulkan_trace_fault_driver.c" \
        -ldl -o "$destination" || :
done
SHARED="$TMP_DIR/shared-foundation"
mkdir -- "$SHARED"
SOURCE="$ROOT_DIR/tests/production_gpu_runtime/vulkan_foundation_kernel.seen"
cp -- "$SOURCE" "$SHARED/source.seen"
sha256sum -- "$SHARED/source.seen" > "$SHARED/source.sha256"
for specification in spirv:vulkan1.2:spv cuda:sm_89:ptx; do
    IFS=: read -r backend target extension <<< "$specification"
    status=0
    run_compiler gpu emit "$SOURCE" --gpu-backend "$backend" --gpu-target "$target" \
        --gpu-entry foundation --gpu-fallback error --module-id p0-accel-foundation \
        --artifact-info --output "$SHARED/$backend.$extension" --reflection "$SHARED/$backend.json" \
        > "$SHARED/$backend.log" 2>&1 || status=$?
    printf '%s\n' "$status" > "$SHARED/$backend.status"
    logs=("shared-foundation/$backend.log" "shared-foundation/$backend.status")
    for artifact in "$backend.$extension" "$backend.json"; do
        if [ -f "$SHARED/$artifact" ]; then logs+=("shared-foundation/$artifact"); fi
    done
    record "foundation-emit-$backend" "$status" native-cpu compile-only \
        'Same original foundation source/entry/module, target-specific emitted bytes and reflection; hardware requires native rows' "${logs[@]}"
done
capture foundation-logical-contract native-cpu compile-only \
    'Actual CUDA+SPIR-V artifact manifests, source hash, logical ABI and comparable canonical IR; not physical ABI equality' \
    python3 "$CHECKER" --root "$ROOT_DIR" --artifacts "$TMP_DIR" foundation || :
capture foundation-spirv-val official-validation compile-only \
    'Official spirv-val validates actual emitted Vulkan 1.2 module' \
    timeout --foreground --kill-after=5s 60s spirv-val --target-env vulkan1.2 "$SHARED/spirv.spv" || :
capture foundation-ptxas official-validation compile-only \
    'Policy-pinned official ptxas invoke/load_profile/require validates and assembles the actual sm_89 PTX' \
    timeout --foreground --kill-after=5s 120s python3 "$CHECKER" --root "$ROOT_DIR" --artifacts "$TMP_DIR" assemble || :
prepare_admission_probes() {
    python3 - "$SHARED/cuda.json" "$TMP_DIR" <<'PY'
import copy, json, sys
from pathlib import Path
original = json.loads(Path(sys.argv[1]).read_text())
for name, number in (("overflow", 1e100), ("fractional", 1.5)):
    value = copy.deepcopy(original)
    matches = [entry for entry in value["entries"] if entry["name"] == "foundation"]
    if len(matches) != 1:
        raise ValueError("foundation entry is not unique")
    matches[0]["required_capabilities"] = number
    (Path(sys.argv[2]) / f"cuda-admission-{name}.json").write_text(json.dumps(value, allow_nan=False))
PY
}
capture cuda-admission-fixtures native-cpu compile-only \
    'Known local reflection mutations exercise bounded numeric admission; these are not native GPU observations' \
    prepare_admission_probes || :
prerequisites_passed() {
    local file
    for file in "$@"; do
        [ -f "$file" ] && [ "$(<"$file")" = 0 ] || return 1
    done
}
cuda_inputs_ready=0
if prerequisites_passed "$TMP_DIR/cuda-sdk-abi-build.status" "$TMP_DIR/cuda-sdk-abi-run.status" \
    "$SHARED/cuda.status" "$TMP_DIR/foundation-logical-contract.status" "$TMP_DIR/foundation-ptxas.status"; then
    cuda_inputs_ready=1
fi
cuda_hardware_ready=0
if [ "$cuda_inputs_ready" = 1 ] && prerequisites_passed "$TMP_DIR/nvidia-inventory.status"; then cuda_hardware_ready=1; fi
cuda_admission_ready=0
if [ "$cuda_inputs_ready" = 1 ] && prerequisites_passed "$TMP_DIR/cuda-admission-fixtures.status"; then cuda_admission_ready=1; fi
vulkan_hardware_ready=0
if prerequisites_passed "$TMP_DIR/vulkan-sdk-abi-build.status" "$TMP_DIR/vulkan-sdk-abi-run.status" \
    "$SHARED/spirv.status" "$TMP_DIR/foundation-logical-contract.status" "$TMP_DIR/foundation-spirv-val.status" \
    "$TMP_DIR/nvidia-inventory.status" "$TMP_DIR/nvidia-icd.status" "$TMP_DIR/vulkan-trace-driver-plain.status"; then
    vulkan_hardware_ready=1
fi
# Preserve validator products in the report as well as on disk, without adding
# duplicate test rows for the same native invocation.
validator_artifacts=()
for artifact in hardware-foundation.json ptxas-identity.json foundation.cubin source.seen source.sha256; do
    if [ -f "$SHARED/$artifact" ]; then validator_artifacts+=("shared-foundation/$artifact"); fi
done
python3 - "$TMP_DIR/checks.json" "${validator_artifacts[@]}" <<'PY'
import json, sys
from pathlib import Path
path = Path(sys.argv[1]); rows = json.loads(path.read_text())
for row in rows:
    if row['name'] == 'foundation-logical-contract':
        row['logs'].extend(sys.argv[2:])
path.write_text(json.dumps(rows, indent=2) + '\n')
PY
# Known NVIDIA libcuda and Vulkan loader/ICD/dbus leaks are accepted only through
# the owner-approved scoped attribution: allocated by approved driver modules and
# no larger than the same-run SDK-only control. Any other leak still fails.
for profile in fast release undefined address; do
    for backend in cuda vulkan; do
        name="$backend"
        # CUDA needs LLVM's reviewed application shadow-gap mapping, not the
        # default inaccessible gap. Bounds/UAF/poison/leak checks stay enabled.
        FIXTURE_ASAN_ADDRESS_LAYOUT=default
        if [ "$backend" = cuda ]; then FIXTURE_ASAN_ADDRESS_LAYOUT=cuda; fi
        FIXTURE_ASAN_DRIVER_LEAK_PROFILE=
        FIXTURE_ASAN_DRIVER_LEAK_CONTROL=
        if prerequisites_passed "$TMP_DIR/$backend-driver-leak-control-build.status"; then
            FIXTURE_ASAN_DRIVER_LEAK_PROFILE=$backend
            FIXTURE_ASAN_DRIVER_LEAK_CONTROL="$TMP_DIR/$backend-driver-leak-control"
        fi
        source="$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/cuda_foundation/src/main.seen"
        compile_status=0
        if [ "$backend" = vulkan ]; then
            source="$ROOT_DIR/tests/production_gpu_runtime/vulkan_trace_contract.seen"
            compile_vulkan_fixture "$profile" "$name" "$source" || compile_status=$?
        else
            compile_fixture "$profile" "$name" "$source" || compile_status=$?
        fi
        record "$backend-$profile-compile" "$compile_status" native-cpu compile-only \
            'Actual native canary compile; release includes thinLTO, undefined/address use the unchanged sanitizer policy' \
            "$name-$profile.compile.log" "$name-$profile.compile.status"
        status=0
        if [ "$compile_status" -ne 0 ]; then
            status=$compile_status
            printf 'Native execution not reached: compile status %s\n' "$compile_status" > "$TMP_DIR/$name-$profile.run.log"
        elif { [ "$backend" = cuda ] && [ "$cuda_hardware_ready" != 1 ]; } || \
            { [ "$backend" = vulkan ] && [ "$vulkan_hardware_ready" != 1 ]; }; then
            status=125
            printf 'Native execution blocked by failed SDK ABI, official artifact or physical-device prerequisite; no fallback\n' \
                > "$TMP_DIR/$name-$profile.run.log"
        elif [ "$backend" = vulkan ]; then
            icd=
            if [ -f "$TMP_DIR/nvidia-icd.path" ]; then IFS= read -r icd < "$TMP_DIR/nvidia-icd.path"; fi
            if [ -z "$icd" ] || [ ! -f "$TMP_DIR/vulkan-fault-driver.so" ] || \
                { [ "$profile" = address ] && ! prerequisites_passed "$TMP_DIR/vulkan-trace-driver-address.status"; }; then
                status=125
                printf 'NVIDIA ICD or required exact H trace driver unavailable; no fallback\n' > "$TMP_DIR/$name-$profile.run.log"
            else
                FIXTURE_ASAN_PRELOAD_LIBRARY="$TMP_DIR/vulkan-fault-driver-address.so"
                VK_DRIVER_FILES="$icd" VK_ICD_FILENAMES="$icd" \
                    SEEN_VULKAN_FIXTURE_COPY_TRACE=1 SEEN_VULKAN_FIXTURE_STREAM_TRACE=1 \
                    SEEN_VULKAN_FIXTURE_FAULT=none LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
                    run_fixture "$profile" "$name" "$SHARED/spirv.spv" "$SHARED/spirv.json" rtx4090 reference || status=$?
                cp -- "$TMP_DIR/$name-$profile.log" "$TMP_DIR/$name-$profile.run.log"
            fi
        else
            FIXTURE_ASAN_PRELOAD_LIBRARY=
            run_fixture "$profile" "$name" "$SHARED/foundation.cubin" "$SHARED/cuda.json" || status=$?
            cp -- "$TMP_DIR/$name-$profile.log" "$TMP_DIR/$name-$profile.run.log"
        fi
        # Persist native status BEFORE parsing: parser failure cannot erase a
        # driver abort, timeout or unsuppressed LeakSanitizer failure.
        printf '%s\n' "$status" > "$TMP_DIR/$name-$profile.run.status"
        checker_status=0
        python3 "$CHECKER" --root "$ROOT_DIR" --artifacts "$TMP_DIR" observe \
            --backend "$backend" --profile "$profile" > "$TMP_DIR/$name-$profile.observer.log" 2>&1 || checker_status=$?
        # Expected native/validation failures already have exactly one hardware
        # row. A checker infrastructure failure must also become required red.
        if [ ! -f "$TMP_DIR/$name-$profile.hardware.json" ]; then
            record "$name-$profile-observer-failure" "${checker_status:-126}" none unsupported \
                'Hardware checker infrastructure failed; raw native status and log remain authoritative' \
                "$name-$profile.observer.log" "$name-$profile.run.log" "$name-$profile.run.status"
        fi
        if [ "$backend" = cuda ] && [ "$compile_status" -eq 0 ] && [ "$cuda_admission_ready" = 1 ]; then
            # Reuse the exact compiled bytes, with separate working/result names
            # so adverse runs cannot overwrite the positive hardware witness.
            ln -- "$TMP_DIR/cuda-$profile" "$TMP_DIR/cuda-admission-$profile"
            cp -- "$TMP_DIR/cuda-$profile.compile.log" "$TMP_DIR/cuda-admission-$profile.compile.log"
            cp -- "$TMP_DIR/cuda-$profile.compile.status" "$TMP_DIR/cuda-admission-$profile.compile.status"
            for mode in missing directory overflow fractional; do
                input="$TMP_DIR/cuda-admission-$mode.json"
                if [ "$mode" = directory ]; then input="$TMP_DIR"; fi
                native_status=0
                FIXTURE_ASAN_PRELOAD_LIBRARY=
                run_fixture "$profile" cuda-admission "$SHARED/foundation.cubin" "$input" || native_status=$?
                probe="cuda-$profile-admission-$mode"
                cp -- "$TMP_DIR/cuda-admission-$profile.log" "$TMP_DIR/$probe.log"
                printf '%s\n' "$native_status" > "$TMP_DIR/$probe.status"
                observed=0
                python3 - "$TMP_DIR/$probe.log" "$native_status" <<'PY' || observed=$?
import sys
from pathlib import Path
if int(sys.argv[2]) != 1:
    raise SystemExit("cold admission must reject with native exit 1, not crash, leak-abort or timeout")
if any(line.startswith("GPU_FOUNDATION_CUDA_EVIDENCE ") for line in Path(sys.argv[1]).read_text().splitlines()):
    raise SystemExit("invalid reflection produced successful GPU evidence")
PY
                logs=("$probe.log" "$probe.status")
                if [ -f "$input" ]; then logs+=("${input#"$TMP_DIR"/}"); fi
                record "$probe" "$observed" native-cpu verified \
                    'Same compiled canary must reject missing/directory/out-of-range/nonintegral reflection with exit 1 and exact Seen owner/atomic drain; no GPU execution claim' \
                    "${logs[@]}"
            done
        fi
    done
done
