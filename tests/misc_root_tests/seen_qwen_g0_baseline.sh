#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
SEEN_BIN=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
ENVIRONMENT_LOCK=${1:-}
OUTPUT=${2:-}
case "$#" in
    0|2) ;;
    *) echo "usage: seen_qwen_g0_baseline.sh [ENVIRONMENT.json OUTPUT.json]" >&2; exit 64 ;;
esac
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-qwen-g0-baseline
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$SEEN_BIN" -- bash "$0" "$@"
fi
SEEN_BIN="${SEEN_CAPPED_REGRESSION_COMPILER:-$SEEN_BIN}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$SEEN_BIN"
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ]; then
    echo "FAIL: Qwen baseline requires the verified hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only
if [ -n "$(git -C "$ROOT_DIR" status --porcelain)" ]; then
    echo "FAIL: BASE-010 requires a clean checkout" >&2
    exit 1
fi
if [ "$#" -eq 0 ]; then
    SESSION=$(mktemp -d "${SEEN_ARTIFACT_ROOT:?}/qwen-g0-baseline.XXXXXX")
    ENVIRONMENT_LOCK="$SESSION/environment.json"
    OUTPUT="$SESSION/baseline.json"
    SEEN_BIN="$SEEN_BIN" bash \
        "$ROOT_DIR/projects/seen_ml/qwen38/tools/capture_environment.sh" "$ENVIRONMENT_LOCK"
fi
case "$ENVIRONMENT_LOCK:$OUTPUT" in
    "$ROOT_DIR"/.seen/*:"$ROOT_DIR"/.seen/*) ;;
    *) echo "FAIL: Qwen baseline evidence must remain below .seen" >&2; exit 64 ;;
esac
[ -f "$ENVIRONMENT_LOCK" ] && [ ! -L "$ENVIRONMENT_LOCK" ] || {
    echo "FAIL: validated environment lock is required" >&2
    exit 1
}
python3 "$ROOT_DIR/scripts/check_qwen_contracts.py" --document \
    "$ROOT_DIR/schemas/qwen/qwen-environment-lock.schema.json" "$ENVIRONMENT_LOCK"
export COMPILER="$SEEN_BIN"
version=$(bash "${SEEN_ATTESTED_COMPILER_RUNNER:?}" "$SEEN_BIN" --version)
version=${version%%$'\n'*}
if [[ ! "$version" =~ ^Seen\ ([0-9]+\.[0-9]+\.[0-9]+([-+][A-Za-z0-9.+-]+)?)$ ]]; then
    echo "FAIL: selected compiler returned an invalid version identity" >&2
    exit 1
fi
export SEEN_VERSION="${BASH_REMATCH[1]}"

EVIDENCE_DIR="${SEEN_ARTIFACT_ROOT:?}/qwen-g0-probes"
mkdir -p "$EVIDENCE_DIR" "${OUTPUT%/*}"
run_probe() {
    local id=$1
    shift
    local status=0
    env -u SEEN_CAPPED_REGRESSION_ACTIVE -u SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE \
        "$@" >"$EVIDENCE_DIR/$id.log" 2>&1 || status=$?
    if [ "$status" -ne 0 ]; then
        cat "$EVIDENCE_DIR/$id.log" >&2
        return "$status"
    fi
    sha256sum "$EVIDENCE_DIR/$id.log" | awk '{print $1}'
}

BASE001=$(run_probe BASE-001 env SEEN_BIN="$SEEN_BIN" \
    "$ROOT_DIR/tests/misc_root_tests/seen_qwen_seen_contracts.sh")
BASE002=$(run_probe BASE-002 env SEEN_BIN="$SEEN_BIN" \
    "$ROOT_DIR/tests/misc_root_tests/seen_qwen_native_dependency.sh")
BASE003=$(run_probe BASE-003 "$ROOT_DIR/tests/misc_root_tests/seen_owned_resource_contract.sh")
BASE004=$(run_probe BASE-004 "$ROOT_DIR/tests/misc_root_tests/seen_qwen_mapped_file_large.sh")
BASE005=$BASE002
BASE006=$(run_probe BASE-006 "$ROOT_DIR/tests/misc_root_tests/seen_release_optimization_contract.sh")
BASE007=$(run_probe BASE-007 python3 "$ROOT_DIR/scripts/check_qwen_contracts.py" \
    --schema-dir "$ROOT_DIR/schemas/qwen" --document \
    "$ROOT_DIR/schemas/qwen/qwen-model-lock.schema.json" \
    "$ROOT_DIR/projects/seen_ml/qwen38/locks/qwen-model-lock.json")
BASE008=$(run_probe BASE-008 "$ROOT_DIR/tests/misc_root_tests/seen_build_instrumentation_contract.sh")
BASE009=$(run_probe BASE-009 "$ROOT_DIR/tests/misc_root_tests/seen_runtime_vulkan_symbols.sh")
BASE010=$(run_probe BASE-010 bash -c \
    'git -C "$1" diff --check && ! git -C "$1" ls-files | grep -E "(^|/)(evidence|artifacts|models)/|\\.(safetensors|sqw)$"' _ "$ROOT_DIR")

export ROOT_DIR ENVIRONMENT_LOCK OUTPUT SEEN_BIN BASE001 BASE002 BASE003 BASE004 BASE005
export BASE006 BASE007 BASE008 BASE009 BASE010
python3 - <<'PY'
import hashlib, json, os
from pathlib import Path

root = Path(os.environ["ROOT_DIR"])
environment = Path(os.environ["ENVIRONMENT_LOCK"])
compiler = Path(os.environ.get("SEEN_BIN", root / "compiler_seen/target/seen"))
document = {
    "schema": "seen-qwen-baseline-evidence-v1",
    "seen_commit": os.popen(f"git -C '{root}' rev-parse HEAD").read().strip(),
    "seen_version": os.environ["SEEN_VERSION"],
    "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
    "environment_sha256": hashlib.sha256(environment.read_bytes()).hexdigest(),
    "tests": [
        {"id": f"BASE-{index:03d}", "status": "pass",
         "evidence_sha256": os.environ[f"BASE{index:03d}"]}
        for index in range(1, 11)
    ],
}
output = Path(os.environ["OUTPUT"])
temporary = output.with_name(f".{output.name}.partial.{os.getpid()}")
temporary.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
os.replace(temporary, output)
PY
python3 "$ROOT_DIR/scripts/check_qwen_contracts.py" --document \
    "$ROOT_DIR/schemas/qwen/qwen-baseline-evidence.schema.json" "$OUTPUT"
sha256sum "$OUTPUT"
