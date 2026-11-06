#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-e
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-vulkan-pipeline.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-vulkan-pipeline
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
seen_prepare_vulkan_reference
export SEEN_VULKAN_FIXTURE_COPY_TRACE=1 SEEN_VULKAN_FIXTURE_STREAM_TRACE=1
cache_fault_driver="$TMP_DIR/cache-fault-driver.so"
timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -fPIC -shared -Wall -Wextra -Werror \
    "$ROOT_DIR/tests/production_gpu_runtime/vulkan_cache_file_fault_driver.c" -o "$cache_fault_driver"
cache_read_driver="$TMP_DIR/cache-read-driver.so"
timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -fPIC -shared -Wall -Wextra -Werror \
    "$ROOT_DIR/tests/production_gpu_runtime/vulkan_cache_read_fault_driver.c" -o "$cache_read_driver"
run_cache_fault() {
    local profile=$1 name=$2 mode=$3 destination="$TMP_DIR/cache-$2-$1-$3"
    if [ "$profile" = address ]; then
        SEEN_CACHE_FAULT_MODE="$mode" SEEN_CACHE_FAULT_STAGE="$destination.pending" \
        SEEN_CACHE_FAULT_DIRECTORY="$TMP_DIR" FIXTURE_ASAN_PRELOAD_LIBRARY="$cache_fault_driver" \
            run_fixture "$profile" "$name" "$destination" "$mode"
    else
        cp -- "$TMP_DIR/$name-$profile.compile.log" "$TMP_DIR/00-active.log"
        timeout --foreground --kill-after=5s 60s env UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
            SEEN_CACHE_FAULT_MODE="$mode" SEEN_CACHE_FAULT_STAGE="$destination.pending" \
            SEEN_CACHE_FAULT_DIRECTORY="$TMP_DIR" LD_PRELOAD="$cache_fault_driver" \
            "$TMP_DIR/$name-$profile" "$destination" "$mode" >> "$TMP_DIR/00-active.log" 2>&1
    fi
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/$name-$profile-$mode.log"
}
cache_read_modes=(metadata-error pread-error pread-unavailable pread-eof pread-short pread-eintr close-error read-close-error expire-partial expire-close)
pipeline_fault_modes=(pipeline-cache-status pipeline-cache-empty pipeline-shader-status pipeline-shader-empty pipeline-set-layout-status pipeline-set-layout-empty pipeline-layout-status pipeline-layout-empty pipeline-create-status pipeline-create-empty pipeline-create-partial pipeline-descriptor-pool-status pipeline-descriptor-pool-empty pipeline-descriptors-status pipeline-descriptors-empty copy-pool-status copy-pool-empty copy-commands-status copy-command-empty pipeline-cache-measure-status pipeline-cache-measure-incomplete pipeline-cache-measure-short pipeline-cache-measure-oversized pipeline-cache-capture-status pipeline-cache-capture-incomplete pipeline-cache-capture-short pipeline-cache-capture-oversized pipeline-ok)
run_cache_read() {
    local profile=$1 name=$2 mode=$3 destination="$TMP_DIR/read-$2-$1-$3"
    if [ "$profile" = address ]; then
        SEEN_CACHE_READ_TARGET="$destination" SEEN_CACHE_READ_MODE="$mode" \
        FIXTURE_ASAN_PRELOAD_LIBRARY="$cache_read_driver" \
            run_fixture "$profile" "$name" "$destination" "$mode"
    else
        cp -- "$TMP_DIR/$name-$profile.compile.log" "$TMP_DIR/00-active.log"
        timeout --foreground --kill-after=5s 60s env UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
            SEEN_CACHE_READ_TARGET="$destination" SEEN_CACHE_READ_MODE="$mode" \
            LD_PRELOAD="$cache_read_driver" "$TMP_DIR/$name-$profile" "$destination" "$mode" \
            >> "$TMP_DIR/00-active.log" 2>&1
    fi
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/$name-$profile-$mode.log"
}
KERNEL="$ROOT_DIR/tests/production_gpu_runtime/vulkan_foundation_kernel.seen"
for target in vulkan1.2 vulkan1.3; do
    run_compiler gpu emit "$KERNEL" --gpu-backend spirv --gpu-target "$target" \
        --output "$TMP_DIR/$target.spv" --reflection "$TMP_DIR/$target.json" > "$TMP_DIR/emit-$target.log" 2>&1
    spirv-val --target-env "$target" "$TMP_DIR/$target.spv"
