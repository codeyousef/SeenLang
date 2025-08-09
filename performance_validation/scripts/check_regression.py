#!/usr/bin/env python3
"""
Check for performance regressions and fail CI if found.
"""

import json
import argparse
import sys
from pathlib import Path
from typing import Dict, List

def check_regression(results_dir: Path, threshold: float = 10.0) -> bool:
    """
    Check if current results show regression.
    
    Returns:
        True if regression detected, False otherwise
    """
    # Load claims validation
    claims_file = results_dir / 'claims_validation.json'
    if claims_file.exists():
        with open(claims_file, 'r', encoding='utf-8-sig') as f:
            claims = json.load(f)
            
            # Check if any critical claims failed
            if 'validations' in claims:
                for validation in claims['validations']:
                    if validation.get('critical', False) and not validation.get('passed', True):
                        print(f"CRITICAL REGRESSION: {validation.get('claim', 'Unknown')}")
                        return True
    
    # Load statistical analysis
    stats_file = results_dir / 'analysis' / 'statistical_analysis.json'
    if not stats_file.exists():
        stats_file = results_dir / 'statistical_analysis.json'
    
    if stats_file.exists():
        with open(stats_file, 'r', encoding='utf-8-sig') as f:
            stats = json.load(f)
            
            # Check Seen performance against competitors
            if 'executive_summary' in stats:
                summary = stats['executive_summary']
                seen_perf = summary.get('seen_performance', {})
                
                total = seen_perf.get('total_benchmarks', 1)
                losses = seen_perf.get('losses', 0)
                
                loss_percentage = (losses / total) * 100 if total > 0 else 0
                
                if loss_percentage > threshold:
                    print(f"REGRESSION: Seen loses {loss_percentage:.1f}% of benchmarks (threshold: {threshold}%)")
                    return True
    
    return False

def main():
    parser = argparse.ArgumentParser(description='Check for performance regressions')
    parser.add_argument('--current', required=True, help='Current results directory')
    parser.add_argument('--threshold', type=float, default=10.0,
                       help='Acceptable loss percentage')
    parser.add_argument('--exit-on-regression', action='store_true',
                       help='Exit with error code if regression detected')
    
    args = parser.parse_args()
    
    current_path = Path(args.current)
    if not current_path.exists():
        print(f"Error: Results directory does not exist: {current_path}")
        sys.exit(1)
    
    has_regression = check_regression(current_path, args.threshold)
    
    if has_regression:
        print("Performance regression detected!")
        if args.exit_on_regression:
            sys.exit(1)
    else:
        print("No performance regression detected")
        sys.exit(0)

if __name__ == '__main__':
    main()