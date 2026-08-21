#!/usr/bin/env python3
import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_test_snapshots", ROOT / "scripts/check_test_snapshots.py")
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(MODULE)


class TestSnapshotContract(unittest.TestCase):
    def test_happy_and_exact_content(self):
        payload = MODULE.validate(MODULE.load(
            ROOT / "tests/fixtures/test-001c/happy/snapshot.json"))
        self.assertEqual(payload["content"], "hello, Seen!\n")

    def test_invalid_fixture(self):
        with self.assertRaises(MODULE.ContractError):
            MODULE.validate(MODULE.load(
                ROOT / "tests/fixtures/test-001c/invalid/snapshot.json"))

    def test_invalid_matrix_and_limit(self):
        base = {"schema": MODULE.SCHEMA, "name": "case", "content": "ok"}
        mutations = [None, {**base, "schema": "bad"},
                     {**base, "name": ""}, {**base, "name": "bad/name"},
                     {**base, "content": None}, {**base, "extra": True}]
        for payload in mutations:
            with self.subTest(payload=payload), self.assertRaises(MODULE.ContractError):
                MODULE.validate(payload)
        with self.assertRaises(MODULE.ContractError):
            MODULE.validate({**base, "content": "x" * (MODULE.MAX_CONTENT_BYTES + 1)})

    def test_safe_names(self):
        self.assertTrue(MODULE.safe_name("linux-x86_64.case"))
        for value in (None, "", ".", ".hidden", "../a", "a/b", "bad name", "é"):
            self.assertFalse(MODULE.safe_name(value))

    def test_fuzz_and_benchmark(self):
        self.assertGreater(MODULE.fuzz(0, 1101), 0)
        payload = MODULE.load(ROOT / "tests/fixtures/test-001c/happy/snapshot.json")
        MODULE.benchmark(payload, 1000)
        with mock.patch.object(MODULE.time, "perf_counter_ns",
                               side_effect=range(0, 100_000_000, 1_000_000)):
            with self.assertRaises(MODULE.ContractError):
                MODULE.benchmark(payload, 0.01)

    def test_load_failures(self):
        with self.assertRaises(MODULE.ContractError):
            MODULE.load(ROOT / "tests/fixtures/test-001c/missing.json")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.json"
            path.write_text("{", encoding="utf-8")
            with self.assertRaises(MODULE.ContractError):
                MODULE.load(path)

    def invoke_main(self, *arguments):
        with mock.patch.object(sys, "argv", ["check_test_snapshots.py", *arguments]):
            with contextlib.redirect_stdout(io.StringIO()):
                return MODULE.main()

    def test_main_modes(self):
        happy = str(ROOT / "tests/fixtures/test-001c/happy/snapshot.json")
        self.assertEqual(self.invoke_main("--validate", happy), 0)
        self.assertEqual(self.invoke_main("--validate", happy,
                                         "--fuzz-seconds", "0.0001"), 0)
        self.assertEqual(self.invoke_main("--validate", happy,
                                         "--benchmark-limit-ms", "10"), 0)
        with self.assertRaises(MODULE.ContractError):
            self.invoke_main("--fuzz-seconds", "-1")
        with self.assertRaises(MODULE.ContractError):
            self.invoke_main("--benchmark-limit-ms", "0")


if __name__ == "__main__":
    unittest.main()
