#!/usr/bin/env python3
"""Run the host-normalized pinned seen-import-graph-v1 microbenchmark."""

from __future__ import annotations

import argparse
import importlib.util
import json
import statistics
import sys
import time
from pathlib import Path

from cpu_benchmark_statistics import paired_median_ratio_ppm


def fail(message: str) -> None:
    raise ValueError(message)


def load_checker(root: Path) -> object:
    spec = importlib.util.spec_from_file_location(
        "check_import_graph", root / "scripts/check_import_graph.py"
    )
    if spec is None or spec.loader is None:
        fail("could not load import-graph checker")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("graph", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        checker = load_checker(root)
        raw = args.graph.read_bytes()
        expected = checker.parse_and_resolve(raw, 1_048_576, 4096, 65_536, 4096)
        baseline = json.loads(args.baseline.read_bytes())
        fields = {
            "baseline_ratio_ppm",
            "iterations_per_sample",
            "max_regression_percent",
            "samples",
            "version",
            "warmups",
        }
        if not isinstance(baseline, dict) or set(baseline) != fields:
            fail("baseline has missing or unknown fields")
        if (
            baseline["version"] != 1
            or baseline["warmups"] != 5
            or baseline["samples"] != 30
            or baseline["max_regression_percent"] != 5
        ):
            fail("benchmark policy must be version 1, 5 warmups, 30 samples, 5 percent")
        iterations = baseline["iterations_per_sample"]
        baseline_ratio = baseline["baseline_ratio_ppm"]
        if (
            isinstance(iterations, bool)
            or not isinstance(iterations, int)
            or not 1 <= iterations <= 100_000
            or isinstance(baseline_ratio, bool)
            or not isinstance(baseline_ratio, int)
            or not 1 <= baseline_ratio <= 100_000_000
        ):
            fail("benchmark iteration or ratio bound is invalid")

        def candidate_sample() -> int:
            started = time.thread_time_ns()
            for _ in range(iterations):
                if checker.parse_and_resolve(raw, 1_048_576, 4096, 65_536, 4096) != expected:
                    fail("import-graph resolution changed")
            return time.thread_time_ns() - started

        document = json.loads(raw)

        def control_sample() -> int:
            started = time.thread_time_ns()
            for _ in range(iterations):
                parsed = json.loads(raw)
                if parsed != document:
                    fail("control parse changed")
            return time.thread_time_ns() - started

        for _ in range(5):
            candidate_sample()
            control_sample()
        candidates: list[int] = []
        controls: list[int] = []
        for sample_index in range(30):
            if sample_index % 2 == 0:
                controls.append(control_sample())
                candidates.append(candidate_sample())
            else:
                candidates.append(candidate_sample())
                controls.append(control_sample())
        candidate_median = int(statistics.median(candidates))
        control_median = int(statistics.median(controls))
        if control_median < 1:
            fail("control median is not positive")
        ratio = paired_median_ratio_ppm(candidates, controls)
        ceiling = baseline_ratio * 105 // 100
        if ratio > ceiling:
            fail(f"normalized ratio {ratio} ppm exceeds 5% ceiling {ceiling} ppm")
        print(
            "import-graph benchmark: "
            f"candidate_median_ns={candidate_median} "
            f"control_median_ns={control_median} ratio_ppm={ratio} "
            f"ceiling_ratio_ppm={ceiling} warmups=5 samples=30 status=pass"
        )
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"import-graph benchmark: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
