#!/usr/bin/env python3
"""Run the TEST-002D 5/30/5 checker regression gate."""
import argparse, importlib.util, json, statistics, sys, time
from pathlib import Path

from cpu_benchmark_statistics import paired_median_ratio_ppm

BASELINE_SCHEMA = "seen-test-leak-soak-v1"
BASELINE_TARGET = "linux-x86_64"
BASELINE_PROVIDERS = (
    "allocations", "async-io-completions", "async-io-requests",
    "child-processes", "committed-vram-bytes", "file-descriptors",
    "gpu-objects", "mapped-windows", "persistent-tasks",
    "persistent-workers", "resident-vram-bytes", "staging-bytes", "threads",
)
BASELINE_FIELDS = {"completed_iterations", "planned_iterations", "providers", "schema", "target"}
BASELINE_READING_FIELDS = {"acquired", "available", "baseline", "final", "peak", "provider", "released"}


def baseline_pairs(items):
    value = {}
    for key, item in items:
        if key in value:
            raise ValueError("baseline duplicate field")
        value[key] = item
    return value


def baseline_integer(value):
    return isinstance(value, int) and not isinstance(value, bool)


def baseline_validate(raw):
    """Frozen accepted happy-path checker used as the performance control."""
    value = json.loads(raw.decode(), object_pairs_hook=baseline_pairs)
    if not isinstance(value, dict) or set(value) != BASELINE_FIELDS:
        raise ValueError("baseline fields changed")
    if value["schema"] != BASELINE_SCHEMA or value["target"] != BASELINE_TARGET:
        raise ValueError("baseline identity changed")
    planned = value["planned_iterations"]
    completed = value["completed_iterations"]
    if (not baseline_integer(planned) or not baseline_integer(completed) or
            not 10_000 <= planned <= 1_000_000 or completed != planned):
        raise ValueError("baseline iterations changed")
    providers = value["providers"]
    if not isinstance(providers, list) or len(providers) != len(BASELINE_PROVIDERS):
        raise ValueError("baseline providers changed")
    available = 0
    for reading, expected in zip(providers, BASELINE_PROVIDERS):
        if not isinstance(reading, dict) or set(reading) != BASELINE_READING_FIELDS:
            raise ValueError("baseline reading fields changed")
        if reading["provider"] != expected or not isinstance(reading["available"], bool):
            raise ValueError("baseline provider changed")
        metrics = ("baseline", "peak", "final", "acquired", "released")
        if any(not baseline_integer(reading[key]) for key in metrics):
            raise ValueError("baseline metric type changed")
        if any(not 0 <= reading[key] <= 1_000_000_000_000_000 for key in metrics):
            raise ValueError("baseline metric bound changed")
        if not reading["available"]:
            if any(reading[key] != 0 for key in metrics):
                raise ValueError("baseline unavailable provider changed")
            continue
        available += 1
        if (reading["peak"] < reading["baseline"] or
                reading["peak"] < reading["final"] or
                (reading["acquired"] > 0 and reading["peak"] <= reading["baseline"]) or
                reading["baseline"] + reading["acquired"] != reading["final"] + reading["released"] or
                reading["final"] != reading["baseline"]):
            raise ValueError("baseline lifecycle changed")
    if available < 1:
        raise ValueError("baseline availability changed")
    committed, resident = providers[4], providers[10]
    if committed["available"] != resident["available"]:
        raise ValueError("baseline VRAM availability changed")
    if committed["available"] and any(
            resident[key] > committed[key] for key in ("baseline", "peak", "final")):
        raise ValueError("baseline VRAM bounds changed")
    return value


def benchmark_clock_ns():
    """Measure benchmark work without charging runner descheduling pauses."""
    return time.thread_time_ns()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path); parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        spec = importlib.util.spec_from_file_location("checker", root / "scripts/check_leak_soak_evidence.py")
        checker = importlib.util.module_from_spec(spec); spec.loader.exec_module(checker)
        raw = args.evidence.read_bytes(); expected = checker.validate(raw)
        if baseline_validate(raw) != expected:
            raise ValueError("frozen performance control disagrees with checker")
        baseline = json.loads(args.baseline.read_bytes())
        required = {"baseline_ratio_ppm", "iterations_per_sample", "max_regression_percent", "samples", "version", "warmups"}
        if set(baseline) != required or baseline["version"] != 1 or baseline["warmups"] != 5 or baseline["samples"] != 30 or baseline["max_regression_percent"] != 5:
            raise ValueError("invalid 5/30/5 baseline")
        iterations = baseline["iterations_per_sample"]
        if not isinstance(iterations, int) or not 1 <= iterations <= 10000:
            raise ValueError("invalid iteration count")
        def candidate():
            start = benchmark_clock_ns()
            for _ in range(iterations):
                if checker.validate(raw) != expected: raise ValueError("validation changed")
            return benchmark_clock_ns() - start
        def reference():
            start = benchmark_clock_ns()
            for _ in range(iterations):
                if baseline_validate(raw) != expected: raise ValueError("control changed")
            return benchmark_clock_ns() - start
        for _ in range(5): candidate(); reference()
        candidates, references = [], []
        for index in range(30):
            if index % 2: candidates.append(candidate()); references.append(reference())
            else: references.append(reference()); candidates.append(candidate())
        # Each candidate is adjacent to its control sample, with order
        # alternating to cancel first-run bias. Thread CPU time excludes hosted
        # runner descheduling pauses; preserving adjacent pairs then controls
        # for frequency changes without combining unrelated sample medians.
        ratio = paired_median_ratio_ppm(candidates, references)
        ceiling = baseline["baseline_ratio_ppm"] * 105 // 100
        if ratio > ceiling: raise ValueError(f"ratio {ratio} exceeds 5% ceiling {ceiling}")
        print(f"leak-soak benchmark: ratio_ppm={ratio} ceiling_ratio_ppm={ceiling} clock=thread_cpu warmups=5 samples=30 status=pass")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"leak-soak benchmark: {error}", file=sys.stderr); return 1
    return 0


if __name__ == "__main__": sys.exit(main())
