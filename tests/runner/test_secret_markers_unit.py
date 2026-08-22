#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("checker", ROOT / "scripts/check_secret_markers.py")
assert SPEC and SPEC.loader
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


class SecretMarkerContractTests(unittest.TestCase):
    def raw(self, fields: list[dict[str, object]], maximum: int = 64) -> bytes:
        return json.dumps({"fields": fields, "max_secret_bytes": maximum, "schema": checker.SCHEMA}).encode()

    def field(self, kind: str = "secret", name: str = "api-token", value: object = "hidden") -> dict[str, object]:
        return {"kind": kind, "name": name, "value": value}

    def test_secret_is_redacted_and_public_value_survives(self) -> None:
        raw = self.raw([self.field(value="plaintext"), self.field("public", "region", "west")])
        rendered = checker.canonical_bytes(checker.validate(raw))
        self.assertNotIn(b"plaintext", rendered)
        self.assertIn(b'"value":"[redacted]"', rendered)
        self.assertIn(b'"value":"west"', rendered)
        self.assertEqual(json.loads(rendered)["redacted"], 1)

    def test_output_is_sorted_by_utf8_identity(self) -> None:
        raw = self.raw([self.field(name="z"), self.field(name="a")])
        fields = json.loads(checker.canonical_bytes(checker.validate(raw)))["fields"]
        self.assertEqual([field["name"] for field in fields], ["a", "z"])

    def test_duplicate_json_fields_are_rejected_without_material(self) -> None:
        raw = b'{"schema":"seen-secret-marker-v1","schema":"wrong","max_secret_bytes":8,"fields":[]}'
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(raw)
        self.assertNotIn("wrong", str(raised.exception))

    def test_duplicate_marker_names_are_rejected(self) -> None:
        raw = self.raw([self.field(value="first-secret"), self.field(value="second-secret")])
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(raw)
        self.assertEqual(raised.exception.code, "invalid")
        self.assertNotIn("first-secret", str(raised.exception))

    def test_limit_and_cancellation_are_fail_closed(self) -> None:
        with self.assertRaises(checker.ContractError) as limited:
            checker.validate(self.raw([self.field(value="12345")], 4))
        self.assertEqual(limited.exception.code, "limit")
        with self.assertRaises(checker.ContractError) as cancelled:
            checker.validate(self.raw([self.field()]), cancelled=True)
        self.assertEqual(cancelled.exception.code, "cancelled")

    def test_validation_rejection_matrix(self) -> None:
        valid = [self.field()]
        cases = [
            (self.raw(valid), 0),
            (b"{", checker.MAX_BYTES),
            (json.dumps({"schema": "wrong", "max_secret_bytes": 8, "fields": valid}).encode(), checker.MAX_BYTES),
            (json.dumps({"schema": checker.SCHEMA, "max_secret_bytes": True, "fields": valid}).encode(), checker.MAX_BYTES),
            (self.raw([], 8), checker.MAX_BYTES),
            (self.raw([{"kind": "secret"}], 8), checker.MAX_BYTES),
            (self.raw([self.field("private")], 8), checker.MAX_BYTES),
            (self.raw([self.field(name="UPPER")], 8), checker.MAX_BYTES),
            (self.raw([self.field(value=1)], 8), checker.MAX_BYTES),
        ]
        for raw, maximum in cases:
            with self.subTest(raw=raw[:40], maximum=maximum):
                with self.assertRaises(checker.ContractError):
                    checker.validate(raw, maximum)
        self.assertFalse(checker.identity(None))

    def test_fuzz_and_empty_fuzz(self) -> None:
        raw = self.raw([self.field()])
        cases, rejected = checker.fuzz(raw, 0.002, 1101)
        self.assertGreater(cases, 0)
        self.assertGreaterEqual(rejected, 0)
        empty_cases, empty_rejected = checker.fuzz(b"", 0.002, 1101)
        self.assertEqual(empty_cases, empty_rejected)

    def test_cli_exit_codes_and_io_redaction(self) -> None:
        raw = self.raw([self.field(value="sensitive-material")])
        with tempfile.TemporaryDirectory() as directory:
            contract = Path(directory) / "contract.json"
            contract.write_bytes(raw)
            self.assertEqual(checker.main(["--validate", str(contract)]), 0)
            self.assertEqual(checker.main(["--validate", str(contract), "--test-cancel-after-read"]), 130)
            self.assertEqual(checker.main(["--validate", str(contract), "--fuzz-seconds", "301"]), 1)
            missing = Path(directory) / "secret-name-must-not-appear.json"
            self.assertEqual(checker.main(["--validate", str(missing)]), 1)


if __name__ == "__main__":
    unittest.main()