done
for profile in fast release undefined address; do
    compile_fixture "$profile" cache-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_cache_file_fault_contract.seen"
    for mode in expire sync-error close-error directory-error fstat-error; do
        run_cache_fault "$profile" cache-fault "$mode"
    done
    compile_fixture "$profile" cache-budget "$ROOT_DIR/tests/production_gpu_runtime/vulkan_cache_budget_contract.seen"
    mkfifo -- "$TMP_DIR/cache-fifo-$profile"
    run_fixture "$profile" cache-budget "$TMP_DIR/cache-budget-$profile.data" "$TMP_DIR/cache-fifo-$profile"
    compile_fixture "$profile" cache-adverse "$ROOT_DIR/tests/production_gpu_runtime/vulkan_cache_adverse_contract.seen"
    run_fixture "$profile" cache-adverse "$TMP_DIR/cache-codec-$profile" contract
    for mode in "${cache_read_modes[@]}"; do run_cache_read "$profile" cache-adverse "$mode"; done
    compile_fixture "$profile" artifact-adverse "$ROOT_DIR/tests/production_gpu_runtime/vulkan_artifact_adverse_contract.seen"
    run_fixture "$profile" artifact-adverse "$TMP_DIR/vulkan1.2.spv" "$TMP_DIR/vulkan1.2.json"
    compile_fixture "$profile" pipeline-import "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_import_contract.seen"
    run_fixture "$profile" pipeline-import
    readelf -d "$TMP_DIR/pipeline-import-$profile" > "$TMP_DIR/import-$profile.dynamic"
    if rg -i 'NEEDED.*(vulkan|cuda|cudart|hip|metal)' "$TMP_DIR/import-$profile.dynamic"; then
        echo 'FAIL: unused native pipeline imports linked a GPU SDK' >&2; exit 1
    fi
    compile_fixture "$profile" pipeline-artifact "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_artifact_contract.seen"
    for target in vulkan1.2 vulkan1.3; do
        run_fixture "$profile" pipeline-artifact "$TMP_DIR/$target.spv" "$TMP_DIR/$target.json" contract
    done
    compile_vulkan_fixture "$profile" pipeline "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_contract.seen"
    for target in vulkan1.2 vulkan1.3; do
        seen_run_vulkan_reference "$profile" pipeline "$TMP_DIR/$target.spv" "$TMP_DIR/$target.json" "$TMP_DIR/cache-$target-$profile" software-reference
    done
    compile_vulkan_fixture "$profile" pipeline-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_fault_contract.seen"
    for mode in "${pipeline_fault_modes[@]}"; do
        seen_run_vulkan_reference "$profile" pipeline-fault "$TMP_DIR/vulkan1.2.spv" "$TMP_DIR/vulkan1.2.json" "$mode" software-reference
        cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/pipeline-fault-$profile-$mode.log"
    done
