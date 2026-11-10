#!/usr/bin/env python3
"""Private executor for the native ARM qualification entrypoint, not certification."""
from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import shutil
import subprocess
import sys
import time
import tempfile

import ci_candidate_io as io
import ci_linux_arm64_inputs as inputs
import linux_arm64_asan_admission as arm_asan

MAX_LOG = 16 * 1024 * 1024
MAX_LOG_TOTAL = 32 * 1024 * 1024
CASES = [("contract", p) for p in ("fast", "release", "undefined", "address")] + [
    ("metadata-fuzz", "fast"), ("benchmark-metadata", "fast"), ("benchmark", "release")]
TARGETS = ("linux-arm64", "macos-arm64", "windows-x86_64", "macos-x86_64")
SOURCE_FILES = (
    "tests/misc_root_tests/seen_inf_simd_c.sh", "scripts/compiled_fixture_support.sh",
    "scripts/run_asan_in_hard_memory_scope.sh", "scripts/run_attested_seen.sh",
    "scripts/safe_rebuild.sh", "scripts/run_capped_regression.sh",
    "scripts/linux_arm64_asan_admission.py", "scripts/linux_arm64_asan_control.c",
    "scripts/linux_arm64_milestone_report.py",
    "scripts/ci_candidate_io.py",
    "tests/inference_simd/arm_dot_contract.seen", "tests/inference_simd/arm_capability_fuzz.seen",
    "tests/inference_simd/dot_benchmark_support.seen",
)
ASAN_PREFIX = "asan-contract-address--"


def require(condition, message):
    if not condition:
        raise ValueError(f"native-arm64: {message}")


def identity(data):
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def host():
    require(platform.system() == "Linux" and platform.machine() in {"aarch64", "arm64"},
            "real Linux ARM64 execution is required")


def absolute(value):
    return io.absolute(Path(os.path.abspath(value)))


def state(work):
    return io.decode(io.read_file(work / "state.json", io.JSON_MAX))


def verify_inputs(root, work):
    saved = state(work)
    result = subprocess.run([sys.executable, str(root / "scripts/ci_linux_arm64_inputs.py"),
                             "verify", "--root", str(root), "--candidate", saved["candidate"],
                             "--directory", str(work / "inputs")], stdout=subprocess.DEVNULL,
                            check=False)
    require(result.returncode == 0, "native input/source/candidate verification failed")
    manifest = io.read_file(work / "inputs/native-candidate.json", 1024 * 1024)
    require(hashlib.sha256(manifest).hexdigest() == saved["native_manifest_sha256"],
            "native manifest changed during execution")
    archive = io.read_file(Path(saved["archive"]), 64 * 1024 * 1024)
    require(hashlib.sha256(archive).hexdigest() == saved["archive_sha256"],
            "input archive changed during execution")
    return io.decode(manifest)


