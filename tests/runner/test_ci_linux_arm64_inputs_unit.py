#!/usr/bin/env python3
"""Consumer-visible ARM transport cases; fixture ELF bytes are never executed."""

import copy
import gzip
import hashlib
import io
import json
import os
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import ci_bootstrap_candidate as candidate
import ci_candidate_io as candidate_io
import ci_linux_arm64_inputs as inputs


def git(root, *args):
    return subprocess.run(["git", "-C", str(root), *args], check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.decode().strip()


def elf(machine=183, suffix=b""):
    # A structurally linked ELF fixture, deliberately not runnable machine code.
    data = bytearray(128)
    data[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HHIQQQIHHHHHH", data, 16,
                     2, machine, 1, 0x400078, 64, 0, 0, 64, 56, 1, 0, 0, 0)
    struct.pack_into("<IIQQQQQQ", data, 64, 1, 5, 0, 0x400000, 0x400000, 128, 128, 4096)
    return bytes(data) + suffix


class VersionTests(unittest.TestCase):
    def setUp(self):
        cache = ROOT / ".seen/ci-linux-arm64-inputs-tests"
        cache.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def test_workspace_project_version(self):
        (self.root / "Seen.toml").write_text(
            'manifest-version = 1\n[project]\nname = "seenlang"\nversion = "0.23.0"\n')
        self.assertEqual(inputs.source_version(self.root), "0.23.0")

    def test_root_version_does_not_override_project(self):
        (self.root / "Seen.toml").write_text(
            'version = "9.9.9"\n[project]\nversion = "0.23.0"\n')
        self.assertEqual(inputs.source_version(self.root), "0.23.0")

    def test_missing_or_invalid_project_version_is_rejected(self):
        for content in (
            'version = "0.23.0"\n',
            'project = "0.23.0"\n',
            'project = [0, 23, 0]\n',
            '[project]\nname = "seenlang"\n',
            '[project]\nversion = 23\n',
            '[project]\nversion = false\n',
            '[project]\nversion = "0.23"\n',
        ):
            with self.subTest(content=content):
                (self.root / "Seen.toml").write_text(content)
                with self.assertRaises(inputs.ArtifactError):
                    inputs.source_version(self.root)


class Tests(unittest.TestCase):
    def setUp(self):
        cache = ROOT / ".seen/ci-linux-arm64-inputs-tests"
        cache.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "source"
        self.root.mkdir()
        for relative in candidate.SOURCE_LIMITS:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((relative + "\n").encode())
        for target, checksum in (
            ("bootstrap/stage1_frozen", "bootstrap/stage1_frozen.sha256"),
            ("bootstrap/stage1_frozen_v3", "bootstrap/stage1_frozen_v3.sha256"),
            ("bootstrap/stage1_frozen.compatibility-manifest.json",
             "bootstrap/stage1_frozen.compatibility-manifest.sha256"),
            ("bootstrap/stage1_frozen.seen-pkg-source.tar.gz",
             "bootstrap/stage1_frozen.seen-pkg-source.sha256"),
        ):
            digest = hashlib.sha256((self.root / target).read_bytes()).hexdigest()
            (self.root / checksum).write_text(f"{digest}  {target}\n")
        (self.root / ".gitignore").write_text("/target/\n/.work/\n")
        (self.root / "Seen.toml").write_text('[project]\nversion = "0.0.1"\n')
        (self.root / "source.seen").write_text("unchanged source fixture\n")
        git(self.root, "init", "--quiet")
        git(self.root, "add", ".")
        self.commit()
        work = self.root / ".work"
        work.mkdir()
        self.compiler = work / "arm-seen"
        self.client = work / "arm-seen-pkg"
        self.origin_compiler = work / "x64-seen"
        self.origin_client = work / "x64-seen-pkg"
        for path, data in ((self.compiler, elf()), (self.client, elf(suffix=b"ARM client")),
                           (self.origin_compiler, elf(62)),
                           (self.origin_client, elf(62, b"x64 client"))):
            path.write_bytes(data)
            path.chmod(0o755)
        self.candidate = self.root / "target/bootstrap-candidate"
        with patch.dict(os.environ, {"SEEN_RELEASE_CPU_BASELINE": "x86-64"}):
            candidate.create(self.root, self.origin_compiler, self.origin_compiler,
                             self.origin_client, self.candidate)
        self.archive = self.root / "target/arm.tar.gz"
        self.output = self.root / "target/native"
        inputs.create(self.root, self.candidate, self.compiler, self.client, self.archive)

    def commit(self):
        git(self.root, "-c", "user.name=Contract Test", "-c", "user.email=contract@example.invalid",
            "commit", "--quiet", "-m", "fixture source")

    def unpack(self):
        inputs.unpack(self.root, self.candidate, self.archive, self.output)

    def members(self):
        with tarfile.open(self.archive, "r:gz") as archive:
            return [(member.name, archive.extractfile(member).read(), member.type, member.mode)
                    for member in archive]

    def repack(self, members, format=tarfile.USTAR_FORMAT):
        with tarfile.open(self.archive, "w:gz", format=format) as archive:
            for name, data, kind, mode in members:
                member = tarfile.TarInfo(name)
                member.type, member.mode = kind, mode
                if kind in (tarfile.REGTYPE, tarfile.AREGTYPE):
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))
                else:
                    member.linkname = "../../source.seen"
                    archive.addfile(member)

    def reject_unpack(self):
        with self.assertRaises((inputs.ArtifactError, OSError, ValueError, tarfile.TarError, EOFError)):
            self.unpack()
        self.assertFalse(self.output.exists())
        self.assertFalse(list(self.output.parent.glob(".bootstrap-candidate.*")))

    def test_roundtrip_is_same_source_cross_built_unverified_not_certification(self):
        self.unpack()
        value, raw, binaries = inputs.verify(self.root, self.candidate, self.output)
        self.assertEqual(value["status"], "cross-built-unverified")
        self.assertEqual(value["target"], "linux-arm64")
        self.assertEqual(value["source_version"], "0.0.1")
        self.assertEqual(value["source_commit"], git(self.root, "rev-parse", "HEAD"))
        self.assertEqual(value["origin_candidate"]["manifest_sha256"],
                         hashlib.sha256((self.candidate / "candidate.json").read_bytes()).hexdigest())
        self.assertEqual(binaries["seen"], self.compiler.read_bytes())
        self.assertEqual(json.loads(raw), value)
        self.assertEqual({p.name for p in self.output.iterdir()}, set(inputs.MEMBERS))
        self.assertFalse((self.root / "target/seen-build/full-release.stamp").exists())
        self.assertEqual((self.output / "seen").stat().st_mode & 0o7777, 0o755)
        self.assertEqual((self.output / "native-candidate.json").stat().st_mode & 0o7777, 0o644)

    def test_public_cli_create_unpack_verify(self):
        other = self.archive.with_name("cli.tar.gz")
        destination = self.output.with_name("cli")
        base = [sys.executable, str(ROOT / "scripts/ci_linux_arm64_inputs.py")]
        common = ["--root", str(self.root), "--candidate", str(self.candidate)]
        for arguments in (["create", *common, "--compiler", str(self.compiler), "--client", str(self.client),
                           "--output", str(other)],
                          ["unpack", *common, "--archive", str(other), "--output", str(destination)],
                          ["verify", *common, "--directory", str(destination)]):
            result = subprocess.run([*base, *arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(json.loads(result.stdout)["status"], "cross-built-unverified")
        (destination / "seen").write_bytes(b"corrupted")
        result = subprocess.run([*base, "verify", *common, "--directory", str(destination)],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"")

    def test_payload_corruption_and_manifest_digest_substitution(self):
        original = self.members()
        for name in ("seen", "seen-pkg", "native-candidate.json"):
            with self.subTest(name=name):
                members = [(n, data + b"corruption" if n == name else data, kind, mode)
                           for n, data, kind, mode in original]
                self.repack(members)
                self.reject_unpack()

    def test_rehashed_wrong_machine_cannot_self_authorize(self):
        members = self.members()
        payload = {name: data for name, data, _, _ in members}
        payload["seen"] = elf(62)
        value = json.loads(payload["native-candidate.json"])
        value["files"]["seen"] = inputs.metadata(payload["seen"])
        payload["native-candidate.json"] = inputs.encode(value)
        self.repack([(name, payload[name], kind, mode) for name, _, kind, mode in members])
        self.reject_unpack()

    def test_create_rejects_nonlinked_nonarm_or_invalid_elf_without_publication(self):
        original = self.compiler.read_bytes()
        bad = [b"LLVM IR", elf(62), elf()[:63]]
        for offset, replacement in ((4, b"\x01"), (5, b"\x02"), (16, b"\x01\x00"),
                                    (54, b"\x00\x00"), (56, b"\x00\x00"),
                                    (32, struct.pack("<Q", 4096)),
                                    (24, struct.pack("<Q", 0x500000)),
                                    (96, struct.pack("<Q", 4096))):
            data = bytearray(original)
            data[offset:offset + len(replacement)] = replacement
            bad.append(bytes(data))
        for index, data in enumerate(bad):
            with self.subTest(index=index):
                self.compiler.write_bytes(data)
                destination = self.archive.with_name(f"bad-{index}.tar.gz")
                with self.assertRaises(inputs.ArtifactError):
                    inputs.create(self.root, self.candidate, self.compiler, self.client, destination)
                self.assertFalse(destination.exists())
        self.compiler.write_bytes(original)

    def test_manifest_source_candidate_status_and_file_shapes_are_not_self_authorized(self):
        original = self.members()
        manifest = json.loads(original[-1][1])
        cases = [("source_commit", "c" * 40), ("source_tree", "d" * 40),
                 ("source_version", "99.0.0"), ("status", "native-qualified"),
                 ("target", "linux-x64"), ("schema", "different"),
                 ("origin_candidate", {"manifest_sha256": "e" * 64}),
                 ("files", {"../seen": manifest["files"]["seen"]})]
        for key, value in cases:
            with self.subTest(key=key):
                changed = copy.deepcopy(manifest)
                changed[key] = value
                self.repack([*original[:-1], (original[-1][0], inputs.encode(changed), tarfile.REGTYPE, 0o644)])
                self.reject_unpack()
        duplicate = original[-1][1].replace(b'{', b'{"schema":"duplicate",', 1)
        self.repack([*original[:-1], (original[-1][0], duplicate, tarfile.REGTYPE, 0o644)])
        self.reject_unpack()

    def test_archive_duplicate_extra_traversal_links_special_and_unsafe_modes(self):
        original = self.members()
        cases = [original + [original[0]], original[:-1],
                 original + [("extra", b"x", tarfile.REGTYPE, 0o644)]]
        for name in ("../seen", "/seen", "./seen", "a/../../seen"):
            cases.append([(name, *original[0][1:]), *original[1:]])
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.DIRTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE):
            cases.append([("seen", b"", kind, 0o755), *original[1:]])
        for mode in (0o7777, 0o777, 0o644):
            cases.append([("seen", original[0][1], tarfile.REGTYPE, mode), *original[1:]])
        cases.append([*original[:-1], ("native-candidate.json", original[-1][1], tarfile.REGTYPE, 0o755)])
        source = (self.root / "source.seen").read_bytes()
        for index, members in enumerate(cases):
            with self.subTest(index=index):
                self.repack(members)
                self.reject_unpack()
                self.assertEqual((self.root / "source.seen").read_bytes(), source)

    def test_archive_extensions_truncation_trailing_data_and_expansion_are_rejected(self):
        original = self.archive.read_bytes()
        raw = gzip.decompress(original)
        nonzero_padding = bytearray(raw)
        nonzero_padding[512 + len(elf())] = 1
        cases = [original[:10], gzip.compress(raw[:1024]), gzip.compress(raw + b"trailer"),
                 gzip.compress(raw[:-1]), gzip.compress(nonzero_padding),
                 gzip.compress(raw) + gzip.compress(b"second gzip member"),
                 gzip.compress(b"\0" * (inputs.TOTAL_MAX + 1))]
        for index, data in enumerate(cases):
            with self.subTest(index=index):
                self.archive.write_bytes(data)
                self.reject_unpack()
        self.archive.write_bytes(original)
        members = self.members()
        self.repack([("a" * 120, *members[0][1:]), *members[1:]], tarfile.PAX_FORMAT)
        self.reject_unpack()
        # A declared oversize is refused before trying to read the missing body.
        member = tarfile.TarInfo("seen")
        member.mode, member.size = 0o755, inputs.BINARY_LIMITS["seen"] + 1
        self.archive.write_bytes(gzip.compress(member.tobuf(format=tarfile.USTAR_FORMAT) + b"\0" * 1024))
        self.reject_unpack()

    def test_dirty_source_and_different_clean_source_are_rejected(self):
        source = self.root / "source.seen"
        source.write_text("changed source\n")
        self.reject_unpack()
        new_archive = self.archive.with_name("dirty.tar.gz")
        with self.assertRaises(inputs.ArtifactError):
            inputs.create(self.root, self.candidate, self.compiler, self.client, new_archive)
        self.assertFalse(new_archive.exists())
        git(self.root, "add", "source.seen")
        self.commit()
        self.reject_unpack()

    def test_exact_candidate_manifest_and_binary_bytes_are_bound(self):
        path = self.candidate / "candidate.json"
        original = path.read_bytes()
        # Still canonically valid JSON and payload, but a different origin manifest.
        path.write_bytes(original + b"\n")
        self.reject_unpack()
        path.write_bytes(original)
        compiler = self.candidate / "seen"
        compiler.write_bytes(elf(62, b"replacement"))
        value = json.loads(original)
        value["files"]["seen"] = inputs.metadata(compiler.read_bytes())
        value["stages"]["stage3"] = value["files"]["seen"]
        path.write_bytes(inputs.encode(value))
        self.reject_unpack()

    def test_symlink_hardlink_traversal_and_nonignored_paths_are_rejected(self):
        alias = self.root / ".work/alias"
        alias.symlink_to(self.compiler.parent, target_is_directory=True)
        destination = self.archive.with_name("unsafe.tar.gz")
        with self.assertRaises(OSError):
            inputs.create(self.root, self.candidate, alias / self.compiler.name, self.client, destination)
        self.assertFalse(destination.exists())
        hardlink = self.compiler.with_name("hardlinked")
        os.link(self.compiler, hardlink)
        with self.assertRaises(inputs.ArtifactError):
            inputs.create(self.root, self.candidate, self.compiler, self.client, destination)
        hardlink.unlink()
        for path in (self.root / "unignored.tar.gz", self.root / "target/../target/unsafe.tar.gz"):
            with self.assertRaises(inputs.ArtifactError):
                inputs.create(self.root, self.candidate, self.compiler, self.client, path)
        archive_alias = self.archive.with_name("alias.tar.gz")
        archive_alias.symlink_to(self.archive)
        with self.assertRaises(OSError):
            inputs.unpack(self.root, self.candidate, archive_alias, self.output)
        self.assertFalse(self.output.exists())
        parent_alias = self.root / ".work/target-alias"
        parent_alias.symlink_to(self.output.parent, target_is_directory=True)
        result = subprocess.run([
            sys.executable, str(ROOT / "scripts/ci_linux_arm64_inputs.py"), "unpack",
            "--root", str(self.root), "--candidate", str(self.candidate),
            "--archive", str(self.archive), "--output", str(parent_alias / "new-directory"),
        ], capture_output=True)
        self.assertEqual(result.returncode, 1)
        self.assertFalse((self.output.parent / "new-directory").exists())

    def test_oversize_and_unsafe_regular_file_inputs_are_rejected(self):
        destination = self.archive.with_name("oversize.tar.gz")
        for path, maximum in ((self.compiler, inputs.BINARY_LIMITS["seen"]),
                              (self.client, inputs.BINARY_LIMITS["seen-pkg"])):
            original = path.read_bytes()
            with path.open("wb") as sink:
                sink.truncate(maximum + 1)
            with self.assertRaises(inputs.ArtifactError):
                inputs.create(self.root, self.candidate, self.compiler, self.client, destination)
            self.assertFalse(destination.exists())
            path.write_bytes(original)
        for mode in (0o644, 0o4755):
            self.compiler.chmod(mode)
            with self.assertRaises(inputs.ArtifactError):
                inputs.create(self.root, self.candidate, self.compiler, self.client, destination)
            self.assertFalse(destination.exists())
        self.compiler.chmod(0o755)
        self.unpack()
        manifest = self.output / "native-candidate.json"
        with manifest.open("wb") as sink:
            sink.truncate(inputs.MANIFEST_MAX + 1)
        with self.assertRaises(inputs.ArtifactError):
            inputs.verify(self.root, self.candidate, self.output)

    def test_verify_rereads_corruption_and_rejects_extra_and_symlink_members(self):
        self.unpack()
        compiler = self.output / "seen"
        original = compiler.read_bytes()
        compiler.write_bytes(original + b"mutation")
        with self.assertRaises(inputs.ArtifactError):
            inputs.verify(self.root, self.candidate, self.output)
        compiler.write_bytes(original)
        extra = self.output / "extra"
        extra.write_bytes(b"extra")
        with self.assertRaises(inputs.ArtifactError):
            inputs.verify(self.root, self.candidate, self.output)
        extra.unlink()
        compiler.unlink()
        compiler.symlink_to(self.compiler)
        with self.assertRaises(OSError):
            inputs.verify(self.root, self.candidate, self.output)

    def test_verify_rejects_binary_substitution_during_readback(self):
        self.unpack()
        original = inputs.read_at
        reads = 0

        def change(fd, name, maximum, **kwargs):
            nonlocal reads
            data = original(fd, name, maximum, **kwargs)
            if name == "seen":
                reads += 1
                if reads == 2:
                    return data + b"racing bytes"
            return data

        with patch.object(inputs, "read_at", side_effect=change):
            with self.assertRaises(inputs.ArtifactError):
                inputs.verify(self.root, self.candidate, self.output)

    def test_source_or_candidate_change_before_publication_is_rejected(self):
        expected = inputs.binding(self.root, self.candidate)
        changed = {**expected, "source_tree": "f" * 40}
        with patch.object(inputs, "binding", side_effect=[expected, changed]):
            self.reject_unpack()
        destination = self.archive.with_name("race.tar.gz")
        with patch.object(inputs, "binding", side_effect=[expected, expected, changed]):
            with self.assertRaises(inputs.ArtifactError):
                inputs.create(self.root, self.candidate, self.compiler, self.client, destination)
        self.assertFalse(destination.exists())

    def test_file_and_directory_publication_never_overwrite(self):
        before = self.archive.read_bytes()
        with self.assertRaises(FileExistsError):
            inputs.create(self.root, self.candidate, self.compiler, self.client, self.archive)
        self.assertEqual(self.archive.read_bytes(), before)
        self.unpack()
        before = (self.output / "native-candidate.json").read_bytes()
        with self.assertRaises(FileExistsError):
            self.unpack()
        self.assertEqual((self.output / "native-candidate.json").read_bytes(), before)

    def test_partial_file_or_directory_is_not_published_and_staging_is_removed(self):
        destination = self.archive.with_name("partial.tar.gz")
        with patch.object(inputs, "rename_new", side_effect=OSError("injected before publication")):
            with self.assertRaises(OSError):
                inputs.create(self.root, self.candidate, self.compiler, self.client, destination)
        self.assertFalse(destination.exists())
        self.assertFalse(list(destination.parent.glob(".arm-inputs.*")))
        original = candidate_io.write_at
        writes = 0

        def fail_second(fd, name, data, mode):
            nonlocal writes
            writes += 1
            if writes == 2:
                raise OSError("injected partial directory write")
            return original(fd, name, data, mode)

        with patch.object(candidate_io, "write_at", side_effect=fail_second):
            with self.assertRaises(OSError):
                self.unpack()
        self.assertFalse(self.output.exists())
        self.assertFalse(list(self.output.parent.glob(".bootstrap-candidate.*")))


if __name__ == "__main__":
    unittest.main()
