#!/usr/bin/env python3
"""Pinned CORE-REL-003 5/30 hard-5-percent validation benchmark."""
from __future__ import annotations
import argparse, importlib.util, json, statistics, sys, time
from pathlib import Path

from cpu_benchmark_statistics import paired_median_ratio_ppm

def main() -> int:
    p=argparse.ArgumentParser(description=__doc__); p.add_argument("plan",type=Path); p.add_argument("baseline",type=Path); a=p.parse_args()
    try:
        root=Path(__file__).resolve().parents[1]; spec=importlib.util.spec_from_file_location("checker",root/"scripts/check_release_optimization.py")
        if spec is None or spec.loader is None: raise ValueError("checker unavailable")
        checker=importlib.util.module_from_spec(spec); spec.loader.exec_module(checker)
        raw=a.plan.read_bytes(); expected=checker.validate(raw); baseline=json.loads(a.baseline.read_bytes())
        required={"baseline_ratio_ppm","iterations_per_sample","max_regression_percent","samples","version","warmups"}
        if not isinstance(baseline,dict) or set(baseline)!=required or baseline["version"]!=1 or baseline["warmups"]!=5 or baseline["samples"]!=30 or baseline["max_regression_percent"]!=5: raise ValueError("policy must be v1 5/30/5")
        iterations=baseline["iterations_per_sample"]; document=json.loads(raw)
        def candidate() -> int:
            start=time.thread_time_ns()
            for _ in range(iterations):
                if checker.validate(raw)!=expected: raise ValueError("changed")
            return time.thread_time_ns()-start
        def control() -> int:
            start=time.thread_time_ns()
            for _ in range(iterations): json.loads(raw)==document or (_ for _ in ()).throw(ValueError("changed"))
            return time.thread_time_ns()-start
        for _ in range(5): candidate(); control()
        cs=[]; ks=[]
        for i in range(30):
            if i%2: cs.append(candidate()); ks.append(control())
            else: ks.append(control()); cs.append(candidate())
        ratio=paired_median_ratio_ppm(cs, ks); ceiling=baseline["baseline_ratio_ppm"]*105//100
        if ratio>ceiling: raise ValueError(f"ratio {ratio} exceeds 5% ceiling {ceiling}")
        print(f"release-optimization benchmark: ratio_ppm={ratio} warmups=5 samples=30 status=pass")
    except (OSError,ValueError,json.JSONDecodeError) as error:
        print(f"release-optimization benchmark: {error}",file=sys.stderr); return 1
    return 0
if __name__=="__main__": sys.exit(main())
