#!/usr/bin/env python3
"""Transport same-source Windows IOCP fixtures and verify their native report."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import struct
import sys
import tarfile
import zlib
from pathlib import Path

import ci_bootstrap_candidate as candidate
from ci_candidate_io import absolute, decode, encode, fail, metadata, read_file
from ci_linux_arm64_inputs import elf, publish_archive, source_version
from release_toolchain_artifact import ArtifactError

SCHEMA = "seen-windows-iocp-inputs-v1"
REPORT_SCHEMA = "seen-windows-iocp-report-v1"
MANIFEST = "windows-iocp-inputs.json"
MANIFEST_MAX = 64 * 1024
FIXTURE_MAX = 32 * 1024 * 1024
TOTAL_MAX = 160 * 1024 * 1024
REPORT_MAX = 256 * 1024
FIXTURES = tuple(f"iocp-{name}-{profile}.exe"
                 for name in ("native", "contract") for profile in ("fast", "release"))
MEMBERS = {**{name: FIXTURE_MAX for name in FIXTURES}, MANIFEST: MANIFEST_MAX}
# The native fixture prints exactly this line only after real overlapped file
# submission, completion, cancellation and cleanup on a Windows kernel.
VERIFIED_LINE = ('{"schema":"seen-io-iocp-v1","backend":"iocp","classification":"verified",'
                 '"scope":"windows-file-contract","submission_overlap":2,'
                 '"native_overlap_verified":false,"fallback_count":0}')
# Upper bounds the Windows runner applies through a kernel job object.
JOB_MEMORY_MAX = 2 * 1024 * 1024 * 1024
JOB_PROCESSES_MAX = 24
FIXTURE_TIMEOUT_MAX = 600


def pe(data: bytes) -> None:
    """Require a linked PE32+ x86-64 console image, not a renamed object."""
    if len(data) < 0x40 or data[:2] != b"MZ":
        fail("input is not a PE image")
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    if offset < 0x40 or offset + 24 + 0x60 > len(data) or data[offset:offset + 4] != b"PE\0\0":
        fail("input has no PE signature")
    machine, _, _, _, _, optional, characteristics = struct.unpack_from("<HHIIIHH", data, offset + 4)
    magic = struct.unpack_from("<H", data, offset + 24)[0]
    subsystem = struct.unpack_from("<H", data, offset + 24 + 0x44)[0]
    if (machine != 0x8664 or magic != 0x20B or optional < 0x70
            or not characteristics & 0x0002 or characteristics & 0x2000 or subsystem != 3):
        fail("input is not a PE32+ x86-64 console executable")


def binding(root: Path, candidate_dir: Path) -> dict:
    """Canonical x64 Linux verification is provenance only for these inputs."""
    value, raw, binaries = candidate.verify(root, candidate_dir)
    for data in binaries.values():
        elf(data, 62)
    result = {
        "schema": SCHEMA, "status": "cross-built-unverified", "target": "windows-x86_64",
        "source_version": source_version(root),
        "source_commit": value["source_commit"], "source_tree": value["source_tree"],
        "origin_candidate": {
            "manifest_sha256": hashlib.sha256(raw).hexdigest(),
            "compiler_sha256": hashlib.sha256(binaries["seen"]).hexdigest(),
        },
    }
    if candidate.source_identity(root)["source_commit"] != result["source_commit"]:
        fail("source changed during Windows provenance binding")
    return result


def verify_payload(root: Path, candidate_dir: Path, raw: bytes, fixtures: dict[str, bytes]) -> dict:
    value = candidate.require_keys(decode(raw), {
        "schema", "status", "target", "source_version", "source_commit", "source_tree",
        "origin_candidate", "files",
    }, "Windows input manifest")
    current = binding(root, candidate_dir)
    if any(value[key] != current[key] for key in current):
        fail("Windows input source or origin candidate identity mismatch")
    files = candidate.require_keys(value["files"], set(FIXTURES), "Windows input files")
    for name in FIXTURES:
        pe(fixtures[name])
        if metadata(fixtures[name]) != candidate.valid_metadata(files[name], FIXTURE_MAX):
            fail(f"Windows payload digest or size mismatch: {name}")
    return value


def create(root: Path, candidate_dir: Path, inputs: Path, output: Path) -> None:
    candidate.ignored_output(root, output)
    expected = binding(root, candidate_dir)
    fixtures = {name: read_file(inputs / name, FIXTURE_MAX, executable=True) for name in FIXTURES}
    raw = encode({**expected, "files": {name: metadata(data) for name, data in fixtures.items()}})
    verify_payload(root, candidate_dir, raw, fixtures)
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:", format=tarfile.USTAR_FORMAT) as archive:
        for name, data in {**fixtures, MANIFEST: raw}.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = 0o644 if name == MANIFEST else 0o755
            archive.addfile(info, io.BytesIO(data))
    packed = gzip.compress(buffer.getvalue(), mtime=0)
    if len(buffer.getvalue()) > TOTAL_MAX or len(packed) > TOTAL_MAX:
        fail("Windows archive exceeds total size limit")
    if binding(root, candidate_dir) != expected:
        fail("source or origin candidate changed before Windows archive publication")
    publish_archive(output, packed)


def read_archive(path: Path) -> dict[str, bytes]:
    with gzip.GzipFile(fileobj=io.BytesIO(read_file(path, TOTAL_MAX))) as compressed:
        data = compressed.read(TOTAL_MAX + 1)
    if len(data) > TOTAL_MAX:
        fail("Windows archive expands beyond size limit")
    payload: dict[str, bytes] = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as archive:
        for member in archive:
            if (member.name not in MEMBERS or member.name in payload
                    or member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE)
                    or member.linkname or member.pax_headers
                    or not 1 <= member.size <= MEMBERS[member.name]):
                fail("unsafe Windows archive member set, type, header, or size")
            source = archive.extractfile(member)
            if source is None:
                fail("unreadable Windows archive member")
            with source:
                payload[member.name] = source.read(MEMBERS[member.name] + 1)
            if len(payload[member.name]) != member.size:
                fail("Windows archive member size mismatch")
    if set(payload) != set(MEMBERS):
        fail("incomplete Windows archive member set")
    return payload


def bounded_int(value: object, low: int, high: int, name: str) -> int:
    if type(value) is not int or not low <= value <= high:
        fail(f"Windows report {name} is outside its required bound")
    return value


def verify_report(root: Path, candidate_dir: Path, archive: Path, report: Path) -> dict:
    payload = read_archive(archive)
    raw = payload[MANIFEST]
    verify_payload(root, candidate_dir, raw, {name: payload[name] for name in FIXTURES})
    value = candidate.require_keys(decode(read_file(report, REPORT_MAX)), {
        "schema", "status", "inputs_manifest_sha256", "os", "job", "results",
    }, "Windows report")
    if value["schema"] != REPORT_SCHEMA or value["status"] != "verified":
        fail("Windows report is not a verified native report")
    if value["inputs_manifest_sha256"] != hashlib.sha256(raw).hexdigest():
        fail("Windows report was produced for different inputs")
    system = candidate.require_keys(value["os"], {"platform", "version", "architecture"}, "Windows OS")
    if (system["platform"] != "Win32NT" or system["architecture"] != "X64"
            or not isinstance(system["version"], str) or not system["version"].startswith("10.0.")):
        fail("Windows report did not come from a native Windows x64 kernel")
    job = candidate.require_keys(value["job"], {
        "job_memory_limit_bytes", "process_memory_limit_bytes", "active_process_limit",
        "fixture_timeout_seconds",
    }, "Windows job limits")
    bounded_int(job["job_memory_limit_bytes"], 1, JOB_MEMORY_MAX, "job memory limit")
    bounded_int(job["process_memory_limit_bytes"], 1, job["job_memory_limit_bytes"], "process memory limit")
    bounded_int(job["active_process_limit"], 1, JOB_PROCESSES_MAX, "active process limit")
    bounded_int(job["fixture_timeout_seconds"], 1, FIXTURE_TIMEOUT_MAX, "fixture timeout")
    results = value["results"]
    if not isinstance(results, list) or [item.get("name") if isinstance(item, dict) else None
                                         for item in results] != list(FIXTURES):
        fail("Windows report must contain every fixture exactly once in order")
    for item in results:
        item = candidate.require_keys(item, {"name", "sha256", "exit_code", "stdout"}, "Windows result")
        if item["sha256"] != hashlib.sha256(payload[item["name"]]).hexdigest():
            fail(f"Windows result executed different bytes: {item['name']}")
        if item["exit_code"] != 0 or not isinstance(item["stdout"], str):
            fail(f"Windows fixture failed: {item['name']}")
        if item["name"].startswith("iocp-native-") and VERIFIED_LINE not in item["stdout"].splitlines():
            fail(f"Windows IOCP fixture did not report verified native execution: {item['name']}")
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="operation", required=True)
    for operation in ("binding", "create", "verify-report"):
        command = subparsers.add_parser(operation)
        command.add_argument("--root", type=Path, required=True)
        command.add_argument("--candidate", type=Path, required=True)
        if operation == "create":
            command.add_argument("--inputs", type=Path, required=True)
        if operation == "verify-report":
            command.add_argument("--archive", type=Path, required=True)
            command.add_argument("--report", type=Path, required=True)
        else:
            command.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        absolute(args.root)
        if args.operation == "binding":
            candidate.ignored_output(args.root, args.output)
            print(encode(binding(args.root, args.candidate)).decode(), end="")
        elif args.operation == "create":
            create(args.root, args.candidate, args.inputs, args.output)
        else:
            verify_report(args.root, args.candidate, args.archive, args.report)
            print("PASS: native Windows IOCP fixtures verified for the exact source and candidate")
        return 0
    except (ArtifactError, OSError, ValueError, tarfile.TarError, EOFError, zlib.error) as error:
        print(f"windows-iocp: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
