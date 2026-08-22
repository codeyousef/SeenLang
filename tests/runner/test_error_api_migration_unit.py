#!/usr/bin/env python3
import contextlib
import copy
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("error_api", ROOT / "scripts/check_error_api_migration.py")
E = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(E)
HAPPY = ROOT / "tests/fixtures/err-001c/happy/contract.json"


class OutputCapture:
    def __init__(self):
        self.buffer = io.BytesIO()
        self.text = io.StringIO()

    def write(self, value):
        return self.text.write(value)

    def flush(self):
        return None

    def value(self):
        return self.buffer.getvalue().decode() + self.text.getvalue()


class ErrorApiMigrationTests(unittest.TestCase):
    def value(self):
        return json.loads(HAPPY.read_bytes())

    def raw(self, value):
        return json.dumps(value, separators=(",", ":"), sort_keys=True).encode() + b"\n"

    def test_happy_and_canonical(self):
        value = E.validate(HAPPY.read_bytes())
        self.assertEqual(E.canonical_bytes(value), HAPPY.read_bytes())
        self.assertTrue(E.valid_path("seen_std/src/io/file.seen"))
        self.assertFalse(E.valid_path("../file.seen"))

    def test_invalid_envelope_and_limits(self):
        for value in (None, {**self.value(), "schema": "bad"}, {**self.value(), "extra": 1}):
            with self.assertRaises(E.ContractError):
                E.validate(self.raw(value))
        with self.assertRaises(E.ContractError):
            E.validate(HAPPY.read_bytes(), max_bytes=1)
        with self.assertRaises(E.ContractError):
            E.validate(HAPPY.read_bytes(), cancelled=True)
        with self.assertRaises(E.ContractError):
            E.validate(b'{"schema":1,"schema":2}')

    def test_module_and_exception_matrix(self):
        for key, value in (("modules", []), ("modules", ["b.seen", "a.seen"]), ("bootstrap_exceptions", [])):
            changed = copy.deepcopy(self.value())
            changed[key] = value
            with self.assertRaises(E.ContractError):
                E.validate(self.raw(changed))

    def test_audit_detects_string_and_legacy_results(self):
        value = self.value()
        value["modules"] = ["seen_std/src/io/file.seen"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "seen_std/src/io/file.seen"
            path.parent.mkdir(parents=True)
            signatures = "\n".join(E.EXCEPTION_SIGNATURES.values())
            path.write_text("/// Bootstrap-safe\n" + signatures + "\nfun bad() r: Result<Int, String> {}\nclass FsFileResult {}\n")
            violations = E.audit(value, root)
            self.assertTrue(any("string-error Result" in item for item in violations))
            self.assertTrue(any("legacy FsFileResult" in item for item in violations))

            path.write_text("no boundary or signatures\n")
            violations = E.audit(value, root)
            self.assertTrue(any("missing bootstrap boundary" in item for item in violations))
            self.assertTrue(any("frozen bootstrap signature" in item for item in violations))

            value["modules"] = ["missing.seen"]
            violations = E.audit(value, root)
            self.assertTrue(any("unavailable" in item for item in violations))

    def test_fuzz(self):
        cases, rejected = E.fuzz(HAPPY.read_bytes(), 0.001, 1101)
        self.assertGreater(cases, 0)
        self.assertGreater(rejected, 0)

    def invoke(self, *args):
        out = OutputCapture()
        err = io.StringIO()
        with mock.patch.object(sys, "stdout", out), contextlib.redirect_stderr(err):
            return E.main(list(args)), out.value(), err.getvalue()

    def test_main_modes(self):
        status, out, _ = self.invoke("--validate", str(HAPPY), "--root", str(ROOT))
        self.assertEqual(status, 0)
        self.assertIn(E.SCHEMA, out)
        status, _, err = self.invoke("--validate", str(HAPPY), "--test-cancel-after-read")
        self.assertEqual(status, 130)
        self.assertIn("err.001c.cancelled", err)
        status, _, err = self.invoke("--validate", str(HAPPY), "--fuzz-seconds", "0.001")
        self.assertEqual(status, 0)
        self.assertIn("seed=1101", err)
        status, _, err = self.invoke("--validate", str(HAPPY), "--fuzz-seconds", "-1")
        self.assertEqual(status, 1)
        self.assertIn("err.001c.limit", err)
        status, _, err = self.invoke("--validate", str(HAPPY.parent / "missing.json"))
        self.assertEqual(status, 1)
        self.assertIn("err.001c.io", err)


if __name__ == "__main__":
    unittest.main()
