#!/usr/bin/env python3
"""Consumer contract cases; fixture bytes are never executed as native programs."""

import copy
import gzip
import hashlib
import importlib.util
import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("ci_bootstrap_candidate", ROOT / "scripts/ci_bootstrap_candidate.py")
candidate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(candidate)
import ci_candidate_io as candidate_io


def git(root, *args):
    return subprocess.run(["git", "-C", str(root), *args], check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.decode().strip()


def observed_scope():
    # A modeled observation at the kernel/systemd boundary, not execution evidence.
    unit = f"seen-memory-guard-{os.getuid()}-123.scope"
    return {
        "unit": unit, "cgroup_path": "/user.slice/" + unit,
        "memory_max_bytes": 8 * 1024**3, "memory_swap_max_bytes": 0,
        "pids_max": 24, "memory_oom_group": 1,
        "derived_memory_ceiling_bytes": 8 * 1024**3,
        "main_vmem_kb": 8 * 1024**2, "opt_vmem_kb": 2 * 1024**2,
        "allocation_limit_bytes": 8 * 1024**3, "writer_rlimit_as_bytes": None,
        "serial_workers": {name: 1 for name in candidate.SERIAL_VARIABLES},
        "serializer_sha256": "a" * 64, "serializer_record_sha256": "b" * 64,
    }


class Tests(unittest.TestCase):
    def setUp(self):
        cache = ROOT / ".seen/ci-bootstrap-candidate-tests"
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
        (self.root / ".gitignore").write_text("/target/\n/.work/\n/compiler_seen/target\n")
        (self.root / "source.seen").write_text("source identity fixture\n")
        git(self.root, "init", "--quiet")
        git(self.root, "add", ".")
        self.commit()
        work = self.root / ".work"
        work.mkdir()
        self.stage2 = work / "stage2"
        self.compiler = work / "stage3"
        self.client = work / "seen-pkg"
        for path, data in ((self.stage2, b"independent stage two bytes\n"),
                           (self.compiler, b"stage three bytes\n"),
                           (self.client, b"package client bytes\n")):
            path.write_bytes(data)
            path.chmod(0o755)
        self.output = self.root / "target/seen-build/bootstrap-candidate"
        self.receipts = self.root / "target/seen-build/acceptance-receipts"
        self.stamp = self.root / "target/seen-build/full-release.stamp"
        self.baseline = patch.dict(os.environ, {"SEEN_RELEASE_CPU_BASELINE": "x86-64"})
        self.baseline.start()
        self.addCleanup(self.baseline.stop)
        candidate.create(self.root, self.compiler, self.stage2, self.client, self.output)

    def commit(self):
        git(self.root, "-c", "user.name=Contract Test", "-c", "user.email=contract@example.invalid",
            "commit", "--quiet", "-m", "fixture source")

    def manifest(self):
        return json.loads((self.output / "candidate.json").read_bytes())

    def write_manifest(self, value):
        (self.output / "candidate.json").write_bytes(candidate.encode(value))

    def journal_all(self):
        with patch.object(candidate, "live_scope", return_value=observed_scope()):
            for group in candidate.GROUPS:
                candidate.record_group(self.root, self.output, group, self.receipts / (group + ".json"))

    def previous_install(self):
        result = {}
        for directory in ("compiler_seen/target", "target/release"):
            for name in ("seen", "seen-pkg"):
                path = self.root / directory / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"previous installed " + name.encode())
                path.chmod(0o750)
                result[path] = (path.read_bytes(), path.stat().st_mode, path.stat().st_uid)
        self.stamp.write_bytes(b"previous certification\n")
        return result

    def assert_unchanged(self, files, stamp=True):
        for path, expected in files.items():
            self.assertEqual((path.read_bytes(), path.stat().st_mode, path.stat().st_uid), expected)
        if stamp:
            self.assertEqual(self.stamp.read_bytes(), b"previous certification\n")

    def reject_finalize(self):
        files = self.previous_install()
        with self.assertRaises((candidate.ArtifactError, OSError)):
            candidate.finalize(self.root, self.output, self.receipts)
        self.assert_unchanged(files)

    def archive(self, members=None):
        path = self.root / "target/candidate.tar.gz"
        if members is None:
            members = [(name, (self.output / name).read_bytes(), tarfile.REGTYPE)
                       for name in ("seen", "seen-pkg", "candidate.json")]
        with tarfile.open(path, "w:gz", format=tarfile.USTAR_FORMAT) as archive:
            for name, data, kind in members:
                info = tarfile.TarInfo(name)
                info.type = kind
                info.mode = 0o7777  # Installer deliberately ignores archive permissions.
                info.uid = 12345
                if kind in (tarfile.REGTYPE, tarfile.AREGTYPE):
                    info.size = len(data)
                    archive.addfile(info, io.BytesIO(data))
                else:
                    info.linkname = "../../source.seen"
                    archive.addfile(info)
        return path

    def test_candidate_is_unverified_and_independently_binds_stage_identities(self):
        value, raw, binaries = candidate.verify(self.root, self.output)
        self.assertEqual(value["status"], "unverified")
        self.assertNotEqual(value["stages"]["stage2"], value["stages"]["stage3"])
        self.assertEqual(binaries["seen"], self.compiler.read_bytes())
        self.assertEqual({p.name for p in self.output.iterdir()}, {"seen", "seen-pkg", "candidate.json"})
        self.assertFalse(self.stamp.exists())
        self.assertFalse((self.root / "compiler_seen/target/seen").exists())

    def test_create_requires_explicit_portable_baseline_and_does_not_overwrite(self):
        another = self.output.with_name("another")
        with patch.dict(os.environ, {"SEEN_RELEASE_CPU_BASELINE": "native"}):
            with self.assertRaises(candidate.ArtifactError):
                candidate.create(self.root, self.compiler, self.stage2, self.client, another)
        self.assertFalse(another.exists())
        before = (self.output / "candidate.json").read_bytes()
        with self.assertRaises(FileExistsError):
            candidate.create(self.root, self.compiler, self.stage2, self.client, self.output)
        self.assertEqual((self.output / "candidate.json").read_bytes(), before)

    def test_corruption_and_nonexecutable_binary_are_rejected(self):
        binary = self.output / "seen"
        original = binary.read_bytes()
        binary.write_bytes(b"corrupted compiler\n")
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output)
        binary.write_bytes(original)
        binary.chmod(0o644)
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output)

    def test_candidate_directory_rejects_extras_and_symlink_binary(self):
        extra = self.output / "unexpected"
        extra.write_bytes(b"extra")
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output)
        extra.unlink()
        binary = self.output / "seen"
        binary.unlink()
        binary.symlink_to(self.compiler)
        with self.assertRaises(OSError):
            candidate.verify(self.root, self.output)

    def test_candidate_parent_symlink_and_traversal_are_rejected(self):
        alias = self.root / ".work/alias"
        alias.symlink_to(self.output.parent, target_is_directory=True)
        with self.assertRaises(OSError):
            candidate.verify(self.root, alias / self.output.name)
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output / ".." / self.output.name)

    def test_input_parent_symlink_is_rejected_without_publishing(self):
        alias = self.root / ".work/alias"
        alias.symlink_to(self.compiler.parent, target_is_directory=True)
        output = self.output.with_name("rejected")
        with self.assertRaises(OSError):
            candidate.create(self.root, alias / self.compiler.name, self.stage2, self.client, output)
        self.assertFalse(output.exists())

    def test_oversize_candidate_files_are_rejected_before_reading(self):
        for name, maximum in {**candidate.BINARY_LIMITS, "candidate.json": candidate.JSON_MAX}.items():
            with self.subTest(name=name):
                path = self.output / name
                original = path.read_bytes()
                with path.open("wb") as sink:
                    sink.truncate(maximum + 1)
                with self.assertRaises(candidate.ArtifactError):
                    candidate.verify(self.root, self.output)
                path.write_bytes(original)

    def test_duplicate_manifest_keys_and_traversing_file_names_are_rejected(self):
        original = (self.output / "candidate.json").read_bytes()
        (self.output / "candidate.json").write_bytes(b'{"schema":"one","schema":"two"}')
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output)
        (self.output / "candidate.json").write_bytes(original)
        value = self.manifest()
        value["files"]["../seen"] = value["files"].pop("seen")
        self.write_manifest(value)
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output)

    def test_manifest_source_baseline_and_stage_substitution_are_rejected(self):
        original = self.manifest()
        changes = (("source_commit", "c" * 40), ("source_tree", "d" * 40),
                   ("cpu_baseline", "native"), ("status", "certified"))
        for field, replacement in changes:
            with self.subTest(field=field):
                value = copy.deepcopy(original)
                value[field] = replacement
                self.write_manifest(value)
                with self.assertRaises(candidate.ArtifactError):
                    candidate.verify(self.root, self.output)
        value = copy.deepcopy(original)
        value["stages"]["stage3"] = value["stages"]["stage2"]
        self.write_manifest(value)
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output)

    def test_dirty_and_different_clean_sources_are_rejected(self):
        source = self.root / "source.seen"
        source.write_text("changed source\n")
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output)
        git(self.root, "add", "source.seen")
        self.commit()
        with self.assertRaises(candidate.ArtifactError):
            candidate.verify(self.root, self.output)

    def test_frozen_checksum_substitution_is_rejected_even_on_clean_source(self):
        seed = self.root / "bootstrap/stage1_frozen"
        seed.write_bytes(b"different seed\n")
        git(self.root, "add", "bootstrap/stage1_frozen")
        self.commit()
        with self.assertRaises(candidate.ArtifactError):
            candidate.source_identity(self.root)

    def test_missing_receipt_blocks_install_and_preserves_old_certification(self):
        self.journal_all()
        for group in ("core", "gpu-runtime", "async"):
            with self.subTest(group=group):
                path = self.receipts / (group + ".json")
                original = path.read_bytes()
                path.unlink()
                self.reject_finalize()
                path.write_bytes(original)

    def test_extra_receipt_blocks_install(self):
        self.journal_all()
        (self.receipts / "extra.json").write_bytes((self.receipts / "core.json").read_bytes())
        self.reject_finalize()

    def test_obsolete_combined_receipt_cannot_replace_required_completion(self):
        self.journal_all()
        obsolete = self.receipts / "gpu-runtime-async.json"
        for replaced in (("gpu-runtime",), ("async",), ("gpu-runtime", "async"),
                         candidate.GROUPS):
            with self.subTest(replaced=replaced):
                originals = {group: (self.receipts / (group + ".json")).read_bytes()
                             for group in replaced}
                receipt = json.loads(originals[replaced[0]])
                receipt["group"] = "gpu-runtime-async"
                obsolete.write_bytes(candidate.encode(receipt))
                for group in replaced:
                    (self.receipts / (group + ".json")).unlink()
                self.reject_finalize()
                obsolete.unlink()
                for group, raw in originals.items():
                    (self.receipts / (group + ".json")).write_bytes(raw)

    def test_duplicate_group_wrong_source_and_candidate_receipts_block_install(self):
        self.journal_all()
        files = self.previous_install()
        for group in candidate.GROUPS:
            path = self.receipts / (group + ".json")
            original = json.loads(path.read_bytes())
            for field, replacement in (
                ("group", "io-simd" if group == "core" else "core"),
                ("source_tree", "c" * 40), ("candidate_manifest_sha256", "d" * 64),
                ("compiler_sha256", "e" * 64),
            ):
                with self.subTest(group=group, field=field):
                    value = copy.deepcopy(original)
                    value[field] = replacement
                    path.write_bytes(candidate.encode(value))
                    with self.assertRaisesRegex(candidate.ArtifactError, "identity mismatch"):
                        candidate.finalize(self.root, self.output, self.receipts)
                    self.assert_unchanged(files)
            path.write_bytes(candidate.encode(original))
        path = self.receipts / "core.json"
        path.write_bytes(path.read_bytes().replace(b'"group":"core"', b'"group":"core","group":"core"'))
        with self.assertRaises(candidate.ArtifactError):
            candidate.finalize(self.root, self.output, self.receipts)
        self.assert_unchanged(files)

    def test_receipt_symlink_and_oversize_are_rejected(self):
        self.journal_all()
        path = self.receipts / "core.json"
        original = path.read_bytes()
        path.unlink()
        path.symlink_to(self.receipts / "io-simd.json")
        self.reject_finalize()
        path.unlink()
        with path.open("wb") as sink:
            sink.truncate(candidate.JSON_MAX + 1)
        self.reject_finalize()

    def test_invalid_scope_receipts_block_install(self):
        self.journal_all()
        path = self.receipts / "core.json"
        original = json.loads(path.read_bytes())
        files = self.previous_install()
        for field, replacement in (
            ("memory_max_bytes", 0), ("memory_swap_max_bytes", 1), ("pids_max", 25),
            ("memory_oom_group", 0), ("opt_vmem_kb", 2097153),
            ("main_vmem_kb", 9 * 1024**2), ("cgroup_path", "/other.scope"),
            ("writer_rlimit_as_bytes", 9 * 1024**3), ("serializer_sha256", "not-a-hash"),
        ):
            with self.subTest(field=field):
                value = copy.deepcopy(original)
                value["scope"][field] = replacement
                path.write_bytes(candidate.encode(value))
                with self.assertRaises(candidate.ArtifactError):
                    candidate.finalize(self.root, self.output, self.receipts)
                self.assert_unchanged(files)
        value = copy.deepcopy(original)
        value["scope"]["serial_workers"]["SEEN_OPT_JOBS"] = 2
        path.write_bytes(candidate.encode(value))
        with self.assertRaises(candidate.ArtifactError):
            candidate.finalize(self.root, self.output, self.receipts)
        self.assert_unchanged(files)

    def test_damaged_compiler_after_receipts_blocks_install(self):
        self.journal_all()
        (self.output / "seen").write_bytes(b"modified after acceptance\n")
        self.reject_finalize()

    def test_recording_outside_actual_scope_never_creates_receipt(self):
        output = self.receipts / "core.json"
        with patch.dict(os.environ, {"SEEN_MEMORY_GUARD_IN_SCOPE": "0"}):
            with self.assertRaises(candidate.ArtifactError):
                candidate.record_group(self.root, self.output, "core", output)
        self.assertFalse(output.exists())

    def test_finalize_installs_only_after_all_groups_and_removes_obsolete_stamp(self):
        self.journal_all()
        self.previous_install()
        candidate.finalize(self.root, self.output, self.receipts)
        for directory in ("compiler_seen/target", "target/release"):
            for name in ("seen", "seen-pkg"):
                path = self.root / directory / name
                self.assertEqual(path.read_bytes(), (self.output / name).read_bytes())
                self.assertEqual(path.stat().st_mode & 0o777, 0o755)
        self.assertFalse(self.stamp.exists())
        self.assertEqual(self.manifest()["status"], "unverified")

    def test_unsafe_install_parent_or_stamp_cannot_overwrite_outside_checkout(self):
        self.journal_all()
        outside = Path(self.temporary.name) / "outside"
        outside.mkdir()
        marker = outside / "seen"
        marker.write_bytes(b"outside bytes")
        (self.root / "compiler_seen").mkdir()
        (self.root / "compiler_seen/target").symlink_to(outside, target_is_directory=True)
        with self.assertRaises(OSError):
            candidate.finalize(self.root, self.output, self.receipts)
        self.assertEqual(marker.read_bytes(), b"outside bytes")
        (self.root / "compiler_seen/target").unlink()
        self.stamp.symlink_to(marker)
        with self.assertRaises(candidate.ArtifactError):
            candidate.finalize(self.root, self.output, self.receipts)
        self.assertEqual(marker.read_bytes(), b"outside bytes")

    def test_replace_failure_restores_previous_bytes_modes_and_ownership(self):
        self.journal_all()
        files = self.previous_install()
        real_replace = candidate_io.os.replace
        calls = 0
        def fail_second(*args, **kwargs):
            nonlocal calls
            calls += 1
            if calls == 2:
                raise OSError("injected replacement failure")
            return real_replace(*args, **kwargs)
        with patch.object(candidate_io.os, "replace", side_effect=fail_second):
            with self.assertRaises(OSError):
                candidate.finalize(self.root, self.output, self.receipts)
        self.assert_unchanged(files, stamp=False)
        self.assertFalse(self.stamp.exists())

    def test_cleanup_sync_failure_reports_committed_uncertified_installation(self):
        self.journal_all()
        files = self.previous_install()
        real_fsync = candidate_io.os.fsync
        with next(iter(files)).open("rb") as original:
            def fail_after_old_inode_unlinked(fd):
                # Detect actual postcommit state, not an incidental call count.
                if os.fstat(original.fileno()).st_nlink == 0:
                    raise OSError("injected cleanup sync failure")
                return real_fsync(fd)
            with patch.object(candidate_io.os, "fsync", side_effect=fail_after_old_inode_unlinked):
                with self.assertRaises(candidate_io.InstallationCleanupError):
                    candidate.finalize(self.root, self.output, self.receipts)
        for path in files:
            self.assertEqual(path.read_bytes(), (self.output / path.name).read_bytes())
        self.assertFalse(self.stamp.exists())

    def test_safe_unpack_ignores_archive_owner_and_modes(self):
        output = self.output.with_name("unpacked")
        candidate.unpack(self.root, self.archive(), output)
        candidate.verify(self.root, output)
        self.assertEqual((output / "seen").stat().st_mode & 0o7777, 0o755)
        self.assertEqual((output / "candidate.json").stat().st_mode & 0o7777, 0o644)

    def test_unpack_rejects_paths_links_duplicates_and_missing_members(self):
        valid = [(name, (self.output / name).read_bytes(), tarfile.REGTYPE)
                 for name in ("seen", "seen-pkg", "candidate.json")]
        cases = [
            [("../../source.seen", b"overwrite", tarfile.REGTYPE), *valid],
            [("./seen", valid[0][1], tarfile.REGTYPE), *valid[1:]],
            [("seen", b"", tarfile.SYMTYPE), *valid[1:]],
            [("seen", b"", tarfile.LNKTYPE), *valid[1:]],
            [("seen", b"", tarfile.DIRTYPE), *valid[1:]],
            [*valid, valid[0]], valid[:2],
        ]
        output = self.output.with_name("rejected-unpack")
        before = (self.root / "source.seen").read_bytes()
        for members in cases:
            with self.subTest(names=[item[0] for item in members]):
                with self.assertRaises(candidate.ArtifactError):
                    candidate.unpack(self.root, self.archive(members), output)
                self.assertFalse(output.exists())
                self.assertEqual((self.root / "source.seen").read_bytes(), before)

    def test_unpack_rejects_corruption_and_bounded_expansion(self):
        output = self.output.with_name("rejected-unpack")
        archive = self.archive()
        archive.write_bytes(archive.read_bytes()[:40])
        with self.assertRaises((candidate.ArtifactError, EOFError, OSError, tarfile.TarError)):
            candidate.unpack(self.root, archive, output)
        self.assertFalse(output.exists())
        archive.write_bytes(gzip.compress(b"\0" * 4097))
        with patch.object(candidate, "TOTAL_MAX", 4096):
            with self.assertRaises(candidate.ArtifactError):
                candidate.unpack(self.root, archive, output)
        self.assertFalse(output.exists())

    def test_unpack_rejects_oversize_declarations_and_compressed_input(self):
        output = self.output.with_name("rejected-unpack")
        archive = self.root / "target/oversize.tar.gz"
        for name, maximum in {**candidate.BINARY_LIMITS, "candidate.json": candidate.JSON_MAX}.items():
            with self.subTest(name=name):
                info = tarfile.TarInfo(name)
                info.size = maximum + 1
                archive.write_bytes(gzip.compress(info.tobuf(format=tarfile.USTAR_FORMAT) + b"\0" * 1024))
                with self.assertRaises(candidate.ArtifactError):
                    candidate.unpack(self.root, archive, output)
                self.assertFalse(output.exists())
        with archive.open("wb") as sink:
            sink.truncate(candidate.TOTAL_MAX + 1)
        with self.assertRaises(candidate.ArtifactError):
            candidate.unpack(self.root, archive, output)
        self.assertFalse(output.exists())

    def test_unpack_rejects_extension_headers_and_post_tar_payload(self):
        output = self.output.with_name("rejected-unpack")
        archive = self.root / "target/extended.tar.gz"
        with tarfile.open(archive, "w:gz", format=tarfile.PAX_FORMAT) as sink:
            for name in ("seen", "seen-pkg", "candidate.json"):
                data = (self.output / name).read_bytes()
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.pax_headers = {"comment": "unneeded extension"}
                sink.addfile(info, io.BytesIO(data))
        with self.assertRaises(candidate.ArtifactError):
            candidate.unpack(self.root, archive, output)
        self.assertFalse(output.exists())
        archive = self.archive()
        raw = gzip.decompress(archive.read_bytes())
        archive.write_bytes(gzip.compress(raw + b"hidden trailing payload"))
        with self.assertRaises(candidate.ArtifactError):
            candidate.unpack(self.root, archive, output)
        self.assertFalse(output.exists())

    def test_unpack_checks_source_and_binary_before_publication(self):
        output = self.output.with_name("rejected-unpack")
        (self.output / "seen").write_bytes(b"corrupt transported bytes")
        with self.assertRaises(candidate.ArtifactError):
            candidate.unpack(self.root, self.archive(), output)
        self.assertFalse(output.exists())
        (self.output / "seen").write_bytes(self.compiler.read_bytes())
        value = self.manifest()
        value["source_commit"] = "f" * 40
        self.write_manifest(value)
        with self.assertRaises(candidate.ArtifactError):
            candidate.unpack(self.root, self.archive(), output)
        self.assertFalse(output.exists())

    def test_record_group_rejects_duplicate_and_mismatched_receipt_names(self):
        output = self.receipts / "core.json"
        with patch.object(candidate, "live_scope", return_value=observed_scope()):
            candidate.record_group(self.root, self.output, "core", output)
            original = output.read_bytes()
            with self.assertRaises(FileExistsError):
                candidate.record_group(self.root, self.output, "core", output)
            self.assertEqual(output.read_bytes(), original)
            with self.assertRaises(candidate.ArtifactError):
                candidate.record_group(self.root, self.output, "io-simd", output)
            self.assertEqual(output.read_bytes(), original)

    def test_optimizer_half_main_bound_and_honest_unlimited_writer(self):
        scope = observed_scope()
        self.assertIsNone(candidate.scope_shape(scope)["writer_rlimit_as_bytes"])
        scope["writer_rlimit_as_bytes"] = 4 * 1024**3
        self.assertEqual(candidate.scope_shape(scope)["writer_rlimit_as_bytes"], 4 * 1024**3)
        scope["writer_rlimit_as_bytes"] = None
        scope["main_vmem_kb"] = 1024**2
        scope["opt_vmem_kb"] = 768 * 1024
        with self.assertRaises(candidate.ArtifactError):
            candidate.scope_shape(scope)

    def test_file_changed_during_read_is_rejected(self):
        path = self.output / "seen"
        real_read = candidate_io.os.read
        changed = False
        def change_after_read(fd, count):
            nonlocal changed
            data = real_read(fd, count)
            if not changed:
                changed = True
                with path.open("ab") as sink:
                    sink.write(b"concurrent mutation")
            return data
        with patch.object(candidate_io.os, "read", side_effect=change_after_read):
            with self.assertRaises(candidate.ArtifactError):
                candidate_io.read_file(path, candidate.COMPILER_MAX, executable=True)


    def test_environment_scope_claim_cannot_replace_actual_kernel_membership(self):
        # Exercise the real /proc read without needing a native compiler or scope.
        proc = Path(f"/proc/{os.getpid()}/cgroup").read_text()
        unit = f"seen-memory-guard-{os.getuid()}-999999999.scope"
        self.assertNotIn(unit, proc)
        with patch.dict(os.environ, {
            "SEEN_MEMORY_GUARD_IN_SCOPE": "1", "SEEN_MEMORY_GUARD_SCOPE_UNIT": unit,
            "SEEN_MEMORY_GUARD_RSS_KB": "8388608", "SEEN_MAIN_VMEM_KB": "8388608",
        }):
            with patch.object(candidate, "command") as systemctl:
                with self.assertRaises(candidate.ArtifactError):
                    candidate.live_scope(self.root)
                systemctl.assert_not_called()

if __name__ == "__main__":
    unittest.main()
