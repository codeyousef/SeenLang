#!/usr/bin/env python3
"""Compare the actual four-backend foundation emission, not physical ABIs.

This is a host-side compile-only check. The existing complete child gates own
independent scalar goldens and official validation; no new algorithm is used.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys


MODULE = "p0-accel-foundation"
TARGETS = (("spirv", "vulkan1.2", "spv"), ("cuda", "sm_89", "ptx"),
           ("hip", "gfx90a", "ll"), ("metal", "macos13", "metal"))
BINDING_KEYS = ("group", "binding", "kind", "access", "type", "value_type", "scalar_type", "lanes")
# gpu_types defines UInt32=3, ReadBuffer=1000, WriteBuffer=2000,
# Uniform=4000. These are logical source types, not target argument layouts.
EXPECTED = {
    "name": "foundation", "workgroup": [8, 1, 1],
    "bindings": [
        {"group": 0, "binding": 0, "kind": "storage", "access": "read",
         "type": 1003, "value_type": 3, "scalar_type": 3, "lanes": 1},
        {"group": 0, "binding": 1, "kind": "storage", "access": "write",
         "type": 2003, "value_type": 3, "scalar_type": 3, "lanes": 1},
        {"group": 0, "binding": 2, "kind": "uniform", "access": "read",
         "type": 4003, "value_type": 3, "scalar_type": 3, "lanes": 1},
    ],
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def hex_digest(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    artifacts = args.artifacts.resolve(strict=True)
    sys.path.insert(0, str(root / "scripts"))
    from gpu_certification_io import manifest_from_log, read_bounded

    source = read_bounded(artifacts / "source.seen", 1048576)
    require(source == read_bounded(root / "tests/production_gpu_runtime/vulkan_foundation_kernel.seen", 1048576),
            "retained source differs from the existing common foundation corpus")
    require(read_bounded(artifacts / "source.sha256", 4096).decode().split()[0] == digest(source),
            "retained source hash differs")
    records = []
    for backend, target, extension in TARGETS:
        require(read_bounded(artifacts / (backend + ".status"), 32).strip() == b"0",
                backend + " emission did not succeed")
        binary = read_bounded(artifacts / (backend + "." + extension), 33554432)
        require(binary, backend + " artifact is empty")
        raw = read_bounded(artifacts / (backend + ".json"), 4194304)
        reflection = json.loads(raw)
        manifest = manifest_from_log(artifacts / (backend + ".log"))
        require(manifest["backend"] == reflection["backend"] == backend and
                manifest["target"] == reflection["target"] == target,
                backend + " backend/target identity differs")
        require(manifest["sha256"] == digest(binary) and manifest["reflection_sha256"] == digest(raw) and
                manifest["byte_count"] == len(binary), backend + " manifest does not bind actual output bytes")
        require(manifest["classification"] == "compile-only" and manifest["official_validation"] == "not-run" and
                manifest["hardware_verified"] is False and manifest["gpu_execution_verified"] is False,
                backend + " emission manifest must not claim execution")
        require(reflection["schema"] == "seen-gpu-reflection-v1" and
                reflection["ir_schema"] == "seen-gpu-ir-v1" and reflection["module"] == MODULE and
                reflection["fallback"] == "error", backend + " reflection input contract differs")
        require(reflection["hardware_verified"] is False and reflection["gpu_execution_verified"] is False,
                backend + " reflection must not claim execution")
        require(hex_digest(reflection["ir_sha256"]), backend + " canonical IR digest is malformed")
        entries = reflection["entries"]
        require(type(entries) is list and len(entries) == 1, backend + " selected entry set differs")
        entry = entries[0]
        bindings = entry["bindings"]
        require(type(bindings) is list and len(bindings) == 3, backend + " user-binding set differs")
        logical = {"name": entry["name"], "workgroup": entry["workgroup"],
                   "bindings": [{key: binding[key] for key in BINDING_KEYS} for binding in bindings]}
        require(logical == EXPECTED, backend + " logical entry/workgroup/user-binding contract differs")
        require(hex_digest(entry["abi_sha256"]), backend + " ABI digest is malformed")
        records.append({"backend": backend, "target": target,
                        "artifact_sha256": digest(binary), "reflection_sha256": digest(raw),
                        "ir_sha256": reflection["ir_sha256"], "logical": logical,
                        "abi_sha256": entry["abi_sha256"],
                        "parameter_abi": entry.get("parameter_abi"),
                        "physical_abi_compared": False})
    # gpu_emit_command lowers the same retained bytes and explicit module ID via
    # lowerGpuSource(text, identity), with no backend argument. buildGpuArtifact
    # borrows that immutable module; reflectGpuModule hashes serializeGpuIr(module)
    # before adding backend-specific reflection. Thus these particular complete
    # module IR hashes ARE comparable. This is not a general cross-backend ABI or
    # optimized backend IR equality rule, and does not compare abi_sha256/layouts.
    require(len({record["ir_sha256"] for record in records}) == 1,
            "same source/module lowered through backend-independent canonical IR differs")
    basis_paths = (
        "compiler_seen/src/tools/gpu_emit_command.seen",
        "compiler_seen/src/codegen/gpu_artifact.seen",
        "compiler_seen/src/codegen/gpu_reflection.seen",
        "compiler_seen/src/codegen/gpu_ir_serialize.seen",
        "compiler_seen/src/codegen/gpu_reflection_layout.seen",
        "compiler_seen/src/typechecker/gpu_types.seen",
    )
    report = {"schema": "seen-shared-foundation-logical-reflection-v1",
              "classification": "compile-only", "execution": "native-cpu",
              "source_sha256": digest(source), "module": MODULE, "selected_entry": "foundation",
              "logical_equal": True, "records": records,
              "ir_comparison": {"comparable": True, "equal": True,
                  "basis": "Identical source bytes/module identity, backend-independent lowerGpuSource, borrowed immutable module, canonical serializeGpuIr digest before backend-specific reflection.",
                  "source_owners": {name: digest(read_bounded(root / name, 1048576)) for name in basis_paths}},
              "physical_abi_compared": False, "official_validation": "not-run",
              "hardware_verified": False, "gpu_execution_verified": False,
              "production_certified": False}
    (artifacts / "logical-reflection.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Four actual foundation emissions share logical bindings and comparable canonical IR; physical ABI equality, official validation and GPU execution are not asserted.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, IndexError, TypeError) as error:
        print("shared-foundation logical reflection: " + str(error), file=sys.stderr)
        raise SystemExit(1)
