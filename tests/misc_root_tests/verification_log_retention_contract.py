#!/usr/bin/env python3
"""Filesystem and budget regressions for failed-verification diagnostics."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("verification_logs", ROOT / "scripts/collect_verification_logs.py")
logs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(logs)


class Retention(unittest.TestCase):
    def setUp(self):
        cache = ROOT / ".seen/verification-log-tests"
        cache.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.run = self.root / ".seen/agent-tools/safe-rebuild/run.fixture"
        self.run.mkdir(parents=True)

    def test_tails_and_priority_preserve_the_actual_stop(self):
        (self.run / "safe_rebuild_verify_stage1_acceptance.log").write_bytes(b"old-prefix\n" + b"TIMEOUT: active gate\n")
        leaf = self.run / "gpu.fixture"
        leaf.mkdir()
        (leaf / "00-active.log").write_bytes(b"leaf bytes")
        with patch.object(logs, "MAX_FILE_BYTES", 21), patch.object(logs, "MAX_TOTAL_BYTES", 26):
            result = logs.collect(self.root)
        self.assertEqual(result["retained_bytes"], 26)
        self.assertEqual(result["files"][0]["original_bytes"], 32)
        destination = self.root / "target/seen-build/verification-logs"
        self.assertEqual((destination / "log-000.txt").read_bytes(), b"TIMEOUT: active gate\n")
        self.assertEqual(json.loads((destination / "index.json").read_text()), result)

    def test_file_limit_is_explicit(self):
        for index in range(3):
            (self.run / f"safe_rebuild_{index}.log").write_text("x")
        with patch.object(logs, "MAX_FILES", 2):
            result = logs.collect(self.root)
        self.assertEqual(len(result["files"]), 2)
        self.assertEqual(result["omitted_files"], 1)

    def test_file_symlink_is_rejected(self):
        outside = self.root / "unrelated.txt"
        outside.write_text("not a log")
        (self.run / "safe_rebuild_bad.log").symlink_to(outside)
        with self.assertRaises(OSError):
            logs.collect(self.root)

    def test_parent_symlink_is_rejected(self):
        outside = self.root / "unrelated"
        outside.mkdir()
        (outside / "00-active.log").write_text("not a log")
        (self.run / "gpu.fixture").symlink_to(outside, target_is_directory=True)
        with self.assertRaises(OSError):
            logs.collect(self.root)

    def test_no_logs_still_records_a_bounded_empty_manifest(self):
        result = logs.collect(self.root)
        self.assertEqual(result["files"], [])
        self.assertEqual(result["retained_bytes"], 0)

    def test_nonregular_log_cannot_block_collection(self):
        os.mkfifo(self.run / "safe_rebuild_pipe.log")
        with self.assertRaises(ValueError):
            logs.collect(self.root)

    def test_discovery_has_an_entry_budget(self):
        for index in range(3):
            (self.run / f"safe_rebuild_{index}.log").write_text("x")
        with patch.object(logs, "MAX_ENTRIES", 2), self.assertRaises(ValueError):
            logs.collect(self.root)


if __name__ == "__main__":
    unittest.main()
