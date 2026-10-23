#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -P "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
COMPILER=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-time-contract
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
RELEASE_TARGET_CPU=${SEEN_RELEASE_CPU_BASELINE:-x86-64}
case "$RELEASE_TARGET_CPU" in
    x86-64|x86-64-v3) ;;
    *)
        echo "FAIL: unsupported time contract CPU baseline: $RELEASE_TARGET_CPU" >&2
        exit 2
        ;;
esac
SOURCE="$ROOT_DIR/tests/fixtures/time/happy/time_contract.seen"
CLOCK_SOURCE="$ROOT_DIR/seen_std/tests/time_basic_fixture.seen"
BASELINE_CHECKER="$ROOT_DIR/scripts/check_x86_executable_baseline.sh"
WORK_DIR=$(mktemp -d "$SEEN_ARTIFACT_ROOT/time-contract.XXXXXX")
cleanup() {
    status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$WORK_DIR" in "$SEEN_ARTIFACT_ROOT"/time-contract.*) rm -rf -- "$WORK_DIR" ;; *) return 1 ;; esac
    else echo "Preserved time artifacts: $WORK_DIR" >&2; fi
    return "$status"
}
trap cleanup EXIT

run_seen() {
    timeout --foreground --kill-after=5s 600s bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

for name in contract startup-clock; do
    input="$SOURCE"
    if [ "$name" = startup-clock ]; then input="$CLOCK_SOURCE"; fi
    run_seen check "$input" >"$WORK_DIR/$name.check.log" 2>&1
    run_seen compile "$input" "$WORK_DIR/$name-fast" --fast --no-cache --no-fork \
        --target-cpu "$RELEASE_TARGET_CPU" >"$WORK_DIR/$name-fast.compile.log" 2>&1
    run_seen compile "$input" "$WORK_DIR/$name-release" --release --lto thin --no-fork \
        --no-cache --target-cpu "$RELEASE_TARGET_CPU" >"$WORK_DIR/$name-release.compile.log" 2>&1
    # Audit both outputs before launching either optimization mode.
    bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/$name-fast"
    bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/$name-release"
    for mode in fast release; do
        binary="$WORK_DIR/$name-$mode"
        if [ "$name" = startup-clock ]; then
            # This native child needs a bounded explicit input snapshot, not
            # the build driver's ambient environment. Kernel scope and rlimits
            # remain inherited; carry its exact allocator/worker budgets.
            timeout --foreground --kill-after=5s 120s env -i \
                SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=42 \
                SEEN_DETERMINISTIC_SEED=0 SEEN_HASH_SEED=0 TZ=UTC LANG=C LC_ALL=C \
                SEEN_MEMORY_LIMIT_BYTES="${SEEN_MEMORY_LIMIT_BYTES:?}" \
                SEEN_MAIN_VMEM_KB="${SEEN_MAIN_VMEM_KB:?}" SEEN_OPT_VMEM_KB="${SEEN_OPT_VMEM_KB:?}" \
                SEEN_LOW_MEMORY=1 SEEN_JOBS=1 SEEN_OPT_JOBS=1 \
                SEEN_ARTIFACT_ROOT="$SEEN_ARTIFACT_ROOT" TMPDIR="${TMPDIR:?}" \
                RIPGREP_CONFIG_PATH="${RIPGREP_CONFIG_PATH:?}" \
                RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 \
                MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1 \
                BLIS_NUM_THREADS=1 GOMAXPROCS=1 RUST_TEST_THREADS=1 CARGO_BUILD_JOBS=1 \
                RPM_BUILD_NCPUS=1 SEEN_LLD_THREADS=1 SEEN_THINLTO_JOBS=1 \
                "$binary" >"$WORK_DIR/$name-$mode.run.log" 2>&1
            grep -Fxq 'PASS: deterministic startup time compatibility' "$WORK_DIR/$name-$mode.run.log"
        else
            timeout --foreground --kill-after=5s 120s "$binary" \
                >"$WORK_DIR/$name-$mode.run.log" 2>&1
        fi
        echo "PASS: time $name $mode native execution"
    done
done

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" \
    "$ROOT_DIR/docs/architecture/native-boundaries.json"

echo "PASS: TIME-001A-E contracts"
