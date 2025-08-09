#!/usr/bin/env python3
"""
Fix report generator to handle all data formats properly
"""

import json
import sys
from pathlib import Path
import re

def fix_statistical_analysis(stats_file):
    """Fix the statistical analysis JSON to have proper structure."""
    with open(stats_file, 'r') as f:
        data = json.load(f)
    
    # Process each benchmark
    for benchmark_name, benchmark_data in data.get('benchmarks', {}).items():
        languages = benchmark_data.get('languages', {})
        
        for lang, lang_data in languages.items():
            if isinstance(lang_data, dict) and 'summary' in lang_data:
                summary = lang_data['summary']
                
                # If summary is a string representation, parse it
                if isinstance(summary, str):
                    parsed_summary = {}
                    
                    # Extract values using regex
                    mean_match = re.search(r'mean=(?:np\.float64\()?([0-9.e+-]+)', summary)
                    std_match = re.search(r'std_dev=(?:np\.float64\()?([0-9.e+-]+)', summary)
                    median_match = re.search(r'median=(?:np\.float64\()?([0-9.e+-]+)', summary)
                    size_match = re.search(r'sample_size=(\d+)', summary)
                    min_match = re.search(r'min_val=(?:np\.float64\()?([0-9.e+-]+)', summary)
                    max_match = re.search(r'max_val=(?:np\.float64\()?([0-9.e+-]+)', summary)
                    
                    if mean_match:
                        parsed_summary['mean'] = float(mean_match.group(1))
                    if std_match:
                        parsed_summary['std_dev'] = float(std_match.group(1))
                    if median_match:
                        parsed_summary['median'] = float(median_match.group(1))
                    if size_match:
                        parsed_summary['sample_size'] = int(size_match.group(1))
                    if min_match:
                        parsed_summary['min'] = float(min_match.group(1))
                    if max_match:
                        parsed_summary['max'] = float(max_match.group(1))
                    
                    # Replace string with parsed dict
                    lang_data['summary'] = parsed_summary
        
        # Also fix comparisons
        comparisons = benchmark_data.get('comparisons', {})
        for comp_name, comp_data in comparisons.items():
            if isinstance(comp_data, str):
                parsed_comp = {}
                
                # Extract comparison values
                p_value_match = re.search(r'p_value=(?:np\.float64\()?([0-9.e+-]+)', comp_data)
                effect_match = re.search(r'effect_size=(?:np\.float64\()?([0-9.e+-]+)', comp_data)
                speedup_match = re.search(r'speedup_ratio=(?:np\.float64\()?([0-9.e+-]+)', comp_data)
                sig_match = re.search(r'is_significant=(?:np\.)?(True|False)', comp_data)
                
                if p_value_match:
                    parsed_comp['p_value'] = float(p_value_match.group(1))
                if effect_match:
                    parsed_comp['effect_size'] = float(effect_match.group(1))
                if speedup_match:
                    parsed_comp['speedup_ratio'] = float(speedup_match.group(1))
                if sig_match:
                    parsed_comp['is_significant'] = sig_match.group(1) == 'True'
                
                # Extract language names
                lang_match = re.search(r"language_a='(\w+)', language_b='(\w+)'", comp_data)
                if lang_match:
                    parsed_comp['language_a'] = lang_match.group(1)
                    parsed_comp['language_b'] = lang_match.group(2)
                
                comparisons[comp_name] = parsed_comp
    
    # Save fixed version
    fixed_file = stats_file.parent / 'statistical_analysis_fixed.json'
    with open(fixed_file, 'w') as f:
        json.dump(data, f, indent=2)
    
    print(f"Fixed statistical analysis saved to: {fixed_file}")
    return fixed_file

def add_reactive_data_to_stats(stats_file, reactive_file):
    """Add reactive benchmark data to statistical analysis."""
    with open(stats_file, 'r') as f:
        stats = json.load(f)
    
    if reactive_file.exists():
        with open(reactive_file, 'r') as f:
            reactive_data = json.load(f)
        
        # Add reactive data to stats
        if 'benchmarks' not in stats:
            stats['benchmarks'] = {}
        
        reactive_bench = stats['benchmarks'].get('reactive_zero_cost', {})
        reactive_bench['languages'] = {}
        
        for lang, lang_data in reactive_data.get('benchmarks', {}).items():
            if isinstance(lang_data, dict):
                results = lang_data.get('results', {})
                reactive_bench['languages'][lang] = {
                    'summary': {
                        'mean': results.get('simple_reactive', 0),
                        'std_dev': 0,  # Not provided in reactive data
                        'sample_size': lang_data.get('iterations', 0),
                        'overhead_percent': results.get('overhead_percent', 0)
                    },
                    'metadata': {
                        'zero_cost': lang_data.get('zero_cost', False),
                        'imperative_time': results.get('imperative', 0),
                        'reactive_time': results.get('simple_reactive', 0)
                    }
                }
        
        stats['benchmarks']['reactive_zero_cost'] = reactive_bench
    
    return stats

def main():
    if len(sys.argv) < 2:
        print("Usage: python fix_report_generator.py <results_directory>")
        sys.exit(1)
    
    results_dir = Path(sys.argv[1])
    stats_file = results_dir / 'statistical_analysis.json'
    reactive_file = results_dir / 'raw_data' / 'reactive' / 'reactive_results.json'
    
    if not stats_file.exists():
        print(f"Statistical analysis file not found: {stats_file}")
        sys.exit(1)
    
    # Fix the statistical analysis file
    fixed_file = fix_statistical_analysis(stats_file)
    
    # Add reactive data if available
    if reactive_file.exists():
        with open(fixed_file, 'r') as f:
            stats = json.load(f)
        
        stats = add_reactive_data_to_stats(Path(fixed_file), reactive_file)
        
        with open(fixed_file, 'w') as f:
            json.dump(stats, f, indent=2)
        
        print(f"Added reactive data to statistical analysis")
    
    # Now regenerate the report with fixed data
    print(f"Run: python scripts/report_generator.py --data-dir {results_dir} --use-fixed")

if __name__ == "__main__":
    main()