#!/usr/bin/env python3
"""Run the pinned host-normalized production-IR policy microbenchmark."""

from __future__ import annotations

import argparse
import importlib.util
import json
import statistics
import sys
import time
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        spec = importlib.util.spec_from_file_location(
            "check_production_ir_policy", root / "scripts/check_production_ir_policy.py")
        if spec is None or spec.loader is None:
            raise ValueError("could not load production-IR checker")
        checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(checker)
        raw = args.input.read_bytes()
        expected = checker.parse_and_validate(raw)
        baseline = json.loads(args.baseline.read_bytes())
        required = {"baseline_ratio_ppm", "iterations_per_sample",
                    "max_regression_percent", "samples", "version", "warmups"}
        if not isinstance(baseline, dict) or set(baseline) != required:
            raise ValueError("baseline has missing or unknown fields")
        if baseline["version"] != 1 or baseline["warmups"] != 5 or \
                baseline["samples"] != 30 or baseline["max_regression_percent"] != 5:
            raise ValueError("benchmark policy must be version 1, 5 warmups, 30 samples, 5 percent")
        iterations = baseline["iterations_per_sample"]
        ratio_baseline = baseline["baseline_ratio_ppm"]
        if isinstance(iterations, bool) or not isinstance(iterations, int) or not 1 <= iterations <= 100_000:
            raise ValueError("benchmark iteration bound is invalid")
        if isinstance(ratio_baseline, bool) or not isinstance(ratio_baseline, int) or not 1 <= ratio_baseline <= 100_000_000:
            raise ValueError("benchmark ratio bound is invalid")

        document = json.loads(raw)

        def candidate() -> int:
            start = time.thread_time_ns()
            for _ in range(iterations):
                if checker.parse_and_validate(raw) != expected:
                    raise ValueError("production-IR plan changed")
            return time.thread_time_ns() - start

        def control() -> int:
            start = time.thread_time_ns()
            for _ in range(iterations):
                if json.loads(raw) != document:
                    raise ValueError("control parse changed")
            return time.thread_time_ns() - start

        for _ in range(5):
            candidate(); control()
        candidates: list[int] = []
        controls: list[int] = []
        for sample in range(30):
            if sample % 2:
                candidates.append(candidate()); controls.append(control())
            else:
                controls.append(control()); candidates.append(candidate())
        candidate_median = int(statistics.median(candidates))
        control_median = int(statistics.median(controls))
        if control_median < 1:
            raise ValueError("control median is not positive")
        ratio = candidate_median * 1_000_000 // control_median
        ceiling = ratio_baseline * 105 // 100
        if ratio > ceiling:
            raise ValueError(f"normalized ratio {ratio} ppm exceeds 5% ceiling {ceiling} ppm")
        print(
            "production-ir benchmark: "
            f"candidate_median_ns={candidate_median} control_median_ns={control_median} "
            f"ratio_ppm={ratio} ceiling_ratio_ppm={ceiling} "
            "warmups=5 samples=30 status=pass")
    except (OSError, json.JSONDecodeError, ValueError, RuntimeError) as error:
        print(f"production-ir benchmark: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
