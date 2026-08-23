#!/usr/bin/env python3

import hashlib
import importlib.util
import tarfile
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "release_toolchain", ROOT / "scripts/release_toolchain_artifact.py"
)
release_toolchain = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release_toolchain)

COMMIT = "a" * 40
TREE = "b" * 40
BASELINE = "x86-64"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixture(root: Path, baseline: str = BASELINE) -> None:
    compiler = root / "compiler_seen/target/seen"
    package_client = compiler.with_name("seen-pkg")
    stamp = root / "target/seen-build/full-release.stamp"
    compiler.parent.mkdir(parents=True)
    stamp.parent.mkdir(parents=True)
    compiler.write_bytes(b"portable compiler\n")
    package_client.write_bytes(b"portable package client\n")
    compiler.chmod(0o755)
    package_client.chmod(0o755)
    stamp.write_text(
        "stamp_version=3\n"
        f"commit={COMMIT}\n"
        f"tree={TREE}\n"
        f"compiler_sha256={digest(compiler)}\n"
        f"package_client_sha256={digest(package_client)}\n"
        f"release_cpu_baseline={baseline}\n",
        encoding="utf-8",
    )


class Tests(unittest.TestCase):
    def test_create_verify_and_install_are_deterministic(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / "source"
            destination = base / "destination"
            source.mkdir()
            destination.mkdir()
            fixture(source)
            first = base / "first.tar.gz"
            second = base / "second.tar.gz"
            release_toolchain.create_archive(source, first, COMMIT, TREE, BASELINE)
            release_toolchain.create_archive(source, second, COMMIT, TREE, BASELINE)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            release_toolchain.validate_archive(first, COMMIT, TREE, BASELINE, None)
            release_toolchain.validate_archive(first, COMMIT, TREE, BASELINE, destination)
            self.assertEqual(
                (destination / "compiler_seen/target/seen").read_bytes(),
                b"portable compiler\n",
            )
            self.assertTrue((destination / "compiler_seen/target/seen").stat().st_mode & 0o111)
            self.assertEqual(
                (destination / "target/seen-build/full-release.stamp").read_text(),
                (source / "target/seen-build/full-release.stamp").read_text(),
            )

    def test_rejects_wrong_identity_and_corruption(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            archive = root / "toolchain.tar.gz"
            release_toolchain.create_archive(root, archive, COMMIT, TREE, BASELINE)
            with self.assertRaises(release_toolchain.ArtifactError):
                release_toolchain.validate_archive(archive, "c" * 40, TREE, BASELINE, None)
            archive.write_bytes(archive.read_bytes()[:32])
            with self.assertRaises((release_toolchain.ArtifactError, OSError, EOFError, tarfile.TarError)):
                release_toolchain.validate_archive(archive, COMMIT, TREE, BASELINE, None)

    def test_rejects_nonportable_stamp(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root, "native")
            with self.assertRaises(release_toolchain.ArtifactError):
                release_toolchain.create_archive(
                    root, root / "toolchain.tar.gz", COMMIT, TREE, BASELINE
                )


if __name__ == "__main__":
    unittest.main()
