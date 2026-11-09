#!/usr/bin/env python3
"""Windows IOCP transport and report cases; fixture PE bytes are never executed."""

import copy
import gzip
import hashlib
import io
import json
import struct
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import ci_candidate_io as candidate_io
import ci_windows_iocp as windows

IDENTITY = {
    "schema": windows.SCHEMA, "status": "cross-built-unverified", "target": "windows-x86_64",
    "source_version": "0.23.0", "source_commit": "a" * 40, "source_tree": "b" * 40,
    "origin_candidate": {"manifest_sha256": "c" * 64, "compiler_sha256": "d" * 64},
}


def pe(machine=0x8664, magic=0x20B, characteristics=0x0022, subsystem=3, suffix=b""):
    # A structurally linked PE32+ header, deliberately not runnable machine code.
    data = bytearray(0x200)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", data, 0x84, machine, 1, 0, 0, 0, 0xF0, characteristics)
    struct.pack_into("<H", data, 0x98, magic)
    struct.pack_into("<H", data, 0x98 + 0x44, subsystem)
    return bytes(data) + suffix


def archive(members):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:", format=tarfile.USTAR_FORMAT) as tar:
        for name, data in members.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = 0o755
            tar.addfile(info, io.BytesIO(data))
    return gzip.compress(buffer.getvalue(), mtime=0)


class PeTests(unittest.TestCase):
    def test_accepts_linked_x64_console_image(self):
        windows.pe(pe())

    def test_rejects_non_executable_shapes(self):
        cases = {
            "elf": b"\x7fELF" + bytes(0x200),
            "arm64": pe(machine=0xAA64),
            "pe32": pe(magic=0x10B),
            "dll": pe(characteristics=0x2022),
            "object": pe(characteristics=0x0020),
            "gui": pe(subsystem=2),
            "truncated": pe()[:0x90],
        }
        for name, data in cases.items():
            with self.subTest(name=name), self.assertRaises(windows.ArtifactError):
                windows.pe(data)


class ReportTests(unittest.TestCase):
    def setUp(self):
        cache = ROOT / ".seen/ci-windows-iocp-tests"
        cache.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        patcher = patch.object(windows, "binding", return_value=copy.deepcopy(IDENTITY))
        patcher.start()
        self.addCleanup(patcher.stop)
        self.fixtures = {name: pe(suffix=name.encode()) for name in windows.FIXTURES}
        self.manifest = candidate_io.encode({**IDENTITY, "files": {
            name: candidate_io.metadata(data) for name, data in self.fixtures.items()}})
        self.archive = self.root / "inputs.tar.gz"
        self.archive.write_bytes(archive({**self.fixtures, windows.MANIFEST: self.manifest}))
        self.report = {
            "schema": windows.REPORT_SCHEMA, "status": "verified",
            "inputs_manifest_sha256": hashlib.sha256(self.manifest).hexdigest(),
            "os": {"platform": "Win32NT", "version": "10.0.26100.0", "architecture": "X64"},
            "job": {"job_memory_limit_bytes": 2 ** 31, "process_memory_limit_bytes": 2 ** 30,
                    "active_process_limit": 24, "fixture_timeout_seconds": 300},
            "results": [{
                "name": name, "sha256": hashlib.sha256(data).hexdigest(), "exit_code": 0,
                "stdout": windows.VERIFIED_LINE + "\n" if name.startswith("iocp-native-") else "",
            } for name, data in self.fixtures.items()],
        }

    def verify(self, report):
        path = self.root / "report.json"
        path.write_text(json.dumps(report))
        return windows.verify_report(self.root, self.root, self.archive, path)

    def test_accepts_verified_native_report(self):
        self.assertEqual(self.verify(self.report)["status"], "verified")

    def test_rejects_unverified_or_mismatched_reports(self):
        def mutate(change):
            report = copy.deepcopy(self.report)
            change(report)
            return report
        cases = {
            "unsupported": mutate(lambda r: r["results"][0].update(stdout=windows.VERIFIED_LINE.replace(
                '"verified"', '"unsupported"'))),
            "exit": mutate(lambda r: r["results"][2].update(exit_code=134)),
            "bytes": mutate(lambda r: r["results"][1].update(sha256="0" * 64)),
            "inputs": mutate(lambda r: r.update(inputs_manifest_sha256="0" * 64)),
            "linux": mutate(lambda r: r["os"].update(platform="Unix")),
            "arm64": mutate(lambda r: r["os"].update(architecture="Arm64")),
            "memory": mutate(lambda r: r["job"].update(job_memory_limit_bytes=2 ** 40)),
            "process": mutate(lambda r: r["job"].update(process_memory_limit_bytes=2 ** 32)),
            "processes": mutate(lambda r: r["job"].update(active_process_limit=0)),
            "missing": mutate(lambda r: r["results"].pop()),
            "order": mutate(lambda r: r["results"].reverse()),
            "status": mutate(lambda r: r.update(status="partial")),
            "extra": mutate(lambda r: r.update(fallback=True)),
        }
        for name, report in cases.items():
            with self.subTest(name=name), self.assertRaises(windows.ArtifactError):
                self.verify(report)

    def test_rejects_tampered_archive(self):
        tampered = dict(self.fixtures)
        tampered[windows.FIXTURES[0]] = pe(suffix=b"other")
        for members in ({**tampered, windows.MANIFEST: self.manifest},
                        {**self.fixtures, windows.MANIFEST: self.manifest, "extra.exe": pe()},
                        {name: self.fixtures[name] for name in windows.FIXTURES[1:]}
                        | {windows.MANIFEST: self.manifest}):
            with self.subTest(members=sorted(members)):
                self.archive.write_bytes(archive(members))
                with self.assertRaises(windows.ArtifactError):
                    self.verify(self.report)


if __name__ == "__main__":
    unittest.main()
