#!/usr/bin/env python3
"""Run the host-normalized pinned CORE-002B generate/consume microbenchmark."""

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
        raw = args.manifest.read_bytes()
        expected = (
            json.dumps(checker.parse_and_validate(raw, 1024 * 1024), indent=2,
                       sort_keys=True) + "\n"
        ).encode("utf-8")
        baseline = json.loads(args.baseline.read_bytes())
        expected_fields = {
            "baseline_ratio_ppm",
            "iterations_per_sample",
            "max_regression_percent",
            "samples",
            "version",
            "warmups",
        }
        if not isinstance(baseline, dict) or set(baseline) != expected_fields:
            fail("baseline has missing or unknown fields")
        if (
            baseline["version"] != 2
            or baseline["warmups"] != 5
            or baseline["samples"] != 30
        ):
            fail("benchmark must use version 2 with 5 warmups and 30 samples")
        if baseline["max_regression_percent"] != 5:
            fail("benchmark regression ceiling must be 5 percent")
        iterations = baseline["iterations_per_sample"]
        baseline_ratio_ppm = baseline["baseline_ratio_ppm"]
        if (
            isinstance(iterations, bool)
            or not isinstance(iterations, int)
            or not 1 <= iterations <= 100_000
            or isinstance(baseline_ratio_ppm, bool)
            or not isinstance(baseline_ratio_ppm, int)
            or not 1 <= baseline_ratio_ppm <= 100_000_000
        ):
            fail("benchmark iteration or baseline bounds are invalid")

        def candidate_sample() -> int:
            started = time.thread_time_ns()
            for _ in range(iterations):
                parsed = checker.parse_and_validate(expected, 1024 * 1024)
                generated = (
                    json.dumps(parsed, indent=2, sort_keys=True) + "\n"
                ).encode("utf-8")
                if generated != expected:
                    fail("generate/consume bytes changed")
            return time.thread_time_ns() - started

        def control_sample() -> int:
            started = time.thread_time_ns()
            for _ in range(iterations):
                parsed = json.loads(expected)
                generated = (
                    json.dumps(parsed, indent=2, sort_keys=True) + "\n"
                ).encode("utf-8")
                if generated != expected:
                    fail("control generate/consume bytes changed")
            return time.thread_time_ns() - started

        for _ in range(5):
            candidate_sample()
            control_sample()
        candidate_samples: list[int] = []
        control_samples: list[int] = []
        for sample_index in range(30):
            if sample_index % 2 == 0:
                control_samples.append(control_sample())
                candidate_samples.append(candidate_sample())
            else:
                candidate_samples.append(candidate_sample())
                control_samples.append(control_sample())
        candidate_median_ns = int(statistics.median(candidate_samples))
        control_median_ns = int(statistics.median(control_samples))
        if control_median_ns < 1:
            fail("control median is not positive")
        ratio_ppm = candidate_median_ns * 1_000_000 // control_median_ns
        maximum_ratio_ppm = baseline_ratio_ppm * 105 // 100
        if ratio_ppm > maximum_ratio_ppm:
            fail(
                f"normalized ratio {ratio_ppm} ppm exceeds 5% ceiling "
                f"{maximum_ratio_ppm} ppm"
            )
        print(
            "compatibility-runtime benchmark: "
            f"candidate_median_ns={candidate_median_ns} "
            f"control_median_ns={control_median_ns} ratio_ppm={ratio_ppm} "
            f"ceiling_ratio_ppm={maximum_ratio_ppm} "
            "warmups=5 samples=30 status=pass"
        )
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"compatibility-runtime benchmark: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