done
FIXTURE_COMPILE_FLAGS=(--coverage)
compile_fixture fast pipeline-artifact-coverage "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_artifact_contract.seen"
LLVM_PROFILE_FILE="$TMP_DIR/artifact.profraw" run_fixture fast pipeline-artifact-coverage "$TMP_DIR/vulkan1.2.spv" "$TMP_DIR/vulkan1.2.json" fuzz
compile_vulkan_fixture fast pipeline-coverage "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_contract.seen"
LLVM_PROFILE_FILE="$TMP_DIR/native.profraw" seen_run_vulkan_reference fast pipeline-coverage "$TMP_DIR/vulkan1.2.spv" "$TMP_DIR/vulkan1.2.json" "$TMP_DIR/cache-coverage" software-reference
compile_fixture fast cache-fault-coverage "$ROOT_DIR/tests/production_gpu_runtime/vulkan_cache_file_fault_contract.seen"
for mode in expire sync-error close-error directory-error fstat-error; do
    LLVM_PROFILE_FILE="$TMP_DIR/cache-$mode.profraw" run_cache_fault fast cache-fault-coverage "$mode"
done
compile_fixture fast cache-budget-coverage "$ROOT_DIR/tests/production_gpu_runtime/vulkan_cache_budget_contract.seen"
mkfifo -- "$TMP_DIR/cache-fifo-coverage"
LLVM_PROFILE_FILE="$TMP_DIR/cache-budget.profraw" run_fixture fast cache-budget-coverage \
    "$TMP_DIR/cache-budget-coverage.data" "$TMP_DIR/cache-fifo-coverage"
compile_fixture fast artifact-adverse-coverage "$ROOT_DIR/tests/production_gpu_runtime/vulkan_artifact_adverse_contract.seen"
LLVM_PROFILE_FILE="$TMP_DIR/artifact-adverse.profraw" run_fixture fast artifact-adverse-coverage "$TMP_DIR/vulkan1.2.spv" "$TMP_DIR/vulkan1.2.json"
compile_fixture fast cache-adverse-coverage "$ROOT_DIR/tests/production_gpu_runtime/vulkan_cache_adverse_contract.seen"
LLVM_PROFILE_FILE="$TMP_DIR/cache-codec.profraw" run_fixture fast cache-adverse-coverage "$TMP_DIR/cache-codec-coverage" contract
cache_read_profiles=("$TMP_DIR/cache-codec.profraw")
for mode in "${cache_read_modes[@]}"; do
    LLVM_PROFILE_FILE="$TMP_DIR/cache-read-$mode.profraw" run_cache_read fast cache-adverse-coverage "$mode"
    cache_read_profiles+=("$TMP_DIR/cache-read-$mode.profraw")
done
seen_source_coverage_tool llvm-profdata merge -sparse --num-threads=1 "${cache_read_profiles[@]}" -o "$TMP_DIR/cache-adverse.profdata"
compile_vulkan_fixture fast pipeline-fault-coverage "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_fault_contract.seen"
pipeline_fault_profiles=()
for mode in "${pipeline_fault_modes[@]}"; do
    LLVM_PROFILE_FILE="$TMP_DIR/pipeline-fault-$mode.profraw" seen_run_vulkan_reference fast pipeline-fault-coverage \
        "$TMP_DIR/vulkan1.2.spv" "$TMP_DIR/vulkan1.2.json" "$mode" software-reference
    pipeline_fault_profiles+=("$TMP_DIR/pipeline-fault-$mode.profraw")
done
seen_source_coverage_tool llvm-profdata merge -sparse --num-threads=1 "${pipeline_fault_profiles[@]}" -o "$TMP_DIR/pipeline-fault.profdata"
seen_source_coverage_tool llvm-profdata merge -sparse --num-threads=1 \
    "$TMP_DIR/cache-expire.profraw" "$TMP_DIR/cache-sync-error.profraw" \
    "$TMP_DIR/cache-close-error.profraw" "$TMP_DIR/cache-directory-error.profraw" \
    "$TMP_DIR/cache-fstat-error.profraw" -o "$TMP_DIR/cache-fault.profdata"
