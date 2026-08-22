#!/usr/bin/env python3
"""Run the TEST-002C 5/30/5 checker regression gate."""
import argparse, importlib.util, json, statistics, sys, time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        spec = importlib.util.spec_from_file_location("checker", root / "scripts/check_benchmark_evidence.py")
        checker = importlib.util.module_from_spec(spec); spec.loader.exec_module(checker)
        raw = args.evidence.read_bytes(); expected = checker.validate(raw)
        control = json.loads(raw); baseline = json.loads(args.baseline.read_bytes())
        required = {"baseline_ratio_ppm", "iterations_per_sample", "max_regression_percent", "samples", "version", "warmups"}
        if set(baseline) != required or baseline["version"] != 1 or baseline["warmups"] != 5 or baseline["samples"] != 30 or baseline["max_regression_percent"] != 5:
            raise ValueError("invalid 5/30/5 baseline")
        iterations = baseline["iterations_per_sample"]
        if not isinstance(iterations, int) or not 1 <= iterations <= 10000:
            raise ValueError("invalid iteration count")
        def candidate():
            start = time.perf_counter_ns()
            for _ in range(iterations):
                if checker.validate(raw) != expected:
                    raise ValueError("validation changed")
            return time.perf_counter_ns() - start
        def reference():
            start = time.perf_counter_ns()
            for _ in range(iterations):
                value = json.loads(raw)
                samples = sorted(value["samples_ns"])
                if set(value) != checker.FIELDS or (samples[14] + samples[15]) // 2 != value["median_ns"] or value["cgroup"] != control["cgroup"]:
                    raise ValueError("control changed")
            return time.perf_counter_ns() - start
        for _ in range(5):
            candidate(); reference()
        candidates, references = [], []
        for index in range(30):
            if index % 2:
                candidates.append(candidate()); references.append(reference())
            else:
                references.append(reference()); candidates.append(candidate())
        ratio = statistics.median(candidates) * 1_000_000 // statistics.median(references)
        ceiling = baseline["baseline_ratio_ppm"] * 105 // 100
        if ratio > ceiling:
            raise ValueError(f"ratio {ratio} exceeds 5% ceiling {ceiling}")
        print(f"benchmark-evidence benchmark: ratio_ppm={ratio} ceiling_ratio_ppm={ceiling} warmups=5 samples=30 status=pass")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"benchmark-evidence benchmark: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
