#!/usr/bin/env python3
import contextlib
import copy
import importlib.util
import io
import json
import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("error_policy", ROOT / "scripts/check_error_policy.py")
E = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(E)
HAPPY = ROOT / "tests/fixtures/err-001d/happy/contract.json"


class OutputCapture:
    def __init__(self): self.buffer = io.BytesIO(); self.text = io.StringIO()
    def write(self, value): return self.text.write(value)
    def flush(self): return None
    def value(self): return self.buffer.getvalue().decode() + self.text.getvalue()


class ErrorPolicyTests(unittest.TestCase):
    def value(self): return json.loads(HAPPY.read_bytes())
    def raw(self, value): return json.dumps(value, separators=(",", ":"), sort_keys=True).encode() + b"\n"

    def test_disposition_matrix(self):
        base = self.value()
        cases = (("never", 0, "permanent"), ("transient", 1, "retry"), ("transient", 3, "exhausted"))
        for retry, attempt, expected in cases:
            value = copy.deepcopy(base); value["error"]["retry"] = retry; value["attempt"] = attempt
            checked = E.validate(self.raw(value)); self.assertEqual(E.classify(checked)["disposition"], expected)
        cancelled = copy.deepcopy(base); cancelled["error"].update(code="err.001b.cancelled", subsystem="cancelled", retry="never")
        self.assertEqual(E.classify(E.validate(self.raw(cancelled)))["disposition"], "cancelled")

    def test_redaction_and_canonical(self):
        value = self.value(); value["error"]["message"] = "/private/secret"; value["error"]["redaction"] = "sensitive"
        rendered = E.canonical_bytes(E.validate(self.raw(value)))
        self.assertIn(b'"message":"[redacted]"', rendered); self.assertNotIn(b"/private/secret", rendered)
        self.assertTrue(E.identity("io.read-1")); self.assertFalse(E.identity("IO"))

    def test_invalid_matrix(self):
        base = self.value()
        changes = (("schema", "bad"), ("attempt", True), ("attempt", -1), ("max_attempts", 0))
        for key, item in changes:
            value = copy.deepcopy(base); value[key] = item
            with self.assertRaises(E.ContractError): E.validate(self.raw(value))
        for key, item in (("code", "BAD"), ("message", 1), ("retry", "later"), ("redaction", "secret")):
            value = copy.deepcopy(base); value["error"][key] = item
            with self.assertRaises(E.ContractError): E.validate(self.raw(value))
        value = copy.deepcopy(base); value["error"].update(code="err.cancelled", subsystem="cancelled", retry="transient")
        with self.assertRaises(E.ContractError): E.validate(self.raw(value))

    def test_envelope_limits_and_json(self):
        with self.assertRaises(E.ContractError): E.validate(b'{"schema":1,"schema":2}')
        with self.assertRaises(E.ContractError): E.validate(HAPPY.read_bytes(), max_bytes=1)
        with self.assertRaises(E.ContractError): E.validate(HAPPY.read_bytes(), cancelled=True)
        value = self.value(); value["extra"] = 1
        with self.assertRaises(E.ContractError): E.validate(self.raw(value))
        value = self.value(); value["error"]["message"] = "x" * 4097
        with self.assertRaises(E.ContractError): E.validate(self.raw(value))

    def test_fuzz(self):
        cases, rejected = E.fuzz(HAPPY.read_bytes(), 0.001, 1101)
        self.assertGreater(cases, 0); self.assertGreater(rejected, 0)

    def invoke(self, *args):
        out = OutputCapture(); err = io.StringIO()
        with mock.patch.object(sys, "stdout", out), contextlib.redirect_stderr(err):
            return E.main(list(args)), out.value(), err.getvalue()

    def test_main_modes(self):
        status, out, _ = self.invoke("--validate", str(HAPPY)); self.assertEqual(status, 0); self.assertIn("retry", out)
        status, _, err = self.invoke("--validate", str(HAPPY), "--test-cancel-after-read"); self.assertEqual(status, 130); self.assertIn("cancelled", err)
        status, _, err = self.invoke("--validate", str(HAPPY), "--fuzz-seconds", "0.001"); self.assertEqual(status, 0); self.assertIn("seed=1101", err)
        status, _, _ = self.invoke("--validate", str(HAPPY), "--fuzz-seconds", "-1"); self.assertEqual(status, 1)
        status, _, err = self.invoke("--validate", str(HAPPY.parent / "missing.json")); self.assertEqual(status, 1); self.assertIn("err.001d.io", err)


if __name__ == "__main__": unittest.main()
