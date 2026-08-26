#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../../../.." && pwd -P)"
OUTPUT=${1:-}
if [ -z "$OUTPUT" ]; then
    echo "usage: capture_environment.sh OUTPUT.json" >&2
    exit 64
fi
if [ "${SEEN_HARD_MEMORY_SCOPE_ACTIVE:-0}" != 1 ] ||
   [ "${SEEN_LOW_MEMORY:-0}" != 1 ] || [ "${SEEN_JOBS:-0}" != 1 ] ||
   [ "${SEEN_OPT_JOBS:-0}" != 1 ]; then
    echo "environment capture requires the verified serial hard-memory scope" >&2
    exit 126
fi
"$ROOT_DIR/scripts/run_in_hard_memory_scope.sh" --verify-only

case "$OUTPUT" in
    "$ROOT_DIR"/.seen/*) ;;
    *) echo "environment evidence must remain below the ignored .seen directory" >&2; exit 64 ;;
esac
if [ -L "$OUTPUT" ]; then
    echo "environment evidence output may not be a symbolic link" >&2
    exit 64
fi
mkdir -p "${OUTPUT%/*}"

SEEN_BIN=${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}
if [ ! -x "$SEEN_BIN" ]; then
    echo "current Seen compiler is missing: $SEEN_BIN" >&2
    exit 1
fi
if [ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=no)" ]; then
    echo "environment lock requires a clean tracked tree" >&2
    exit 1
fi

export CAPTURED_AT KERNEL OS_NAME CPU_MODEL CPU_COUNT RAM_BYTES BUILD_LIMIT_BYTES
export GPU_NAME GPU_UUID GPU_MEMORY_BYTES GPU_CC GPU_PCI DRIVER POWER_LIMIT CLOCK_POLICY
export CUDA_TOOLKIT CUBLASLT_VERSION LLVM_VERSION SEEN_SHA256 REPOSITORY_COMMIT
export AMBIENT_CELSIUS BENCHMARK_ENV OUTPUT

CAPTURED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
KERNEL=$(uname -r)
OS_NAME=$(uname -s)
CPU_MODEL=$(awk -F: '/model name/ {sub(/^[[:space:]]+/, "", $2); print $2; exit}' /proc/cpuinfo)
CPU_COUNT=$(getconf _NPROCESSORS_ONLN)
RAM_BYTES=$(awk '/^MemTotal:/ {print $2 * 1024; exit}' /proc/meminfo)
BUILD_LIMIT_BYTES=$((SEEN_MEMORY_GUARD_RSS_KB * 1024))
IFS=',' read -r GPU_NAME GPU_UUID GPU_MEMORY_MIB GPU_CC GPU_PCI DRIVER POWER_LIMIT CLOCK_POLICY < <(
    nvidia-smi --query-gpu=name,uuid,memory.total,compute_cap,pci.bus_id,driver_version,power.limit,clocks.current.graphics \
        --format=csv,noheader,nounits -i 0)
GPU_MEMORY_BYTES=$(( ${GPU_MEMORY_MIB// /} * 1024 * 1024 ))
CUDA_TOOLKIT=$(/opt/cuda/bin/nvcc --version | awk '/release/ {gsub(/,/, "", $5); print $5; exit}')
CUBLASLT_VERSION=$(awk '/#define CUBLAS_VER_(MAJOR|MINOR|PATCH)/ {printf "%s%s", separator, $3; separator="."}' /opt/cuda/include/cublas_api.h)
LLVM_VERSION=$(llvm-config --version)
SEEN_SHA256=$(sha256sum "$SEEN_BIN"); SEEN_SHA256=${SEEN_SHA256%% *}
REPOSITORY_COMMIT=$(git -C "$ROOT_DIR" rev-parse HEAD)
AMBIENT_CELSIUS=${SEEN_AMBIENT_CELSIUS:-}
BENCHMARK_ENV=$(env | LC_ALL=C sort | awk -F= '
    $1 == "CUDA_VISIBLE_DEVICES" || $1 == "OMP_NUM_THREADS" ||
    $1 == "OPENBLAS_NUM_THREADS" || $1 == "MKL_NUM_THREADS" ||
    $1 == "SEEN_JOBS" || $1 == "SEEN_OPT_JOBS" ||
    $1 == "SEEN_LOW_MEMORY" || $1 == "SEEN_MEMORY_LIMIT_BYTES" ||
    $1 == "SEEN_MEMORY_GUARD_RSS_KB" {print}')

python3 - <<'PY'
import json
import os
from pathlib import Path

def integer(name):
    return int(float(os.environ[name].strip()))

def stripped(name):
    return os.environ[name].strip()

output = Path(os.environ["OUTPUT"])
document = {
    "schema": "seen-qwen-environment-lock-v1",
    "captured_at": os.environ["CAPTURED_AT"],
    "os": {"name": os.environ["OS_NAME"], "kernel": os.environ["KERNEL"]},
    "cpu": {"model": stripped("CPU_MODEL"), "logical_processors": integer("CPU_COUNT")},
    "memory": {"total_bytes": integer("RAM_BYTES"), "build_limit_bytes": integer("BUILD_LIMIT_BYTES")},
    "gpu": {"name": stripped("GPU_NAME"), "uuid": stripped("GPU_UUID"),
            "memory_bytes": integer("GPU_MEMORY_BYTES"), "compute_capability": stripped("GPU_CC"),
            "pci_id": stripped("GPU_PCI"), "power_limit_watts": float(stripped("POWER_LIMIT")),
            "graphics_clock_mhz": float(stripped("CLOCK_POLICY"))},
    "nvidia": {"driver": stripped("DRIVER"), "cuda_toolkit": stripped("CUDA_TOOLKIT"),
               "cublaslt": stripped("CUBLASLT_VERSION")},
    "compiler": {"seen_sha256": os.environ["SEEN_SHA256"], "llvm": os.environ["LLVM_VERSION"]},
    "repository": {"commit": os.environ["REPOSITORY_COMMIT"], "dirty": False},
    "conditions": {"ambient_celsius": float(os.environ["AMBIENT_CELSIUS"])
                   if os.environ["AMBIENT_CELSIUS"] else None,
                   "variables": os.environ["BENCHMARK_ENV"].splitlines()},
}
temporary = output.with_name(f".{output.name}.partial.{os.getpid()}")
with temporary.open("x", encoding="utf-8") as handle:
    json.dump(document, handle, indent=2, sort_keys=True)
    handle.write("\n")
    handle.flush()
    os.fsync(handle.fileno())
os.replace(temporary, output)
PY

python3 "$ROOT_DIR/scripts/check_qwen_contracts.py" \
    --document "$ROOT_DIR/schemas/qwen/qwen-environment-lock.schema.json" "$OUTPUT"
sha256sum "$OUTPUT"
