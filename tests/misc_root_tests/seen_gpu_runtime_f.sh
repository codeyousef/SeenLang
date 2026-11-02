#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-f
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-runtime-f.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-runtime-f
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
export SEEN_VULKAN_FIXTURE_COPY_TRACE=1 SEEN_VULKAN_FIXTURE_STREAM_TRACE=1
for profile in plain address; do
    flags=()
    destination="$TMP_DIR/vulkan-fault-driver.so"
    if [ "$profile" = address ]; then
        flags=(-fno-omit-frame-pointer -fsanitize=address)
        destination="$TMP_DIR/vulkan-fault-driver-address.so"
    fi
    timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -Wall -Wextra -Werror \
        -fPIC -shared "${flags[@]}" "$ROOT_DIR/tests/production_gpu_runtime/vulkan_graph_fault_driver.c" \
        -ldl -o "$destination"
done
KERNEL="$ROOT_DIR/tests/production_gpu_runtime/vulkan_foundation_kernel.seen"
run_compiler gpu emit "$KERNEL" --gpu-backend spirv --gpu-target vulkan1.2 \
    --output "$TMP_DIR/graph.spv" --reflection "$TMP_DIR/graph.json" > "$TMP_DIR/emit.log" 2>&1
spirv-val --target-env vulkan1.2 "$TMP_DIR/graph.spv"
run_graph_fault() {
    local profile=$1 fault=$2 stage=$3 expected=$4
    SEEN_VULKAN_FIXTURE_FAULT="$fault" LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
        run_fixture "$profile" graph "$TMP_DIR/graph.spv" "$TMP_DIR/graph.json" software-reference "$stage" "$expected"
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/graph-$profile-$fault.log"
}
for profile in fast release undefined address; do
    compile_vulkan_fixture "$profile" host-transfer "$ROOT_DIR/tests/production_gpu_runtime/vulkan_host_transfer_contract.seen"
    seen_run_vulkan_reference "$profile" host-transfer software-reference --faults
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/host-transfer-$profile-native.log"
    compile_vulkan_fixture "$profile" timestamps "$ROOT_DIR/tests/production_gpu_runtime/vulkan_timestamps_contract.seen"
    seen_run_vulkan_reference "$profile" timestamps software-reference --faults
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/timestamps-$profile-native.log"
    compile_vulkan_fixture "$profile" graph "$ROOT_DIR/tests/production_gpu_runtime/vulkan_graph_contract.seen"
    for mode in plain timed; do
        SEEN_VULKAN_FIXTURE_GRAPH_REUSE=1 seen_run_vulkan_reference "$profile" graph \
            "$TMP_DIR/graph.spv" "$TMP_DIR/graph.json" software-reference "$mode"
        cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/graph-$profile-$mode.log"
    done
    seen_run_vulkan_reference "$profile" graph "$TMP_DIR/graph.spv" "$TMP_DIR/graph.json" software-reference checked
    SEEN_VULKAN_FIXTURE_FAULT=graph-later-device-lost LD_PRELOAD="$TMP_DIR/vulkan-fault-driver.so" \
        run_fixture "$profile" graph "$TMP_DIR/graph.spv" "$TMP_DIR/graph.json" software-reference checked-loss
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/graph-$profile-checked-loss.log"
    for fault in copy-pool-status copy-commands-status copy-begin-status copy-end-status pipeline-descriptor-pool-status pipeline-descriptors-status; do
        run_graph_fault "$profile" "$fault" prepare-fault gpu.out_of_memory
    done
    for fault in copy-pool-empty copy-command-empty pipeline-descriptor-pool-empty pipeline-descriptors-empty; do
        run_graph_fault "$profile" "$fault" prepare-fault gpu.invalid
    done
    run_graph_fault "$profile" copy-submit-oom submit-fault gpu.out_of_memory
    run_graph_fault "$profile" copy-submit-unknown submit-fault gpu.unavailable
    run_graph_fault "$profile" copy-submit-lost submit-fault gpu.device_lost
    run_graph_fault "$profile" copy-poll-oom poll-fault gpu.out_of_memory
    run_graph_fault "$profile" copy-poll-lost poll-fault gpu.device_lost
done
python3 - "$TMP_DIR" "$COMPILER" <<'PY'
import hashlib
import json
import platform
import sys
from pathlib import Path

root = Path(sys.argv[1])
compiler = Path(sys.argv[2])
with compiler.open("rb") as source:
    digest = hashlib.file_digest(source, "sha256").hexdigest()
profiles = []
for profile in ("fast", "release", "undefined", "address"):
    statuses = {}
    for name in ("host-transfer", "timestamps", "graph"):
        statuses[name] = int((root / f"{name}-{profile}.run.status").read_text().strip())
    if any(statuses.values()):
        raise SystemExit("FAIL: Runtime-F qualification report found a failed consumer")
    profiles.append({"profile": profile, "exit_statuses": statuses,
                     "native_case_logs": sorted(path.name for path in root.glob(f"graph-{profile}-*.log"))})
group = next(line.split("::", 1)[1] for line in Path("/proc/self/cgroup").read_text().splitlines()
             if line.startswith("0::"))
control = Path("/sys/fs/cgroup") / group.lstrip("/")
limits = {name: int((control / name).read_text().strip())
          for name in ("memory.max", "memory.swap.max", "pids.max")}
if limits["memory.swap.max"] != 0:
    raise SystemExit("FAIL: Runtime-F report observed enabled cgroup swap")
report = {"schema": "seen-runtime-f-evidence-v1",
          "target": f"{platform.system().lower()}-{platform.machine()}",
          "compiler_sha256": digest, "device_class": "software-reference",
          "classification": "verified", "physical_multi_gpu_executed": False,
          "cgroup": group, "limits": limits, "profiles": profiles,
          "peer_acceptance_entrypoint": "tests/misc_root_tests/seen_gpu_cuda_peer.sh",
          "unexecuted_platforms": ["linux-arm64", "windows-x86_64", "macos-x86_64", "macos-arm64"]}
(root / "runtime-f-evidence.json").write_text(json.dumps(report, indent=2) + "\n")
PY
echo 'PASS: Runtime-F persistent native command graphs and nonblocking timestamp contracts (software-reference; peer/multi-device acceptance separate)'
