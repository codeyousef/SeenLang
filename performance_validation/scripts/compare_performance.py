#!/usr/bin/env python3
"""
Compare performance between baseline and current results.
Used in CI/CD to detect performance regressions.
"""

import json
import argparse
import sys
from pathlib import Path
from typing import Dict, List, Tuple
import numpy as np

class PerformanceComparator:
    def __init__(self, threshold: float = 5.0):
        """
        Initialize comparator with regression threshold.
        
        Args:
            threshold: Percentage threshold for regression detection
        """
        self.threshold = threshold
        self.regressions = []
        self.improvements = []
        self.unchanged = []
    
    def load_results(self, path: Path) -> Dict:
        """Load benchmark results from directory."""
        results = {}
        
        # Try to load statistical analysis first
        stats_file = path / 'analysis' / 'statistical_analysis.json'
        if stats_file.exists():
            with open(stats_file, 'r', encoding='utf-8-sig') as f:
                data = json.load(f)
                return data.get('benchmarks', {})
        
        # Fall back to raw data
        raw_dir = path / 'raw_data'
        if raw_dir.exists():
            for json_file in raw_dir.glob('*.json'):
                with open(json_file, 'r', encoding='utf-8-sig') as f:
                    data = json.load(f)
                    benchmark_name = json_file.stem
                    results[benchmark_name] = data
        
        return results
    
    def compare(self, baseline: Dict, current: Dict) -> Dict:
        """Compare baseline and current performance."""
        comparison = {
            'regressions': [],
            'improvements': [],
            'unchanged': [],
            'missing': [],
            'new': []
        }
        
        # Check each benchmark in baseline
        for bench_name, baseline_data in baseline.items():
            if bench_name not in current:
                comparison['missing'].append(bench_name)
                continue
            
            current_data = current[bench_name]
            
            # Compare each language implementation
            if 'languages' in baseline_data and 'languages' in current_data:
                for lang in baseline_data['languages']:
                    if lang not in current_data['languages']:
                        continue
                    
                    baseline_lang = baseline_data['languages'][lang]
                    current_lang = current_data['languages'][lang]
                    
                    # Extract mean times
                    baseline_mean = self._extract_mean(baseline_lang)
                    current_mean = self._extract_mean(current_lang)
                    
                    if baseline_mean and current_mean:
                        change_percent = ((current_mean - baseline_mean) / baseline_mean) * 100
                        
                        result = {
                            'benchmark': bench_name,
                            'language': lang,
                            'baseline': baseline_mean,
                            'current': current_mean,
                            'change_percent': change_percent
                        }
                        
                        if change_percent > self.threshold:
                            comparison['regressions'].append(result)
                        elif change_percent < -self.threshold:
                            comparison['improvements'].append(result)
                        else:
                            comparison['unchanged'].append(result)
        
        # Check for new benchmarks
        for bench_name in current:
            if bench_name not in baseline:
                comparison['new'].append(bench_name)
        
        return comparison
    
    def _extract_mean(self, data: Dict) -> float:
        """Extract mean time from various data formats."""
        if 'summary' in data:
            summary = data['summary']
            if isinstance(summary, dict) and 'mean' in summary:
                return summary['mean']
            elif isinstance(summary, str):
                # Parse string representation
                import re
                match = re.search(r'mean=(?:np\.float64\()?([0-9.e+-]+)', summary)
                if match:
                    return float(match.group(1))
        
        if 'average_time' in data:
            return data['average_time']
        
        if 'times' in data and isinstance(data['times'], list):
            return np.mean(data['times'])
        
        return None
    
    def generate_report(self, comparison: Dict) -> str:
        """Generate markdown report of comparison."""
        report = "# Performance Comparison Report\n\n"
        
        # Summary
        total_regressions = len(comparison['regressions'])
        total_improvements = len(comparison['improvements'])
        
        if total_regressions > 0:
            report += f"⚠️ **{total_regressions} performance regressions detected**\n\n"
        else:
            report += "✅ **No performance regressions detected**\n\n"
        
        if total_improvements > 0:
            report += f"🚀 **{total_improvements} performance improvements found**\n\n"
        
        # Regressions
        if comparison['regressions']:
            report += "## Performance Regressions\n\n"
            report += "| Benchmark | Language | Baseline | Current | Change |\n"
            report += "|-----------|----------|----------|---------|--------|\n"
            
            for reg in sorted(comparison['regressions'], key=lambda x: x['change_percent'], reverse=True):
                report += f"| {reg['benchmark']} | {reg['language']} | "
                report += f"{reg['baseline']:.4f}s | {reg['current']:.4f}s | "
                report += f"**+{reg['change_percent']:.1f}%** ⚠️ |\n"
        
        # Improvements
        if comparison['improvements']:
            report += "\n## Performance Improvements\n\n"
            report += "| Benchmark | Language | Baseline | Current | Change |\n"
            report += "|-----------|----------|----------|---------|--------|\n"
            
            for imp in sorted(comparison['improvements'], key=lambda x: x['change_percent']):
                report += f"| {imp['benchmark']} | {imp['language']} | "
                report += f"{imp['baseline']:.4f}s | {imp['current']:.4f}s | "
                report += f"**{imp['change_percent']:.1f}%** ✅ |\n"
        
        # Unchanged
        if comparison['unchanged']:
            report += f"\n## Unchanged ({len(comparison['unchanged'])} benchmarks)\n\n"
            report += "Performance within ±{self.threshold}% threshold.\n"
        
        # Missing/New
        if comparison['missing']:
            report += f"\n## Missing Benchmarks\n\n"
            report += "The following benchmarks were not found in current results:\n"
            for bench in comparison['missing']:
                report += f"- {bench}\n"
        
        if comparison['new']:
            report += f"\n## New Benchmarks\n\n"
            report += "The following benchmarks are new:\n"
            for bench in comparison['new']:
                report += f"- {bench}\n"
        
        return report

def main():
    parser = argparse.ArgumentParser(description='Compare performance results')
    parser.add_argument('--baseline', required=True, help='Baseline results directory')
    parser.add_argument('--current', required=True, help='Current results directory')
    parser.add_argument('--threshold', type=float, default=5.0, 
                       help='Regression threshold percentage')
    parser.add_argument('--output', required=True, help='Output markdown file')
    
    args = parser.parse_args()
    
    comparator = PerformanceComparator(threshold=args.threshold)
    
    # Load results
    baseline_path = Path(args.baseline)
    current_path = Path(args.current)
    
    if not baseline_path.exists():
        print(f"Error: Baseline path does not exist: {baseline_path}")
        sys.exit(1)
    
    if not current_path.exists():
        print(f"Error: Current path does not exist: {current_path}")
        sys.exit(1)
    
    baseline_results = comparator.load_results(baseline_path)
    current_results = comparator.load_results(current_path)
    
    # Compare
    comparison = comparator.compare(baseline_results, current_results)
    
    # Generate report
    report = comparator.generate_report(comparison)
    
    # Write report
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(report)
    
    print(f"Comparison report written to: {output_path}")
    
    # Exit with error if regressions found
    if comparison['regressions']:
        print(f"ERROR: {len(comparison['regressions'])} performance regressions detected!")
        sys.exit(1)
    else:
        print("SUCCESS: No performance regressions detected")
        sys.exit(0)

if __name__ == '__main__':
    main()