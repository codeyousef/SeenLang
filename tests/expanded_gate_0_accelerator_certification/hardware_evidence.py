#!/usr/bin/env python3
"""Strict development-only RTX4090 observations for canonical accelerator D.

Native exit statuses and raw logs are authoritative, including sanitizer failures
that occur after a complete hardware observation. Never turn those into passes.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import statistics
import sys

from logical_reflection import BINDING_KEYS, EXPECTED, MODULE


PROFILES = ("fast", "release", "undefined", "address")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def bounded(path, maximum=64 * 1024**2):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= maximum,
            "missing, linked or oversized artifact: " + str(path))
    return path.read_text(errors="strict")


def load(path):
    return json.loads(bounded(path, 4 * 1024**2))


def save(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def uuid(value):
    require(isinstance(value, str), "UUID is not a string")
    normalized = value.removeprefix("GPU-").replace("-", "").lower()
    require(re.fullmatch(r"[0-9a-f]{32}", normalized) and int(normalized, 16) != 0,
            "invalid/empty physical-device UUID")
    return normalized


def inventory(artifacts):
    require(bounded(artifacts / "nvidia-inventory.status", 32).strip() == "0",
            "actual NVIDIA driver inventory command failed")
    matches = []
    for line in bounded(artifacts / "nvidia-inventory.log", 65536).splitlines():
        fields = [field.strip() for field in line.split(",")]
        require(len(fields) == 3, "malformed NVIDIA inventory")
        if fields[0] == "NVIDIA GeForce RTX 4090":
            matches.append({"name": fields[0], "uuid": uuid(fields[1]), "driver_version": fields[2]})
    require(len(matches) == 1, "requires exactly one installed NVIDIA GeForce RTX 4090")
    require(matches[0]["driver_version"], "empty NVIDIA driver version")
    return matches[0]


def select_icd(artifacts):
    explicit = os.environ.get("SEEN_NVIDIA_VULKAN_ICD")
    candidates = [Path(explicit)] if explicit else sorted(
        path for directory in (Path("/etc/vulkan/icd.d"), Path("/usr/share/vulkan/icd.d"))
        for path in directory.glob("*nvidia*.json"))
    valid = []
    for path in candidates:
        require(path.is_absolute(), "NVIDIA ICD manifest must be absolute")
        require(path.is_file() and not path.is_symlink(), "NVIDIA ICD must be a regular installed manifest")
        manifest = load(path)
        library = manifest["ICD"]["library_path"]
        require(isinstance(library, str) and re.search(r"(?:libGLX_nvidia|libvulkan_nvidia)\.so(?:\.|$)", library),
                "ICD is not an installed NVIDIA driver (software fallback forbidden)")
        valid.append((path, library))
    require(valid, "installed NVIDIA Vulkan ICD unavailable; no software fallback")
    path, library = valid[0]
    save(artifacts / "nvidia-icd.json", {"manifest": str(path), "library": library,
        "manifest_sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "software_fallback": False})
    (artifacts / "nvidia-icd.path").write_text(str(path) + "\n")
    print(str(path))


def assemble(root, artifacts):
    sys.path.insert(0, str(root / "scripts"))
    from check_gpu_ptx_toolchain import invoke, load_profile, require as tool_require, verify_installation
    profile = load_profile()
    identity = verify_installation(profile)
    shared = artifacts / "shared-foundation"
    status, output, diagnostic = invoke(profile, "ptxas", ["--gpu-name=sm_89", "--warn-on-spills",
        str(shared / "cuda.ptx"), "--output-file", str(shared / "foundation.cubin")])
    print(output, end="")
    print(diagnostic, end="", file=sys.stderr)
    save(shared / "ptxas-identity.json", identity)
    tool_require(status == 0 and not diagnostic, "foundation official ptxas assembly failed")
    tool_require((shared / "foundation.cubin").is_file() and
                 20 <= (shared / "foundation.cubin").stat().st_size <= 33554432,
                 "official ptxas output extent invalid")


def foundation(root, artifacts):
    sys.path.insert(0, str(root / "scripts"))
    from gpu_certification_io import manifest_from_log
    shared = artifacts / "shared-foundation"
    source = (shared / "source.seen").read_bytes()
    require(source == (root / "tests/production_gpu_runtime/vulkan_foundation_kernel.seen").read_bytes(),
            "retained common foundation source differs")
    records = []
    for backend, target, extension in (("cuda", "sm_89", "ptx"), ("spirv", "vulkan1.2", "spv")):
        require(bounded(shared / (backend + ".status"), 32).strip() == "0", backend + " emission failed")
        binary_path = shared / (backend + "." + extension)
        require(binary_path.is_file() and 0 < binary_path.stat().st_size <= 33554432, "invalid emitted artifact")
        reflection_path = shared / (backend + ".json")
        reflection = load(reflection_path)
        manifest = manifest_from_log(shared / (backend + ".log"))
        require(manifest["backend"] == reflection["backend"] == backend and
                manifest["target"] == reflection["target"] == target, "backend/target mismatch")
        binary_hash = hashlib.sha256(binary_path.read_bytes()).hexdigest()
        reflection_hash = hashlib.sha256(reflection_path.read_bytes()).hexdigest()
        require(manifest["sha256"] == binary_hash and manifest["reflection_sha256"] == reflection_hash and
                manifest["byte_count"] == binary_path.stat().st_size, "emission manifest does not bind bytes")
        require(reflection["schema"] == "seen-gpu-reflection-v1" and reflection["ir_schema"] == "seen-gpu-ir-v1"
                and reflection["module"] == MODULE and reflection["fallback"] == "error", "reflection contract mismatch")
        require(reflection["hardware_verified"] is False and reflection["gpu_execution_verified"] is False,
                "emission is not hardware execution evidence")
        entries = reflection["entries"]
        require(isinstance(entries, list) and len(entries) == 1, "selected entry mismatch")
        entry = entries[0]
        logical = {"name": entry["name"], "workgroup": entry["workgroup"],
                   "bindings": [{key: binding[key] for key in BINDING_KEYS} for binding in entry["bindings"]]}
        require(logical == EXPECTED, "common foundation entry/workgroup/user bindings differ")
        require(re.fullmatch(r"[0-9a-f]{64}", reflection["ir_sha256"]), "invalid canonical IR digest")
        records.append({"backend": backend, "target": target, "artifact_sha256": binary_hash,
                        "reflection_sha256": reflection_hash, "ir_sha256": reflection["ir_sha256"]})
    require(records[0]["ir_sha256"] == records[1]["ir_sha256"], "identical source/module canonical IR differs")
    save(shared / "hardware-foundation.json", {"source_sha256": hashlib.sha256(source).hexdigest(),
        "module": MODULE, "entry": "foundation", "records": records, "logical_equal": True,
        "physical_abi_compared": False, "production_certified": False})


def record(text, prefix):
    matches = [json.loads(line[len(prefix):]) for line in text.splitlines() if line.startswith(prefix)]
    require(len(matches) == 1 and isinstance(matches[0], dict), "expected exactly one " + prefix.strip())
    return matches[0]


def number(value, minimum=0):
    return type(value) in (int, float) and math.isfinite(value) and value >= minimum


def common(consumer, backend, expected):
    require(consumer["backend"] == backend and consumer["native_execution"] is True and
            consumer["target"] == "rtx4090", "actual physical backend execution not observed")
    require(consumer["device_name"] == expected["name"] and uuid(consumer["device_uuid"]) == expected["uuid"],
            "native device name/UUID differs from actual NVIDIA inventory")
    require(consumer["driver_version"] not in (None, "", 0), "missing native driver version")
    for field, value in (("warmup_count", 5), ("measured_count", 30), ("soak_count", 1024)):
        require(type(consumer[field]) is int and consumer[field] == value, "incorrect " + field)
    for field in ("allocations_drained", "atomics_drained"):
        require(consumer[field] is True, "Seen ownership not drained: " + field)
    require(consumer["physical_vram_bytes"] is None, "physical VRAM must remain explicitly unmeasured")
    medians = {}
    for clock in ("gpu", "cpu"):
        samples = consumer[clock + "_nanoseconds"]
        require(isinstance(samples, list) and len(samples) == 30 and all(number(value) for value in samples),
                "invalid raw30 " + clock + " samples")
        medians[clock + "_nanoseconds"] = statistics.median(samples)
    return medians


def cuda(consumer):
    require(consumer["compute_capability"] == [8, 9], "requires actual sm_89 CUDA device")
    require(consumer["successful_hot_cycles"] == 1059 and consumer["native_query_count"] >= 1059,
            "CUDA warmup/measured/soak native cycles incomplete")
    require(consumer["transfer"] == {"cold_upload_bytes": 300, "final_readback_bytes": 152,
        "hot_upload_bytes": 0, "hot_readback_bytes": 0, "uniform_argument_bytes_per_launch": 16},
        "CUDA exact transfer accounting differs")
    require(consumer["device_allocation_bytes"] == 300 and consumer["host_arena_bytes"] == 2048,
            "CUDA requested allocation bounds differ")
    for field in ("fallback_count", "cpu_fallback_count", "fallback_attempt_count", "native_status", "checked_fault"):
        require(type(consumer[field]) is int and consumer[field] == 0, "nonzero CUDA " + field)
    require(consumer["fallback_policy"] == "error" and consumer["resources_drained"] is True and
            consumer["validated_elements"] == 37 and consumer["physical_vram_status"] == "unmeasured",
            "CUDA output/ownership/fallback/measurement contract failed")
    require(type(consumer["self_peak_rss_kib"]) is int and consumer["self_peak_rss_kib"] > 0 and
            consumer["rss_scope"] == "native-process-rusage-self", "CUDA native process RSS was not observed")


def vulkan(consumer, native):
    require(consumer["successful_hot_cycles"] == native["hot_submissions"] == 1054 and
            native["hot_polls"] >= 1054 and native["hot_command_recordings"] == 1,
            "Vulkan actual persistent native work incomplete")
    require(consumer["cache_restored"] is True and native["pipeline_cache_create_calls"] >= 2 and
            native["pipeline_cache_nonempty_initial_calls"] >= 1 and
            native["pipeline_cache_initial_bytes"] == consumer["cache_payload_bytes"] > 0,
            "Vulkan cache payload was not restored into SDK")
    require(native["drained"] is True and native["native_live_objects"] == 0 and
            native["requested_device_memory_live_bytes"] == 0, "Vulkan native owner drain failed")
    for field in ("hot_acquisitions", "hot_descriptor_sets_attempted", "forbidden_calls",
                  "forbidden_fence_calls", "forbidden_global_wait_calls"):
        require(type(native[field]) is int and native[field] == 0, "forbidden Vulkan native work: " + field)
    for field in ("resource_drain", "per_cycle_allocation_stable"):
        require(consumer[field] is True, "Vulkan ownership contract failed: " + field)
    fallback = consumer["fallback"]
    require(set(fallback) == {"admitted", "succeeded", "failed", "abandoned", "pending_attempts",
                             "live_attempts", "out_of_memory_succeeded", "device_lost_succeeded"} and
            all(type(value) is int and value == 0 for value in fallback.values()), "Vulkan fallback used")
    require(consumer["transfer"] == {"initial_host_upload_bytes": 152, "hot_host_upload_bytes": 0,
        "final_readback_bytes": 148, "timestamp_query_result_bytes_per_query_read": 32},
        "Vulkan exact transfer accounting differs")
    require(native["timestamp_query_read_calls"] >= 1054 and
            native["timestamp_query_result_requested_bytes"] == 32 * native["timestamp_query_read_calls"],
            "Vulkan native timestamp transfer accounting differs")
    require(number(native["requested_device_memory_peak_bytes"], 1) and
            native["requested_device_memory_total_bytes"] >= native["requested_device_memory_peak_bytes"],
            "Vulkan requested device allocation accounting absent")
    require(native["physical_vram_bytes"] is None and native["physical_vram_status"] == "not_measured" and
            consumer["physical_vram_measurement"] == "not-measured", "Vulkan physical VRAM unmeasured boundary lost")


def observe(artifacts, backend, profile):
    stem = backend + "-" + profile
    report = {"backend": backend, "profile": profile, "production_certified": False,
              "classification": "unsupported", "native_execution_validated": False,
              "validation_errors": [], "self_peak_rss_kib": None, "rss_status": "unmeasured",
              "physical_vram_bytes": None, "physical_vram_status": "unmeasured"}
    status = 126
    try:
        raw_status = bounded(artifacts / (stem + ".run.status"), 32).strip()
        require(re.fullmatch(r"[0-9]{1,3}", raw_status) and int(raw_status) <= 255, "invalid native exit status")
        status = int(raw_status)
        report["native_exit_status"] = status
        text = bounded(artifacts / (stem + ".run.log"))
        consumer = record(text, "GPU_FOUNDATION_CUDA_EVIDENCE " if backend == "cuda" else "GPU_RUNTIME_H_EVIDENCE ")
        report["consumer"] = consumer
        expected = inventory(artifacts)
        report["driver_inventory"] = expected
        report["medians"] = common(consumer, backend, expected)
        if backend == "cuda":
            cuda(consumer)
            report.update(self_peak_rss_kib=consumer["self_peak_rss_kib"], rss_status="actual-child-getrusage")
        else:
            native = record(text, "GPU_RUNTIME_H_TRACE ")
            report["native_trace"] = native
            vulkan(consumer, native)
            rss = native.get("self_peak_rss_kib")
            if rss is not None:
                require(type(rss) is int and rss >= 0, "invalid actual child RSS")
                report.update(self_peak_rss_kib=rss, rss_status="actual-child-getrusage")
        report.update(classification="experimental-hardware", native_execution_validated=True)
    except (OSError, ValueError, KeyError, TypeError, IndexError, OverflowError) as error:
        report["validation_errors"].append(str(error))
    if status:
        report["validation_errors"].append("native process failed (sanitizer/driver failures are unsuppressed): " + str(status))
    final_status = status or (1 if report["validation_errors"] else 0)
    report["exit_status"] = final_status
    filename = stem + ".hardware.json"
    checks_path = artifacts / "checks.json"
    checks = load(checks_path)
    name = "rtx4090-" + stem
    require(not any(row["name"] == name for row in checks), "duplicate hardware check")
    checks.append({"name": name, "exit_status": final_status, "required": True,
        "classification": report["classification"],
        "execution": "native-gpu" if report["native_execution_validated"] else "none",
        "logs": [stem + ".run.log", stem + ".run.status", filename],
        "details": {"production_certified": False, "native_exit_status": status,
                    "observation_validated": report["native_execution_validated"],
                    "validation_errors": report["validation_errors"]}})
    save(checks_path, checks)
    # Publish the completion marker only after the required row is written to
    # checks.json; the shell otherwise records an observer infrastructure error.
    save(artifacts / filename, report)
    print(json.dumps({"backend": backend, "profile": profile, "exit_status": final_status,
                      "native_execution_validated": report["native_execution_validated"]}))
    return final_status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("operation", choices=("select-icd", "assemble", "foundation", "observe"))
    parser.add_argument("--backend", choices=("cuda", "vulkan"))
    parser.add_argument("--profile", choices=PROFILES)
    args = parser.parse_args()
    if args.operation == "select-icd":
        select_icd(args.artifacts)
    elif args.operation == "assemble":
        assemble(args.root, args.artifacts)
    elif args.operation == "foundation":
        foundation(args.root, args.artifacts)
    else:
        require(args.backend and args.profile, "observation requires backend and profile")
        return observe(args.artifacts, args.backend, args.profile)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, TypeError, IndexError, OverflowError) as error:
        print("hardware evidence: " + str(error), file=sys.stderr)
        raise SystemExit(1)
