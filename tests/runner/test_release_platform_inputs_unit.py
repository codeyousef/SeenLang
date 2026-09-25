"""Fail-closed checks for staged macOS/Windows release input identity."""

from __future__ import annotations

import io
import hashlib
import json
import subprocess
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts/release_platform_inputs.py"
VERSION = "0.22.7"


class ReleasePlatformInputsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.commit = subprocess.check_output(
            ["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True
        ).strip()
        self.make_archives()

    def make_archives(self) -> None:
        mac_root = f"seen-{VERSION}-macos-arm64/"
        mac = self.directory / f"seen-{VERSION}-macos-arm64.tar.gz"
        mac_compiler = b"\xcf\xfa\xed\xfe" + b"compiler"
        mac_package = b"\xcf\xfa\xed\xfe" + b"package"
        with tarfile.open(mac, "w:gz") as archive:
            root_member = tarfile.TarInfo(mac_root)
            root_member.type = tarfile.DIRTYPE
            archive.addfile(root_member)
            for name, data in {
                "bin/seen": mac_compiler,
                "bin/seen-pkg": mac_package,
                "bin/compatibility-manifest.json": json.dumps({"release_version": VERSION}).encode(),
                "share/seen/release-provenance.env": (
                    f"release_version={VERSION}\nsource_commit={self.commit}\n"
                    "platform=macos-arm64\n"
                    f"compiler_sha256={hashlib.sha256(mac_compiler).hexdigest()}\n"
                    f"package_client_sha256={hashlib.sha256(mac_package).hexdigest()}\n"
                ).encode(),
            }.items():
                member = tarfile.TarInfo(mac_root + name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        windows_root = f"seen-{VERSION}-windows-x64/"
        windows = self.directory / f"seen-{VERSION}-windows-x64.zip"
        win_compiler = b"MZcompiler"
        win_package = b"MZpackage"
        with zipfile.ZipFile(windows, "w") as archive:
            for name, data in {
                "bin/seen.exe": win_compiler,
                "bin/seen-pkg.exe": win_package,
                "bin/compatibility-manifest.json": json.dumps({"release_version": VERSION}).encode(),
                "share/seen/release-provenance.env": (
                    f"release_version={VERSION}\nsource_commit={self.commit}\n"
                    "platform=windows-x64\n"
                    f"compiler_sha256={hashlib.sha256(win_compiler).hexdigest()}\n"
                    f"package_client_sha256={hashlib.sha256(win_package).hexdigest()}\n"
                ).encode(),
            }.items():
                archive.writestr(windows_root + name, data)
        (self.directory / f"Seen-{VERSION}-windows-x64-setup.exe").write_bytes(
            b"MZinstaller"
        )

    def check(self, mode: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(CHECKER), mode, "--root", str(ROOT),
             "--version", VERSION, "--input-dir", str(self.directory)],
            text=True, capture_output=True, check=False,
        )

    def test_exact_inputs_pass_and_mutation_fails(self) -> None:
        self.assertEqual(self.check("create").returncode, 0)
        self.assertEqual(self.check("verify").returncode, 0)
        installer = self.directory / f"Seen-{VERSION}-windows-x64-setup.exe"
        installer.write_bytes(installer.read_bytes() + b"changed")
        self.assertNotEqual(self.check("verify").returncode, 0)

    def test_missing_and_foreign_provenance_fail(self) -> None:
        windows = self.directory / f"seen-{VERSION}-windows-x64.zip"
        windows.unlink()
        self.assertNotEqual(self.check("create").returncode, 0)
        self.make_archives()
        self.commit = "0" * 40
        self.make_archives()
        self.assertNotEqual(self.check("create").returncode, 0)


if __name__ == "__main__":
    unittest.main()
