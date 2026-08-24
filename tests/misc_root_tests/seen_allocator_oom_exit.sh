#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-allocator-oom-exit
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-allocator-oom-exit.XXXXXX")"
CHECKED_FIXTURE="$ROOT_DIR/compiler_seen/tests/allocator_oom_exit.seen"
CHECKED_BINARY="$TMP_DIR/allocator-checked-oom-exit"
CHECKED_COMPILE_LOG="$TMP_DIR/checked.compile.log"
ALIGNED_FIXTURE="$ROOT_DIR/compiler_seen/tests/allocator_aligned_oom_exit.seen"
ALIGNED_BINARY="$TMP_DIR/allocator-aligned-oom-exit"
ALIGNED_COMPILE_LOG="$TMP_DIR/aligned.compile.log"

cleanup() {
    local status=$?
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-allocator-oom-exit.*)
            if [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ]; then
                rm -rf -- "$TMP_DIR"
            else
                echo "ERROR: refusing to clean unsafe allocator regression path: $TMP_DIR" >&2
                status=1
            fi
            ;;
        *)
            echo "ERROR: unexpected allocator regression path: $TMP_DIR" >&2
            status=1
            ;;
    esac
    trap - EXIT
    exit "$status"
}
trap cleanup EXIT

compile_fixture() {
    local label=$1
    local fixture=$2
    local binary=$3
    local compile_log=$4

    if ! timeout --foreground --kill-after=10s 600s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile "$fixture" "$binary" \
        --fast --no-cache >"$compile_log" 2>&1; then

        echo "FAIL: $label allocator OOM fixture did not compile" >&2
        tail -c 32768 "$compile_log" >&2 || true
        exit 1
    fi
}

compile_fixture checked "$CHECKED_FIXTURE" "$CHECKED_BINARY" \
    "$CHECKED_COMPILE_LOG"
compile_fixture aligned "$ALIGNED_FIXTURE" "$ALIGNED_BINARY" \
    "$ALIGNED_COMPILE_LOG"

python3 - \
    "$CHECKED_BINARY" "compiler-emitted reallocation" \
    "$ALIGNED_BINARY" "seen_arr_push_str aligned resize" <<'PY'
import os
import signal
import subprocess
import sys
import time

MAX_DIAGNOSTIC_BYTES = 32768


def run_case(binary: str, expected_context: str) -> None:
    started = time.monotonic()
    process = subprocess.Popen(
        [binary],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=2.0)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        raise SystemExit(
            "FAIL: allocator OOM process did not terminate within two seconds\n"
            + stdout
            + stderr
        )

    elapsed = time.monotonic() - started
    if process.returncode != 134:
        raise SystemExit(
            f"FAIL: allocator OOM process returned {process.returncode}, expected 134\n"
            + stdout
            + stderr
        )
    if "Seen allocation failure" not in stderr:
        raise SystemExit("FAIL: allocator OOM diagnostic was missing\n" + stderr)
    if expected_context not in stderr:
        raise SystemExit(
            f"FAIL: allocator OOM diagnostic lacked {expected_context!r}\n"
            + stderr
        )
    if len(stderr.encode("utf-8")) > MAX_DIAGNOSTIC_BYTES:
        raise SystemExit(
            "FAIL: allocator OOM diagnostic exceeded "
            f"{MAX_DIAGNOSTIC_BYTES} bytes"
        )
    if elapsed >= 2.0:
        raise SystemExit(f"FAIL: allocator OOM exit took {elapsed:.3f}s")
    try:
        os.killpg(process.pid, 0)
    except ProcessLookupError:
        pass
    else:
        os.killpg(process.pid, signal.SIGKILL)
        raise SystemExit("FAIL: allocator OOM process group remained alive")


arguments = sys.argv[1:]
if len(arguments) != 4:
    raise SystemExit("FAIL: allocator OOM regression case binding is invalid")
for offset in range(0, len(arguments), 2):
    run_case(arguments[offset], arguments[offset + 1])
PY

echo "PASS: checked and aligned allocator OOM paths exit promptly with status 134"
