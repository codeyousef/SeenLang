#!/usr/bin/env python3
"""Branch matrix for the CORE-REL-001 machine-diagnostic oracle."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_machine_diagnostic", ROOT / "scripts/check_machine_diagnostic.py")
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def error(**changes: object) -> dict[str, object]:
    value: dict[str, object] = {
        "causes": [], "code": "E_TEST", "message": "failure",
        "native_code": None, "operation": "emit", "redaction": "public",
        "retry": "never", "subsystem": "compiler.test",
    }
    value.update(changes)
    return value


def document(**changes: object) -> bytes:
    value: dict[str, object] = {
        "context": {
            "backend": "cuda", "device_capability": "sm_89",
            "entry_point": "kernel", "fallback_reason": "",
            "maturity": "verified",
            "source": {"column": 2, "file": "src/kernel.seen", "line": 1},
            "target": "linux-x86_64",
        },
        "error": error(),
        "platform": "linux-x86_64",
        "schema": "seen-machine-diagnostic-input-v1",
    }
    value.update(changes)
    return json.dumps(value).encode()


class MachineDiagnosticTests(unittest.TestCase):
    def assert_code(self, code: str, raw: bytes, **limits: int | bool) -> None:
        with self.assertRaises(CHECKER.DiagnosticError) as raised:
            CHECKER.parse_and_validate(raw, **limits)
        self.assertEqual(code, raised.exception.code)

    def test_happy_is_canonical_and_preserves_stable_code(self) -> None:
        rendered = CHECKER.parse_and_validate(document())
        self.assertEqual("seen-machine-diagnostic-v1", rendered["schema"])
        self.assertEqual("E_TEST", rendered["error"]["code"])
        self.assertEqual("verified", rendered["context"]["maturity"])

    def test_limits_cancel_and_platform(self) -> None:
        self.assert_code("limit", document(), max_bytes=0)
        self.assert_code("limit", document(), max_path_bytes=0)
        self.assert_code("limit", document(), max_bytes=1)
        self.assert_code("cancelled", document(), cancelled=True)
        self.assert_code("platform", document(platform="plan9"))
        self.assert_code("limit", document(error=error(message="x" * 4097)))
        self.assert_code(
            "limit", document(error=error(causes=[error() for _ in range(9)])))

    def test_schema_shape_duplicates_bom_and_types(self) -> None:
        self.assert_code("invalid", document(schema="other"))
        self.assert_code("invalid", document(extra=True))
        self.assert_code("invalid", b'{"schema":1,"schema":2}')
        self.assert_code("invalid", b"\xef\xbb\xbf" + document())
        self.assert_code("invalid", b"{")
        self.assert_code("invalid", document(context=[]))
        self.assert_code("invalid", document(error=[]))

    def test_context_maturity_identity_and_location(self) -> None:
        base = json.loads(document())
        for maturity in CHECKER.MATURITY:
            base["context"]["maturity"] = maturity
            CHECKER.parse_and_validate(json.dumps(base).encode())
        base["context"]["maturity"] = "ready"
        self.assert_code("invalid", json.dumps(base).encode())
        base = json.loads(document())
        base["context"]["backend"] = "bad backend"
        self.assert_code("invalid", json.dumps(base).encode())
        for path in ("/root.seen", "../root.seen", "a//b.seen", "a\\b.seen"):
            base = json.loads(document())
            base["context"]["source"]["file"] = path
            self.assert_code("invalid", json.dumps(base).encode())
        base = json.loads(document())
        base["context"]["source"] = {"column": 0, "file": "", "line": 1}
        self.assert_code("invalid", json.dumps(base).encode())

    def test_error_classes_native_code_and_cause_depth(self) -> None:
        self.assert_code("invalid", document(error=error(retry="sometimes")))
        self.assert_code("invalid", document(error=error(redaction="secret")))
        self.assert_code("invalid", document(error=error(native_code=True)))
        nested = error()
        for _ in range(9):
            nested = error(causes=[nested])
        self.assert_code("limit", document(error=nested))

    def test_sensitive_fields_are_redacted(self) -> None:
        raw = json.loads(document())
        raw["error"] = error(message="secret", redaction="sensitive")
        raw["context"]["source"]["file"] = "private/source.seen"
        raw["context"]["fallback_reason"] = "secret fallback"
        rendered = CHECKER.parse_and_validate(json.dumps(raw).encode())
        encoded = json.dumps(rendered)
        self.assertNotIn("secret", encoded)
        self.assertNotIn("private/", encoded)
        self.assertIn("<redacted>", encoded)

    def test_fuzz_is_bounded_and_deterministic(self) -> None:
        CHECKER.fuzz(document(), 0.05, 1101)

    def test_cli_success_fuzz_cancel_and_io(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "diagnostic.json"
            path.write_bytes(document())
            with contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(0, CHECKER.main([str(path)]))
                self.assertEqual(0, CHECKER.main([
                    str(path), "--fuzz-seconds", "0.002", "--seed", "1101"]))
                self.assertEqual(1, CHECKER.main([
                    str(path), "--fuzz-seconds", "301"]))
                self.assertEqual(1, CHECKER.main([
                    str(path), "--test-cancel-after-read"]))
                os.environ["SEEN_MACHINE_DIAGNOSTIC_TEST_HOOKS"] = "1"
                try:
                    self.assertEqual(130, CHECKER.main([
                        str(path), "--test-cancel-after-read"]))
                finally:
                    del os.environ["SEEN_MACHINE_DIAGNOSTIC_TEST_HOOKS"]
                self.assertEqual(1, CHECKER.main([str(path) + ".missing"]))


if __name__ == "__main__":
    unittest.main()
