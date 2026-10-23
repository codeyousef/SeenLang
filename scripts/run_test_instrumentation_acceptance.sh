#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
COMPILER="${1:-$ROOT/compiler_seen/target/seen}"

fail() { echo "ERROR: TEST-002A execution acceptance: $*" >&2; exit 1; }
[ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" = 1 ] || fail "verified hard memory scope is required"
[ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" = 1 ] || fail "project artifact wrapper is required"
[ "${SEEN_JOBS:-0}" = 1 ] && [ "${SEEN_OPT_JOBS:-0}" = 1 ] || fail "serial worker limits are required"
[ -x "$COMPILER" ] || fail "compiler is unavailable"
PACKAGE_CLIENT="${SEEN_PACKAGE_CLIENT:-$ROOT/compiler_seen/target/seen-pkg}"
[ -f "$PACKAGE_CLIENT" ] && [ -x "$PACKAGE_CLIENT" ] && [ ! -L "$PACKAGE_CLIENT" ] ||
    fail "the selected package client is unavailable or unsafe"
for tool in llvm-profdata llvm-cov; do command -v "$tool" >/dev/null || fail "$tool is unavailable"; done

RUN="$SEEN_ARTIFACT_ROOT/test-002a-execution"
mkdir -p -- "$RUN"
RUN_REL="${RUN#"$ROOT"/}"
[ "$RUN_REL" != "$RUN" ] || fail "artifact root is outside the checkout"

run_source_probe() {
    local name=$1 source=$2
    (
        cd "$ROOT"
        SEEN_COMPILER_SOURCE_ROOT="$ROOT" SEEN_PACKAGE_CLIENT="$PACKAGE_CLIENT" \
            "$COMPILER" compile "$source" "$RUN/$name" \
            --fast --no-cache --coverage --no-fork
    ) >"$RUN/$name-build.log" 2>&1
    LLVM_PROFILE_FILE="$RUN/$name.profraw" \
        timeout --kill-after=5s 120s "$RUN/$name" >"$RUN/$name.log" 2>&1
    llvm-profdata merge -sparse --num-threads=1 "$RUN/$name.profraw" -o "$RUN/$name.profdata"
    llvm-cov export --num-threads=1 "$RUN/$name" -instr-profile "$RUN/$name.profdata" >"$RUN/$name.json"
}
SOURCE_FIXTURE="$ROOT/tests/compiler_regressions/source_coverage_locations.seen"
run_source_probe source-coverage-locations "$SOURCE_FIXTURE"
python3 "$ROOT/scripts/check_seen_source_coverage.py" "$RUN/source-coverage-locations.json" "$SOURCE_FIXTURE"
run_source_probe source-coverage-encoding "$ROOT/compiler_seen/tests/source_coverage_encoding.seen"
run_source_probe source-coverage-scopes "$ROOT/compiler_seen/tests/source_coverage_scopes.seen"

# Building the instrumented host uses the bounded serial fork path so temporary
# module heaps are reclaimed. The instrumented host itself compiles its GPU
# probe without forks, keeping the recorded execution counts unambiguous.
HOST="$RUN/seen-instrumented"
HOST_BUILD="$RUN/compiler-build.json"
HOST_BUILD_REL="$RUN_REL/compiler-build.json"
HOST_LOG="$RUN/compiler.log"
HOST_RAW="$RUN/compiler-%p.profraw"
HOST_PROFILE="$RUN/compiler.profdata"
HOST_COVERAGE="$RUN/compiler.coverage.txt"
GPU="$RUN/gpu-emitter-test"
GPU_BUILD="$RUN/gpu-build.json"
GPU_BUILD_REL="$RUN_REL/gpu-build.json"
GPU_LOG="$RUN/gpu.log"
GPU_RAW="$RUN/gpu-%p.profraw"
GPU_PROFILE="$RUN/gpu.profdata"
GPU_COVERAGE="$RUN/gpu.coverage.txt"

(
    cd "$ROOT"
    SEEN_COMPILER_SOURCE_ROOT="$ROOT" SEEN_PACKAGE_CLIENT="$PACKAGE_CLIENT" \
        "$COMPILER" compile compiler_seen/src/main_compiler.seen "$HOST" \
        --fast --no-cache --coverage --sanitize undefined \
        --instrumentation-report "$HOST_BUILD_REL"
) >"$HOST_LOG" 2>&1

LLVM_PROFILE_FILE="$HOST_RAW" "$HOST" --version >>"$HOST_LOG" 2>&1
(
    cd "$ROOT"
    LLVM_PROFILE_FILE="$HOST_RAW" SEEN_COMPILER_SOURCE_ROOT="$ROOT" \
        SEEN_PACKAGE_CLIENT="$PACKAGE_CLIENT" \
        "$HOST" compile compiler_seen/tests/test_002a_gpu_emitters.seen "$GPU" \
        --fast --no-cache --coverage --sanitize undefined \
        --instrumentation-report "$GPU_BUILD_REL" --no-fork
) >>"$HOST_LOG" 2>&1
LLVM_PROFILE_FILE="$GPU_RAW" "$GPU" "$RUN/gpu-probe.seen" >"$GPU_LOG" 2>&1
LLVM_PROFILE_FILE="$HOST_RAW" "$HOST" gpu emit "$RUN/gpu-probe.seen" \
    --gpu-backend spirv --gpu-target vulkan1.3 --module-id test-002a \
    --artifact-info --output "$RUN/gpu-probe.spv" --reflection "$RUN/gpu-probe.json" \
    >>"$HOST_LOG" 2>&1

llvm-profdata merge -sparse --num-threads=1 "$RUN"/compiler-*.profraw -o "$HOST_PROFILE"
llvm-profdata merge -sparse --num-threads=1 "$RUN"/gpu-*.profraw -o "$GPU_PROFILE"
llvm-cov report --num-threads=1 "$HOST" -instr-profile "$HOST_PROFILE" >"$HOST_COVERAGE"
llvm-cov report --num-threads=1 "$GPU" -instr-profile "$GPU_PROFILE" >"$GPU_COVERAGE"
llvm-cov export --num-threads=1 "$HOST" -instr-profile "$HOST_PROFILE" >"$RUN/compiler.coverage.json"
llvm-cov export --num-threads=1 "$GPU" -instr-profile "$GPU_PROFILE" >"$RUN/gpu.coverage.json"


llvm-profdata merge -sparse --num-threads=1 "$HOST_PROFILE" "$RUN/source-coverage-encoding.profdata" \
    "$RUN/source-coverage-scopes.profdata" -o "$RUN/implementation.profdata"
llvm-cov export --num-threads=1 -object "$HOST" -object "$RUN/source-coverage-encoding" \
    -object "$RUN/source-coverage-scopes" -instr-profile "$RUN/implementation.profdata" \
    >"$RUN/implementation-coverage.json"
python3 "$ROOT/scripts/check_source_coverage_thresholds.py" "$RUN/implementation-coverage.json" \
    --source compiler_seen/src/codegen/source_coverage_emit.seen \
    --source compiler_seen/src/codegen/source_coverage_encoding.seen \
    --source compiler_seen/src/codegen/source_coverage_state.seen \
    --source seen_std/src/crypto/md5.seen >"$RUN/implementation-coverage-summary.json"

python3 "$ROOT/scripts/check_test_instrumentation.py" --derive-execution \
    --compiler-build-report "$HOST_BUILD" --gpu-build-report "$GPU_BUILD" \
    --compiler-profdata "$HOST_PROFILE" --test-profdata "$GPU_PROFILE" \
    --compiler-coverage "$HOST_COVERAGE" --test-coverage "$GPU_COVERAGE" \
    --compiler-coverage-json "$RUN/compiler.coverage.json" \
    --test-coverage-json "$RUN/gpu.coverage.json" \
    --gpu-artifact "$RUN/gpu-probe.spv" \
    --compiler-log "$HOST_LOG" --test-log "$GPU_LOG"
for profile in ci coverage sanitizer-undefined sanitizer-thread; do
    cli_log="$RUN/cli-$profile.log"
    if ! SEEN_COMPILER_SOURCE_ROOT="$ROOT" SEEN_PACKAGE_CLIENT="$PACKAGE_CLIENT" \
        "$COMPILER" test "$ROOT" \
        --filter test_002a_instrumentation --profile "$profile" \
        --jobs 1 --timeout 10m \
        --report "json:$RUN_REL/cli-$profile.json" \
        --report "junit:$RUN_REL/cli-$profile.xml" >"$cli_log" 2>&1; then
        tail -c 32768 "$cli_log" >&2
        fail "test CLI profile $profile failed"
    fi
    grep -Fq 'test result: 1 passed; 0 failed;' "$cli_log" ||
        fail "test CLI profile $profile did not execute the selected fixture"
    python3 "$ROOT/scripts/check_test_reporters.py" --format json \
        --validate "$RUN/cli-$profile.json" >/dev/null
    python3 "$ROOT/scripts/check_test_reporters.py" --format junit \
        --validate "$RUN/cli-$profile.xml" >/dev/null
done
echo "PASS: TEST-002A instrumented compiler, GPU emitter, runtime, and ABI execution"
