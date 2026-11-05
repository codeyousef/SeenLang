#!/usr/bin/env python3
"""Rejection contracts only; no synthetic fixture can qualify ARM execution."""
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import linux_arm64_milestone_report as report


class Tests(unittest.TestCase):
    @unittest.skipIf(platform.system() == "Linux" and platform.machine() in {"aarch64", "arm64"},
                     "this negative case requires a genuinely non-ARM host")
    def test_non_native_host_never_creates_report(self):
        cache = ROOT / ".seen/linux-arm64-executor-tests"
        cache.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=cache) as temporary:
            output = Path(temporary) / "report.json"
            result = subprocess.run(["bash", str(ROOT / "scripts/verify_linux_arm64_milestone.sh"),
                                     "--candidate", str(Path(temporary) / "absent-candidate"),
                                     "--archive", str(Path(temporary) / "absent-archive"),
                                     "--report", str(output)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 126)
            self.assertFalse(output.exists())

    def test_incomplete_success_claim_rejected(self):
        with self.assertRaises(ValueError):
            report.validate_completion({"suite_status": 0, "production_certified": False})

    def test_missing_samples_rejected(self):
        with self.assertRaises(ValueError):
            report.metrics("BENCH: candidate_ns=12\nBENCH: scalar_ns=12\n")

    def test_scalar_or_slow_samples_cannot_qualify(self):
        for variant, candidate, ratio in ((0, 100, 1000000), (1, 106, 1060000)):
            with self.subTest(variant=variant, ratio=ratio):
                telemetry = (f"BENCH: candidate_ns={candidate}\nBENCH: scalar_ns=100\n" * 30 +
                             f"BENCH: selected_variant={variant}\nBENCH: median_ratio_ppm={ratio}\n" +
                             "BENCH: allocator_peak_bytes=100\n")
                with self.assertRaises(ValueError):
                    report.metrics(telemetry)

    def test_unbounded_or_oversized_scope_rejected(self):
        for memory, swap, tasks in (("max", "0", "24"), ("1025", "0", "24"),
                                    ("1024", "1", "24"), ("1024", "0", "25")):
            with self.subTest(memory=memory, swap=swap, tasks=tasks):
                controls = {"memory.max": memory, "memory.swap.max": swap,
                            "memory.oom.group": "1", "pids.max": tasks}
                with self.assertRaises(ValueError):
                    report.validate_scope(controls, 1024, 24)


class EvidenceReadTests(unittest.TestCase):
    def setUp(self):
        cache = ROOT / ".seen/linux-arm64-executor-tests"
        cache.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.document = {"logs": {}, "asan_evidence": {}}
        for name in {"suite.log"} | {f"{name}-{profile}.log" for name, profile in report.CASES}:
            self.put(name, b"retained suite evidence\n", "logs")
        self.put(report.ASAN_PREFIX + "insufficient.out", b"", "asan_evidence")

    def put(self, name, data, category):
        (self.directory / name).write_bytes(data)
        self.document[category][name] = report.identity(data)

    def test_empty_before_main_control_output_is_retained_but_empty_suite_is_rejected(self):
        payload = report.read_evidence(self.directory, self.document)
        self.assertEqual(payload[report.ASAN_PREFIX + "insufficient.out"], b"")
        self.put("suite.log", b"", "logs")
        with self.assertRaises(report.io.ArtifactError):
            report.read_evidence(self.directory, self.document)

    def test_changed_bytes_and_sanitizer_alias_of_suite_log_are_rejected(self):
        (self.directory / "suite.log").write_bytes(b"substituted suite evidence\n")
        with self.assertRaises(ValueError):
            report.read_evidence(self.directory, self.document)
        self.put("suite.log", b"retained suite evidence\n", "logs")
        self.document["asan_evidence"]["suite.log"] = self.document["logs"]["suite.log"]
        with self.assertRaises(ValueError):
            report.read_evidence(self.directory, self.document)

    def test_cumulative_sanitizer_transport_bound_is_enforced_before_overread(self):
        block = b"x" * (11 * 1024 * 1024)
        for index in range(3):
            self.put(report.ASAN_PREFIX + f"bounded-{index}.out", block, "asan_evidence")
        with self.assertRaises(report.io.ArtifactError):
            report.read_evidence(self.directory, self.document)


if __name__ == "__main__":
    unittest.main()
