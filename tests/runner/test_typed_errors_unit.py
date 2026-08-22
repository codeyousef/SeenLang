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
SPEC = importlib.util.spec_from_file_location(
    "typed_errors", ROOT / "scripts/check_typed_errors.py"
)
T = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(T)
HAPPY = ROOT / "tests/fixtures/err-001b/happy/contract.json"


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


class TypedErrorTests(unittest.TestCase):
    def value(self):
        return json.loads(HAPPY.read_bytes())

    def raw(self, value):
        return json.dumps(value, separators=(",", ":"), sort_keys=True).encode() + b"\n"

    def test_happy_categories_and_canonical_output(self):
        base = self.value()
        for kind in sorted(T.KINDS):
            value = copy.deepcopy(base)
            value["kind"] = kind
            value["retry"] = "never" if kind in ("cancelled", "parse") else "transient"
            validated = T.validate(self.raw(value))
            rendered = T.canonical_bytes(validated)
            self.assertIn(f'"code":"err.001b.{kind}"'.encode(), rendered)
            self.assertIn(f'"subsystem":"{kind}"'.encode(), rendered)
        self.assertTrue(T.identity("io.read-1"))
        self.assertFalse(T.identity("IO"))

    def test_invalid_envelope_and_json(self):
        base = self.value()
        for value in (None, {**base, "schema": "bad"}, {**base, "extra": 1}):
            with self.assertRaises(T.ContractError):
                T.validate(self.raw(value))
        for raw in (b"", b'{"kind":1,"kind":2}', b"\xff"):
            with self.assertRaises(T.ContractError):
                T.validate(raw)

    def test_invalid_field_matrix(self):
        base = self.value()
        changes = (
            ("kind", "other"),
            ("operation", "Read"),
            ("message", 1),
            ("retry", "later"),
            ("redaction", "secret"),
            ("native_code", True),
            ("native_code", 1 << 63),
        )
        for key, value in changes:
            changed = copy.deepcopy(base)
            changed[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(T.ContractError):
                T.validate(self.raw(changed))
        for kind in ("cancelled", "parse"):
            changed = copy.deepcopy(base)
            changed["kind"] = kind
            with self.assertRaises(T.ContractError):
                T.validate(self.raw(changed))

    def test_limits_cancel_and_redaction(self):
        base = self.value()
        base["message"] = "x" * 4097
        with self.assertRaises(T.ContractError):
            T.validate(self.raw(base))
        with self.assertRaises(T.ContractError):
            T.validate(HAPPY.read_bytes(), max_bytes=1)
        with self.assertRaises(T.ContractError):
            T.validate(HAPPY.read_bytes(), cancelled=True)
        sensitive = self.value()
        sensitive["message"] = "/private/secret/path"
        sensitive["redaction"] = "sensitive"
        rendered = T.canonical_bytes(T.validate(self.raw(sensitive)))
        self.assertNotIn(b"/private/secret/path", rendered)
        self.assertIn(b"[redacted]", rendered)

    def test_fuzz(self):
        cases, rejected = T.fuzz(HAPPY.read_bytes(), 0.001, 1101)
        self.assertGreater(cases, 0)
        self.assertGreater(rejected, 0)

    def invoke(self, *args):
        out = OutputCapture()
        err = io.StringIO()
        with mock.patch.object(sys, "stdout", out), contextlib.redirect_stderr(err):
            return T.main(list(args)), out.value(), err.getvalue()

    def test_main_modes(self):
        status, out, _ = self.invoke("--validate", str(HAPPY))
        self.assertEqual(status, 0)
        self.assertIn("err.001b.network", out)
        status, _, err = self.invoke(
            "--validate", str(HAPPY), "--test-cancel-after-read"
        )
        self.assertEqual(status, 130)
        self.assertIn("err.001b.cancelled", err)
        status, _, _ = self.invoke("--validate", str(HAPPY), "--fuzz-seconds", "-1")
        self.assertEqual(status, 1)
        status, _, err = self.invoke("--validate", str(HAPPY.parent / "missing.json"))
        self.assertEqual(status, 1)
        self.assertIn("err.001b.io", err)


if __name__ == "__main__":
    unittest.main()
