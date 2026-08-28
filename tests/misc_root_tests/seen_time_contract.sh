#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
RELEASE_TARGET_CPU=${SEEN_RELEASE_CPU_BASELINE:-x86-64}
case "$RELEASE_TARGET_CPU" in
    x86-64|x86-64-v3) ;;
    *)
        echo "FAIL: unsupported time contract CPU baseline: $RELEASE_TARGET_CPU" >&2
        exit 2
        ;;
esac
SOURCE="$ROOT_DIR/tests/fixtures/time/happy/time_contract.seen"
RUNNER="$ROOT_DIR/scripts/run_with_project_artifacts.sh"
BASELINE_CHECKER="$ROOT_DIR/scripts/check_x86_executable_baseline.sh"
mkdir -p "$ROOT_DIR/.seen/agent-tools/time-contract"
WORK_DIR=$(mktemp -d "$ROOT_DIR/.seen/agent-tools/time-contract/run.XXXXXX")
cleanup() {
    status=$?
    if [ "$status" -eq 0 ]; then rm -rf "$WORK_DIR"
    else echo "Preserved failed time artifacts: $WORK_DIR" >&2; fi
}
trap cleanup EXIT

run_seen() {
    "$RUNNER" time-contract --keep-on-failure -- "$SEEN_BIN" "$@"
}

run_seen check "$SOURCE" >"$WORK_DIR/check.log" 2>&1
run_seen compile "$SOURCE" "$WORK_DIR/fast" --fast --no-cache --no-fork \
    --target-cpu "$RELEASE_TARGET_CPU" --jobs 1 --opt-jobs 1 \
    >"$WORK_DIR/fast.log" 2>&1
bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/fast"
"$WORK_DIR/fast"
run_seen compile "$SOURCE" "$WORK_DIR/release" --release --lto thin --no-fork \
    --no-cache --target-cpu "$RELEASE_TARGET_CPU" --jobs 1 --opt-jobs 1 \
    >"$WORK_DIR/release.log" 2>&1
bash "$BASELINE_CHECKER" "$RELEASE_TARGET_CPU" "$WORK_DIR/release"
"$WORK_DIR/release"

python3 "$ROOT_DIR/scripts/check_native_boundaries.py" \
    "$ROOT_DIR/docs/architecture/native-boundaries.json"

echo "PASS: TIME-001A-E contracts"
