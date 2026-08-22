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
SPEC = importlib.util.spec_from_file_location("checker", ROOT / "scripts/check_benchmark_evidence.py")
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)
FIXTURE = ROOT / "tests/fixtures/test-002c/happy/evidence.json"


def evidence():
    return json.loads(FIXTURE.read_text())


class Tests(unittest.TestCase):
    def reject(self, value, code=None):
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(json.dumps(value).encode())
        if code:
            self.assertEqual(raised.exception.code, code)

    def test_happy_and_primitives(self):
        value = evidence()
        self.assertEqual(checker.validate(json.dumps(value).encode())["median_ns"], 1014)
        self.assertEqual(checker.canonical(value), checker.canonical(value))
        self.assertEqual(checker.pairs([("a", 1)]), {"a": 1})
        with self.assertRaises(checker.ContractError):
            checker.pairs([("a", 1), ("a", 2)])
        for value in (None, "", "a" * 129, "é"):
            self.assertFalse(checker.text(value, 128))
        self.assertTrue(checker.text("safe command", 128))
        for value in (None, "0" * 39, "G" * 40):
            self.assertFalse(checker.commit(value))
        self.assertTrue(checker.commit("0" * 40))
        self.assertTrue(checker.integer(1))
        self.assertFalse(checker.integer(True))

    def test_json_cancel_and_bounds(self):
        raw = FIXTURE.read_bytes()
        for malformed in (b"{", b"\xff", b'{"schema":1,"schema":2}', b"[]"):
            with self.assertRaises(checker.ContractError):
                checker.validate(malformed)
        for maximum in (0, True, checker.MAX_BYTES + 1, len(raw) - 1):
            with self.assertRaises(checker.ContractError):
                checker.validate(raw, maximum)
        with self.assertRaises(checker.ContractError) as raised:
            checker.validate(raw, cancelled=True)
        self.assertEqual(raised.exception.code, "cancelled")

    def test_identity_matrix(self):
        good = evidence()
        cases = {"schema": ["bad"], "target": ["macos"], "name": ["", "é"],
                 "backend": [""], "maturity": ["unknown"], "hardware": [""],
                 "toolchain": [""], "command": [""], "source_commit": ["0" * 39],
                 "baseline_commit": ["G" * 40]}
        for key, bads in cases.items():
            for bad in bads:
                value = copy.deepcopy(good); value[key] = bad; self.reject(value)
        value = copy.deepcopy(good); value["extra"] = 1; self.reject(value)

    def test_cgroup_and_policy_matrix(self):
        good = evidence()
        cases = {"memory_max_bytes": [0, True], "memory_swap_max_bytes": [1],
                 "pids_max": [0, 25], "jobs": [2], "opt_jobs": [2]}
        for key, bads in cases.items():
            for bad in bads:
                value = copy.deepcopy(good); value["cgroup"][key] = bad; self.reject(value)
        for bad in (None, {}, {**good["cgroup"], "extra": 1}):
            value = copy.deepcopy(good); value["cgroup"] = bad; self.reject(value)
        for key, bad in (("warmups", 4), ("max_regression_percent", 6)):
            value = copy.deepcopy(good); value[key] = bad; self.reject(value, "limit")

    def test_samples_metrics_and_regression(self):
        good = evidence()
        for samples in ([1] * 29, [1] * 29 + [0], [2, 1] + list(range(2, 30)), [1] * 29 + [True]):
            value = copy.deepcopy(good); value["samples_ns"] = samples; self.reject(value)
        cases = {"median_ns": 1, "baseline_median_ns": 0,
                 "bandwidth_bytes_per_second": 0, "peak_rss_bytes": 0,
                 "peak_vram_bytes": -1, "transfer_bytes": -1, "fallback_count": -1,
                 "correctness": False, "designated_kernel": 1, "hardware_executed": 1}
        for key, bad in cases.items():
            value = copy.deepcopy(good); value[key] = bad; self.reject(value)
        for key, bad in (("fallback_count", 1), ("hardware_executed", False),
                         ("maturity", "compile-only"), ("baseline_median_ns", 965)):
            value = copy.deepcopy(good); value[key] = bad; self.reject(value, "regression")

    def test_cpu_non_designated_and_fuzz(self):
        for key, bad in (("peak_vram_bytes", 1), ("transfer_bytes", 1), ("hardware_executed", True)):
            value = evidence(); value.update({"backend": "cpu", "designated_kernel": False,
                "hardware_executed": False, "peak_vram_bytes": 0, "transfer_bytes": 0})
            value[key] = bad; self.reject(value, "invalid")
        value = evidence(); value["designated_kernel"] = False; value["baseline_median_ns"] = 1
        self.assertEqual(checker.validate(json.dumps(value).encode())["name"], "matmul-f32")
        cases, rejected = checker.fuzz(FIXTURE.read_bytes(), .002, 1101)
        self.assertGreater(cases, 0); self.assertGreaterEqual(rejected, 0)

    def test_main(self):
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
