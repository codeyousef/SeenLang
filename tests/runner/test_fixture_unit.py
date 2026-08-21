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
    "check_test_fixture", ROOT / "scripts/check_test_fixture.py")
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(MODULE)
HAPPY = ROOT / "tests/fixtures/test-001d/happy/fixture.json"


class TestFixtureContract(unittest.TestCase):
    def happy(self):
        return MODULE.load(HAPPY)

    def test_happy(self):
        fixture = MODULE.validate(self.happy())
        self.assertEqual(fixture["seed"], 1101)
        self.assertEqual([item["path"] for item in fixture["files"]],
                         ["input/a.txt", "input/b.txt"])

    def test_invalid_fixture(self):
        with self.assertRaises(MODULE.ContractError):
            MODULE.validate(MODULE.load(
                ROOT / "tests/fixtures/test-001d/invalid/fixture.json"))
        with tempfile.TemporaryDirectory() as temporary:
            duplicate = Path(temporary) / "duplicate.json"
            duplicate.write_text('{"schema":"seen-test-fixture-v1",'
                                 '"schema":"seen-test-fixture-v1"}',
                                 encoding="utf-8")
            with self.assertRaises(MODULE.ContractError):
                MODULE.load(duplicate)

    def test_invalid_matrix(self):
        base = self.happy()
        mutations = [None, {**base, "schema": "bad"},
                     {**base, "name": "../bad"}, {**base, "name": "."},
                     {**base, "seed": True},
                     {**base, "target": "unknown"}, {**base, "files": None},
                     {**base, "environment": None}, {**base, "extra": True}]
        for payload in mutations:
            with self.subTest(payload=payload), self.assertRaises(MODULE.ContractError):
                MODULE.validate(payload)

    def test_file_and_environment_validation(self):
        base = self.happy()
        bad_files = [[None], [{"path": "/bad", "content": "x"}],
                     [{"path": "a/./b", "content": "x"}],
                     [{"path": ".hidden", "content": "x"}],
                     [{"path": "a", "content": None}],
                     [{"path": "b", "content": "x"},
                      {"path": "a", "content": "x"}]]
        for files in bad_files:
            with self.assertRaises(MODULE.ContractError):
                MODULE.validate({**base, "files": files})
        bad_environment = [[None], [{"name": "PATH", "value": "x"}],
                           [{"name": "SEEN_TEST_SECRET", "value": "x"}],
                           [{"name": "TZ", "value": "bad\n"}],
                           [{"name": "TZ", "value": "x"},
                            {"name": "LANG", "value": "C"}]]
        for environment in bad_environment:
            with self.assertRaises(MODULE.ContractError):
                MODULE.validate({**base, "environment": environment})

    def test_limits(self):
        base = self.happy()
        with self.assertRaises(MODULE.ContractError):
            MODULE.validate({**base, "files": [
                {"path": f"{index:04d}", "content": ""}
                for index in range(MODULE.MAX_FILES + 1)]})
        with self.assertRaises(MODULE.ContractError):
            MODULE.validate({**base, "files": [{
                "path": "large", "content": "x" * (MODULE.MAX_CONTENT_BYTES + 1)}]})
        with self.assertRaises(MODULE.ContractError):
            MODULE.validate({**base, "environment": [
                {"name": f"SEEN_TEST_{index:04d}", "value": ""}
                for index in range(MODULE.MAX_ENVIRONMENT + 1)]})

    def test_materialize_and_cleanup(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            workspace = MODULE.materialize(self.happy(), base)
            self.assertEqual((workspace.root / "input/a.txt").read_bytes(), b"alpha\n")
            with self.assertRaises(MODULE.ContractError):
                MODULE.materialize(self.happy(), base)
            MODULE.cleanup(workspace)
            self.assertFalse(workspace.root.exists())
            MODULE.cleanup(workspace)
            with self.assertRaises(MODULE.ContractError):
                MODULE.cleanup(object())

    def test_materialize_rejects_symlink_base(self):
        with tempfile.TemporaryDirectory() as temporary:
            physical = Path(temporary).resolve() / "physical"
            physical.mkdir()
            link = Path(temporary).resolve() / "link"
            link.symlink_to(physical, target_is_directory=True)
            with self.assertRaises(MODULE.ContractError):
                MODULE.materialize(self.happy(), link)

    def test_fuzz_benchmark_and_load_failure(self):
        self.assertGreater(MODULE.fuzz(0, 1101), 0)
        MODULE.benchmark(self.happy(), 1000)
        with mock.patch.object(MODULE.time, "perf_counter_ns",
                               side_effect=range(0, 100_000_000, 1_000_000)):
            with self.assertRaises(MODULE.ContractError):
                MODULE.benchmark(self.happy(), 0.01)
        with self.assertRaises(MODULE.ContractError):
            MODULE.load(HAPPY.with_name("missing.json"))
        with tempfile.TemporaryDirectory() as temporary:
            empty = Path(temporary) / "empty.json"
            empty.write_bytes(b"")
            with self.assertRaises(MODULE.ContractError):
                MODULE.load(empty)

    def invoke_main(self, *arguments):
        with mock.patch.object(sys, "argv", ["check_test_fixture.py", *arguments]):
            with contextlib.redirect_stdout(io.StringIO()):
                return MODULE.main()

    def test_main_modes(self):
        happy = str(HAPPY)
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
