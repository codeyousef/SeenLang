#!/usr/bin/env python3
"""
Performance Regression Detection Script

Checks current benchmark results against baseline to detect performance regressions.
"""

import json
import os
import sys
from pathlib import Path


def check_regression(current_file, baseline_file, threshold=5.0):
    """Check for performance regression"""
    try:
        if not Path(baseline_file).exists():
            print(f"⚠️  No baseline found at {baseline_file}")
            return False
            
        with open(current_file, 'r') as f:
            current = json.load(f)
            
        with open(baseline_file, 'r') as f:
            baseline = json.load(f)
            
        regressions = []
        
        # Check each benchmark for regression
        current_benchmarks = current.get('benchmarks', {})
        baseline_benchmarks = baseline.get('benchmarks', {})
        
        for benchmark in current_benchmarks:
            if benchmark in baseline_benchmarks:
                current_time = current_benchmarks[benchmark].get('mean', 0)
                baseline_time = baseline_benchmarks[benchmark].get('mean', 0)
                
                if baseline_time > 0:
                    regression_pct = ((current_time - baseline_time) / baseline_time) * 100
                    
                    if regression_pct > threshold:
                        regressions.append({
                            'benchmark': benchmark,
                            'regression': regression_pct,
                            'current': current_time,
                            'baseline': baseline_time
                        })
                        
        if regressions:
            print(f"❌ Found {len(regressions)} performance regressions:")
            for reg in regressions:
                print(f"  - {reg['benchmark']}: {reg['regression']:.1f}% slower")
                print(f"    Current: {reg['current']:.3f}s, Baseline: {reg['baseline']:.3f}s")
            
            # Set output for GitHub Actions
            if 'GITHUB_OUTPUT' in os.environ:
                with open(os.environ['GITHUB_OUTPUT'], 'a') as f:
                    f.write(f"regressions_found=true\n")
                    f.write(f"regression_count={len(regressions)}\n")
            return True
        else:
            print("✅ No significant performance regressions detected")
            if 'GITHUB_OUTPUT' in os.environ:
                with open(os.environ['GITHUB_OUTPUT'], 'a') as f:
                    f.write(f"regressions_found=false\n")
                    f.write(f"regression_count=0\n")
            return False
            
    except Exception as e:
        print(f"⚠️  Error checking regression: {e}")
        return False


def main():
    """Main regression checking function"""
    # Check for regressions in each result file
    regression_found = False
    result_files = [
        "results/lexer_validation_results.json",
        "results/memory_overhead_investigation.json", 
        "results/reactive_validation_results.json",
        "results/compilation_speed_results.json"
    ]
    
    threshold = float(os.environ.get('REGRESSION_THRESHOLD', '5.0'))
    
    for result_file in result_files:
        if Path(result_file).exists():
            baseline_file = f"baselines/{Path(result_file).name}"
            print(f"Checking {result_file} against {baseline_file}")
            if check_regression(result_file, baseline_file, threshold):
                regression_found = True
        else:
            print(f"⚠️  Result file not found: {result_file}")
    
    # Exit with error code if regressions found
    sys.exit(1 if regression_found else 0)


if __name__ == "__main__":
    main()