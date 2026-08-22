#!/usr/bin/env python3
import copy
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("checker", ROOT / "scripts/check_leak_soak_evidence.py")
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)
FIXTURE = ROOT / "tests/fixtures/test-002d/happy/evidence.json"


def evidence():
    return json.loads(FIXTURE.read_text())


class Tests(unittest.TestCase):
    def reject(self, value, code=None):
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(json.dumps(value).encode())
        if code:
            self.assertEqual(raised.exception.code, code)

    def test_happy_canonical_and_primitives(self):
        value = evidence()
        self.assertEqual(checker.validate(json.dumps(value).encode())["planned_iterations"], 10000)
        self.assertEqual(checker.canonical(value), checker.canonical(value))
        self.assertEqual(checker.pairs([("a", 1)]), {"a": 1})
        with self.assertRaises(checker.ContractError):
            checker.pairs([("a", 1), ("a", 2)])
        self.assertTrue(checker.integer(1)); self.assertFalse(checker.integer(True))

    def test_json_cancel_and_byte_bounds(self):
        raw = FIXTURE.read_bytes()
        for malformed in (b"{", b"\xff", b'{"schema":1,"schema":2}', b"[]"):
            with self.assertRaises(checker.ContractError): checker.validate(malformed)
        for maximum in (0, True, checker.MAX_BYTES + 1, len(raw) - 1):
            with self.assertRaises(checker.ContractError): checker.validate(raw, maximum)
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(raw, cancelled=True)
        self.assertEqual(raised.exception.code, "cancelled")

    def test_top_level_matrix(self):
        good = evidence()
        for key, bads in {"schema": ["bad"], "target": ["macos"],
                          "planned_iterations": [True, 9999, 1000001],
                          "completed_iterations": [True, 9999]}.items():
            for bad in bads:
                value = copy.deepcopy(good); value[key] = bad; self.reject(value)
        value = copy.deepcopy(good); value["extra"] = 1; self.reject(value)
        for providers in (None, [], good["providers"][:-1]):
            value = copy.deepcopy(good); value["providers"] = providers; self.reject(value, "limit")

    def test_reading_shape_identity_and_types(self):
        good = evidence()
        for bad in (None, {}, {**good["providers"][0], "extra": 1}):
            value = copy.deepcopy(good); value["providers"][0] = bad; self.reject(value)
        value = copy.deepcopy(good); value["providers"][0]["provider"] = "threads"; self.reject(value)
        value = copy.deepcopy(good); value["providers"][0]["available"] = 1; self.reject(value)
        for key in ("baseline", "peak", "final", "acquired", "released"):
            value = copy.deepcopy(good); value["providers"][0][key] = True; self.reject(value)
            value = copy.deepcopy(good); value["providers"][0][key] = -1; self.reject(value, "limit")
            value = copy.deepcopy(good); value["providers"][0][key] = checker.MAX_VALUE + 1; self.reject(value, "limit")

    def test_unsupported_and_lifecycle_matrix(self):
        good = evidence()
        value = copy.deepcopy(good); value["providers"][0].update({"available": False, "baseline": 0, "peak": 0, "final": 0, "acquired": 0, "released": 0})
        self.assertEqual(checker.validate(json.dumps(value).encode())["providers"][0]["available"], False)
        for key in ("baseline", "peak", "final", "acquired", "released"):
            value = copy.deepcopy(good); value["providers"][0].update({"available": False, "baseline": 0, "peak": 0, "final": 0, "acquired": 0, "released": 0}); value["providers"][0][key] = 1; self.reject(value)
        value = copy.deepcopy(good); value["providers"][0]["peak"] = 0; self.reject(value)
        value = copy.deepcopy(good); value["providers"][0]["baseline"] = 2; value["providers"][0]["peak"] = 2; self.reject(value)
        value = copy.deepcopy(good); value["providers"][0]["released"] = 9999; self.reject(value)
        value = copy.deepcopy(good); value["providers"][0]["final"] = 1; value["providers"][0]["released"] = 9999; self.reject(value, "leak")
        value = copy.deepcopy(good)
        for reading in value["providers"]: reading.update({"available": False, "baseline": 0, "peak": 0, "final": 0, "acquired": 0, "released": 0})
        self.reject(value, "invalid")

    def test_vram_matrix(self):
        good = evidence()
        value = copy.deepcopy(good); value["providers"][10].update({"available": False, "baseline": 0, "peak": 0, "final": 0, "acquired": 0, "released": 0}); self.reject(value)
        value = copy.deepcopy(good); value["providers"][10]["peak"] = 8193; self.reject(value)
        value = copy.deepcopy(good)
        for index in (4, 10): value["providers"][index].update({"available": False, "baseline": 0, "peak": 0, "final": 0, "acquired": 0, "released": 0})
        self.assertFalse(checker.validate(json.dumps(value).encode())["providers"][4]["available"])

    def test_fuzz_and_main(self):
        cases, rejected = checker.fuzz(FIXTURE.read_bytes(), .002, 1101)
        self.assertGreater(cases, 0); self.assertGreaterEqual(rejected, 0)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); source = root / "evidence.json"; source.write_bytes(FIXTURE.read_bytes())
            original_stdout, original_stderr = sys.stdout, sys.stderr
            class Output:
                def __init__(self): self.buffer = io.BytesIO()
            try:
                sys.stdout, sys.stderr = Output(), io.StringIO()
                self.assertEqual(checker.main(["--evidence", str(source)]), 0)
                self.assertEqual(checker.main(["--evidence", str(source), "--test-cancel-after-read"]), 130)
                self.assertEqual(checker.main(["--evidence", str(source), "--fuzz-seconds", "301"]), 1)
                self.assertEqual(checker.main(["--evidence", str(root / "missing")]), 1)
                link = root / "link.json"; link.symlink_to(source)
                self.assertEqual(checker.main(["--evidence", str(link)]), 1)
            finally:
                sys.stdout, sys.stderr = original_stdout, original_stderr


if __name__ == "__main__":
    unittest.main()
