#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
COMPILER="${1:-$ROOT/compiler_seen/target/seen}"

fail() { echo "ERROR: TEST-002A execution acceptance: $*" >&2; exit 1; }
[ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" = 1 ] || fail "verified hard memory scope is required"
[ "${SEEN_PROJECT_ARTIFACT_WRAPPER:-0}" = 1 ] || fail "project artifact wrapper is required"
[ "${SEEN_JOBS:-0}" = 1 ] && [ "${SEEN_OPT_JOBS:-0}" = 1 ] || fail "serial worker limits are required"
[ -x "$COMPILER" ] || fail "compiler is unavailable"
for tool in llvm-profdata llvm-cov glslc; do command -v "$tool" >/dev/null || fail "$tool is unavailable"; done

RUN="$SEEN_ARTIFACT_ROOT/test-002a-execution"
mkdir -p -- "$RUN"
RUN_REL="${RUN#"$ROOT"/}"
[ "$RUN_REL" != "$RUN" ] || fail "artifact root is outside the checkout"

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
    SEEN_COMPILER_SOURCE_ROOT="$ROOT" SEEN_PACKAGE_CLIENT="$ROOT/compiler_seen/target/seen-pkg" \
        "$COMPILER" compile compiler_seen/src/main_compiler.seen "$HOST" \
        --fast --no-cache --coverage --sanitize undefined \
        --instrumentation-report "$HOST_BUILD_REL" --no-fork
) >"$HOST_LOG" 2>&1

LLVM_PROFILE_FILE="$HOST_RAW" "$HOST" --version >>"$HOST_LOG" 2>&1
(
    cd "$ROOT"
    LLVM_PROFILE_FILE="$HOST_RAW" SEEN_COMPILER_SOURCE_ROOT="$ROOT" \
        SEEN_PACKAGE_CLIENT="$ROOT/compiler_seen/target/seen-pkg" \
        "$HOST" compile compiler_seen/tests/test_002a_gpu_emitters.seen "$GPU" \
        --fast --no-cache --coverage --sanitize undefined --emit-glsl \
        --instrumentation-report "$GPU_BUILD_REL" --no-fork
) >>"$HOST_LOG" 2>&1
LLVM_PROFILE_FILE="$GPU_RAW" "$GPU" >"$GPU_LOG" 2>&1

llvm-profdata merge -sparse "$RUN"/compiler-*.profraw -o "$HOST_PROFILE"
llvm-profdata merge -sparse "$RUN"/gpu-*.profraw -o "$GPU_PROFILE"
llvm-cov report "$HOST" -instr-profile "$HOST_PROFILE" >"$HOST_COVERAGE"
llvm-cov report "$GPU" -instr-profile "$GPU_PROFILE" >"$GPU_COVERAGE"

python3 "$ROOT/scripts/check_test_instrumentation.py" --derive-execution \
    --compiler-build-report "$HOST_BUILD" --gpu-build-report "$GPU_BUILD" \
    --compiler-profdata "$HOST_PROFILE" --test-profdata "$GPU_PROFILE" \
    --compiler-coverage "$HOST_COVERAGE" --test-coverage "$GPU_COVERAGE" \
    --gpu-glsl "$GPU.shaders/test002aVectorAdd.comp.glsl" \
    --compiler-log "$HOST_LOG" --test-log "$GPU_LOG"
echo "PASS: TEST-002A instrumented compiler, GPU emitter, runtime, and ABI execution"
