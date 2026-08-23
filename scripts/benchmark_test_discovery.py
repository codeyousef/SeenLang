#!/usr/bin/env python3
"""Pinned TEST-001A 5/30 hard-5-percent discovery benchmark."""

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
    parser.add_argument("manifest", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        spec = importlib.util.spec_from_file_location(
            "discovery", root / "scripts/discover_seen_tests.py")
        if spec is None or spec.loader is None:
            raise ValueError("discovery module unavailable")
        discovery = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(discovery)
        raw = args.manifest.read_bytes()
        expected = discovery.validate(raw)
        baseline = json.loads(args.baseline.read_bytes())
        required = {"baseline_ratio_ppm", "iterations_per_sample",
                    "max_regression_percent", "samples", "version", "warmups"}
        if set(baseline) != required or baseline["version"] != 1 or \
                baseline["warmups"] != 5 or baseline["samples"] != 30 or \
                baseline["max_regression_percent"] != 5:
            raise ValueError("policy must be v1 5/30/5")
        iterations = baseline["iterations_per_sample"]
        document = json.loads(raw)

        def candidate() -> int:
            start = time.thread_time_ns()
            for _ in range(iterations):
                if discovery.validate(raw) != expected:
                    raise ValueError("discovery result changed")
            return time.thread_time_ns() - start

        def control() -> int:
            start = time.thread_time_ns()
            for _ in range(iterations):
                if json.loads(raw) != document:
                    raise ValueError("control result changed")
            return time.thread_time_ns() - start

        for _ in range(5):
            candidate()
            control()
        candidates: list[int] = []
        controls: list[int] = []
        for index in range(30):
            if index % 2:
                candidates.append(candidate())
                controls.append(control())
            else:
                controls.append(control())
                candidates.append(candidate())
        ratio = statistics.median(candidates) * 1_000_000 // statistics.median(controls)
        ceiling = baseline["baseline_ratio_ppm"] * 105 // 100
        if ratio > ceiling:
            raise ValueError(f"ratio {ratio} exceeds 5% ceiling {ceiling}")
        print(f"test-discovery benchmark: ratio_ppm={ratio} warmups=5 samples=30 status=pass")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"test-discovery benchmark: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
