#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import contextlib
import io
import json
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("check_test_runner", ROOT / "scripts/check_test_runner.py")
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(MODULE)


class TestRunnerContract(unittest.TestCase):
    def test_happy(self):
        report = MODULE.validate(MODULE.load(ROOT / "tests/fixtures/test-001b/happy/report.json"))
        self.assertEqual(report["failed"], 1)

    def test_invalid(self):
        with self.assertRaises(MODULE.ContractError):
            MODULE.validate(MODULE.load(ROOT / "tests/fixtures/test-001b/invalid/report.json"))

    def test_invalid_matrix(self):
        base = {"schema": MODULE.SCHEMA, "results": [], "passed": 0,
                "failed": 0, "skipped": 0, "exit_code": 0}
        mutations = [None, {**base, "schema": "bad"},
                     {**base, "results": "bad"},
                     {**base, "results": [None]},
                     {**base, "results": [{"path": "/bad", "status": "passed", "exit_code": 0}]},
                     {**base, "results": [{"path": "a", "status": "unknown", "exit_code": 0}]},
                     {**base, "results": [{"path": "a", "status": "passed", "exit_code": 1}]},
                     {**base, "passed": True}, {**base, "exit_code": 1}]
        for payload in mutations:
            with self.subTest(payload=payload), self.assertRaises(MODULE.ContractError):
                MODULE.validate(payload)

    def test_paths(self):
        self.assertTrue(MODULE.canonical_path("compiler_seen/tests/a.seen"))
        for path in ("", "/tmp/a", "a/../b", "a\\b", "a//b", "bad name"):
            self.assertFalse(MODULE.canonical_path(path))

    def test_fuzz_and_benchmark(self):
        self.assertGreater(MODULE.fuzz(0, 1101), 0)
        MODULE.benchmark(MODULE.load(ROOT / "tests/fixtures/test-001b/happy/report.json"), 1000)
        with mock.patch.object(MODULE.time, "perf_counter_ns", side_effect=range(0, 100_000_000, 1_000_000)):
            with self.assertRaises(MODULE.ContractError):
                MODULE.benchmark(MODULE.load(ROOT / "tests/fixtures/test-001b/happy/report.json"), 0.01)

    def test_load_failure(self):
        with self.assertRaises(MODULE.ContractError):
            MODULE.load(ROOT / "tests/fixtures/test-001b/missing.json")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.json"
            path.write_text("{", encoding="utf-8")
            with self.assertRaises(MODULE.ContractError):
                MODULE.load(path)

    def invoke_main(self, *arguments):
        with mock.patch.object(sys, "argv", ["check_test_runner.py", *arguments]):
            with contextlib.redirect_stdout(io.StringIO()):
                return MODULE.main()

    def test_main_modes(self):
        happy = str(ROOT / "tests/fixtures/test-001b/happy/report.json")
        self.assertEqual(self.invoke_main("--validate", happy), 0)
        self.assertEqual(self.invoke_main("--validate", happy, "--fuzz-seconds", "0.0001"), 0)
        self.assertEqual(self.invoke_main("--validate", happy, "--benchmark-limit-ms", "10"), 0)
        with self.assertRaises(MODULE.ContractError):
            self.invoke_main("--fuzz-seconds", "-1")
        with self.assertRaises(MODULE.ContractError):
            self.invoke_main("--benchmark-limit-ms", "0")


if __name__ == "__main__":
    unittest.main()
