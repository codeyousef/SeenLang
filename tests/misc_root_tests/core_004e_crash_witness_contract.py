#!/usr/bin/env python3
"""Security, completeness, and budget failures; not a native SIGILL reproducer."""
import copy
import errno
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "verification_logs", ROOT / "scripts/collect_verification_logs.py")
logs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(logs)
crash = logs.crash


class CrashWitness(unittest.TestCase):
    def setUp(self):
        cache = ROOT / ".seen/crash-witness-tests"
        cache.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.run = self.root / ".seen/agent-tools/safe-rebuild/run.fixture"
        self.work = self.run / "core-004e.fixture"
        self.work.mkdir(parents=True)
        self.compiler = self.root / "compiler"
        self.compiler.write_bytes(b"compiler identity only\n")
        for relative in crash.SOURCES.values():
            source = self.root / relative
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_bytes(b"source identity only\n")
        self.program = self.work / "arguments-thin-lto"
        # Synthetic header for file-boundary tests only. No execution or claim
        # that these bytes reproduce the original native compiler failure.
        self.elf = b"\x7fELF\x02\x01" + bytes(58) + b"payload" * 32
        self.program.write_bytes(self.elf)
        self.witness = self.work / "crash-witness-thin-lto-short"
        self.destination = self.root / "target/seen-build/verification-logs"

    def prepare(self):
        return crash.prepare(self.root, self.work, "thin-lto", "short", self.compiler)

    def publish(self, prepared=None, status=-signal.SIGILL):
        if prepared is None:
            prepared = self.prepare()
        with patch.object(Path, "cwd", return_value=self.root):
            return crash.retain(self.root, self.work, "thin-lto", "short", prepared, status)

    def rewrite_metadata(self, report):
        (self.witness / "witness.json").write_text(json.dumps(report))

    def outcome(self, result):
        self.assertEqual(len(result["crash_witnesses"]), 1)
        return result["crash_witnesses"][0]

    def test_non_signal_exits_never_create_a_witness(self):
        prepared = self.prepare()
        for status in (0, 1, 128 + signal.SIGILL):
            self.assertIsNone(self.publish(prepared, status))
        self.assertFalse(self.witness.exists())

    def test_changed_program_is_explicitly_omitted_without_changing_signal(self):
        prepared = self.prepare()
        self.program.write_bytes(self.elf[:-1] + b"!")
        report = self.publish(prepared)
        self.assertEqual(report["returncode"], -signal.SIGILL)
        self.assertEqual(report["signal"], signal.SIGILL)
        self.assertEqual(report["executable"], {"omission": "changed"})
        self.assertFalse((self.witness / "program.elf").exists())
        result = logs.collect(self.root)
        self.assertEqual(self.outcome(result)["omission"], "changed")

    def test_source_identity_changes_are_visible_not_an_atomicity_claim(self):
        prepared = self.prepare()
        (self.root / crash.SOURCES["context"]).write_bytes(b"changed source\n")
        report = self.publish(prepared)
        context = report["identities"]["context"]
        self.assertNotEqual(context["before"], context["after"])
        self.assertEqual((self.witness / "program.elf").read_bytes(), self.elf)
        self.assertEqual(set(path.name for path in self.witness.iterdir()),
                         {"witness.json", "program.elf"})

    def test_symlink_and_fifo_programs_cannot_be_retained(self):
        self.program.unlink()
        outside = self.root / "unrelated"
        outside.write_bytes(self.elf)
        for kind in ("symlink", "fifo"):
            with self.subTest(kind=kind):
                if kind == "symlink":
                    self.program.symlink_to(outside)
                else:
                    os.mkfifo(self.program)
                state, content = crash.snapshot(self.root, self.program,
                                                crash.MAX_ELF_BYTES, keep=True, elf=True)
                self.assertEqual(state, {"omission": "unsafe-or-unavailable"})
                self.assertIsNone(content)
                self.program.unlink()

    def test_parent_symlink_and_lexical_escape_are_not_followed(self):
        outside = self.root / "unrelated"
        outside.mkdir()
        (outside / "secret").write_bytes(b"must not copy")
        link = self.work / "link"
        link.symlink_to(outside, target_is_directory=True)
        for path in (link / "secret", self.work / ".." / "secret"):
            state, content = crash.snapshot(self.root, path, 1024, keep=True)
            self.assertEqual(state, {"omission": "unsafe-or-unavailable"})
            self.assertIsNone(content)

    def test_overlarge_files_are_not_read_or_tailed(self):
        with patch.object(crash.os, "read", side_effect=AssertionError("must not read")):
            state, content = crash.snapshot(self.root, self.program, 63, keep=True, elf=True)
        self.assertEqual(state, {"omission": "over-limit"})
        self.assertIsNone(content)

    def test_short_read_and_replacement_never_return_counterfeit_bytes(self):
        with patch.object(crash.os, "read", return_value=b""):
            state, content = crash.snapshot(self.root, self.program, 1024, keep=True, elf=True)
        self.assertEqual(state, {"omission": "changed"})
        self.assertIsNone(content)
        original_read = os.read
        def replace(descriptor, maximum):
            block = original_read(descriptor, maximum)
            replacement = self.work / "replacement"
            replacement.write_bytes(self.elf)
            replacement.replace(self.program)
            return block
        with patch.object(crash.os, "read", side_effect=replace):
            state, content = crash.snapshot(self.root, self.program, 1024, keep=True, elf=True)
        self.assertEqual(state, {"omission": "changed"})
        self.assertIsNone(content)

    def test_non_elf_is_omitted_even_when_it_is_stable(self):
        self.program.write_bytes(b"#!/bin/sh\nexit 132\n")
        report = self.publish()
        self.assertEqual(report["executable"], {"omission": "not-elf"})
        self.assertFalse((self.witness / "program.elf").exists())

    def test_interrupted_metadata_publication_does_not_upload_an_unbound_elf(self):
        create = crash.create_file
        def interrupt(directory, name, content):
            if name == "witness.json":
                raise OSError("interrupted")
            create(directory, name, content)
        with patch.object(crash, "create_file", side_effect=interrupt):
            with self.assertRaises(OSError):
                self.publish()
        self.assertTrue((self.witness / "program.elf").exists())
        result = logs.collect(self.root)
        self.assertEqual(result["files"], [])
        self.assertEqual(self.outcome(result)["omission"], "unsafe-or-unavailable")

    def test_collector_rejects_non_whitelisted_metadata_without_copying_it(self):
        original = self.publish()
        changes = []
        row = copy.deepcopy(original)
        row["command"]["environment"]["PRIVATE_TOKEN"] = "must-not-upload"
        changes.append(row)
        row = copy.deepcopy(original)
        row["command"]["argv"].append("private argument")
        changes.append(row)
        row = copy.deepcopy(original)
        row["source_contents"] = "must-not-upload"
        changes.append(row)
        row = copy.deepcopy(original)
        row["compile"]["compiler"] = "../../secret"
        changes.append(row)
        row = copy.deepcopy(original)
        row["returncode"] = 128 + signal.SIGILL
        changes.append(row)
        for report in changes:
            with self.subTest(report=report):
                self.rewrite_metadata(report)
                state, content = crash.snapshot(self.root, self.witness / "witness.json",
                                                crash.MAX_METADATA_BYTES, keep=True)
                self.assertNotIn("omission", state)
                with self.assertRaises((ValueError, TypeError)):
                    crash.validate(json.loads(content), self.work.relative_to(self.root).as_posix())
        result = logs.collect(self.root)
        self.assertEqual(result["files"], [])
        self.assertEqual(self.outcome(result)["omission"], "invalid-metadata")

    def test_collector_rejects_partial_and_overlarge_metadata(self):
        self.publish()
        (self.witness / "witness.json").write_bytes(b'{"schema":')
        result = logs.collect(self.root)
        self.assertEqual(result["files"], [])
        self.assertEqual(self.outcome(result)["omission"], "invalid-metadata")
        # A second isolated destination for the independently overlarge marker.
        self.destination.rename(self.destination.with_name("previous-logs"))
        (self.witness / "witness.json").write_bytes(b"x" * (crash.MAX_METADATA_BYTES + 1))
        result = logs.collect(self.root)
        self.assertEqual(result["files"], [])
        self.assertEqual(self.outcome(result)["omission"], "over-limit")

    def test_tampered_witness_elf_never_enters_the_upload(self):
        self.publish()
        (self.witness / "program.elf").write_bytes(self.elf[:-1] + b"!")
        result = logs.collect(self.root)
        self.assertEqual(self.outcome(result)["omission"], "identity-mismatch")
        self.assertEqual([Path(row["source"]).name for row in result["files"]], ["witness.json"])

    def test_witness_elf_symlink_is_reported_not_opened(self):
        self.publish()
        binary = self.witness / "program.elf"
        binary.unlink()
        binary.symlink_to(self.program)
        result = logs.collect(self.root)
        self.assertEqual(self.outcome(result)["omission"], "unsafe-or-unavailable")
        self.assertFalse(any(row["file"].endswith(".elf") for row in result["files"]))

    def test_collector_budget_omits_whole_elf_and_includes_index_reservation(self):
        self.publish()
        metadata_bytes = (self.witness / "witness.json").stat().st_size
        (self.run / "safe_rebuild_verify.log").write_bytes(b"original failure\n" * 100)
        limit = logs.MAX_INDEX_BYTES + metadata_bytes + len(self.elf) - 1
        with patch.object(logs, "MAX_TOTAL_BYTES", limit), patch.object(logs, "MAX_FILES", 3):
            result = logs.collect(self.root)
        self.assertEqual(self.outcome(result)["omission"], "budget")
        self.assertFalse(any(row["file"].endswith(".elf") for row in result["files"]))
        uploaded = list(self.destination.iterdir())
        self.assertLessEqual(len(uploaded), 3)
        self.assertLessEqual(sum(path.stat().st_size for path in uploaded), limit)
        self.assertEqual(result["retained_bytes"], sum(row["retained_bytes"] for row in result["files"]))

    def test_collector_keeps_complete_bound_elf_without_broad_native_traversal(self):
        self.publish()
        (self.work / "unrelated.ll").write_bytes(b"private source IR")
        nested = self.work / "native"
        nested.mkdir()
        os.mkfifo(nested / "witness.json")
        os.mkfifo(self.witness / "secret")
        (self.witness / "source.seen").write_bytes(b"private source")
        result = logs.collect(self.root)
        self.assertTrue(self.outcome(result)["retained_elf"])
        retained = {Path(row["source"]).name: (self.destination / row["file"]).read_bytes()
                    for row in result["files"]}
        self.assertEqual(set(retained), {"witness.json", "program.elf"})
        self.assertEqual(retained["program.elf"], self.elf)
        self.assertLessEqual(len(list(self.destination.iterdir())), logs.MAX_FILES)
        self.assertLessEqual(sum(path.stat().st_size for path in self.destination.iterdir()),
                             logs.MAX_TOTAL_BYTES)


    def test_marker_replacement_during_elf_collection_is_reported(self):
        self.publish()
        snapshot = crash.snapshot
        def replace_marker(root, path, maximum, **options):
            result = snapshot(root, path, maximum, **options)
            if path.name == "program.elf":
                marker = self.witness / "witness.json"
                replacement = self.witness / "replacement"
                replacement.write_bytes(marker.read_bytes())
                replacement.replace(marker)
            return result
        with patch.object(crash, "snapshot", side_effect=replace_marker):
            result = logs.collect(self.root)
        self.assertEqual(self.outcome(result)["omission"], "changed")
        self.assertFalse(any(row["file"].endswith(".elf") for row in result["files"]))

    def test_witness_parent_symlink_is_not_followed(self):
        self.publish()
        moved = self.work / "unrelated-witness"
        self.witness.rename(moved)
        self.witness.symlink_to(moved, target_is_directory=True)
        result = logs.collect(self.root)
        self.assertEqual(result["files"], [])
        self.assertEqual(self.outcome(result)["omission"], "unsafe-or-unavailable")

    def test_standalone_fixture_is_discovered_without_a_rebuild_run(self):
        moved = self.root / ".seen/agent-tools/core-004e.standalone"
        self.work.rename(moved)
        self.work = moved
        self.program = moved / "arguments-thin-lto"
        self.witness = moved / "crash-witness-thin-lto-short"
        self.publish()
        result = logs.collect(self.root)
        self.assertTrue(self.outcome(result)["retained_elf"])

    def test_supported_capped_scopes_retain_the_complete_witness(self):
        for scope in ("seen-core-004e", "stage1-acceptance-quick", "stage1-acceptance-verify"):
            with self.subTest(scope=scope):
                moved = self.root / ".seen/agent-tools" / scope / "run.native" / "core-004e.fixture"
                moved.parent.mkdir(parents=True)
                self.work.rename(moved)
                self.work = moved
                self.program = moved / "arguments-thin-lto"
                self.witness = moved / "crash-witness-thin-lto-short"
                self.publish()
                result = logs.collect(self.root)
                self.assertTrue(self.outcome(result)["retained_elf"])
                self.destination.rename(self.destination.with_name("previous-" + scope))
                (self.witness / "witness.json").unlink()
                (self.witness / "program.elf").unlink()
                self.witness.rmdir()

    def test_duplicate_metadata_keys_never_upload_hidden_private_values(self):
        report = self.publish()
        original = json.dumps(report)
        variants = (
            '{"command":{"environment":{"PRIVATE_TOKEN":"must-not-upload"}},' + original[1:],
            original.replace('"command": {',
                             '"command": {"environment":{"PRIVATE_TOKEN":"must-not-upload"},', 1),
            original.replace('"environment": {', '"environment": {"LANG":"must-not-upload",', 1),
        )
        for index, content in enumerate(variants):
            with self.subTest(index=index):
                (self.witness / "witness.json").write_text(content)
                result = logs.collect(self.root)
                self.assertEqual(result["files"], [])
                self.assertEqual(self.outcome(result)["omission"], "invalid-metadata")
                self.assertTrue(all(b"must-not-upload" not in path.read_bytes()
                                    for path in self.destination.iterdir()))
                self.destination.rename(self.destination.with_name(f"rejected-{index}"))

    def test_interrupted_collector_write_never_publishes_partial_elf(self):
        self.publish()
        fdopen = os.fdopen
        class InterruptedWriter:
            def __init__(self, descriptor, mode):
                self.stream = fdopen(descriptor, mode)
            def __enter__(self):
                return self
            def __exit__(self, *arguments):
                self.stream.close()
            def write(self, content):
                if content.startswith(b"\x7fELF"):
                    self.stream.write(content[:11])
                    raise OSError(errno.ENOSPC, "injected full filesystem")
                return self.stream.write(content)
        with patch.object(logs.os, "fdopen", side_effect=InterruptedWriter):
            result = logs.collect(self.root)
        self.assertEqual(self.outcome(result)["omission"], "publication-failed")
        self.assertFalse(any(path.suffix == ".elf" for path in self.destination.iterdir()))
        self.assertFalse(list(self.destination.parent.glob(".verification-publish-*")))

    def test_private_tmp_compiler_alias_is_anchored_by_real_mount_identity(self):
        for name in ("seen_quick_rebuild", "seen_verify_rebuild"):
            (self.run / name).write_bytes(self.compiler.read_bytes())
            script = (
                "import importlib.util,json,os,sys;from pathlib import Path;"
                "s=importlib.util.spec_from_file_location('w',sys.argv[1]);"
                "w=importlib.util.module_from_spec(s);s.loader.exec_module(w);"
                "root,work,artifact=map(Path,sys.argv[2:5]);"
                "os.environ['SEEN_ARTIFACT_ROOT']=str(artifact);"
                "p=w.prepare(root,work,'thin-lto','short',Path('/tmp')/sys.argv[5]);"
                "print(json.dumps(p['paths']))"
            )
            result = subprocess.run([
                "bwrap", "--die-with-parent", "--bind", "/", "/",
                "--bind", str(self.run), "/tmp", "--", sys.executable, "-c", script,
                str(ROOT / "scripts/core_004e_crash_witness.py"),
                str(self.root), str(self.work), str(self.run), name,
            ], check=True, capture_output=True, text=True, timeout=10)
            self.assertEqual(json.loads(result.stdout)["compiler"],
                             (self.run / name).relative_to(self.root).as_posix())
        with patch.dict(os.environ, {"SEEN_ARTIFACT_ROOT": str(self.run)}):
            with self.assertRaises(ValueError):
                crash.anchored_compiler(self.root, Path("/tmp/seen_quick_rebuild"))

    def test_aggregate_witness_budget_is_shared_and_index_owns_a_file_slot(self):
        self.publish()
        metadata_bytes = (self.witness / "witness.json").stat().st_size
        with patch.object(Path, "cwd", return_value=self.root):
            prepared = crash.prepare(self.root, self.work, "thin-lto", "long", self.compiler)
            crash.retain(self.root, self.work, "thin-lto", "long", prepared, -signal.SIGILL)
        limit = metadata_bytes + len(self.elf)
        with patch.object(crash, "MAX_WITNESS_BYTES", limit), patch.object(logs, "MAX_FILES", 3):
            result = logs.collect(self.root)
        self.assertLessEqual(sum(row["retained_bytes"] for row in result["files"]), limit)
        self.assertLessEqual(len(list(self.destination.iterdir())), 3)
        self.assertTrue(any(row.get("omission") == "budget" for row in result["crash_witnesses"]))


if __name__ == "__main__":
    unittest.main()
