#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-p0-accel-gate-a
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "${SEEN_ARTIFACT_ROOT:?}/p0-accel-a.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=p0-accel-a
FIXTURE_RUN_SECONDS=120
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
FIXTURES="$ROOT_DIR/tests/expanded_gate_0_accelerator_certification/cpu_isolation"
PROBE="$FIXTURES/probe_evidence.py"
printf '[]\n' > "$TMP_DIR/checks.json"
# Bound every raw trace/file; retain actual evidence on success and failure.
ulimit -S -f 65536
record() {
    local name=$1 status=$2 execution=$3 classification=$4 detail=$5
    shift 5
    python3 "$PROBE" record --checks "$TMP_DIR/checks.json" --name "$name" \
        --status "$status" --execution "$execution" --classification "$classification" \
        --detail "$detail" "$@"
}
finish() {
    local status=$? report_status=0 retention_status=0
    trap - EXIT
    if [ "$status" -ne 0 ]; then
        printf 'Harness exit status: %s\n' "$status" > "$TMP_DIR/harness-exit.log"
        record harness-exit "$status" none unsupported 'Harness or required observer failure; no discovery-isolation claim' harness-exit.log || :
    fi
    python3 "$PROBE" retain-traces --checks "$TMP_DIR/checks.json" \
        > "$TMP_DIR/trace-retention.json" 2>&1 || retention_status=$?
    record trace-retention "$retention_status" official-validation verified \
        'Raw syscall traces compressed before manifest publication with raw/compressed SHA-256 and explicit retention bounds; overflow fails, never implies no probes' \
        trace-retention.json || retention_status=$?
    if [ "$status" -eq 0 ] && [ "$retention_status" -ne 0 ]; then status=$retention_status; fi
    python3 "$ROOT_DIR/scripts/accelerator_gate_evidence.py" --phase a \
        --compiler "$COMPILER" --root "$ROOT_DIR" --artifacts "$TMP_DIR" \
        --checks "$TMP_DIR/checks.json" --output "$TMP_DIR/accelerator-a-evidence.json" || report_status=$?
    printf 'CPU isolation evidence retained: %s\n' "$TMP_DIR" >&2
    if [ "$status" -ne 0 ]; then exit "$status"; fi
    exit "$report_status"
}
trap finish EXIT
status=0
STRACE="${SEEN_STRACE:-$(command -v strace || true)}"
if [[ "$STRACE" != /* ]] || [ ! -x "$STRACE" ]; then status=1; fi
printf 'strace observer: %s\n' "$STRACE" >> "$TMP_DIR/prerequisites.log"
for tool in cmake readelf python3 cat; do
    command -v "$tool" >> "$TMP_DIR/prerequisites.log" 2>&1 || status=$?
done
if [ "$status" -eq 0 ]; then
    timeout --foreground --kill-after=2s 5s "$STRACE" --version >> "$TMP_DIR/prerequisites.log" 2>&1 || status=$?
    sha256sum -- "$STRACE" >> "$TMP_DIR/prerequisites.log" 2>&1 || status=$?
fi
record observer-prerequisites "$status" official-validation compile-only \
    'strace is required; missing tools or denied ptrace fail, never imply zero SDK probes' prerequisites.log
[ "$status" -eq 0 ] || exit 126
CAT="$(command -v cat)"
status=0
TMPDIR="$TMP_DIR" python3 "$PROBE" contract > "$TMP_DIR/observer-contract.json" 2>&1 || status=$?
record observer-contract "$status" official-validation verified \
    'Behavioral parser negatives distinguish ordinary failed host PATH search from real SDK probes; synthetic cases are not hardware observation' \
    observer-contract.json
[ "$status" -eq 0 ] || exit 1
POISON="$TMP_DIR/poison-sdk"
TRAPS="$TMP_DIR/tool-traps"
mkdir -p -- "$POISON" "$TRAPS"
export SEEN_CPU_ISOLATION_REAL_PKG_CONFIG
SEEN_CPU_ISOLATION_REAL_PKG_CONFIG="$(command -v pkg-config || command -v pkgconf || true)"
for tool in nvcc ptxas nvlink fatbinary cuobjdump nvidia-smi glslc glslangValidator \
    spirv-as spirv-val hipcc hipconfig rocminfo rocm-smi metal metallib xcrun pkg-config pkgconf; do
    cp -- "$FIXTURES/tool_trap.sh" "$TRAPS/$tool"
    chmod 0700 "$TRAPS/$tool"
done
# The attested bounded toolchain must stay first. Any GPU tool there (e.g.
# glslc) is still observed by strace; no shared toolchain file is changed.
export PATH="${SEEN_BOUNDED_TOOLCHAIN_DIR:?}:$TRAPS:$PATH"
export SEEN_ATTESTED_SEEN_PATH="$PATH"
export CUDA_HOME="$POISON" CUDA_PATH="$POISON" CUDA_ROOT="$POISON"
export CUDAToolkit_ROOT="$POISON" CUDACXX="$POISON/bin/nvcc"
export CMAKE_CUDA_COMPILER="$POISON/bin/nvcc"
export VULKAN_SDK="$POISON" VK_ICD_FILENAMES="$POISON/invalid-icd.json"
export VK_DRIVER_FILES="$POISON/invalid-icd.json" VK_LAYER_PATH="$POISON/layers"
export HIP_PATH="$POISON" ROCM_PATH="$POISON"
python3 - "$TMP_DIR/environment.json" <<'PY'
import json, os, sys
keys = ('CUDA_HOME', 'CUDA_PATH', 'CUDA_ROOT', 'CUDAToolkit_ROOT', 'CUDACXX',
        'CMAKE_CUDA_COMPILER', 'VULKAN_SDK', 'VK_ICD_FILENAMES',
        'VK_DRIVER_FILES', 'VK_LAYER_PATH', 'HIP_PATH', 'ROCM_PATH',
        'SEEN_ATTESTED_SEEN_PATH')
with open(sys.argv[1], 'w', encoding='utf-8') as stream:
    json.dump({key: os.environ[key] for key in keys}, stream, indent=2)
    stream.write('\n')
PY
trace_command() {
    local trace=$1 seconds=$2
    shift 2
    timeout --foreground --kill-after=5s "${seconds}s" "$STRACE" -f -s 4096 \
        -e trace=%file,%process -o "$trace" -- "$@"
}
# Calibrate both an attempted SDK file probe and an actually executed tool
# tripwire before treating an empty finding set as meaningful observation.
export SEEN_CPU_ISOLATION_TOOL_LOG="$TMP_DIR/control-tool-trap.log"
: > "$SEEN_CPU_ISOLATION_TOOL_LOG"
status=0
trace_command "$TMP_DIR/control-tool.trace" 15 "$TRAPS/nvcc" --version \
    > "$TMP_DIR/control-tool.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$TMP_DIR/control-tool.status"
observed=0
python3 "$PROBE" trace --trace "$TMP_DIR/control-tool.trace" --poison "$POISON" \
    --expect-exec "$TRAPS/nvcc" --expect-detection --trap-log "$SEEN_CPU_ISOLATION_TOOL_LOG" \
    > "$TMP_DIR/control-tool-observation.json" 2>&1 || observed=$?
if [ "$status" -ne 97 ] || [ ! -s "$SEEN_CPU_ISOLATION_TOOL_LOG" ]; then observed=1; fi
record observer-tool-positive-control "$observed" official-validation verified \
    'Actual rejecting tool invocation must be observed; raw command exit must be 97' \
    control-tool.trace control-tool.log control-tool.status control-tool-trap.log control-tool-observation.json
[ "$observed" -eq 0 ] || exit 1
status=0
trace_command "$TMP_DIR/control-file.trace" 15 "$CAT" "$POISON/include/cuda.h" \
    > "$TMP_DIR/control-file.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$TMP_DIR/control-file.status"
observed=0
python3 "$PROBE" trace --trace "$TMP_DIR/control-file.trace" --poison "$POISON" \
    --expect-exec "$CAT" --expect-detection > "$TMP_DIR/control-file-observation.json" 2>&1 || observed=$?
if [ "$status" -ne 1 ]; then observed=1; fi
record observer-file-positive-control "$observed" official-validation verified \
    'Actual failed SDK-header open must be observed; no host SDK is touched' \
    control-file.trace control-file.log control-file.status control-file-observation.json
[ "$observed" -eq 0 ] || exit 1
run_compiler() {
    trace_command "${TRACE_FILE:?}" 300 bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}
inspect_trace() {
    local name=$1 expected=$2
    local status=0
    python3 "$PROBE" trace --trace "$TMP_DIR/$name.trace" --poison "$POISON" \
        --expect-exec "$expected" --trap-log "$SEEN_CPU_ISOLATION_TOOL_LOG" \
        > "$TMP_DIR/$name-observation.json" 2>&1 || status=$?
    record "$name-observation" "$status" official-validation verified \
        'Observed Linux SDK file/tool probes, including failed lookup; source .seen imports and generic ld.so.cache are not SDK discovery' \
        "$name.trace" "$name-observation.json" "${SEEN_CPU_ISOLATION_TOOL_LOG##*/}"
}
inspect_elf() {
    local name=$1 binary=$2 status=0
    timeout --foreground --kill-after=5s 15s readelf -d "$binary" \
        > "$TMP_DIR/$name.dynamic" 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then
        python3 "$PROBE" elf --dynamic "$TMP_DIR/$name.dynamic" \
            > "$TMP_DIR/$name-linkage.json" 2>&1 || status=$?
    else
        printf 'readelf failed with status %s\n' "$status" > "$TMP_DIR/$name-linkage.json"
    fi
    record "$name-linkage" "$status" official-validation compile-only \
        'Actual readelf NEEDED entries, not proof of absence of SDK discovery' "$name.dynamic" "$name-linkage.json"
}
inspect_elf compiler "$COMPILER"
# Reuse the existing opt-in CUDA feature-disabled regression against its real
# CMake source, with a fresh build directory, actual probe tracing and traps.
export SEEN_CPU_ISOLATION_TOOL_LOG="$TMP_DIR/feature-disabled-tool-trap.log"
: > "$SEEN_CPU_ISOLATION_TOOL_LOG"
status=0
trace_command "$TMP_DIR/feature-disabled.trace" 120 cmake \
    -S "$ROOT_DIR/seen_runtime/cuda" -B "$TMP_DIR/cuda-disabled" -DSEEN_ENABLE_CUDA=OFF \
    > "$TMP_DIR/feature-disabled.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$TMP_DIR/feature-disabled.status"
