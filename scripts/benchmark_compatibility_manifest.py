#!/usr/bin/env python3
"""Run the pinned CORE-002A compatibility-validator microbenchmark."""

from __future__ import annotations

import argparse
import importlib.util
import json
import statistics
import sys
import time
from pathlib import Path


def fail(message: str) -> None:
    raise ValueError(message)


def load_checker(root: Path) -> object:
    spec = importlib.util.spec_from_file_location(
        "check_compatibility_manifest",
        root / "scripts/check_compatibility_manifest.py",
    )
    if spec is None or spec.loader is None:
        fail("could not load compatibility-manifest checker")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        checker = load_checker(root)
        manifest = checker.validate(json.loads(args.manifest.read_bytes()))
        baseline = json.loads(args.baseline.read_bytes())
        expected_fields = {
            "baseline_median_ns",
            "iterations_per_sample",
            "max_regression_percent",
            "samples",
            "version",
            "warmups",
        }
        if not isinstance(baseline, dict) or set(baseline) != expected_fields:
            fail("baseline has missing or unknown fields")
        if baseline["version"] != 1 or baseline["warmups"] != 5 or baseline["samples"] != 30:
            fail("benchmark must use version 1 with 5 warmups and 30 samples")
        if baseline["max_regression_percent"] != 5:
            fail("benchmark regression ceiling must be 5 percent")
        iterations = baseline["iterations_per_sample"]
        baseline_ns = baseline["baseline_median_ns"]
        if (
            isinstance(iterations, bool)
            or not isinstance(iterations, int)
            or not 1 <= iterations <= 100_000
            or isinstance(baseline_ns, bool)
            or not isinstance(baseline_ns, int)
            or not 1 <= baseline_ns <= 10_000_000_000
        ):
            fail("benchmark iteration or baseline bounds are invalid")

        def sample() -> int:
            started = time.thread_time_ns()
            for _ in range(iterations):
                checker.validate(manifest)
            return time.thread_time_ns() - started

        for _ in range(5):
            sample()
        samples = [sample() for _ in range(30)]
        median_ns = int(statistics.median(samples))
        maximum_ns = baseline_ns * 105 // 100
        if median_ns > maximum_ns:
            fail(
                f"median {median_ns} ns exceeds 5% ceiling {maximum_ns} ns"
            )
        print(
            f"compatibility-manifest benchmark: median_ns={median_ns} "
            f"ceiling_ns={maximum_ns} warmups=5 samples=30 status=pass"
        )
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"compatibility-manifest benchmark: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
