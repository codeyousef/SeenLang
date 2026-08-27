#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
FIXTURE="$ROOT_DIR/tests/compiler_regressions/fel_1538_safetensors_reader.seen"
HEADER="$ROOT_DIR/tests/fixtures/fel-1538/qwn-023b-header.zlib.b64"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
ASAN_RUNNER="$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh"
SCOPE=seen-safetensors-reader-ownership

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi

bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
[ "${SEEN_PROJECT_ARTIFACT_NAMESPACE_ACTIVE:-0}" = 1 ] || {
    echo "FAIL: FEL-1538 project-local artifact namespace is inactive" >&2
    exit 126
}
[ "${SEEN_LOW_MEMORY:-0}" = 1 ] && [ "${SEEN_JOBS:-0}" = 1 ] &&
    [ "${SEEN_OPT_JOBS:-0}" = 1 ] || {
    echo "FAIL: FEL-1538 serial low-memory settings are inactive" >&2
    exit 126
}
[ -x "$COMPILER" ] && [ ! -L "$COMPILER" ] || {
    echo "FAIL: FEL-1538 compiler is unavailable or unsafe" >&2
    exit 1
}
PACKAGE_CLIENT="$ROOT_DIR/compiler_seen/target/seen-pkg"
[ -x "$PACKAGE_CLIENT" ] && [ ! -L "$PACKAGE_CLIENT" ] || {
    echo "FAIL: FEL-1538 matching package client is unavailable or unsafe" >&2
    exit 1
}
export SEEN_PACKAGE_CLIENT="$PACKAGE_CLIENT"
[ -f "$ASAN_RUNNER" ] && [ ! -L "$ASAN_RUNNER" ] || {
    echo "FAIL: FEL-1538 bounded ASan runner is missing or unsafe" >&2
    exit 1
}
if ! ulimit -S -s 8192 2>/dev/null || [ "$(ulimit -S -s)" != 8192 ]; then
    echo "FAIL: FEL-1538 requires a verified 8192 KiB stack" >&2
    exit 126
fi

WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/fel-1538-safetensors.XXXXXX")"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$WORK_DIR" in
            "$SEEN_ARTIFACT_ROOT"/fel-1538-safetensors.*)
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
                ;;
            *) status=1 ;;
        esac
    else
        echo "Preserved FEL-1538 artifacts: $WORK_DIR" >&2
    fi
    exit "$status"
}
trap cleanup EXIT
mkdir -p -- "$WORK_DIR/data" "$WORK_DIR/bin" "$WORK_DIR/ir" "$WORK_DIR/logs"

python3 - "$HEADER" "$WORK_DIR/data" <<'PY'
import base64
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib

header_path = Path(sys.argv[1])
output = Path(sys.argv[2])
header = zlib.decompress(base64.b64decode(header_path.read_text().encode()))
document = json.loads(header)
payload_bytes = max(
    value["data_offsets"][1]
    for key, value in document.items()
    if key != "__metadata__"
)

mask64 = (1 << 64) - 1
state = 20260827
payload = bytearray(payload_bytes)
cursor = 0
while cursor < payload_bytes:
    value = state
    value ^= value >> 12
    value ^= (value << 25) & mask64
    value ^= value >> 27
    state = value & mask64
    bits = ((state * 0x2545F4914F6CDD1D) & mask64) >> 40
    number = (bits / float(1 << 23) - 1.0) * 0.05
    payload[cursor:cursor + 4] = struct.pack("<f", number)
    cursor += 4

qwen = struct.pack("<Q", len(header)) + header + payload
expected = "16ecca9cb396099db0c92d835840264e7b45d12cd6221d7af5462ac8576c94a9"
if hashlib.sha256(qwen).hexdigest() != expected:
    raise SystemExit("FEL-1538 locked Qwen fixture digest mismatch")
(output / "qwn-023b.safetensors").write_bytes(qwen)

def encoded(raw: str, payload_length: int) -> bytes:
    data = raw.encode("utf-8")
    return struct.pack("<Q", len(data)) + data + bytes(payload_length)

(output / "minimal.safetensors").write_bytes(encoded(
    '{"x":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}', 4))
(output / "duplicate.safetensors").write_bytes(encoded(
    '{"x":{"dtype":"F32","shape":[1],"data_offsets":[0,4]},'
    '"x":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}', 4))
