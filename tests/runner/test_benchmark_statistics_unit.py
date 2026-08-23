#!/usr/bin/env python3

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from cpu_benchmark_statistics import paired_median_ratio_ppm


class Tests(unittest.TestCase):
    def test_preserves_adjacent_pairs(self):
        self.assertEqual(
            paired_median_ratio_ppm([360, 720, 380, 760], [100, 200, 100, 200]),
            3_700_000,
        )

    def test_rejects_invalid_samples(self):
        for candidates, controls in (
            ([], []),
            ([1], []),
            ([0], [1]),
            ([1], [0]),
            ([True], [1]),
            ([1.0], [1]),
        ):
            with self.assertRaises(ValueError):
                paired_median_ratio_ppm(candidates, controls)


if __name__ == "__main__":
    unittest.main()
