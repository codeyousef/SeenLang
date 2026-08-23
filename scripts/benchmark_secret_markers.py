#!/usr/bin/env python3
"""Run the pinned host-normalized P0-SECRET-001 benchmark."""

from __future__ import annotations

import argparse
import importlib.util
import json
import statistics
import sys
import time
from pathlib import Path

from cpu_benchmark_statistics import paired_median_ratio_ppm


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("contract", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        spec = importlib.util.spec_from_file_location("secret_checker", root / "scripts/check_secret_markers.py")
        if spec is None or spec.loader is None:
            raise ValueError("secret marker checker unavailable")
        checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(checker)
        raw = args.contract.read_bytes()
        expected = checker.canonical_bytes(checker.validate(raw))
        control_expected = json.loads(raw)
        baseline = json.loads(args.baseline.read_bytes())
        required = {"baseline_ratio_ppm", "iterations_per_sample", "max_regression_percent", "samples", "version", "warmups"}
        if not isinstance(baseline, dict) or set(baseline) != required:
            raise ValueError("invalid benchmark baseline")
        if baseline["version"] != 1 or baseline["warmups"] != 5 or baseline["samples"] != 30 or baseline["max_regression_percent"] != 5:
            raise ValueError("benchmark policy must be version 1, 5 warmups, 30 samples, 5 percent")
        iterations = baseline["iterations_per_sample"]
        if isinstance(iterations, bool) or not isinstance(iterations, int) or not 1 <= iterations <= 100000:
            raise ValueError("invalid benchmark iterations")

        def candidate() -> int:
            started = time.thread_time_ns()
            for _ in range(iterations):
                if checker.canonical_bytes(checker.validate(raw)) != expected:
                    raise ValueError("secret rendering changed")
            return time.thread_time_ns() - started

        def control() -> int:
            started = time.thread_time_ns()
            for _ in range(iterations):
                if json.loads(raw) != control_expected:
                    raise ValueError("control changed")
            return time.thread_time_ns() - started

        for _ in range(5):
            candidate(); control()
        candidates: list[int] = []
        controls: list[int] = []
        for index in range(30):
            if index % 2:
                candidates.append(candidate()); controls.append(control())
            else:
                controls.append(control()); candidates.append(candidate())
        ratio = paired_median_ratio_ppm(candidates, controls)
        ceiling = baseline["baseline_ratio_ppm"] * 105 // 100
        if ratio > ceiling:
            raise ValueError(f"normalized ratio {ratio} ppm exceeds 5% ceiling {ceiling} ppm")
        print(f"secret-marker benchmark: ratio_ppm={ratio} ceiling_ratio_ppm={ceiling} warmups=5 samples=30 status=pass")
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"secret-marker benchmark: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
