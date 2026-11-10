#!/usr/bin/env python3
"""Upper-bound admission cases, including the observed non-page-aligned cap."""

import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "scripts/memory_budget.sh"


class Tests(unittest.TestCase):
    def admit(self, limit, page_bytes):
        return subprocess.run(
            ["bash", "-c", 'source "$1"; seen_memory_page_floor_kb "$2" "$3"',
             "memory-budget", str(HELPER), str(limit), str(page_bytes)],
            capture_output=True, text=True, timeout=5,
        )

    def test_observed_kernel_rounding_preserves_the_requested_upper_bound(self):
        result = self.admit(2097151, 4096)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(int(result.stdout), 2097148)

    def test_supported_page_geometries_and_exact_boundaries(self):
        for limit, page_bytes, expected in (
            (4, 4096, 4), (7, 4096, 4), (8, 4096, 8),
            (16, 16384, 16), (31, 16384, 16), (32, 16384, 32),
            (64, 65536, 64), (127, 65536, 64), (128, 65536, 128),
            (9999999999999999, 4096, 9999999999999996),
        ):
            with self.subTest(limit=limit, page_bytes=page_bytes):
                result = self.admit(limit, page_bytes)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(int(result.stdout), expected)
                self.assertLessEqual(int(result.stdout), limit)

    def test_a_budget_smaller_than_one_kernel_page_is_rejected(self):
        for limit, page_bytes in ((1, 4096), (15, 16384), (63, 65536)):
            with self.subTest(limit=limit, page_bytes=page_bytes):
                result = self.admit(limit, page_bytes)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")

    def test_noncanonical_or_overflowing_arithmetic_inputs_are_rejected(self):
        for limit in ("0", "-1", "01", "1+1", "1<<63", "10000000000000000"):
            with self.subTest(limit=limit):
                result = self.admit(limit, 4096)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")
        for page_bytes in ("0", "1023", "4095", "1+1", "1000000000"):
            with self.subTest(page_bytes=page_bytes):
                result = self.admit(2097152, page_bytes)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
