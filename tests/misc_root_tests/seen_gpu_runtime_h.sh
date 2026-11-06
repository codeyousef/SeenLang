#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-gpu-runtime-h
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/gpu-runtime-h.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=gpu-runtime-h
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
source "$ROOT_DIR/scripts/vulkan_reference_fixture_support.sh"
seen_prepare_vulkan_reference
export SEEN_VULKAN_FIXTURE_COPY_TRACE=1 SEEN_VULKAN_FIXTURE_STREAM_TRACE=1
python3 "$ROOT_DIR/scripts/check_native_boundaries.py" "$ROOT_DIR/docs/architecture/native-boundaries.json" > "$TMP_DIR/native-boundaries.log"
python3 "$ROOT_DIR/scripts/check_native_inventory.py" --root "$ROOT_DIR" \
    --check "$ROOT_DIR/docs/architecture/native-inventory.json" > "$TMP_DIR/native-inventory.log"
timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -Wall -Wextra -Werror \
    "$ROOT_DIR/tests/production_gpu_runtime/vulkan_abi_oracle.c" -o "$TMP_DIR/vulkan-abi-oracle"
"$TMP_DIR/vulkan-abi-oracle" > "$TMP_DIR/vulkan-abi-oracle.log"
for profile in plain address; do
    flags=()
    destination="$TMP_DIR/vulkan-fault-driver.so"
    if [ "$profile" = address ]; then
        flags=(-fno-omit-frame-pointer -fsanitize=address)
        destination="$TMP_DIR/vulkan-fault-driver-address.so"
    fi
    timeout --foreground --kill-after=5s 60s clang -std=c11 -O1 -g -Wall -Wextra -Werror \
        -fPIC -shared "${flags[@]}" "$ROOT_DIR/tests/production_gpu_runtime/vulkan_trace_fault_driver.c" \
        -ldl -o "$destination"
done
run_compiler --version > "$TMP_DIR/compiler-version.txt" 2>&1
clang --version > "$TMP_DIR/native-compiler-version.txt"
git -C "$ROOT_DIR" rev-parse HEAD > "$TMP_DIR/source-base-commit.txt"
run_compiler gpu emit "$ROOT_DIR/tests/production_gpu_runtime/vulkan_foundation_kernel.seen" \
    --gpu-backend spirv --gpu-target vulkan1.2 --output "$TMP_DIR/trace.spv" \
    --reflection "$TMP_DIR/trace.json" > "$TMP_DIR/emit.log" 2>&1
spirv-val --target-env vulkan1.2 "$TMP_DIR/trace.spv"
for profile in fast release undefined address; do
    compile_vulkan_fixture "$profile" trace "$ROOT_DIR/tests/production_gpu_runtime/vulkan_trace_contract.seen"
    seen_run_vulkan_reference "$profile" trace "$TMP_DIR/trace.spv" "$TMP_DIR/trace.json" software-reference reference
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/trace-$profile-reference.log"
    cp -- "$TMP_DIR/trace-$profile.run.status" "$TMP_DIR/trace-$profile-reference.status"
    for probe in fence buffer cache global-wait; do
        status=0
        seen_run_vulkan_reference "$profile" trace "$TMP_DIR/trace.spv" "$TMP_DIR/trace.json" \
            software-reference "probe-$probe" || status=$?
        cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/trace-$profile-probe-$probe.log"
        cp -- "$TMP_DIR/trace-$profile.run.status" "$TMP_DIR/trace-$profile-probe-$probe.status"
        if [ "$status" -ne 90 ]; then
            echo "FAIL: $profile $probe guard returned $status instead of designated exit 90" >&2
            exit 1
        fi
    done
done
python3 - "$TMP_DIR" "$COMPILER" "$ROOT_DIR" <<'PY'
import hashlib
import json
import math
import platform
import statistics
import sys
from pathlib import Path

root = Path(sys.argv[1])

def record(path, prefix):
    matches = [json.loads(line[len(prefix):]) for line in path.read_text().splitlines() if line.startswith(prefix)]
    if len(matches) != 1:
        raise SystemExit(f"FAIL: {path.name}: expected exactly one {prefix.strip()} record")
    return matches[0]

def require(condition, message):
    if not condition:
        raise SystemExit("FAIL: Runtime-H " + message)

