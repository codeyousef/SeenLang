#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("checker", ROOT / "scripts/check_owned_resources.py")
assert SPEC and SPEC.loader
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


class OwnedResourceContractTests(unittest.TestCase):
    def raw(self, operations: list[dict[str, str]], maximum: int = 4) -> bytes:
        return json.dumps({"max_resources": maximum, "operations": operations, "schema": checker.SCHEMA}).encode()

    def operation(self, kind: str, owner: str = "owner-a", resource: str = "resource-1", target: str = "") -> dict[str, str]:
        return {"kind": kind, "owner": owner, "resource": resource, "target": target}

    def test_happy_move_and_release(self) -> None:
        raw = self.raw([
            self.operation("acquire"),
            self.operation("move", target="owner-b"),
            self.operation("use", owner="owner-b"),
            self.operation("release", owner="owner-b"),
        ])
        result = checker.evaluate(checker.validate(raw))
        self.assertEqual((result["moves"], result["released"], result["active"]), (1, 1, 0))

    def test_old_owner_is_rejected(self) -> None:
        raw = self.raw([self.operation("acquire"), self.operation("move", target="owner-b"), self.operation("use")])
        with self.assertRaisesRegex(checker.ContractError, "no longer owns") as raised:
            checker.evaluate(checker.validate(raw))
        self.assertEqual(raised.exception.code, "use-after-move")

    def test_duplicate_acquire_is_rejected(self) -> None:
        raw = self.raw([self.operation("acquire"), self.operation("acquire")])
        with self.assertRaises(checker.ContractError):
            checker.evaluate(checker.validate(raw))

    def test_leak_is_rejected(self) -> None:
        raw = self.raw([self.operation("acquire")])
        with self.assertRaisesRegex(checker.ContractError, "remain") as raised:
            checker.evaluate(checker.validate(raw))
        self.assertEqual(raised.exception.code, "leak")

    def test_resource_bound_is_enforced(self) -> None:
        raw = self.raw([self.operation("acquire"), self.operation("acquire", resource="resource-2")], maximum=1)
        with self.assertRaises(checker.ContractError) as raised:
            checker.evaluate(checker.validate(raw))
        self.assertEqual(raised.exception.code, "limit")

    def test_duplicate_fields_are_rejected(self) -> None:
        raw = b'{"schema":"seen-owned-resource-v1","schema":"seen-owned-resource-v1","max_resources":1,"operations":[]}'
        with self.assertRaises(checker.ContractError):
            checker.validate(raw)

    def test_cancellation_is_fail_closed(self) -> None:
        raw = self.raw([self.operation("acquire")])
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(raw, cancelled=True)
        self.assertEqual(raised.exception.code, "cancelled")

    def test_validation_rejection_matrix(self) -> None:
        valid = [self.operation("acquire"), self.operation("release")]
        cases = [
            (self.raw(valid), 0),
            (b"{", checker.MAX_BYTES),
            (json.dumps({"schema": "wrong", "max_resources": 1, "operations": valid}).encode(), checker.MAX_BYTES),
            (json.dumps({"schema": checker.SCHEMA, "max_resources": True, "operations": valid}).encode(), checker.MAX_BYTES),
            (self.raw([]), checker.MAX_BYTES),
            (self.raw([{"kind": "acquire"}]), checker.MAX_BYTES),
            (self.raw([self.operation("unknown")]), checker.MAX_BYTES),
            (self.raw([self.operation("acquire", owner="OWNER")]), checker.MAX_BYTES),
            (self.raw([self.operation("move")]), checker.MAX_BYTES),
            (self.raw([self.operation("use", target="owner-b")]), checker.MAX_BYTES),
        ]
        for raw, maximum in cases:
            with self.subTest(raw=raw[:40], maximum=maximum):
                with self.assertRaises(checker.ContractError):
                    checker.validate(raw, maximum)
        self.assertFalse(checker.identity(None))
        self.assertTrue(checker.identity("", empty=True))

    def test_inactive_resource_and_canonical_output(self) -> None:
        released = self.raw([self.operation("acquire"), self.operation("release"), self.operation("release")])
        with self.assertRaisesRegex(checker.ContractError, "not active"):
            checker.evaluate(checker.validate(released))
        missing = self.raw([self.operation("use")])
        with self.assertRaises(checker.ContractError):
            checker.evaluate(checker.validate(missing))
        complete = checker.validate(self.raw([self.operation("acquire"), self.operation("release")]))
        self.assertEqual(json.loads(checker.canonical_bytes(complete))["active"], 0)

    def test_fuzz_and_cli_exit_codes(self) -> None:
        raw = self.raw([self.operation("acquire"), self.operation("release")])
        cases, rejected = checker.fuzz(raw, 0.002, 1101)
        self.assertGreater(cases, 0)
        self.assertGreaterEqual(rejected, 0)
        empty_cases, empty_rejected = checker.fuzz(b"", 0.002, 1101)
        self.assertEqual(empty_cases, empty_rejected)
        with tempfile.TemporaryDirectory() as directory:
            valid = Path(directory) / "valid.json"
            valid.write_bytes(raw)
            self.assertEqual(checker.main(["--validate", str(valid)]), 0)
            self.assertEqual(checker.main(["--validate", str(valid), "--test-cancel-after-read"]), 130)
            self.assertEqual(checker.main(["--validate", str(valid), "--fuzz-seconds", "301"]), 1)
            self.assertEqual(checker.main(["--validate", str(Path(directory) / "missing.json")]), 1)


if __name__ == "__main__":
    unittest.main()