coverage_status=0
seen_verify_source_coverage "$TMP_DIR/pipeline" \
    --fixture "$TMP_DIR/pipeline-artifact-coverage-fast" "$TMP_DIR/artifact.profraw" \
    --fixture "$TMP_DIR/pipeline-coverage-fast" "$TMP_DIR/native.profraw" \
    --fixture "$TMP_DIR/cache-fault-coverage-fast" "$TMP_DIR/cache-fault.profdata" \
    --fixture "$TMP_DIR/cache-budget-coverage-fast" "$TMP_DIR/cache-budget.profraw" \
    --fixture "$TMP_DIR/artifact-adverse-coverage-fast" "$TMP_DIR/artifact-adverse.profraw" \
    --fixture "$TMP_DIR/cache-adverse-coverage-fast" "$TMP_DIR/cache-adverse.profdata" \
    --fixture "$TMP_DIR/pipeline-fault-coverage-fast" "$TMP_DIR/pipeline-fault.profdata" \
    --source seen_std/src/compute/vulkan_pipeline_artifact.seen \
    --source seen_std/src/compute/vulkan_pipeline_cache.seen \
    --source seen_std/src/compute/vulkan_pipeline_cache_file.seen \
    --source seen_std/src/compute/vulkan_pipeline_device.seen \
    --source seen_std/src/compute/vulkan_pipeline.seen \
    --source seen_std/src/compute/vulkan_pipeline_dispatch.seen || coverage_status=$?
python3 - "$TMP_DIR/pipeline-coverage.json" "$TMP_DIR/pipeline-coverage-branches.json" <<'PY'
import json,pathlib,sys
report=json.loads(pathlib.Path(sys.argv[1]).read_text())
scope={'vulkan_pipeline_artifact.seen','vulkan_pipeline_cache.seen','vulkan_pipeline_cache_file.seen',
       'vulkan_pipeline_device.seen','vulkan_pipeline.seen','vulkan_pipeline_dispatch.seen'}
files=[{'filename':item['filename'],'branches':item.get('branches',[])}
       for unit in report['data'] for item in unit['files']
       if pathlib.Path(item['filename']).name in scope]
pathlib.Path(sys.argv[2]).write_text(json.dumps({'files':files},separators=(',',':'))+'\n')
PY
[ "$coverage_status" = 0 ] || exit "$coverage_status"
python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" "$TMP_DIR/pipeline-coverage.json" \
    --source seen_std/src/compute/vulkan_pipeline_artifact.seen --min-lines 90 --min-branches 90 \
    > "$TMP_DIR/artifact-parser-coverage-summary.json"
python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" "$TMP_DIR/pipeline-coverage.json" \
    --source seen_std/src/compute/vulkan_pipeline_cache.seen --min-lines 90 --min-branches 90 \
    > "$TMP_DIR/cache-parser-coverage-summary.json"
# Real hardware is never a fallback for a missing software class, nor the reverse.
if [ "${SEEN_VULKAN_PIPELINE_HARDWARE:-}" = rtx4090 ]; then
    case "${SEEN_VULKAN_HARDWARE_ICD:-}" in /*) ;; *) echo 'FAIL: designated hardware requires an absolute ICD path' >&2; exit 1 ;; esac
    [ -f "$SEEN_VULKAN_HARDWARE_ICD" ] && [ ! -L "$SEEN_VULKAN_HARDWARE_ICD" ] || exit 1
    export VK_DRIVER_FILES="$SEEN_VULKAN_HARDWARE_ICD"
    FIXTURE_COMPILE_FLAGS=()
    compile_vulkan_fixture fast pipeline-hardware "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_contract.seen"
    seen_run_vulkan_reference fast pipeline-hardware "$TMP_DIR/vulkan1.2.spv" "$TMP_DIR/vulkan1.2.json" "$TMP_DIR/cache-hardware" rtx4090
elif [ -n "${SEEN_VULKAN_PIPELINE_HARDWARE:-}" ]; then
    echo 'FAIL: unknown designated pipeline hardware class' >&2; exit 1
fi
echo 'PASS: reflected native Vulkan pipeline dispatch cache four-profile software-reference and parser ownership gates'
