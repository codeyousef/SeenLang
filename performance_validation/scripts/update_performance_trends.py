#!/usr/bin/env python3
"""
Performance Trends Tracking Script

Updates the performance trends database with current benchmark results.
"""

import json
import csv
import os
import sys
from pathlib import Path
from datetime import datetime


def extract_key_metrics(results_dir):
    """Extract key performance metrics from benchmark results"""
    results_dir = Path(results_dir)
    
    # Default values
    metrics = {
        'lexer_tokens_per_sec': 0,
        'memory_overhead_pct': 0,
        'reactive_overhead_pct': 0,
        'compilation_time_sec': 0
    }
    
    try:
        # Extract lexer performance
        lexer_file = results_dir / 'lexer_validation_results.json'
        if lexer_file.exists():
            with open(lexer_file) as f:
                data = json.load(f)
                metrics['lexer_tokens_per_sec'] = data.get('benchmarks', {}).get('average_tokens_per_second', 0)
                
        # Extract memory overhead
        memory_file = results_dir / 'memory_overhead_investigation.json'
        if memory_file.exists():
            with open(memory_file) as f:
                data = json.load(f)
                # Get average overhead
                overheads = [v for k, v in data.get('benchmarks', {}).items() 
                           if 'overhead_percent' in k and isinstance(v, (int, float))]
                if overheads:
                    metrics['memory_overhead_pct'] = sum(overheads) / len(overheads)
                    
        # Extract reactive overhead
        reactive_file = results_dir / 'reactive_validation_results.json'
        if reactive_file.exists():
            with open(reactive_file) as f:
                data = json.load(f)
                # Get average reactive overhead
                overheads = [v for k, v in data.get('benchmarks', {}).items() 
                           if 'overhead_percent' in k and isinstance(v, (int, float))]
                if overheads:
                    metrics['reactive_overhead_pct'] = sum(overheads) / len(overheads)
                    
        # Extract compilation time
        compilation_file = results_dir / 'compilation_speed_results.json'
        if compilation_file.exists():
            with open(compilation_file) as f:
                data = json.load(f)
                # Get average Seen compilation time
                times = []
                for project, project_data in data.get('results', {}).items():
                    if 'seen' in project_data and 'mean' in project_data['seen']:
                        times.append(project_data['seen']['mean'])
                if times:
                    metrics['compilation_time_sec'] = sum(times) / len(times)
                    
    except Exception as e:
        print(f"⚠️  Error extracting metrics: {e}")
    
    return metrics


def update_trends_database(metrics, timestamp, commit_sha, db_path):
    """Update the performance trends CSV database"""
    db_path = Path(db_path)
    db_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Create database if it doesn't exist
    if not db_path.exists():
        with open(db_path, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow([
                'timestamp', 'commit', 'lexer_tokens_per_sec', 
                'memory_overhead_pct', 'reactive_overhead_pct', 'compilation_time_sec'
            ])
    
    # Append new data point
    with open(db_path, 'a', newline='') as f:
        writer = csv.writer(f)
        writer.writerow([
            timestamp,
            commit_sha[:8],  # Short commit hash
            metrics['lexer_tokens_per_sec'],
            metrics['memory_overhead_pct'], 
            metrics['reactive_overhead_pct'],
            metrics['compilation_time_sec']
        ])
    
    print(f"✅ Added performance data point: {timestamp}")
    return True


def generate_trends_report(db_path, output_path):
    """Generate a simple trends report"""
    db_path = Path(db_path)
    output_path = Path(output_path)
    
    if not db_path.exists():
        print("⚠️  No trends database found")
        return
    
    try:
        # Read trends data
        with open(db_path, 'r') as f:
            reader = csv.DictReader(f)
            data = list(reader)
        
        if len(data) < 2:
            print("⚠️  Not enough data points for trends analysis")
            return
        
        # Generate simple report
        report = []
        report.append("# Performance Trends Report\n")
        report.append(f"Generated: {datetime.now().isoformat()}\n\n")
        report.append(f"Total data points: {len(data)}\n\n")
        
        # Latest vs previous comparison
        latest = data[-1]
        previous = data[-2] if len(data) > 1 else data[-1]
        
        report.append("## Latest Performance\n")
        report.append(f"- **Commit**: {latest['commit']}\n")
        report.append(f"- **Timestamp**: {latest['timestamp']}\n")
        report.append(f"- **Lexer**: {float(latest['lexer_tokens_per_sec']) / 1_000_000:.1f}M tokens/sec\n")
        report.append(f"- **Memory Overhead**: {float(latest['memory_overhead_pct']):.1f}%\n")
        report.append(f"- **Reactive Overhead**: {float(latest['reactive_overhead_pct']):.1f}%\n")
        report.append(f"- **Compilation Time**: {float(latest['compilation_time_sec']):.3f}s\n\n")
        
        if len(data) > 1:
            report.append("## Change from Previous\n")
            
            def format_change(current, prev, unit=""):
                if prev == 0:
                    return "N/A"
                change = ((current - prev) / prev) * 100
                symbol = "📈" if change > 0 else "📉" if change < 0 else "➡️"
                return f"{symbol} {change:+.1f}%{unit}"
            
            lexer_change = format_change(
                float(latest['lexer_tokens_per_sec']), 
                float(previous['lexer_tokens_per_sec'])
            )
            memory_change = format_change(
                float(latest['memory_overhead_pct']), 
                float(previous['memory_overhead_pct'])
            )
            reactive_change = format_change(
                float(latest['reactive_overhead_pct']), 
                float(previous['reactive_overhead_pct'])
            )
            compilation_change = format_change(
                float(latest['compilation_time_sec']), 
                float(previous['compilation_time_sec'])
            )
            
            report.append(f"- **Lexer**: {lexer_change}\n")
            report.append(f"- **Memory Overhead**: {memory_change}\n")
            report.append(f"- **Reactive Overhead**: {reactive_change}\n")
            report.append(f"- **Compilation Time**: {compilation_change}\n\n")
        
        # Write report
        with open(output_path, 'w') as f:
            f.writelines(report)
        
        print(f"✅ Generated trends report: {output_path}")
        
    except Exception as e:
        print(f"❌ Error generating trends report: {e}")


def main():
    """Main function"""
    if len(sys.argv) < 4:
        print("Usage: python update_performance_trends.py <results_dir> <timestamp> <commit_sha> [db_path]")
        sys.exit(1)
    
    results_dir = sys.argv[1]
    timestamp = sys.argv[2]
    commit_sha = sys.argv[3]
    db_path = sys.argv[4] if len(sys.argv) > 4 else '.github/performance_db/trends.csv'
    
    print(f"Updating performance trends database...")
    print(f"Results directory: {results_dir}")
    print(f"Timestamp: {timestamp}")
    print(f"Commit: {commit_sha[:8]}")
    print(f"Database: {db_path}")
    
    # Extract metrics from results
    metrics = extract_key_metrics(results_dir)
    print(f"Extracted metrics: {metrics}")
    
    # Update database
    if update_trends_database(metrics, timestamp, commit_sha, db_path):
        # Generate trends report
        report_path = Path(db_path).parent / 'trends_report.md'
        generate_trends_report(db_path, report_path)
    
    print("✅ Performance trends update completed")


if __name__ == "__main__":
    main()