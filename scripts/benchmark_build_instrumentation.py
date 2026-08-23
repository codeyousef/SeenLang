#!/usr/bin/env python3
"""Run the pinned host-normalized CORE-REL-002 validation benchmark."""

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
    parser.add_argument("evidence", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        spec = importlib.util.spec_from_file_location(
            "check_build_instrumentation",
            root / "scripts/check_build_instrumentation.py")
        if spec is None or spec.loader is None:
            raise ValueError("could not load instrumentation checker")
        checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(checker)
        raw = args.evidence.read_bytes()
        expected = checker.validate(raw)
        baseline = json.loads(args.baseline.read_bytes())
        required = {"baseline_ratio_ppm", "iterations_per_sample",
                    "max_regression_percent", "samples", "version", "warmups"}
        if not isinstance(baseline, dict) or set(baseline) != required or \
                baseline["version"] != 1 or baseline["warmups"] != 5 or \
                baseline["samples"] != 30 or \
                baseline["max_regression_percent"] != 5:
            raise ValueError("benchmark policy must be v1 with 5/30/5 settings")
        iterations = baseline["iterations_per_sample"]
        ceiling_base = baseline["baseline_ratio_ppm"]
        if isinstance(iterations, bool) or not isinstance(iterations, int) or \
                not 1 <= iterations <= 100_000:
            raise ValueError("invalid iteration bound")

        def candidate() -> int:
            start = time.thread_time_ns()
            for _ in range(iterations):
                if checker.validate(raw) != expected:
                    raise ValueError("validation changed")
            return time.thread_time_ns() - start

        document = json.loads(raw)
        def control() -> int:
            start = time.thread_time_ns()
            for _ in range(iterations):
                if json.loads(raw) != document:
                    raise ValueError("control changed")
            return time.thread_time_ns() - start

        for _ in range(5): candidate(); control()
        candidates: list[int] = []
        controls: list[int] = []
        for sample in range(30):
            if sample % 2: candidates.append(candidate()); controls.append(control())
            else: controls.append(control()); candidates.append(candidate())
        ratio = int(statistics.median(candidates)) * 1_000_000 // \
            int(statistics.median(controls))
        ceiling = ceiling_base * 105 // 100
        if ratio > ceiling:
            raise ValueError(f"ratio {ratio} exceeds 5% ceiling {ceiling}")
        print(f"build-instrumentation benchmark: ratio_ppm={ratio} "
              "warmups=5 samples=30 status=pass")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"build-instrumentation benchmark: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