accepted=0
if [ "$status" -ne 1 ] || ! grep -Fq 'Seen CUDA discovery is gated' "$TMP_DIR/feature-disabled.log"; then accepted=1; fi
record feature-disabled "$accepted" official-validation compile-only \
    'Existing explicit SEEN_ENABLE_CUDA=OFF must reject before CUDA SDK/tool discovery; raw exit status retained' \
    feature-disabled.log feature-disabled.status environment.json
inspect_trace feature-disabled "$(command -v cmake)"
# Existing import witnesses plus real CPU arithmetic, typed errors, catalog
# ownership/snapshots. Success depends on native exit/assertions, never PASS text.
names=(cpu-contract cuda-import vulkan-import backend error discovery)
sources=("$FIXTURES/cpu_contract.seen"
    "$ROOT_DIR/tests/production_gpu_runtime/cuda_peer_import_cpu.seen"
    "$ROOT_DIR/tests/production_gpu_runtime/vulkan_pipeline_import_contract.seen"
    "$ROOT_DIR/tests/production_gpu_runtime/backend_contract.seen"
    "$ROOT_DIR/tests/production_gpu_runtime/error_contract.seen"
    "$ROOT_DIR/tests/production_gpu_runtime/discovery_contract.seen")
for profile in fast release undefined address; do
    for index in "${!names[@]}"; do
        name=${names[$index]}; id="$name-$profile"
        export SEEN_CPU_ISOLATION_TOOL_LOG="$TMP_DIR/$id-compile-tool-trap.log"
        : > "$SEEN_CPU_ISOLATION_TOOL_LOG"
        TRACE_FILE="$TMP_DIR/$id-compile.trace"
        status=0
        compile_fixture "$profile" "$name" "${sources[$index]}" || status=$?
        record "$id-compile" "$status" official-validation compile-only \
            'Actual attested CPU-default compile with poisoned GPU SDK environment and rejecting tool traps; no accelerator flags' \
            "$id.compile.log" "$id.compile.status" environment.json
        inspect_trace "$id-compile" "$COMPILER"
        if [ "$status" -ne 0 ]; then continue; fi
        inspect_elf "$id" "$TMP_DIR/$id"
        export SEEN_CPU_ISOLATION_TOOL_LOG="$TMP_DIR/$id-run-tool-trap.log"
        : > "$SEEN_CPU_ISOLATION_TOOL_LOG"
        status=0
        if [ "$profile" = address ]; then
            # LSan cannot operate under ptrace. Preserve its normal leak checks,
            # existing hard-scope runner and native result; do not disable leaks.
            run_fixture "$profile" "$name" || status=$?
            boundary='Native ASan/LSan in approved hard scope with tool traps; runtime filesystem/dlopen probes unobserved because LSan is incompatible with ptrace. Compile file/process probes are observed independently.'
        else
            UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1:print_stacktrace=0 \
                trace_command "$TMP_DIR/$id-run.trace" 120 "$TMP_DIR/$id" \
                > "$TMP_DIR/$id.log" 2>&1 || status=$?
            printf '%s\n' "$status" > "$TMP_DIR/$id.run.status"
            inspect_trace "$id-run" "$TMP_DIR/$id"
            boundary='Actual native CPU assertions and exit status under file/process observation; no GPU execution claim'
        fi
        observed=0
        python3 "$PROBE" tool-log --log "$SEEN_CPU_ISOLATION_TOOL_LOG" \
            > "$TMP_DIR/$id-run-tools.json" 2>&1 || observed=$?
        if [ "$status" -eq 0 ] && [ "$observed" -ne 0 ]; then status=$observed; fi
        record "$id-run" "$status" native-cpu verified "$boundary" \
            "$id.log" "$id.run.status" "$id-run-tool-trap.log" "$id-run-tools.json"
    done
