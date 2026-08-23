#!/usr/bin/env python3
"""Run the CORE-004B pinned 5/30/5 manifest-validation gate."""
import argparse, importlib.util, json, statistics, sys, time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path); parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        spec = importlib.util.spec_from_file_location("checker", root / "scripts/check_release_artifact_manifest.py")
        checker = importlib.util.module_from_spec(spec); spec.loader.exec_module(checker)
        raw = args.manifest.read_bytes(); expected = checker.parse(raw); control = json.loads(raw)
        baseline = json.loads(args.baseline.read_bytes())
        required = {"baseline_ratio_ppm", "iterations_per_sample", "max_regression_percent", "samples", "version", "warmups"}
        if set(baseline) != required or baseline["version"] != 1 or baseline["warmups"] != 5 or baseline["samples"] != 30 or baseline["max_regression_percent"] != 5:
            raise ValueError("invalid 5/30/5 baseline")
        iterations = baseline["iterations_per_sample"]
        if not isinstance(iterations, int) or not 1 <= iterations <= 10000: raise ValueError("invalid iteration count")
        def candidate():
            start = time.thread_time_ns()
            for _ in range(iterations):
                if checker.parse(raw) != expected: raise ValueError("validation changed")
            return time.thread_time_ns() - start
        def reference():
            start = time.thread_time_ns()
            for _ in range(iterations):
                value = json.loads(raw); artifacts = value["artifacts"]
                if len(artifacts) != 4 or [item["role"] for item in artifacts] != list(checker.ROLES): raise ValueError("control changed")
                if any(len(item["sha256"]) != 64 for item in artifacts) or value["source_digest"] != control["source_digest"]: raise ValueError("control changed")
            return time.thread_time_ns() - start
        for _ in range(5): candidate(); reference()
        candidates, references = [], []
        for index in range(30):
            if index % 2: candidates.append(candidate()); references.append(reference())
            else: references.append(reference()); candidates.append(candidate())
        ratio = statistics.median(candidates) * 1_000_000 // statistics.median(references)
        ceiling = baseline["baseline_ratio_ppm"] * 105 // 100
        if ratio > ceiling: raise ValueError(f"ratio {ratio} exceeds 5% ceiling {ceiling}")
        print(f"release-artifact manifest benchmark: ratio_ppm={ratio} ceiling_ratio_ppm={ceiling} warmups=5 samples=30 status=pass")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"release-artifact manifest benchmark: {error}", file=sys.stderr); return 1
    return 0


if __name__ == "__main__": sys.exit(main())
