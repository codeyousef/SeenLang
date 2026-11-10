#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-d
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-vulkan-stream.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-vulkan-stream
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/source_coverage_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
export SEEN_VULKAN_FIXTURE_COPY_TRACE=1 SEEN_VULKAN_FIXTURE_STREAM_TRACE=1
faults=(copy-semaphore-status copy-semaphore-empty timeline-none stream-submit-oom
    stream-submit-lost stream-submit-unknown copy-poll-oom copy-poll-timeout
    copy-poll-lost stream-post-submit-timeout)
codes=(gpu.out_of_memory gpu.invalid gpu.unsupported_capability gpu.out_of_memory
    gpu.device_lost gpu.unavailable gpu.out_of_memory none gpu.device_lost none)
run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/vulkan_stream_contract.seen" > "$TMP_DIR/frontend.log" 2>&1
for profile in fast release undefined address; do
    compile_vulkan_fixture "$profile" stream "$ROOT_DIR/tests/production_gpu_runtime/vulkan_stream_contract.seen"
    seen_run_vulkan_reference "$profile" stream
    rg -Fq 'VULKAN_STREAM_TRACE semaphore=2/2 submit=203 empty=203 waits=102' "$TMP_DIR/stream-$profile.log"
    compile_vulkan_fixture "$profile" stream-adverse "$ROOT_DIR/tests/production_gpu_runtime/vulkan_stream_adverse_contract.seen"
    seen_run_vulkan_reference "$profile" stream-adverse
    compile_vulkan_fixture "$profile" stream-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_stream_fault_contract.seen"
    for index in "${!faults[@]}"; do
        seen_run_vulkan_fault "$profile" stream-fault "${faults[index]}" "${codes[index]}"
    done
done
FIXTURE_COMPILE_FLAGS=(--coverage); coverage=()
for name in stream stream-adverse; do
    export LLVM_PROFILE_FILE="$TMP_DIR/$name.profraw"
    source_name=vulkan_stream_contract.seen
    if [ "$name" = stream-adverse ]; then source_name=vulkan_stream_adverse_contract.seen; fi
    compile_vulkan_fixture fast "coverage-$name" "$ROOT_DIR/tests/production_gpu_runtime/$source_name"
    seen_run_vulkan_reference fast "coverage-$name"
    coverage+=(--fixture "$TMP_DIR/coverage-$name-fast" "$TMP_DIR/$name.profraw")
done
compile_vulkan_fixture fast coverage-stream-fault "$ROOT_DIR/tests/production_gpu_runtime/vulkan_stream_fault_contract.seen"
for index in "${!faults[@]}"; do
    seen_run_vulkan_fault fast coverage-stream-fault "${faults[index]}" "${codes[index]}"
done
seen_source_coverage_tool llvm-profdata merge -sparse --num-threads=1 \
    "$TMP_DIR"/fault-coverage-stream-fault-*.profraw -o "$TMP_DIR/fault.profdata"
coverage+=(--fixture "$TMP_DIR/coverage-stream-fault-fast" "$TMP_DIR/fault.profdata")
seen_verify_source_coverage "$TMP_DIR/streams" "${coverage[@]}" \
    --source seen_std/src/compute/vulkan_stream.seen
FIXTURE_COMPILE_FLAGS=(); unset LLVM_PROFILE_FILE
for purpose in move share; do
    expected=E_OWNERSHIP_USE_AFTER_MOVE
    if [ "$purpose" = share ]; then expected=E_TYPE_BOUND; fi
    if run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/vulkan_stream_${purpose}_rejected.seen" > "$TMP_DIR/$purpose-rejected.log" 2>&1; then
        echo 'FAIL: native stream or event escaped move ownership or creator confinement' >&2; exit 1
    fi
    rg -Fq "$expected" "$TMP_DIR/$purpose-rejected.log"
done
echo 'PASS: bounded native Vulkan streams timeline events cross-stream ordering uncertain acceptance retry ownership and software-reference source coverage'