def prepare(args):
    candidate, archive, report = map(absolute, (args.candidate, args.archive, args.report))
    with io.directory(report.parent) as parent:
        try:
            os.stat(report.name, dir_fd=parent, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise ValueError("native-arm64: report must be a new file")
    payload = io.read_file(archive, 64 * 1024 * 1024)
    subprocess.run([sys.executable, str(args.root / "scripts/ci_linux_arm64_inputs.py"),
                    "unpack", "--root", str(args.root), "--candidate", str(candidate),
                    "--archive", str(archive), "--output", str(args.work / "inputs")], check=True)
    raw = io.read_file(args.work / "inputs/native-candidate.json", 1024 * 1024)
    io.publish_json(args.work / "state.json", {
        "candidate": str(candidate), "archive": str(archive), "report": str(report),
        "archive_sha256": hashlib.sha256(payload).hexdigest(),
        "native_manifest_sha256": hashlib.sha256(raw).hexdigest(),
    })
    verify_inputs(args.root, args.work)


def virtual(path, maximum=65536):
    return io.read_file(path, maximum, virtual=True).decode("ascii").strip()


def counters(text):
    result = {}
    for line in text.splitlines():
        name, number = line.split()
        require(number.isdecimal() and name not in result, "invalid kernel event counters")
        result[name] = int(number)
    return result


def validate_scope(controls, cap, tasks):
    require(controls["memory.max"].isdecimal() and 0 < int(controls["memory.max"]) <= cap,
            "aggregate memory limit is absent or exceeds the live-derived cap")
    require(controls["memory.swap.max"] == "0" and controls["memory.oom.group"] == "1",
            "zero swap and group OOM containment are required")
    require(controls["pids.max"].isdecimal() and 0 < int(controls["pids.max"]) <= min(tasks, 24),
            "aggregate task limit is absent or excessive")


def scope_snapshot():
    membership = virtual(Path(f"/proc/{os.getpid()}/cgroup"))
    require(membership.startswith("0::/") and "\n" not in membership,
            "unified cgroup membership is required")
    relative = membership[3:]
    require(".." not in Path(relative).parts, "unsafe cgroup membership")
    group = Path("/sys/fs/cgroup") / relative.lstrip("/")
    unit = os.environ["SEEN_MEMORY_GUARD_SCOPE_UNIT"]
    actual = subprocess.run(["systemctl", "--user", "show", unit, "--property=ControlGroup", "--value"],
                            capture_output=True, text=True, timeout=10, check=True).stdout.strip()
    require(actual == relative, "transient unit does not own the executing cgroup")
    controls = {key: virtual(group / key) for key in
                ("memory.max", "memory.swap.max", "memory.oom.group", "pids.max")}
    validate_scope(controls, int(os.environ["SEEN_MEMORY_GUARD_RSS_KB"]) * 1024,
                   int(os.environ["SEEN_MEMORY_GUARD_TASKS_MAX"]))
    require(0 < int(os.environ["SEEN_OPT_VMEM_KB"]) <= 2097152,
            "LLVM budget exceeds 2 GiB")
    require(all(os.environ.get(key) == "1" for key in
                ("SEEN_LOW_MEMORY", "SEEN_JOBS", "SEEN_OPT_JOBS")), "serial workers required")
    soft, _ = resource.getrlimit(resource.RLIMIT_AS)
    require(0 < soft <= int(os.environ["SEEN_MAIN_VMEM_KB"]) * 1024,
            "finite compiler virtual-memory admission is required")
    return {"unit": unit, "path": relative, "controls": controls,
            "memory_events": counters(virtual(group / "memory.events")),
            "pids_events": counters(virtual(group / "pids.events")),
            "memory_peak_bytes": int(virtual(group / "memory.peak")),
            "compiler_rlimit_as_bytes": soft,
            "optimizer_vmem_kb": int(os.environ["SEEN_OPT_VMEM_KB"])}


def capabilities():
    libc = ctypes.CDLL(None, use_errno=True)
    libc.getauxval.argtypes = [ctypes.c_ulong]
    libc.getauxval.restype = ctypes.c_ulong
    ctypes.set_errno(0)
    hwcap = libc.getauxval(16)  # Linux AT_HWCAP, AArch64 ASIMD=1 and SVE=22.
    require(ctypes.get_errno() == 0, "getauxval(AT_HWCAP) failed")
    require(hwcap & (1 << 1), "native ASIMD/NEON capability is required")
    result = {"observer": "Linux AT_HWCAP and PR_SVE_GET_VL", "hwcap": hwcap,
              "neon": "supported", "sve": "unsupported", "sve_vector_bytes": None}
    if hwcap & (1 << 22):
        libc.prctl.restype = ctypes.c_int
        ctypes.set_errno(0)
        value = libc.prctl(ctypes.c_int(51), ctypes.c_ulong(0), ctypes.c_ulong(0),
                           ctypes.c_ulong(0), ctypes.c_ulong(0))
        error = ctypes.get_errno()
        result["sve_prctl_result"], result["sve_prctl_errno"] = value, error
        if value < 0:
            require(error in {errno.EINVAL, errno.ENOSYS}, "SVE state probe failed, not unsupported")
        else:
            length = value & 0xffff
            require(16 <= length <= 256 and length % 16 == 0, "invalid OS SVE vector length")
            result.update(sve="supported", sve_vector_bytes=length)
    return result


def metrics(text):
    values = {}
    for line in text.splitlines():
        match = re.fullmatch(r"BENCH: (candidate_ns|scalar_ns|median_ratio_ppm|allocator_peak_bytes|selected_variant)=([0-9]{1,20})", line)
        if match:
            values.setdefault(match[1], []).append(int(match[2]))
    require(len(values.get("candidate_ns", [])) == len(values.get("scalar_ns", [])) == 30,
            "benchmark lacks thirty real paired samples")
    for key in ("median_ratio_ppm", "allocator_peak_bytes", "selected_variant"):
        require(len(values.get(key, [])) == 1, f"benchmark lacks unique {key}")
    require(all(0 < n <= 10_000_000_000 for key in ("candidate_ns", "scalar_ns") for n in values[key]),
            "benchmark sample duration is invalid")
    ratios = sorted(a * 1_000_000 // b for a, b in zip(values["candidate_ns"], values["scalar_ns"]))
    median = (ratios[14] + ratios[15]) // 2
    require(median == values["median_ratio_ppm"][0] and median <= 1_050_000,
            "paired performance ceiling or measured median failed")
    require(values["selected_variant"][0] in {1, 2}, "native backend was not selected")
    return {key: value if key.endswith("_ns") else value[0] for key, value in values.items()}


def validate_completion(report):
    require(report.get("schema") == "seen-linux-arm64-milestone-v1" and
            report.get("status") == "native-suite-passed" and
            type(report.get("suite_status")) is int and report["suite_status"] == 0 and
            report.get("production_certified") is False and
            report.get("hardware_certified") is False and
            report.get("native_compiler_executed") is True and
            report.get("performance_enforced") is True,
            "complete non-publishing native suite result required")
    expected = {f"{name}-{profile}" for name, profile in CASES}
    stages = report.get("profiles", {})
    require(set(stages) == expected, "all native profiles and fuzz/benchmark executions are required")
    require(all(type(value.get("compile_status")) is int and value["compile_status"] == 0 and
                type(value.get("run_status")) is int and value["run_status"] == 0
                for value in stages.values()), "native profile execution failed")
    require(set(report.get("cross_objects", {})) == set(TARGETS), "cross-object coverage is incomplete")
    observed = report.get("benchmark", {})
    text = "\n".join(f"BENCH: {key}={number}"
                     for key, value in observed.items()
                     for number in (value if isinstance(value, list) else [value]))
    require(metrics(text) == observed, "measured native benchmark report is incomplete")
    require(set(report.get("logs", {})) == {"suite.log"} | {f"{key}.log" for key in expected},
            "complete native logs are required")


def read_evidence(directory, report):
    asan_metadata = report["asan_evidence"]
    require(isinstance(asan_metadata, dict) and 0 < len(asan_metadata) <= 128 and
            all(name.startswith(ASAN_PREFIX) and Path(name).name == name for name in asan_metadata),
            "invalid or excessive native sanitizer evidence inventory")
    expected = {"suite.log"} | {f"{name}-{profile}.log" for name, profile in CASES}
    require(set(report["logs"]) == expected, "native suite log inventory is incomplete")
    payload, totals = {}, {"suite": 0, "asan": 0}
    for name, metadata in {**report["logs"], **asan_metadata}.items():
        require(Path(name).name == name, "unsafe native evidence member")
        sanitizer = name.startswith(ASAN_PREFIX)
        category = "asan" if sanitizer else "suite"
        data = io.read_file(directory / name, min(MAX_LOG, MAX_LOG_TOTAL - totals[category]),
                            allow_empty=sanitizer)
        require(identity(data) == metadata, f"native evidence changed: {name}")
        totals[category] += len(data)
        payload[name] = data
    return payload


def validate_asan_files(root, executable, files):
    """Recheck retained native controls; these bytes are evidence, not authorization."""
    admission = io.decode(files[ASAN_PREFIX + "admission.json"])
    require(admission.get("schema") == "seen-native-arm64-asan-admission-v1" and
            admission.get("accepted") is True and
            admission.get("kernel_before") == admission.get("kernel_after") and
            admission.get("target_rlimit_as") == [arm_asan.CAP, arm_asan.CAP],
            "complete native finite-ASan controls are required")
    owner = admission["kernel_before"]
    address_limits = owner.get("rlimit_as", [])
    require(isinstance(address_limits, list) and len(address_limits) == 2 and
            type(address_limits[0]) is int and
            0 < address_limits[0] <= int(owner.get("limits", {}).get("memory.max", 0)),
            "native admission owner exceeded its ordinary aggregate-sized address budget")
    require({key: admission["target"][key] for key in ("bytes", "sha256")} == executable and
            admission["target"]["elf"]["machine"] == 183,
            "ASan controls do not belong to the executed native address fixture")
    for name in ("linux_arm64_asan_admission.py", "linux_arm64_asan_control.c",
                 "run_asan_in_hard_memory_scope.sh"):
        recorded = admission["helpers"][name]
        require({key: recorded[key] for key in ("bytes", "sha256")} ==
                identity(io.read_file(root / "scripts" / name, MAX_LOG)),
                "native ASan admission source changed")
    require(identity(files[ASAN_PREFIX + "control"]) ==
            {key: admission["control"][key] for key in ("bytes", "sha256")},
            "native sanitizer control executable changed")
    inputs.elf(files[ASAN_PREFIX + "control"])
    for mode in ("clean", "heap", "insufficient"):
        result = io.decode(files[ASAN_PREFIX + mode + ".status.json"])
        require(type(result.get("returncode")) is int and
                result["rlimit_as_bytes"] ==
                (arm_asan.INSUFFICIENT_CAP if mode == "insufficient" else arm_asan.CAP),
                "native sanitizer control limits or status are missing")
        try:
            arm_asan.validate_control(mode, result["returncode"],
                                      files[ASAN_PREFIX + mode + ".out"],
                                      files[ASAN_PREFIX + mode + ".err"])
        except RuntimeError as error:
            raise ValueError(str(error)) from error
    try:
        layout = arm_asan.layout_evidence(files[ASAN_PREFIX + "clean.out"],
                                          files[ASAN_PREFIX + "clean.err"])
    except RuntimeError as error:
        raise ValueError(str(error)) from error
    require(layout == admission["layout"], "retained native shadow layout changed")


def collect_asan_files(root, artifact, raw, executable):
    paths = re.findall(rb"^sanitizer-arm64-admission accepted evidence=(/[^\r\n]+)$", raw, re.M)
    require(len(paths) == 1, "one fresh successful native ASan admission is required")
    proof = Path(paths[0].decode())
    require(proof.parent == artifact and proof.name.startswith("asan-arm64-admission-"),
            "native sanitizer evidence escaped this run's artifact root")
    files = {}
    with io.directory(proof) as fd:
        names = os.listdir(fd)
        require(0 < len(names) <= 128, "native sanitizer evidence inventory exceeds its bound")
        total = 0
        for name in names:
            data = io.read_at(fd, name, min(MAX_LOG, MAX_LOG_TOTAL - total), allow_empty=True)
            total += len(data)
            files[ASAN_PREFIX + name] = data
    require(sum(map(len, files.values())) <= MAX_LOG_TOTAL, "native sanitizer evidence exceeds bound")
    validate_asan_files(root, executable, files)
    return files


def compiler_identity(stdout, source_version):
    lines = stdout.decode().splitlines()
    expected = f"Seen {source_version}"
    require(bool(lines) and lines[0] == expected,
            "actual native compiler version does not match its source")
    return expected


def successful_status(path):
    require(io.read_file(path, 16) == b"0\n", f"fixture did not succeed: {path.name}")


def fixture_directories(root):
    result = set()
    with os.scandir(root) as entries:
        for number, entry in enumerate(entries):
            require(number < 1024, "artifact directory inventory exceeds bound")
            if entry.name.startswith("inf-simd-c."):
                require(entry.is_dir(follow_symlinks=False), "unsafe SIMD fixture directory")
                result.add(Path(entry.path))
    return result


def run(args):
    manifest = verify_inputs(args.root, args.work)
    compiler = args.work / "inputs/seen"
    require(os.environ.get("COMPILER") == str(compiler) and
            os.environ.get("SEEN_CAPPED_REGRESSION_COMPILER") == str(compiler), "compiler binding changed")
    require(os.environ.get("SEEN_PACKAGE_CLIENT") == str(args.work / "inputs/seen-pkg"),
            "adjacent package client binding changed")
    require(os.environ.get("SEEN_COMPILER_SOURCE_ROOT") == str(args.root), "current source root required")
    require(os.environ.get("SEEN_REQUIRE_ARM_PERF") == "1" and
            os.environ.get("FIXTURE_TARGET_CPU") == "native", "native performance policy changed")
    artifact = Path(os.environ["SEEN_ARTIFACT_ROOT"])
    before_dirs = fixture_directories(artifact)
    before, caps = scope_snapshot(), capabilities()
    harness = args.root / "tests/misc_root_tests/seen_inf_simd_c.sh"
    started = time.monotonic_ns()
    with (args.work / "suite.log").open("xb") as log:
        result = subprocess.run(["bash", str(harness)], stdout=log, stderr=subprocess.STDOUT, check=False)
    # Keep the exact native failure status; neither ASan nor a resource failure is a skip.
    if result.returncode:
        return result.returncode if result.returncode > 0 else 128 - result.returncode
    elapsed = time.monotonic_ns() - started
    # Containment read-back diagnostics go to stderr; only compiler stdout is identity.
    with (args.work / "native-version.log").open("xb") as version_log, \
            (args.work / "native-version.err").open("xb") as version_err:
        version_status = subprocess.run(
            ["bash", str(args.root / "scripts/run_attested_seen.sh"), str(compiler), "--version"],
            stdout=version_log, stderr=version_err, check=False).returncode
    if version_status:
        return version_status if version_status > 0 else 128 - version_status
    version_bytes = io.read_file(args.work / "native-version.log", 4096)
    compiler_version = compiler_identity(version_bytes, manifest["source_version"])
    serializer_record = io.read_file(Path(os.environ["SEEN_FORK_SERIALIZER_ATTESTATION"]), 8192)
    serializer = io.read_file(Path(os.environ["SEEN_FORK_SERIALIZER_SO"]), MAX_LOG)
    after = scope_snapshot()
    require(before["path"] == after["path"] and before["controls"] == after["controls"], "scope changed")
    for category, keys in (("memory_events", ("oom", "oom_kill", "oom_group_kill")),
                           ("pids_events", ("max",))):
        require(all(after[category].get(key, 0) == before[category].get(key, 0) for key in keys),
                "kernel containment failure occurred")
    require(capabilities() == caps, "native capabilities changed during qualification")
    new_dirs = fixture_directories(artifact) - before_dirs
    require(len(new_dirs) == 1, "complete suite did not produce exactly one fresh fixture set")
    fixture = new_dirs.pop()
    logs = {"suite.log": (io.read_file(args.work / "suite.log", MAX_LOG), 0o644)}
    stages, capability_rows = {}, {}
    for name, profile in CASES:
        key = f"{name}-{profile}"
        for stage in ("compile", "run"):
            successful_status(fixture / f"{key}.{stage}.status")
        raw = io.read_file(fixture / f"{key}.log", MAX_LOG)
        logs[f"{key}.log"] = (raw, 0o644)
        binary = io.read_file(fixture / key, 64 * 1024 * 1024, executable=True)
        require(binary[:6] == b"\x7fELF\x02\x01" and int.from_bytes(binary[18:20], "little") == 183,
                f"{key} is not a genuinely native AArch64 ELF")
        stages[key] = {"compile_status": 0, "run_status": 0, "executable": identity(binary)}
        if name == "contract":
            rows = [json.loads(line) for line in raw.decode().splitlines() if line.startswith("{")]
            require(len(rows) == 2, "duplicate or missing native capability observations")
            rows = {row["variant"]: row for row in rows}
            require(set(rows) == {"neon", "sve"}, "missing observed ARM fixture capabilities")
            for variant in ("neon", "sve"):
                expected = "verified" if caps[variant] == "supported" else "unsupported"
                require(rows[variant].get("classification") == expected,
                        "fixture capabilities disagree with actual OS/CPU observation")
            capability_rows[profile] = rows
    require(sum(len(data) for data, _ in logs.values()) <= MAX_LOG_TOTAL, "retained logs exceed bound")
    benchmark = metrics(logs["benchmark-release.log"][0].decode())
    require(benchmark["selected_variant"] != 2 or caps["sve"] == "supported", "SVE selected without OS support")
    asan_files = collect_asan_files(args.root, artifact, logs["contract-address.log"][0],
                                    stages["contract-address"]["executable"])
    cross = {}
    for target in TARGETS:
        objects = sorted((fixture / f"cross-{target}").glob("*.ll.o"))
        require(0 < len(objects) <= 128, f"incomplete cross-object set: {target}")
        digest = hashlib.sha256()
        for obj in objects:
            data = io.read_file(obj, 16 * 1024 * 1024)
            if target == "linux-arm64":
                require(data[:6] == b"\x7fELF\x02\x01" and int.from_bytes(data[18:20], "little") == 183,
                        "ARM cross object has wrong architecture")
            digest.update(obj.name.encode() + b"\0" + hashlib.sha256(data).digest())
        cross[target] = {"classification": "compile-only", "object_count": len(objects),
                         "object_inventory_sha256": digest.hexdigest()}
    verify_inputs(args.root, args.work)
    sources = {}
    for relative in SOURCE_FILES:
        sources[relative] = identity(io.read_file(args.root / relative, MAX_LOG))
    io.publish_directory(args.work / "evidence", {
        **logs, **{name: (data, 0o644) for name, data in asan_files.items()},
    })
    report = {"schema": "seen-linux-arm64-milestone-v1", "status": "native-suite-passed",
              "production_certified": False, "hardware_certified": False,
              "native_compiler_executed": True, "input_manifest": manifest,
              "input_manifest_sha256": state(args.work)["native_manifest_sha256"],
              "compiler_version": compiler_version,
              "compiler_version_status": version_status,
              "compiler_version_log": {"path": str(args.work / "native-version.log"), **identity(version_bytes)},
              "serializer": identity(serializer), "serializer_attestation": serializer_record.decode(),
              "host": {"system": platform.system(), "machine": platform.machine(),
                       "kernel_release": platform.release(), "kernel_version": platform.version()},
              "capabilities": caps, "fixture_capabilities": capability_rows,
              "scope_before": before, "scope_after": after, "suite_status": result.returncode,
              "suite_elapsed_ns": elapsed, "fuzz": {"seed": 1101, "duration_seconds": 60,
              "evidence": "successful execution of source-bound metadata-fuzz-fast"},
              "profiles": stages, "benchmark": benchmark, "cross_objects": cross,
              "source_files": sources, "logs": {name: identity(data) for name, (data, _) in logs.items()},
              "asan_evidence": {name: identity(data) for name, data in asan_files.items()},
              "evidence_directory": str(args.work / "evidence"),
              "command": ["bash", str(harness)], "performance_enforced": True}
    io.publish_json(args.work / "suite-result.json", report)
    # Delete only this exact freshly discovered fixture directory after retaining
    # bounded real logs, executable identities, and cross-object inventories.
    shutil.rmtree(fixture)
    return 0


def publish(args):
    manifest = verify_inputs(args.root, args.work)
    report = io.decode(io.read_file(args.work / "suite-result.json", io.JSON_MAX))
    validate_completion(report)
    require(report["input_manifest"] == manifest, "report input identity changed")
    require(report.get("compiler_version_status") == 0 and
            report.get("compiler_version") == f"Seen {manifest['source_version']}",
            "successful actual native compiler identity execution required")
    require(identity(io.read_file(args.work / "native-version.log", 4096)) ==
            {key: report["compiler_version_log"][key] for key in ("bytes", "sha256")},
            "native compiler identity log changed")
    setup = io.read_file(args.work / "setup.log", MAX_LOG)
    payload = {name: (data, 0o644) for name, data in
               read_evidence(args.work / "evidence", report).items()}
    validate_asan_files(args.root, report["profiles"]["contract-address"]["executable"],
                        {name: data for name, (data, _) in payload.items() if name.startswith(ASAN_PREFIX)})
    payload["native-version.log"] = (io.read_file(args.work / "native-version.log", 4096), 0o644)
    payload["setup.log"] = (setup, 0o644)
    destination = Path(state(args.work)["report"])
    io.publish_directory(destination.parent / "evidence", payload)
    report["setup_log"] = {"path": str(args.work / "setup.log"), **identity(setup)}
    report["guarded_route_status"] = 0
    io.publish_json(Path(state(args.work)["report"]), report)


def verify_report(args):
    """Verify same-run transported proof without executing ARM or the x64 candidate."""
    report_path = absolute(args.report)
    report = io.decode(io.read_file(report_path, io.JSON_MAX))
    validate_completion(report)
    require(report.get("host", {}).get("system") == "Linux" and
            report.get("host", {}).get("machine") in {"aarch64", "arm64"} and
            report.get("guarded_route_status") == 0,
            "completed real native guarded route required")
    artifact = absolute(os.environ["SEEN_ARTIFACT_ROOT"])
    with tempfile.TemporaryDirectory(prefix="native-arm-receipt-", dir=artifact) as directory:
        verified = Path(directory) / "inputs"
        inputs.unpack(args.root, absolute(args.candidate), absolute(args.archive), verified)
        manifest, raw, _ = inputs.verify(args.root, absolute(args.candidate), verified)
        require(report["input_manifest"] == manifest and
                report["input_manifest_sha256"] == hashlib.sha256(raw).hexdigest(),
                "native qualification belongs to another source or candidate")
    require(set(report["source_files"]) == set(SOURCE_FILES), "native source evidence is incomplete")
    for relative, metadata in report["source_files"].items():
        require(identity(io.read_file(args.root / relative, MAX_LOG)) == metadata,
                f"native qualification source changed: {relative}")
    before, after = report["scope_before"], report["scope_after"]
    require(before["path"] == after["path"] and before["unit"] == after["unit"] and
            before["controls"] == after["controls"], "native qualification scope changed")
    validate_scope(before["controls"], int(before["controls"]["memory.max"]), 24)
    require(0 < before["optimizer_vmem_kb"] <= 2097152 and
            0 < before["compiler_rlimit_as_bytes"] <= int(before["controls"]["memory.max"]),
            "native compiler or LLVM address budget exceeds its finite bound")
    for category, keys in (("memory_events", ("max", "oom", "oom_kill", "oom_group_kill")),
                           ("pids_events", ("max",))):
        require(all(before[category].get(key, 0) == after[category].get(key, 0) == 0 for key in keys),
                "native qualification encountered a kernel resource fault")
    payload = read_evidence(report_path.parent / "evidence", report)
    validate_asan_files(args.root, report["profiles"]["contract-address"]["executable"], payload)
    for name, metadata in (("native-version.log", report["compiler_version_log"]),
                           ("setup.log", report["setup_log"])):
        data = io.read_file(report_path.parent / "evidence" / name, MAX_LOG)
        require(identity(data) == {key: metadata[key] for key in ("bytes", "sha256")},
                f"native route evidence changed: {name}")
    require(type(report["compiler_version_status"]) is int and report["compiler_version_status"] == 0 and
            report["compiler_version"] == f"Seen {report['input_manifest']['source_version']}",
            "actual native compiler version evidence is missing")
    print("Native Linux ARM64 source/candidate, full-suite, finite-ASan and performance evidence verified.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "verify-inputs", "run", "publish", "verify-report"))
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--work", type=Path)
    for option in ("candidate", "archive", "report"):
        parser.add_argument(f"--{option}")
    args = parser.parse_args()
    try:
        if args.action == "verify-report":
            require(all((args.candidate, args.archive, args.report)), "verify-report requires all inputs")
            verify_report(args)
            return 0
        host()
        require(args.work is not None, "native execution requires its owned work directory")
        with io.directory(args.root), io.directory(args.work):
            pass
        if args.action == "prepare":
            require(all((args.candidate, args.archive, args.report)), "prepare requires all inputs")
            prepare(args)
        elif args.action == "verify-inputs":
            verify_inputs(args.root, args.work)
        elif args.action == "run":
            return run(args)
        else:
            publish(args)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"native-arm64: {error}", file=sys.stderr)
        return 126
    return 0


if __name__ == "__main__":
    sys.exit(main())
