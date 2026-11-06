#!/usr/bin/env python3
"""Retain this aggregate's child observations without interpreting PASS text."""
import argparse
import hashlib
import json
import re
import shutil
from pathlib import Path

MAX_FILE = 64 * 1024**2
MAX_REPORT = 2 * 1024**2
PROFILES = r"(?:fast|release|undefined|address)"
# Baseline run-status files are written by the existing execution harness, not
# inferred from child exit zero, compilation, source text, or success wording.
BASELINES = {
    "seen_gpu_runtime_a": rf"backend-{PROFILES}\.run\.status",
    "seen_gpu_runtime_a_vulkan": rf"vulkan_provider-{PROFILES}\.run\.status",
    "seen_gpu_runtime_b": rf"vulkan_resource-{PROFILES}\.run\.status",
    "seen_gpu_runtime_c": rf"vulkan_timeline-{PROFILES}\.run\.status",
    "seen_gpu_runtime_d": rf"stream-{PROFILES}\.run\.status",
    "seen_gpu_runtime_e": rf"pipeline-{PROFILES}\.run\.status",
    "seen_gpu_runtime_f": rf"graph-{PROFILES}\.run\.status",
    "seen_gpu_runtime_g": rf"recovery-{PROFILES}\.run\.status",
    # H overwrites trace-*.run.status while executing its required negative
    # probes. Use its retained reference statuses, never those expected failures.
    "seen_gpu_runtime_h": rf"trace-{PROFILES}-reference\.status",
    "seen_gpu_vulkan_mapping": rf"mapping-{PROFILES}\.run\.status",
    "seen_gpu_vulkan_transfer": rf"transfer-{PROFILES}\.run\.status",
    "seen_gpu_vulkan_staging": rf"staging-{PROFILES}\.run\.status",
    "seen_cuda_event_query": rf"query-{PROFILES}-reference\.status",
    "seen_gpu_cuda_staging": rf"staging-{PROFILES}-reference\.status",
    "seen_gpu_cuda_peer": rf"peer-{PROFILES}-reference\.status",
}
REPORTS = ("runtime-f-evidence.json", "runtime-h-evidence.json",
           "lifecycle-fast.json", "lifecycle-release.json",
           "lifecycle-undefined.json", "lifecycle-address.json")


def bounded(path, ceiling=MAX_FILE):
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"not regular evidence: {path}")
    with path.open("rb") as source:
        data = source.read(ceiling + 1)
    if len(data) > ceiling:
        raise ValueError(f"evidence exceeds ceiling: {path}")
    return data


def identity(path):
    data = bounded(path)
    return {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}


def directories(root, prefix):
    return {path.name for path in root.glob(prefix + ".*")
            if path.is_dir() and not path.is_symlink()}


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def append_check(root, name, status, execution, classification, logs, details):
    path = root / "checks.json"
    rows = json.loads(bounded(path, 256 * 1024))
    rows.append({"name": name, "exit_status": status, "execution": execution,
                 "classification": classification, "required": False,
                 "logs": logs, "details": details})
    write_json(path, rows)