done
# Reuse compiler ownership negatives, preserving actual statuses/diagnostics.
for kind in move share; do
    id="context-$kind-rejected"
    export SEEN_CPU_ISOLATION_TOOL_LOG="$TMP_DIR/$id-tool-trap.log"
    : > "$SEEN_CPU_ISOLATION_TOOL_LOG"
    TRACE_FILE="$TMP_DIR/$id.trace"
    status=0
    run_compiler check "$ROOT_DIR/tests/production_gpu_runtime/context_${kind}_rejected.seen" \
        > "$TMP_DIR/$id.log" 2>&1 || status=$?
    printf '%s\n' "$status" > "$TMP_DIR/$id.status"
    code=E_OWNERSHIP_USE_AFTER_MOVE
    if [ "$kind" = share ]; then code=E_TYPE_BOUND; fi
    accepted=0
    python3 "$PROBE" diagnostic --log "$TMP_DIR/$id.log" --status "$status" --code "$code" \
        > "$TMP_DIR/$id-diagnostic.json" 2>&1 || accepted=$?
    record "$id" "$accepted" official-validation compile-only \
        'Actual ownership rejection requires compiler exit 1 and the typed diagnostic, not timeout or containment failure' \
        "$id.log" "$id.status" "$id-diagnostic.json"
    inspect_trace "$id" "$COMPILER"
done
# The shared helper decides success from all required actual command records.
echo 'CPU-default isolation exercised; evidence distinguishes native CPU, SDK probe observation and ASan observation limits'