(output / "unknown.safetensors").write_bytes(encoded(
    '{"x":{"dtype":"F32","shape":[1],"data_offsets":[0,4],"extra":0}}', 4))
(output / "dtype.safetensors").write_bytes(encoded(
    '{"x":{"dtype":"F8","shape":[4],"data_offsets":[0,4]}}', 4))
(output / "overlap.safetensors").write_bytes(encoded(
    '{"x":{"dtype":"U8","shape":[2],"data_offsets":[0,2]},'
    '"y":{"dtype":"U8","shape":[2],"data_offsets":[1,3]}}', 3))
(output / "range.safetensors").write_bytes(encoded(
    '{"x":{"dtype":"F32","shape":[2],"data_offsets":[0,4]}}', 4))
(output / "truncated.safetensors").write_bytes(b"SEENBAD")
PY

assert_ir_contract() {
    local compile_log="$WORK_DIR/logs/ir-contract-compile.log"
    local compile_status=0

    timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile "$FIXTURE" \
            --emit-module-ir-dir "$WORK_DIR/ir" --stop-after-ir --no-cache \
            --fast \
            >"$compile_log" 2>&1 || compile_status=$?
    if [ "$compile_status" -ne 0 ]; then
        tail -c 32768 -- "$compile_log" >&2 || true
        return "$compile_status"
    fi
    if ! grep -Fq '&member — address of field storage' "$WORK_DIR"/ir/*.ll; then
        echo "FAIL: FEL-1538 member-field address lowering is absent from IR" >&2
        return 1
    fi
    if grep -E 'declare i32 @seen_mapped_(file|window)_[^(]*\([^)]*\).*memory\(inaccessiblemem: readwrite\)' \
        "$WORK_DIR"/ir/*.ll; then
        echo "FAIL: FEL-1538 mapped out-parameter ABI has an invalid memory effect" >&2
        return 1
    fi
    grep -Eq 'declare i32 @seen_mapped_file_window\([^)]*\) nounwind$' \
        "$WORK_DIR"/ir/*.ll
    grep -Eq 'declare i32 @seen_mapped_(file|window)_close\([^)]*\) nounwind$' \
        "$WORK_DIR"/ir/*.ll
}

compile_and_run() {
    local label=$1
    shift
    local binary="$WORK_DIR/bin/fel-1538-$label"
    local compile_log="$WORK_DIR/logs/$label-compile.log"
    local run_log="$WORK_DIR/logs/$label-run.log"
    local compile_status=0
    local run_status=0

    timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" compile "$FIXTURE" "$binary" --no-cache \
            "$@" \
            >"$compile_log" 2>&1 || compile_status=$?
    if [ "$compile_status" -ne 0 ]; then
        tail -c 32768 -- "$compile_log" >&2 || true
        return "$compile_status"
    fi

    if [ "$label" = address ]; then
        timeout --foreground --kill-after=5s 120s \
            env SEEN_FEL1538_ROOT="$WORK_DIR/data" \
            bash "$ASAN_RUNNER" --target-root "$WORK_DIR/bin" \
                --compile-log "$compile_log" -- "$binary" \
                >"$run_log" 2>&1 || run_status=$?
    else
        timeout --foreground --kill-after=5s 120s \
            env SEEN_FEL1538_ROOT="$WORK_DIR/data" \
                UBSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
                "$binary" >"$run_log" 2>&1 || run_status=$?
    fi
    if grep -Eiq 'ERROR: (AddressSanitizer|LeakSanitizer)|SUMMARY: (AddressSanitizer|UndefinedBehaviorSanitizer)|runtime error:' "$run_log"; then
        tail -c 32768 -- "$run_log" >&2 || true
        return 1
    fi
    if [ "$run_status" -ne 0 ]; then
        tail -c 32768 -- "$run_log" >&2 || true
        return "$run_status"
    fi
    grep -Fq 'PASS: FEL-1538 bounded Safetensors reader ownership' "$run_log"
}

assert_ir_contract
compile_and_run fast --fast
compile_and_run release --release --lto thin --target-cpu=x86-64
compile_and_run undefined --fast --sanitize undefined
compile_and_run address --fast --sanitize address

echo "PASS: FEL-1538 Safetensors reader regression"
