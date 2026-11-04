#!/usr/bin/env python3
"""Journal an unverified Stage-3 candidate, check group receipts, then install it.

Hashes bind bytes, not executions. Only the owning CI jobs authorize completion;
record-group is called after successful canonical acceptance in its live scope.
This helper never certifies a bootstrap, writes a full stamp, or publishes a release.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import os
import re
import resource
import selectors
import subprocess
import sys
import tarfile
import time
from pathlib import Path

from ci_candidate_io import (
    JSON_MAX, absolute, decode, directory, encode, entries, fail, install_files,
    invalidate_stamp,
    metadata, publish_directory, publish_json, read_at, read_file, unchanged_directory,
)
from release_toolchain_artifact import ArtifactError, SHA256, validate_identity

SCHEMA = "seen-bootstrap-candidate-v1"
RECEIPT_SCHEMA = "seen-acceptance-receipt-v1"
GROUPS = ("core", "io-simd", "gpu-emitter", "gpu-runtime-async")
COMPILER_MAX = 32 * 1024 * 1024
CLIENT_MAX = 16 * 1024 * 1024
TOTAL_MAX = 64 * 1024 * 1024
BINARY_LIMITS = {"seen": COMPILER_MAX, "seen-pkg": CLIENT_MAX}
# Existing immutable bootstrap inputs, not newly invented execution evidence.
SOURCE_LIMITS = {
    "bootstrap/stage1_frozen": COMPILER_MAX,
    "bootstrap/stage1_frozen.sha256": JSON_MAX,
    "bootstrap/stage1_frozen_v3": COMPILER_MAX,
    "bootstrap/stage1_frozen_v3.sha256": JSON_MAX,
    "bootstrap/stage1_frozen.compatibility-manifest.json": JSON_MAX,
    "bootstrap/stage1_frozen.compatibility-manifest.sha256": JSON_MAX,
    "bootstrap/stage1_frozen.seen-pkg-source.tar.gz": CLIENT_MAX,
    "bootstrap/stage1_frozen.seen-pkg-source.sha256": JSON_MAX,
    "bootstrap/source_commit.txt": JSON_MAX,
    "releases/compatibility-manifest.json": JSON_MAX,
}
SERIAL_VARIABLES = (
    "SEEN_JOBS", "SEEN_OPT_JOBS", "RAYON_NUM_THREADS", "OMP_NUM_THREADS",
    "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "NUMEXPR_NUM_THREADS",
    "VECLIB_MAXIMUM_THREADS", "BLIS_NUM_THREADS", "GOMAXPROCS", "RUST_TEST_THREADS",
    "CARGO_BUILD_JOBS", "RPM_BUILD_NCPUS", "SEEN_LLD_THREADS", "SEEN_THINLTO_JOBS",
)
SCOPE_PATTERN = re.compile(r"seen-memory-guard-(0|[1-9][0-9]*)-([1-9][0-9]*)\.scope")


def command(root: Path, *args: str) -> str:
    environment = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    deadline = time.monotonic() + 60
    with subprocess.Popen(args, cwd=root, env=environment, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT) as process:
        try:
            data = bytearray()
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                while True:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0 or not selector.select(remaining):
                        raise subprocess.TimeoutExpired(args, 60)
                    chunk = os.read(process.stdout.fileno(), JSON_MAX + 1 - len(data))
                    if not chunk:
                        break
                    data.extend(chunk)
                    if len(data) > JSON_MAX:
                        fail("command output exceeds size limit")
            if process.wait(timeout=max(0, deadline - time.monotonic())):
                fail(f"command failed: {args[0]}")
            return data.decode("utf-8").strip()
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()


def source_identity(root: Path) -> dict:
    with directory(root) as fd:
        if command(root, "git", "rev-parse", "--show-toplevel") != str(root):
            fail("root is not the exact Git checkout")
        if command(root, "git", "status", "--porcelain=v1", "--untracked-files=all"):
            fail("source checkout is not clean")
        commit = command(root, "git", "rev-parse", "HEAD")
        tree = command(root, "git", "rev-parse", "HEAD^{tree}")
        validate_identity(commit, tree, "x86-64")
        required = {}
        contents = {}
        for relative, maximum in SOURCE_LIMITS.items():
            data = read_file(root / relative, maximum)
            required[relative] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
            contents[relative] = data
        for target, checksum in (
            ("bootstrap/stage1_frozen", "bootstrap/stage1_frozen.sha256"),
            ("bootstrap/stage1_frozen_v3", "bootstrap/stage1_frozen_v3.sha256"),
            ("bootstrap/stage1_frozen.compatibility-manifest.json",
             "bootstrap/stage1_frozen.compatibility-manifest.sha256"),
            ("bootstrap/stage1_frozen.seen-pkg-source.tar.gz",
             "bootstrap/stage1_frozen.seen-pkg-source.sha256"),
        ):
            fields = contents[checksum].decode("ascii").split()
            if len(fields) != 2 or fields[1] != target or fields[0] != required[target]["sha256"]:
                fail(f"frozen input checksum mismatch: {target}")
        # Source is checked again after reads; no modified tree can be silently bound.
        if command(root, "git", "status", "--porcelain=v1", "--untracked-files=all"):
            fail("source changed while binding inputs")
        if command(root, "git", "rev-parse", "HEAD") != commit:
            fail("source commit changed while binding inputs")
        unchanged_directory(root, fd)
        return {"source_commit": commit, "source_tree": tree, "required_files": required}


def inside(root: Path, path: Path) -> None:
    absolute(path)
    if path == root or root not in path.parents:
        fail("artifact path escaped checkout")


def ignored_output(root: Path, path: Path) -> None:
    inside(root, path)
    relative = str(path.relative_to(root))
    command(root, "git", "check-ignore", "--no-index", "--", relative)


def require_keys(value: object, expected: set[str], context: str) -> dict:
    if not isinstance(value, dict) or set(value) != expected:
        fail(f"unexpected {context} shape")
    return value


def valid_metadata(value: object, maximum: int, mode: int = 0o755) -> dict:
    value = require_keys(value, {"bytes", "mode", "sha256"}, "file metadata")
    if (type(value["bytes"]) is not int or not 1 <= value["bytes"] <= maximum
            or type(value["mode"]) is not int or value["mode"] != mode
            or not isinstance(value["sha256"], str) or not SHA256.fullmatch(value["sha256"])):
        fail("invalid file metadata")
    return value


def verify_payload(root: Path, raw: bytes, binaries: dict[str, bytes]) -> dict:
    value = require_keys(decode(raw), {
        "schema", "status", "cpu_baseline", "source_commit", "source_tree",
        "required_files", "stages", "files",
    }, "candidate manifest")
    if value["schema"] != SCHEMA or value["status"] != "unverified" or value["cpu_baseline"] != "x86-64":
        fail("candidate is not an unverified portable bootstrap candidate")
    identity = source_identity(root)
    if any(value[key] != identity[key] for key in identity):
        fail("candidate source identity mismatch")
    files = require_keys(value["files"], set(BINARY_LIMITS), "candidate files")
    stages = require_keys(value["stages"], {"stage2", "stage3"}, "stage identities")
    valid_metadata(stages["stage2"], COMPILER_MAX)
    valid_metadata(stages["stage3"], COMPILER_MAX)
    for name, maximum in BINARY_LIMITS.items():
        if metadata(binaries[name]) != valid_metadata(files[name], maximum):
            fail(f"candidate binary digest or size mismatch: {name}")
    if stages["stage3"] != files["seen"]:
        fail("Stage-3 identity differs from candidate compiler")
    if sum(map(len, binaries.values())) + len(raw) >= TOTAL_MAX:
        fail("candidate exceeds total size limit")
    return value


def verify(root: Path, candidate_dir: Path) -> tuple[dict, bytes, dict[str, bytes]]:
    inside(root, candidate_dir)
    with directory(candidate_dir) as fd:
        entries(fd, {"seen", "seen-pkg", "candidate.json"})
        raw = read_at(fd, "candidate.json", JSON_MAX)
        binaries = {name: read_at(fd, name, maximum, executable=True)
                    for name, maximum in BINARY_LIMITS.items()}
        value = verify_payload(root, raw, binaries)
        entries(fd, {"seen", "seen-pkg", "candidate.json"})
        if read_at(fd, "candidate.json", JSON_MAX) != raw:
            fail("candidate manifest changed during verification")
        unchanged_directory(candidate_dir, fd)
        return value, raw, binaries


def unpack(root: Path, archive_path: Path, output: Path) -> None:
    ignored_output(root, output)
    packed = read_file(archive_path, TOTAL_MAX)
    # Bound decompression before tarfile can parse extension headers or allocate.
    with gzip.GzipFile(fileobj=io.BytesIO(packed)) as compressed:
        tar_bytes = compressed.read(TOTAL_MAX + 1)
    if len(tar_bytes) > TOTAL_MAX:
        fail("candidate archive expands beyond size limit")
    expected = {**BINARY_LIMITS, "candidate.json": JSON_MAX}
    payload = {}
    offset = 0
    with tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r:") as archive:
        for member in archive:
            if (member.name not in expected or member.name in payload
                    or member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE)
                    or member.issparse() or member.pax_headers or archive.pax_headers
                    or member.offset != offset or member.offset_data != offset + 512
                    or not 1 <= member.size <= expected[member.name]):
                fail("unsafe candidate archive member set, type, header, or size")
            source = archive.extractfile(member)
            if source is None:
                fail("unreadable candidate archive member")
            with source:
                data = source.read(expected[member.name] + 1)
            if len(data) != member.size:
                fail("candidate archive member size mismatch")
            payload[member.name] = data
            offset = member.offset_data + ((member.size + 511) // 512) * 512
    if set(payload) != set(expected) or any(tar_bytes[offset:]):
        fail("candidate archive entry set or trailing data is invalid")
    if len(tar_bytes) - offset < 1024:
        fail("candidate archive end markers are missing")
    binaries = {name: payload[name] for name in BINARY_LIMITS}
    verify_payload(root, payload["candidate.json"], binaries)
    publish_directory(output, {
        name: (data, 0o644 if name == "candidate.json" else 0o755)
        for name, data in payload.items()
    })


def create(root: Path, compiler: Path, stage2: Path, package_client: Path, output: Path) -> None:
    ignored_output(root, output)
    if os.environ.get("SEEN_RELEASE_CPU_BASELINE") != "x86-64":
        fail("candidate creation requires the explicit x86-64 build baseline")
    identity = source_identity(root)
    stage2_data = read_file(stage2, COMPILER_MAX, executable=True)
    binaries = {
        "seen": read_file(compiler, COMPILER_MAX, executable=True),
        "seen-pkg": read_file(package_client, CLIENT_MAX, executable=True),
    }
    value = {
        "schema": SCHEMA, "status": "unverified", "cpu_baseline": "x86-64",
        **identity,
        "stages": {"stage2": metadata(stage2_data), "stage3": metadata(binaries["seen"])},
        "files": {name: metadata(data) for name, data in binaries.items()},
    }
    raw = encode(value)
    if sum(map(len, binaries.values())) + len(raw) >= TOTAL_MAX:
        fail("candidate exceeds total size limit")
    if source_identity(root) != identity:
        fail("source changed during candidate creation")
    publish_directory(output, {
        **{name: (data, 0o755) for name, data in binaries.items()},
        "candidate.json": (raw, 0o644),
    })


def positive(value: object, name: str) -> int:
    if isinstance(value, str) and re.fullmatch(r"[1-9][0-9]{0,15}", value):
        return int(value)
    fail(f"invalid positive limit: {name}")


def scope_shape(scope: object) -> dict:
    value = require_keys(scope, {
        "unit", "cgroup_path", "memory_max_bytes", "memory_swap_max_bytes", "pids_max",
        "memory_oom_group", "derived_memory_ceiling_bytes", "main_vmem_kb", "opt_vmem_kb",
        "allocation_limit_bytes", "writer_rlimit_as_bytes", "serial_workers",
        "serializer_sha256", "serializer_record_sha256",
    }, "scope receipt")
    unit = value["unit"]
    path = value["cgroup_path"]
    if (not isinstance(unit, str) or not SCOPE_PATTERN.fullmatch(unit)
            or not isinstance(path, str) or not path.startswith("/")
            or any(part in {"", ".", ".."} for part in path.split("/")[1:])
            or not path.endswith("/" + unit)):
        fail("invalid scope identity")
    for key in ("memory_max_bytes", "pids_max", "derived_memory_ceiling_bytes", "main_vmem_kb",
                "opt_vmem_kb", "allocation_limit_bytes"):
        if type(value[key]) is not int or value[key] <= 0:
            fail(f"invalid scope limit: {key}")
    if (type(value["memory_swap_max_bytes"]) is not int or value["memory_swap_max_bytes"] != 0
            or type(value["memory_oom_group"]) is not int or value["memory_oom_group"] != 1
            or value["pids_max"] > 24
            or value["memory_max_bytes"] > value["derived_memory_ceiling_bytes"]
            or value["main_vmem_kb"] * 1024 > value["memory_max_bytes"]
            or value["opt_vmem_kb"] > min(2097152, value["main_vmem_kb"] // 2)
            or value["allocation_limit_bytes"] > value["memory_max_bytes"]):
        fail("scope limits exceed containment policy")
    limit = value["writer_rlimit_as_bytes"]
    if limit is not None and (type(limit) is not int or not 0 < limit <= value["main_vmem_kb"] * 1024):
        fail("writer address-space limit exceeds declared main limit")
    workers = require_keys(value["serial_workers"], set(SERIAL_VARIABLES), "serial workers")
    if any(type(count) is not int or count != 1 for count in workers.values()):
        fail("nonserial worker receipt")
    for key in ("serializer_sha256", "serializer_record_sha256"):
        if not isinstance(value[key], str) or not SHA256.fullmatch(value[key]):
            fail("invalid serializer identity")
    return value


def live_scope(root: Path) -> dict:
    unit = os.environ.get("SEEN_MEMORY_GUARD_SCOPE_UNIT", "")
    match = SCOPE_PATTERN.fullmatch(unit)
    if not match or int(match[1]) != os.getuid() or os.environ.get("SEEN_MEMORY_GUARD_IN_SCOPE") != "1":
        fail("actual verified scope is required")
    proc_path = Path(f"/proc/{os.getpid()}/cgroup")
    proc = read_file(proc_path, JSON_MAX, virtual=True).decode("ascii").splitlines()
    paths = [line[3:] for line in proc if line.startswith("0::")]
    if len(paths) != 1 or not paths[0].endswith("/" + unit):
        fail("process is outside the recorded scope")
    path = paths[0]
    control_group = command(root, "systemctl", "--user", "show", unit, "--property=ControlGroup", "--value")
    if control_group != path:
        fail("systemd scope does not own the current cgroup")
    cgroup = Path("/sys/fs/cgroup" + path)
    def limits() -> dict:
        with directory(cgroup) as fd:
            values = {name: read_at(fd, name, 128, virtual=True).decode("ascii").strip()
                      for name in ("memory.max", "memory.swap.max", "pids.max", "memory.oom.group")}
            unchanged_directory(cgroup, fd)
            return values
    before = limits()
    if before["memory.swap.max"] != "0" or before["memory.oom.group"] != "1":
        fail("scope swap/OOM policy is not enforced")
    value = {
        "unit": unit, "cgroup_path": path,
        "memory_max_bytes": positive(before["memory.max"], "memory.max"),
        "memory_swap_max_bytes": 0, "memory_oom_group": 1,
        "pids_max": positive(before["pids.max"], "pids.max"),
        "derived_memory_ceiling_bytes": positive(os.environ.get("SEEN_MEMORY_GUARD_RSS_KB"), "derived cap") * 1024,
        "main_vmem_kb": positive(os.environ.get("SEEN_MAIN_VMEM_KB"), "main VMEM"),
        "opt_vmem_kb": positive(os.environ.get("SEEN_OPT_VMEM_KB"), "opt VMEM"),
        "allocation_limit_bytes": positive(os.environ.get("SEEN_MEMORY_LIMIT_BYTES"), "allocation budget"),
        "writer_rlimit_as_bytes": None,
        "serial_workers": {},
    }
    soft, _hard = resource.getrlimit(resource.RLIMIT_AS)
    value["writer_rlimit_as_bytes"] = None if soft == resource.RLIM_INFINITY else soft
    for name in SERIAL_VARIABLES:
        if os.environ.get(name) != "1":
            fail(f"serial worker policy missing: {name}")
        value["serial_workers"][name] = 1
    config = Path(os.environ.get("RIPGREP_CONFIG_PATH", ""))
    inside(root, config)
    if read_file(config, 128) != b"--threads=1\n":
        fail("ripgrep serial policy is invalid")
    serializer = Path(os.environ.get("SEEN_FORK_SERIALIZER_SO", ""))
    record = Path(os.environ.get("SEEN_FORK_SERIALIZER_ATTESTATION", ""))
    work_root = Path(os.environ.get("SEEN_REBUILD_WORK_ROOT", ""))
    inside(root, work_root)
    inside(work_root, serializer)
    inside(work_root, record)
    serializer_bytes = read_file(serializer, CLIENT_MAX)
    raw = read_file(record, JSON_MAX)
    lines = raw.decode("ascii").splitlines(keepends=True)
    fields = {}
    for line in lines:
        key, separator, item = line.rstrip("\n").partition("=")
        if not separator or key in fields:
            fail("ambiguous serializer record")
        fields[key] = item
    require_keys(fields, {
        "version", "serializer_sha256", "source_sha256", "selftest_source_sha256",
        "dynamic_selftest", "scope_unit", "verified_memory_max_bytes",
        "verified_memory_swap_max_bytes", "verified_pids_max", "body_sha256",
    }, "serializer record")
    expected = {
        "version": "seen-fork-serializer-attestation-v2",
        "serializer_sha256": hashlib.sha256(serializer_bytes).hexdigest(),
        "source_sha256": hashlib.sha256(read_file(root / "scripts/fork_serializer.c", CLIENT_MAX)).hexdigest(),
        "selftest_source_sha256": hashlib.sha256(read_file(root / "scripts/fork_serializer_selftest.c", CLIENT_MAX)).hexdigest(),
        "dynamic_selftest": "passed", "scope_unit": unit,
        "verified_memory_max_bytes": str(value["memory_max_bytes"]),
        "verified_memory_swap_max_bytes": "0", "verified_pids_max": str(value["pids_max"]),
    }
    if (len(lines) != 10 or not lines[-1].startswith("body_sha256=")
            or fields["body_sha256"] != hashlib.sha256("".join(lines[:9]).encode("ascii")).hexdigest()
            or any(fields[key] != item for key, item in expected.items())):
        fail("serializer record does not match actual scope and sources")
    value["serializer_sha256"] = expected["serializer_sha256"]
    value["serializer_record_sha256"] = hashlib.sha256(raw).hexdigest()
    if limits() != before or read_file(proc_path, JSON_MAX, virtual=True).decode("ascii").splitlines() != proc:
        fail("scope changed during receipt recording")
    return scope_shape(value)


def receipt_identity(value: dict, raw: bytes) -> dict:
    return {
        "candidate_manifest_sha256": hashlib.sha256(raw).hexdigest(),
        "source_commit": value["source_commit"], "source_tree": value["source_tree"],
        "compiler_sha256": value["files"]["seen"]["sha256"],
        "package_client_sha256": value["files"]["seen-pkg"]["sha256"],
    }


def record_group(root: Path, candidate_dir: Path, group: str, output: Path) -> None:
    if group not in GROUPS or output.name != group + ".json":
        fail("unknown group or incorrect receipt filename")
    ignored_output(root, output)
    value, raw, _binaries = verify(root, candidate_dir)
    scope = live_scope(root)
    # Recheck transferred bytes and clean source after observing the live scope.
    if verify(root, candidate_dir)[1] != raw:
        fail("candidate changed during group recording")
    publish_json(output, {
        "schema": RECEIPT_SCHEMA, "kind": "completion-journal", "group": group,
        **receipt_identity(value, raw), "scope": scope,
    })


def finalize(root: Path, candidate_dir: Path, receipt_dir: Path) -> None:
    inside(root, receipt_dir)
    value, raw, binaries = verify(root, candidate_dir)
    expected_identity = receipt_identity(value, raw)
    expected_names = {group + ".json" for group in GROUPS}
    with directory(receipt_dir) as fd:
        entries(fd, expected_names)
        for group in GROUPS:
            receipt = require_keys(decode(read_at(fd, group + ".json", JSON_MAX)), {
                "schema", "kind", "group", "scope", *expected_identity,
            }, "acceptance receipt")
            if (receipt["schema"] != RECEIPT_SCHEMA or receipt["kind"] != "completion-journal"
                    or receipt["group"] != group
                    or any(receipt[key] != item for key, item in expected_identity.items())):
                fail(f"acceptance receipt identity mismatch: {group}")
            scope_shape(receipt["scope"])
        entries(fd, expected_names)
        unchanged_directory(receipt_dir, fd)
    if verify(root, candidate_dir)[1] != raw:
        fail("candidate changed before installation")
    # Invalidate obsolete certification only after every input passed validation.
    invalidate_stamp(root)
    install_files(root, {
        f"{directory_name}/{name}": data
        for directory_name in ("compiler_seen/target", "target/release")
        for name, data in binaries.items()
    })


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    children = parser.add_subparsers(dest="command", required=True)
    for name in ("create", "verify", "unpack", "record-group", "finalize"):
        child = children.add_parser(name)
        child.add_argument("--root", required=True, type=Path)
        if name == "create":
            child.add_argument("--compiler", required=True, type=Path)
            child.add_argument("--stage2", required=True, type=Path)
            child.add_argument("--package-client", required=True, type=Path)
            child.add_argument("--output", required=True, type=Path)
        elif name == "unpack":
            child.add_argument("--archive", required=True, type=Path)
            child.add_argument("--output", required=True, type=Path)
        else:
            child.add_argument("--candidate-dir", required=True, type=Path)
        if name == "record-group":
            child.add_argument("--group", required=True, choices=GROUPS)
            child.add_argument("--output", required=True, type=Path)
        if name == "finalize":
            child.add_argument("--receipt-dir", required=True, type=Path)
    args = parser.parse_args()
    try:
        absolute(args.root)
        if args.command == "create":
            create(args.root, args.compiler, args.stage2, args.package_client, args.output)
        elif args.command == "verify":
            verify(args.root, args.candidate_dir)
        elif args.command == "unpack":
            unpack(args.root, args.archive, args.output)
        elif args.command == "record-group":
            record_group(args.root, args.candidate_dir, args.group, args.output)
        else:
            finalize(args.root, args.candidate_dir, args.receipt_dir)
        return 0
    except (ArtifactError, OSError, UnicodeError, subprocess.SubprocessError,
            tarfile.TarError, EOFError, AttributeError) as error:
        print(f"bootstrap-candidate: {error}", file=sys.stderr)
        for note in getattr(error, "__notes__", ()):
            print(f"bootstrap-candidate: {note}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
