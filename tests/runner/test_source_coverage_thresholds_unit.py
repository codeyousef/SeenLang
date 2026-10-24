#!/usr/bin/env python3
import json
import sys
import tempfile
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import check_source_coverage_thresholds as checker


class Tests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.previous_root = checker.ROOT
        checker.ROOT = Path(self.directory.name)
        self.report = checker.ROOT / "coverage.json"
        for name in ("a.seen", "b.seen"):
            (checker.ROOT / name).write_text("fun example() r: Int { return 0 }\n")

    def tearDown(self):
        checker.ROOT = self.previous_root
        self.directory.cleanup()

    def write(self, counts):
        files = [{"filename": name, "summary": {
            "lines": {"count": lines, "covered": hit_lines},
            "branches": {"count": branches, "covered": hit_branches}}}
            for name, lines, hit_lines, branches, hit_branches in counts]
        self.report.write_text(json.dumps({"type": "llvm.coverage.json.export",
                                           "data": [{"files": files}]}))

    def test_aggregate_scope_and_hashes(self):
        self.write([("a.seen", 100, 100, 100, 90), ("b.seen", 100, 80, 100, 70)])
        result = checker.evaluate(self.report, ["a.seen", "b.seen"], 90, 80)
        self.assertTrue(result["passed"])
        self.assertEqual(result["totals"]["lines"]["covered"], 180)
        self.assertTrue(all(len(item["sha256"]) == 64 for item in result["sources"]))

    def test_rounding_cannot_pass_the_gate(self):
        self.write([("a.seen", 20000, 17999, 100, 80)])
        self.assertFalse(checker.evaluate(self.report, ["a.seen"], 90, 80)["passed"])

    def test_empty_branch_evidence_fails(self):
        self.write([("a.seen", 100, 100, 0, 0)])
        self.assertFalse(checker.evaluate(self.report, ["a.seen"], 90, 80)["passed"])

    def test_scope_must_be_explicit_and_unambiguous(self):
        self.write([("a.seen", 100, 100, 100, 100)])
        for sources in ([], ["a.seen", "a.seen"], ["../a.seen"], ["/a.seen"], ["b.seen"]):
            with self.subTest(sources=sources), self.assertRaises(ValueError):
                checker.evaluate(self.report, sources, 90, 80)
        self.write([("a.seen", 100, 100, 100, 100), ("other/a.seen", 100, 100, 100, 100)])
        with self.assertRaises(ValueError): checker.evaluate(self.report, ["a.seen"], 90, 80)

    def test_corrupt_branch_counts_fail(self):
        for total, covered in ((1, 2), (1, -1), (True, 1), (1, True)):
            self.write([("a.seen", 100, 100, total, covered)])
            with self.subTest(total=total, covered=covered), self.assertRaises(ValueError):
                checker.evaluate(self.report, ["a.seen"], 90, 80)


if __name__ == "__main__":
    unittest.main()