profiles = []
for profile in ("fast", "release", "undefined", "address"):
    require(int((root / f"trace-{profile}-reference.status").read_text()) == 0, profile + " native consumer failed")
    log = root / f"trace-{profile}-reference.log"
    consumer = record(log, "GPU_RUNTIME_H_EVIDENCE ")
    native = record(log, "GPU_RUNTIME_H_TRACE ")
    require(consumer["native_execution"] and consumer["backend"] == "vulkan" and consumer["target"] == "software-reference", profile + " backend identity changed")
    require(consumer["warmup_count"] == 5 and consumer["measured_count"] == 30 and consumer["soak_count"] == 1024, profile + " sample/soak bounds changed")
    require(consumer["successful_hot_cycles"] == 1054 and native["hot_submissions"] == 1054 and native["hot_polls"] >= 1054, profile + " native hot work was not observed")
    require(native["hot_command_recordings"] == 1, profile + " hot command was not recorded exactly once")
    require(consumer["cache_restored"] and native["pipeline_cache_create_calls"] >= 2 and native["pipeline_cache_nonempty_initial_calls"] >= 1 and native["pipeline_cache_initial_bytes"] == consumer["cache_payload_bytes"] > 0, profile + " cache payload did not reach the SDK")
    require(native["drained"] and native["native_live_objects"] == 0 and native["requested_device_memory_live_bytes"] == 0, profile + " native owners were retained")
    for field in ("hot_acquisitions", "hot_descriptor_sets_attempted", "forbidden_calls", "forbidden_fence_calls", "forbidden_global_wait_calls"):
        require(native[field] == 0, profile + " forbidden native operation: " + field)
    for field in ("resource_drain", "per_cycle_allocation_stable", "allocations_drained", "atomics_drained"):
        require(consumer[field], profile + " consumer ownership contract: " + field)
    require(all(value == 0 for value in consumer["fallback"].values()), profile + " CPU fallback was used")
    require(consumer["transfer"] == {"initial_host_upload_bytes": 152, "hot_host_upload_bytes": 0, "final_readback_bytes": 148, "timestamp_query_result_bytes_per_query_read": 32}, profile + " transfer accounting changed")
    require(native["timestamp_query_read_calls"] >= 1054 and native["timestamp_query_result_requested_bytes"] == 32 * native["timestamp_query_read_calls"], profile + " timestamp read accounting changed")
    medians = {}
    for clock in ("gpu", "cpu"):
        samples = consumer[clock + "_nanoseconds"]
        require(len(samples) == 30 and all(isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value >= 0 for value in samples), profile + " invalid " + clock + " samples")
        medians[clock + "_nanoseconds"] = statistics.median(samples)
    probes = []
    for probe in ("fence", "buffer", "cache", "global-wait"):
        path = root / f"trace-{profile}-probe-{probe}.log"
        violation = record(path, "GPU_RUNTIME_H_VIOLATION ")
        require(int((root / f"trace-{profile}-probe-{probe}.status").read_text()) == 90 and violation["exit_code"] == 90 and violation["kind"] == "forbidden_native_call" and violation["hot"], profile + " negative guard did not execute: " + probe)
        expected_operation = {"fence": "vkCreateFence", "buffer": "buffer",
                              "cache": "vkCreatePipelineCache", "global-wait": "vkDeviceWaitIdle"}[probe]
        require(violation["operation"] == expected_operation, profile + " wrong negative guard executed: " + probe)
        probes.append({"probe": probe, "exit_status": 90, "violation": violation})
    profiles.append({"profile": profile, "consumer": consumer, "native": native, "medians": medians, "negative_probes": probes})
group = next(line.split("::", 1)[1] for line in Path("/proc/self/cgroup").read_text().splitlines() if line.startswith("0::"))
control = Path("/sys/fs/cgroup") / group.lstrip("/")
limits = {name: int((control / name).read_text().strip()) for name in ("memory.max", "memory.swap.max", "pids.max")}
require(limits["memory.swap.max"] == 0, "report observed enabled cgroup swap")
with Path(sys.argv[2]).open("rb") as source:
    digest = hashlib.file_digest(source, "sha256").hexdigest()
source_root = Path(sys.argv[3])
source_paths = sorted((source_root / "seen_std/src/compute").glob("*.seen"))
source_paths.extend(source_root / "tests/production_gpu_runtime" / name for name in (
    "vulkan_trace_contract.seen", "vulkan_trace_fault_driver.c", "vulkan_recovery_fault_driver.c",
    "vulkan_graph_fault_driver.c", "vulkan_fault_driver.c", "vulkan_abi_oracle.c"))
source_paths.append(source_root / "tests/misc_root_tests/seen_gpu_runtime_h.sh")
source_paths.extend(source_root / "docs/architecture" / name for name in (
    "native-boundaries.json", "native-inventory.json"))
source_hashes = {str(path.relative_to(source_root)): hashlib.sha256(path.read_bytes()).hexdigest()
                 for path in source_paths}
report = {"schema": "seen-runtime-h-evidence-v1", "target": f"{platform.system().lower()}-{platform.machine()}", "compiler_sha256": digest, "classification": "software-reference-not-physical-qualification", "production_certified": False, "cgroup": group, "limits": limits, "profiles": profiles, "unexecuted_platforms": ["linux-arm64", "windows-x86_64", "macos-x86_64", "macos-arm64"], "physical_vram_bytes": None, "physical_vram_status": "not-measured"}
report.update({"command": ["bash", "tests/misc_root_tests/seen_gpu_runtime_h.sh"],
               "source_base_commit": (root / "source-base-commit.txt").read_text().strip(),
               "source_sha256": source_hashes,
               "compiler_version": (root / "compiler-version.txt").read_text().strip(),
               "native_compiler_version": (root / "native-compiler-version.txt").read_text().strip(),
               "os_release": platform.release()})
(root / "runtime-h-evidence.json").write_text(json.dumps(report, indent=2) + "\n")
PY
echo 'PASS: Runtime-H actual native hot-loop trace, cache restore, 5 warmups, 30 samples, 1024-cycle soak and designated negative guards (software-reference only)'
