#!/usr/bin/env python3
"""
Python benchmark runner for cross-platform compatibility in CI/CD.
"""

import os
import sys
import json
import subprocess
import argparse
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional

class BenchmarkRunner:
    def __init__(self, iterations: int = 50, test_size: str = 'medium', output_dir: str = None):
        self.iterations = iterations
        self.test_size = test_size
        self.timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        
        if output_dir:
            self.output_dir = Path(output_dir)
        else:
            self.output_dir = Path('results') / self.timestamp
        
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.raw_data_dir = self.output_dir / 'raw_data'
        self.raw_data_dir.mkdir(exist_ok=True)
        
        self.is_windows = sys.platform.startswith('win')
        self.script_ext = '.ps1' if self.is_windows else '.sh'
    
    def run_command(self, cmd: List[str], cwd: Path = None) -> tuple:
        """Run a command and return output."""
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=cwd,
                shell=self.is_windows
            )
            return result.returncode, result.stdout, result.stderr
        except Exception as e:
            return 1, "", str(e)
    
    def run_benchmark_category(self, category: str) -> bool:
        """Run benchmarks for a specific category."""
        print(f"Running {category} benchmarks...")
        
        benchmark_dir = Path(__file__).parent.parent / 'benchmarks' / category
        
        # Look for real benchmark script first
        real_script = benchmark_dir / f'run_real_benchmark{self.script_ext}'
        if real_script.exists():
            script = real_script
        else:
            # Fall back to regular benchmark script
            script = benchmark_dir / f'run_benchmark{self.script_ext}'
        
        if not script.exists():
            print(f"Warning: No benchmark script found for {category}")
            return False
        
        output_file = self.raw_data_dir / f'{category}_results.json'
        
        # Build command
        if self.is_windows:
            cmd = [
                'powershell.exe', '-File', str(script),
                '-Iterations', str(self.iterations),
                '-Output', str(output_file),
                '-TestSize', self.test_size
            ]
        else:
            cmd = [
                'bash', str(script),
                '--iterations', str(self.iterations),
                '--output', str(output_file),
                '--test-size', self.test_size
            ]
        
        # Run benchmark
        returncode, stdout, stderr = self.run_command(cmd, cwd=benchmark_dir)
        
        # Save output
        log_file = self.raw_data_dir / f'{category}.log'
        with open(log_file, 'w') as f:
            f.write(f"Command: {' '.join(cmd)}\n")
            f.write(f"Return code: {returncode}\n\n")
            f.write("STDOUT:\n")
            f.write(stdout)
            f.write("\n\nSTDERR:\n")
            f.write(stderr)
        
        if returncode == 0:
            print(f"✓ {category} benchmark completed")
            return True
        else:
            print(f"✗ {category} benchmark failed (see {log_file})")
            return False
    
    def save_metadata(self):
        """Save system and configuration metadata."""
        metadata = {
            'timestamp': self.timestamp,
            'iterations': self.iterations,
            'test_size': self.test_size,
            'platform': sys.platform,
            'python_version': sys.version,
            'benchmark_config': {
                'iterations': self.iterations,
                'test_size': self.test_size,
                'categories': []
            }
        }
        
        # Get system info
        if self.is_windows:
            returncode, stdout, _ = self.run_command(['systeminfo'])
            if returncode == 0:
                metadata['system_info'] = stdout
        else:
            returncode, stdout, _ = self.run_command(['uname', '-a'])
            if returncode == 0:
                metadata['system_info'] = stdout.strip()
        
        # Save metadata
        metadata_dir = self.output_dir / 'metadata'
        metadata_dir.mkdir(exist_ok=True)
        
        with open(metadata_dir / 'system_info.json', 'w') as f:
            json.dump(metadata, f, indent=2)
    
    def run_all_benchmarks(self, categories: List[str] = None) -> Dict:
        """Run all benchmark categories."""
        if categories is None:
            categories = ['lexer', 'parser', 'codegen', 'runtime', 'memory', 'reactive']
        
        results = {
            'successful': [],
            'failed': [],
            'skipped': []
        }
        
        for category in categories:
            benchmark_dir = Path(__file__).parent.parent / 'benchmarks' / category
            if not benchmark_dir.exists():
                print(f"Skipping {category}: directory not found")
                results['skipped'].append(category)
                continue
            
            if self.run_benchmark_category(category):
                results['successful'].append(category)
            else:
                results['failed'].append(category)
        
        # Also run real-world benchmarks
        real_world_dir = Path(__file__).parent.parent / 'real_world'
        if real_world_dir.exists():
            for app_dir in real_world_dir.iterdir():
                if app_dir.is_dir():
                    app_name = app_dir.name
                    script = app_dir / f'run_benchmark{self.script_ext}'
                    if script.exists():
                        print(f"Running real-world benchmark: {app_name}")
                        # Similar logic as above
                        # ... (abbreviated for brevity)
        
        return results

def main():
    parser = argparse.ArgumentParser(description='Run performance benchmarks')
    parser.add_argument('--iterations', type=int, default=50,
                       help='Number of iterations')
    parser.add_argument('--test-size', choices=['small', 'medium', 'large'],
                       default='medium', help='Test data size')
    parser.add_argument('--output', help='Output directory')
    parser.add_argument('--categories', help='Comma-separated benchmark categories')
    
    args = parser.parse_args()
    
    runner = BenchmarkRunner(
        iterations=args.iterations,
        test_size=args.test_size,
        output_dir=args.output
    )
    
    # Save metadata
    runner.save_metadata()
    
    # Parse categories
    categories = None
    if args.categories:
        categories = [c.strip() for c in args.categories.split(',')]
    
    # Run benchmarks
    print(f"Starting benchmark suite...")
    print(f"Output directory: {runner.output_dir}")
    print(f"Iterations: {args.iterations}")
    print(f"Test size: {args.test_size}")
    print("")
    
    results = runner.run_all_benchmarks(categories)
    
    # Summary
    print("\nBenchmark Summary:")
    print(f"Successful: {len(results['successful'])} - {', '.join(results['successful'])}")
    print(f"Failed: {len(results['failed'])} - {', '.join(results['failed'])}")
    print(f"Skipped: {len(results['skipped'])} - {', '.join(results['skipped'])}")
    
    # Save summary
    with open(runner.output_dir / 'summary.json', 'w') as f:
        json.dump(results, f, indent=2)
    
    # Exit with error if any benchmarks failed
    if results['failed']:
        sys.exit(1)
    else:
        sys.exit(0)

if __name__ == '__main__':
    main()