def collect(args, root, aggregate):
    before = set(json.loads(bounded(args.snapshot, 256 * 1024)))
    created = sorted(directories(root, args.prefix) - before)
    if args.status == 0 and len(created) != 1:
        raise ValueError("successful child did not retain exactly one current-run artifact directory")
    destination = aggregate / "children" / args.child
    destination.mkdir(parents=True)
    manifest = {"child": args.child, "child_exit_status": args.status,
                "hardware_opt_in": args.hardware_opt_in, "boundary": args.boundary,
                "production_certified": False, "directories": [],
                "successful_baselines": [], "hardware_baselines": [], "nested_reports": []}
    logs = [args.child + ".log", args.child + ".status"]
    hardware_logs = []
    pattern = BASELINES.get(args.child)
    if pattern is None:
        # A future canonical Runtime-C is still required and retained, but it
        # cannot silently inherit a device-execution claim from its name.
        manifest["boundary_limit"] = "No known baseline status contract for this child."
    for directory in created:
        source = root / directory
        item = {"path": directory, "statuses": [], "reports": []}
        statuses = sorted(source.glob("*.status"))
        if len(statuses) > 4096:
            raise ValueError("child status inventory exceeds ceiling")
        for path in statuses:
            status = int(bounded(path, 32).decode().strip())
            if not 0 <= status <= 255:
                raise ValueError("invalid child native status")
            observed = {"path": path.name, "exit_status": status, **identity(path)}
            # Inventory all statuses; baseline bytes below also enter the
            # aggregate's declared logs so disposable cleanup cannot erase them.
            stem = path.name.removesuffix(".run.status").removesuffix(".compile.status").removesuffix(".status")
            candidates = [source / (stem + ".log")]
            if path.name.endswith(".compile.status"):
                candidates.insert(0, source / (stem + ".compile.log"))
            for log in candidates:
                if log.is_file():
                    observed["log"] = {"path": log.name, **identity(log)}
                    break
            item["statuses"].append(observed)
            baseline = pattern is not None and re.fullmatch(pattern, path.name)
            pipeline_hardware = args.child == "seen_gpu_runtime_e" and re.fullmatch(
                rf"pipeline-hardware-{PROFILES}\.run\.status", path.name)
            if baseline or pipeline_hardware:
                if status == 0 and "log" not in observed:
                    raise ValueError("successful baseline has no retained native log")
                relative = Path("children") / args.child / directory
                target = aggregate / relative
                target.mkdir(exist_ok=True)
                names = [path.name]
                if "log" in observed:
                    names.append(observed["log"]["path"])
                for name in names:
                    shutil.copyfile(source / name, target / name)
                    (hardware_logs if pipeline_hardware else logs).append(str(relative / name))
                if status == 0:
                    key = "hardware_baselines" if pipeline_hardware else "successful_baselines"
                    manifest[key].append({"directory": directory, **observed})
        for name in REPORTS:
            path = source / name
            if not path.exists():
                continue
            data = bounded(path, MAX_REPORT)
            value = json.loads(data)
            if not isinstance(value, dict):
                raise ValueError("nested child report must be an object")
            relative = Path("children") / args.child / directory / name
            target = aggregate / relative
            target.parent.mkdir(exist_ok=True)
            target.write_bytes(data)
            report = {"path": str(relative), **identity(target)}
            item["reports"].append(report)
            manifest["nested_reports"].append(report)
            logs.append(str(relative))
        manifest["directories"].append(item)
    manifest_name = str(Path("children") / args.child / "observations.json")
    write_json(aggregate / manifest_name, manifest)
    # At most two general runtime reports or four lifecycle reports per child.
    if len(logs) + 1 > 16:
        raise ValueError("nested report catalog exceeds evidence row ceiling")
    logs.append(manifest_name)
    executed = bool(manifest["successful_baselines"])
    if args.status == 0 and pattern is not None and args.boundary != "cuda-hardware" and not executed:
        raise ValueError("successful runtime child has no completed baseline witness")
    if args.boundary == "cuda-hardware" and args.hardware_opt_in != "1":
        executed = False
    if executed:
        execution = "native-cpu" if args.boundary in ("cpu", "software-reference") else "native-gpu"
        classification = "experimental-hardware" if args.boundary == "cuda-hardware" else "verified"
    else:
        execution, classification = "none", "unsupported"
    append_check(aggregate, args.child + "-" + args.boundary, 0 if executed else 126,
                 execution, classification, logs,
                 {"successful_baselines": len(manifest["successful_baselines"]),
                  "child_exit_status": args.status, "production_certified": False,
                  "evidence_boundary": "Completed baseline profiles only; the required full-child row remains authoritative. Software-reference is not physical GPU qualification; opted-in CUDA evidence is experimental, not certification.",
                  "hardware_opt_in": args.hardware_opt_in})
    if args.child == "seen_gpu_runtime_e":
        hardware = args.hardware_opt_in == "rtx4090" and bool(manifest["hardware_baselines"])
        append_check(aggregate, "vulkan-pipeline-physical-hardware", 0 if hardware else 126,
                     "native-gpu" if hardware else "none",
                     "experimental-hardware" if hardware else "unsupported",
                     [manifest_name, args.child + ".log", args.child + ".status", *hardware_logs],
                     {"production_certified": False,
                      "hardware_opt_in": args.hardware_opt_in,
                      "evidence_boundary": "Only completed pipeline-hardware profiles; reference profiles never establish physical execution."})
    if args.child == "seen_gpu_cuda_peer":
        # This entry point has no structured distinct-device execution report.
        # Do not turn its same-device successes or GPU inventory into a peer
        # positive; leave that boundary unexecuted even after a hardware run.
        append_check(aggregate, "cuda-distinct-device-peer-positive", 126, "none",
                     "unsupported", [manifest_name, args.child + ".log"],
                     {"production_certified": False,
                      "evidence_boundary": "No structured positive distinct-device route witness collected; single-device transfers and adverse SDK capability probes are not peer-positive evidence."})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--aggregate", type=Path, required=True)
    parser.add_argument("--prefix", required=True)
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--before", action="store_true")
    parser.add_argument("--child")
    parser.add_argument("--status", type=int)
    parser.add_argument("--boundary", choices=("cpu", "software-reference", "cuda-hardware"))
    parser.add_argument("--hardware-opt-in", default="unset")
    args = parser.parse_args()
    root = args.artifacts.resolve(strict=True)
    aggregate = args.aggregate.resolve(strict=True)
    if not aggregate.is_relative_to(root) or not args.snapshot.resolve().is_relative_to(aggregate):
        raise ValueError("child evidence escaped aggregate artifacts")
    if not re.fullmatch(r"[a-z0-9-]+", args.prefix):
        raise ValueError("unsafe child artifact prefix")
    if args.before:
        write_json(args.snapshot, sorted(directories(root, args.prefix)))
    else:
        if not args.child or not re.fullmatch(r"seen_[a-z0-9_]+", args.child):
            raise ValueError("invalid child name")
        if args.status is None or not 0 <= args.status <= 255 or args.boundary is None:
            raise ValueError("missing child status/boundary")
        collect(args, root, aggregate)


if __name__ == "__main__":
    main()
