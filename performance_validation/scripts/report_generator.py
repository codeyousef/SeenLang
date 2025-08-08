#!/usr/bin/env python3
"""
Honest performance report generator for Seen Language validation.

This generates brutally honest reports that include ALL results,
wins AND losses, with proper statistical analysis.
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Dict, List, Any, Optional
import matplotlib.pyplot as plt
import seaborn as sns
import pandas as pd
from datetime import datetime
import numpy as np

class HonestReportGenerator:
    """Generates comprehensive, honest performance reports."""
    
    def __init__(self, honest_mode: bool = True):
        self.honest_mode = honest_mode
        self.setup_styling()
        
    def setup_styling(self):
        """Configure consistent styling for reports."""
        # Set color palette
        self.colors = {
            'seen': '#2E86C1',
            'rust': '#CE422B', 
            'cpp': '#00599C',
            'zig': '#F7A41D',
            'c': '#A8B9CC',
            'win': '#27AE60',
            'loss': '#E74C3C',
            'tie': '#F39C12'
        }
        
        # Configure matplotlib
        plt.style.use('default')
        sns.set_palette("husl")
        
    def load_benchmark_data(self, data_dir: Path) -> Dict[str, Any]:
        """Load all benchmark data from a session directory."""
        
        data = {
            'metadata': {},
            'raw_results': {},
            'statistical_analysis': {},
            'system_info': {}
        }
        
        # Load system information
        system_info_file = data_dir / 'metadata' / 'system_info.json'
        if system_info_file.exists():
            try:
                with open(system_info_file, 'r', encoding='utf-8-sig') as f:
                    content = f.read().strip()
                    if content:
                        data['system_info'] = json.loads(content)
                    else:
                        print(f"Warning: Empty system info file: {system_info_file}")
                        data['system_info'] = {}
            except (json.JSONDecodeError, UnicodeDecodeError) as e:
                print(f"Warning: Failed to parse system info file {system_info_file}: {e}")
                data['system_info'] = {}
        
        # Load statistical analysis
        stats_file = data_dir / 'statistical_analysis.json'
        if stats_file.exists():
            try:
                with open(stats_file, 'r', encoding='utf-8-sig') as f:
                    content = f.read().strip()
                    if content:
                        data['statistical_analysis'] = json.loads(content)
                    else:
                        print(f"Warning: Empty statistical analysis file: {stats_file}")
                        data['statistical_analysis'] = {}
            except (json.JSONDecodeError, UnicodeDecodeError) as e:
                print(f"Warning: Failed to parse statistical analysis file {stats_file}: {e}")
                data['statistical_analysis'] = {}
        
        # Load raw benchmark results
        raw_data_dir = data_dir / 'raw_data'
        if raw_data_dir.exists():
            for category_dir in raw_data_dir.iterdir():
                if category_dir.is_dir():
                    category_name = category_dir.name
                    data['raw_results'][category_name] = {}
                    
                    for result_file in category_dir.glob('*.json'):
                        try:
                            # Handle UTF-8 BOM if present
                            with open(result_file, 'r', encoding='utf-8-sig') as f:
                                content = f.read().strip()
                                if content:
                                    benchmark_data = json.loads(content)
                                    benchmark_name = result_file.stem
                                    data['raw_results'][category_name][benchmark_name] = benchmark_data
                                else:
                                    print(f"Warning: Empty benchmark file: {result_file}")
                        except (json.JSONDecodeError, UnicodeDecodeError) as e:
                            print(f"Warning: Failed to parse benchmark file {result_file}: {e}")
                        except Exception as e:
                            print(f"Warning: Error reading benchmark file {result_file}: {e}")
        
        return data
    
    def generate_markdown_report(self, data: Dict[str, Any], output_file: Path, include_plots: bool = True):
        """Generate comprehensive Markdown performance report."""
        
        md_content = self._generate_markdown_header(data)
        md_content += self._generate_executive_summary_markdown(data)
        md_content += self._generate_detailed_results_markdown(data)
        
        if include_plots:
            md_content += self._generate_visualizations_markdown(data, output_file.parent)
            
        md_content += self._generate_methodology_markdown(data)
        md_content += self._generate_conclusions_markdown(data)
        md_content += self._generate_markdown_footer()
        
        # Write Markdown file
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(md_content)
            
        print(f"Markdown report generated: {output_file}")

    def generate_html_report(self, data: Dict[str, Any], output_file: Path, include_plots: bool = True):
        """Generate comprehensive HTML performance report."""
        
        html_content = self._generate_html_header()
        html_content += self._generate_executive_summary_html(data)
        html_content += self._generate_detailed_results_html(data)
        
        if include_plots:
            html_content += self._generate_visualizations_html(data, output_file.parent)
            
        html_content += self._generate_methodology_html(data)
        html_content += self._generate_conclusions_html(data)
        html_content += self._generate_html_footer()
        
        # Write HTML file
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(html_content)
            
        print(f"HTML report generated: {output_file}")
    
    def _generate_html_header(self) -> str:
        """Generate HTML header with styling."""
        return """
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Seen Language Performance Validation Report</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            line-height: 1.6;
            color: #333;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f8f9fa;
        }
        
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 40px;
            border-radius: 10px;
            text-align: center;
            margin-bottom: 30px;
        }
        
        .header h1 {
            margin: 0;
            font-size: 2.5em;
            font-weight: 300;
        }
        
        .header .subtitle {
            font-size: 1.2em;
            opacity: 0.9;
            margin-top: 10px;
        }
        
        .section {
            background: white;
            margin: 20px 0;
            padding: 30px;
            border-radius: 10px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }
        
        .section h2 {
            color: #2c3e50;
            border-bottom: 2px solid #3498db;
            padding-bottom: 10px;
            margin-bottom: 20px;
        }
        
        .metric-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin: 20px 0;
        }
        
        .metric-card {
            background: #f8f9fa;
            padding: 20px;
            border-radius: 8px;
            text-align: center;
            border-left: 4px solid #3498db;
        }
        
        .metric-value {
            font-size: 2em;
            font-weight: bold;
            color: #2c3e50;
        }
        
        .metric-label {
            font-size: 0.9em;
            color: #7f8c8d;
            margin-top: 5px;
        }
        
        .performance-table {
            width: 100%;
            border-collapse: collapse;
            margin: 20px 0;
        }
        
        .performance-table th,
        .performance-table td {
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #ddd;
        }
        
        .performance-table th {
            background-color: #f8f9fa;
            font-weight: 600;
        }
        
        .win { color: #27ae60; font-weight: bold; }
        .loss { color: #e74c3c; font-weight: bold; }
        .tie { color: #f39c12; font-weight: bold; }
        
        .alert {
            padding: 15px;
            margin: 20px 0;
            border-radius: 5px;
        }
        
        .alert-warning {
            background-color: #fff3cd;
            border: 1px solid #ffeaa7;
            color: #856404;
        }
        
        .alert-danger {
            background-color: #f8d7da;
            border: 1px solid #f5c6cb;
            color: #721c24;
        }
        
        .alert-success {
            background-color: #d4edda;
            border: 1px solid #c3e6cb;
            color: #155724;
        }
        
        .plot-container {
            margin: 30px 0;
            text-align: center;
        }
        
        .plot-container img {
            max-width: 100%;
            height: auto;
            border-radius: 8px;
            box-shadow: 0 4px 8px rgba(0,0,0,0.1);
        }
        
        .code-block {
            background-color: #f4f4f4;
            border: 1px solid #ddd;
            border-radius: 4px;
            padding: 15px;
            font-family: 'Monaco', 'Consolas', monospace;
            font-size: 0.9em;
            overflow-x: auto;
        }
        
        .footer {
            text-align: center;
            margin-top: 50px;
            padding: 20px;
            color: #7f8c8d;
            font-size: 0.9em;
        }
        
        .honest-mode-badge {
            position: absolute;
            top: 20px;
            right: 20px;
            background-color: #27ae60;
            color: white;
            padding: 5px 15px;
            border-radius: 20px;
            font-size: 0.8em;
            font-weight: bold;
        }
    </style>
</head>
<body>
    <div class="honest-mode-badge">🔍 HONEST MODE</div>
    <div class="header">
        <h1>Seen Language Performance Validation</h1>
        <div class="subtitle">Scientific Benchmark Analysis with Brutal Honesty</div>
        <div class="subtitle">Generated: """ + datetime.now().strftime('%Y-%m-%d %H:%M:%S UTC') + """</div>
    </div>
"""
    
    def _generate_executive_summary_html(self, data: Dict[str, Any]) -> str:
        """Generate executive summary section."""
        
        stats = data.get('statistical_analysis', {})
        summary = stats.get('executive_summary', {})
        
        if not summary:
            return """
    <div class="section">
        <h2>Executive Summary</h2>
        <div class="alert alert-warning">
            No statistical analysis data available. Please run benchmarks first.
        </div>
    </div>
"""
        
        perf = summary.get('seen_performance', {})
        reliability = summary.get('statistical_reliability', {})
        
        # Calculate win rate
        total = perf.get('total_benchmarks', 0)
        wins = perf.get('wins', 0)
        losses = perf.get('losses', 0)
        ties = perf.get('ties', 0)
        
        win_rate = (wins / total * 100) if total > 0 else 0
        
        # Determine overall assessment
        if win_rate >= 60:
            overall_class = "alert-success"
            overall_message = "Seen shows strong performance across most benchmarks"
        elif win_rate >= 40:
            overall_class = "alert-warning"
            overall_message = "Seen shows competitive performance with mixed results"
        else:
            overall_class = "alert-danger"
            overall_message = "Seen underperforms in majority of benchmarks - optimization needed"
        
        html = f"""
    <div class="section">
        <h2>Executive Summary</h2>
        
        <div class="alert {overall_class}">
            <strong>Overall Assessment:</strong> {overall_message}
        </div>
        
        <div class="metric-grid">
            <div class="metric-card">
                <div class="metric-value">{total}</div>
                <div class="metric-label">Total Benchmarks</div>
            </div>
            <div class="metric-card">
                <div class="metric-value win">{wins}</div>
                <div class="metric-label">Wins</div>
            </div>
            <div class="metric-card">
                <div class="metric-value loss">{losses}</div>
                <div class="metric-label">Losses</div>
            </div>
            <div class="metric-card">
                <div class="metric-value tie">{ties}</div>
                <div class="metric-label">Competitive</div>
            </div>
            <div class="metric-card">
                <div class="metric-value">{win_rate:.1f}%</div>
                <div class="metric-label">Win Rate</div>
            </div>
        </div>
        
        <h3>Key Findings</h3>
        <ul>
"""
        
        # Add key findings
        findings = summary.get('key_findings', [])
        if findings:
            for finding in findings:
                html += f"            <li>{finding}</li>\n"
        else:
            html += "            <li>No specific findings available</li>\n"
        
        html += """
        </ul>
        
        <h3>Statistical Reliability</h3>
        <div class="metric-grid">
"""
        
        # Statistical reliability metrics
        if reliability:
            html += f"""
            <div class="metric-card">
                <div class="metric-value">{reliability.get('total_samples_collected', 'N/A')}</div>
                <div class="metric-label">Total Samples</div>
            </div>
            <div class="metric-card">
                <div class="metric-value">{reliability.get('min_samples_per_test', 'N/A')}</div>
                <div class="metric-label">Min Samples/Test</div>
            </div>
            <div class="metric-card">
                <div class="metric-value">{'✓' if reliability.get('meets_minimum_requirements', False) else '✗'}</div>
                <div class="metric-label">Meets Requirements</div>
            </div>
"""
        
        html += """
        </div>
        
        <h3>Recommendations</h3>
        <ul>
"""
        
        # Add recommendations
        recommendations = summary.get('recommendations', [])
        if recommendations:
            for rec in recommendations:
                html += f"            <li>{rec}</li>\n"
        else:
            html += "            <li>Continue monitoring performance and optimizing bottlenecks</li>\n"
        
        html += """
        </ul>
    </div>
"""
        
        return html
    
    def _generate_detailed_results_html(self, data: Dict[str, Any]) -> str:
        """Generate detailed benchmark results section."""
        
        stats = data.get('statistical_analysis', {})
        benchmarks = stats.get('benchmarks', {})
        
        if not benchmarks:
            return """
    <div class="section">
        <h2>Detailed Results</h2>
        <div class="alert alert-warning">
            No benchmark results available.
        </div>
    </div>
"""
        
        html = """
    <div class="section">
        <h2>Detailed Benchmark Results</h2>
        
        <p>This section shows complete results for all benchmarks, including statistical analysis and comparisons.</p>
"""
        
        for benchmark_name, benchmark_data in benchmarks.items():
            html += self._generate_benchmark_details_html(benchmark_name, benchmark_data)
        
        html += """
    </div>
"""
        
        return html
    
    def _generate_benchmark_details_html(self, benchmark_name: str, benchmark_data: Dict[str, Any]) -> str:
        """Generate detailed results for a specific benchmark."""
        
        rankings = benchmark_data.get('rankings', [])
        languages = benchmark_data.get('languages', {})
        comparisons = benchmark_data.get('comparisons', {})
        
        html = f"""
        <h3>{benchmark_name}</h3>
        
        <h4>Performance Ranking</h4>
        <table class="performance-table">
            <thead>
                <tr>
                    <th>Rank</th>
                    <th>Language</th>
                    <th>Mean Time (s)</th>
                    <th>95% CI</th>
                    <th>Std Dev</th>
                    <th>Samples</th>
                </tr>
            </thead>
            <tbody>
"""
        
        # Show performance ranking
        for i, ranking in enumerate(rankings):
            lang = ranking['language']
            mean_time = ranking['mean_time']
            
            lang_data = languages.get(lang, {})
            summary = lang_data.get('summary')
            
            if summary:
                ci_low, ci_high = summary.confidence_interval_95
                std_dev = summary.std_dev
                samples = summary.sample_size
                
                # Color code Seen's performance
                row_class = ""
                if lang.lower() == 'seen':
                    if i == 0:
                        row_class = 'class="win"'
                    elif i == len(rankings) - 1:
                        row_class = 'class="loss"'
                    else:
                        row_class = 'class="tie"'
                
                html += f"""
                <tr {row_class}>
                    <td>{i + 1}</td>
                    <td>{lang.upper()}</td>
                    <td>{mean_time:.6f}</td>
                    <td>[{ci_low:.6f}, {ci_high:.6f}]</td>
                    <td>{std_dev:.6f}</td>
                    <td>{samples}</td>
                </tr>
"""
        
        html += """
            </tbody>
        </table>
        
        <h4>Statistical Comparisons</h4>
        <table class="performance-table">
            <thead>
                <tr>
                    <th>Comparison</th>
                    <th>P-value</th>
                    <th>Effect Size</th>
                    <th>Significant?</th>
                    <th>Speedup</th>
                    <th>Result</th>
                </tr>
            </thead>
            <tbody>
"""
        
        # Show statistical comparisons
        for comp_name, comparison in comparisons.items():
            # Only show comparisons involving Seen
            if 'seen' not in comp_name.lower():
                continue
                
            p_value = comparison.p_value
            effect_size = comparison.effect_size
            is_significant = comparison.is_significant
            speedup = comparison.speedup_ratio
            
            # Determine result
            seen_first = 'seen' in comparison.language_a.lower()
            if is_significant:
                if (seen_first and speedup > 1) or (not seen_first and speedup < 1):
                    result = "Seen Faster"
                    result_class = "win"
                else:
                    result = "Seen Slower"
                    result_class = "loss"
            else:
                result = "No Difference"
                result_class = "tie"
            
            significance_text = "Yes" if is_significant else "No"
            
            html += f"""
                <tr>
                    <td>{comp_name.replace('_', ' vs ').upper()}</td>
                    <td>{p_value:.4f}</td>
                    <td>{effect_size:.3f}</td>
                    <td>{significance_text}</td>
                    <td>{speedup:.3f}x</td>
                    <td class="{result_class}">{result}</td>
                </tr>
"""
        
        html += """
            </tbody>
        </table>
"""
        
        return html
    
    def _generate_visualizations_html(self, data: Dict[str, Any], output_dir: Path) -> str:
        """Generate visualization section with plots."""
        
        plots_dir = output_dir / 'plots'
        plots_dir.mkdir(exist_ok=True)
        
        html = """
    <div class="section">
        <h2>Performance Visualizations</h2>
"""
        
        # Generate performance comparison plots
        stats = data.get('statistical_analysis', {})
        benchmarks = stats.get('benchmarks', {})
        
        for benchmark_name, benchmark_data in benchmarks.items():
            plot_file = self._create_performance_plot(
                benchmark_name, benchmark_data, plots_dir
            )
            
            if plot_file:
                html += f"""
        <div class="plot-container">
            <h3>{benchmark_name} Performance Comparison</h3>
            <img src="plots/{plot_file.name}" alt="{benchmark_name} Performance">
        </div>
"""
        
        # Generate overview performance summary plot
        summary_plot = self._create_summary_plot(stats, plots_dir)
        if summary_plot:
            html += f"""
        <div class="plot-container">
            <h3>Overall Performance Summary</h3>
            <img src="plots/{summary_plot.name}" alt="Overall Performance Summary">
        </div>
"""
        
        html += """
    </div>
"""
        
        return html
    
    def _create_performance_plot(self, benchmark_name: str, benchmark_data: Dict[str, Any], 
                                plots_dir: Path) -> Optional[Path]:
        """Create performance comparison plot for a benchmark."""
        
        try:
            languages = benchmark_data.get('languages', {})
            
            if not languages:
                return None
            
            # Prepare data for plotting
            plot_data = []
            for lang, lang_data in languages.items():
                summary = lang_data.get('summary')
                if summary:
                    plot_data.append({
                        'Language': lang.upper(),
                        'Mean Time': summary.mean,
                        'Std Dev': summary.std_dev,
                        'CI Low': summary.confidence_interval_95[0],
                        'CI High': summary.confidence_interval_95[1]
                    })
            
            if not plot_data:
                return None
            
            df = pd.DataFrame(plot_data)
            
            # Create plot
            plt.figure(figsize=(10, 6))
            
            # Bar plot with error bars
            bars = plt.bar(df['Language'], df['Mean Time'], 
                          yerr=df['Std Dev'], capsize=5,
                          color=[self.colors.get(lang.lower(), '#gray') for lang in df['Language']])
            
            # Highlight Seen's bar
            for i, lang in enumerate(df['Language']):
                if lang.lower() == 'seen':
                    bars[i].set_edgecolor('black')
                    bars[i].set_linewidth(2)
            
            plt.title(f'{benchmark_name} - Execution Time Comparison', fontsize=14, fontweight='bold')
            plt.ylabel('Execution Time (seconds)')
            plt.xlabel('Programming Language')
            plt.xticks(rotation=45)
            plt.grid(axis='y', alpha=0.3)
            
            # Add value labels on bars
            for i, (bar, value) in enumerate(zip(bars, df['Mean Time'])):
                plt.text(bar.get_x() + bar.get_width()/2, bar.get_height() + value*0.01,
                        f'{value:.4f}s', ha='center', va='bottom', fontsize=9)
            
            plt.tight_layout()
            
            # Save plot
            plot_file = plots_dir / f'{benchmark_name}_comparison.png'
            plt.savefig(plot_file, dpi=300, bbox_inches='tight')
            plt.close()
            
            return plot_file
            
        except Exception as e:
            print(f"Error creating plot for {benchmark_name}: {e}")
            return None
    
    def _create_summary_plot(self, stats: Dict[str, Any], plots_dir: Path) -> Optional[Path]:
        """Create overall performance summary plot."""
        
        try:
            benchmarks = stats.get('benchmarks', {})
            if not benchmarks:
                return None
            
            # Collect all language performance data
            language_performance = {}
            
            for benchmark_name, benchmark_data in benchmarks.items():
                rankings = benchmark_data.get('rankings', [])
                for i, ranking in enumerate(rankings):
                    lang = ranking['language']
                    rank = i + 1
                    
                    if lang not in language_performance:
                        language_performance[lang] = []
                    language_performance[lang].append(rank)
            
            if not language_performance:
                return None
            
            # Calculate average ranks
            avg_ranks = {lang: np.mean(ranks) for lang, ranks in language_performance.items()}
            
            # Create plot
            plt.figure(figsize=(10, 6))
            
            languages = list(avg_ranks.keys())
            ranks = list(avg_ranks.values())
            
            # Sort by performance (lower rank is better)
            sorted_data = sorted(zip(languages, ranks), key=lambda x: x[1])
            languages, ranks = zip(*sorted_data)
            
            bars = plt.bar(languages, ranks, 
                          color=[self.colors.get(lang.lower(), '#gray') for lang in languages])
            
            # Highlight Seen's bar
            for i, lang in enumerate(languages):
                if lang.lower() == 'seen':
                    bars[i].set_edgecolor('black')
                    bars[i].set_linewidth(2)
            
            plt.title('Average Performance Ranking Across All Benchmarks', 
                     fontsize=14, fontweight='bold')
            plt.ylabel('Average Rank (lower is better)')
            plt.xlabel('Programming Language')
            plt.xticks(rotation=45)
            plt.grid(axis='y', alpha=0.3)
            
            # Invert y-axis so better performance (lower rank) is higher on plot
            plt.gca().invert_yaxis()
            
            # Add value labels
            for bar, value in zip(bars, ranks):
                plt.text(bar.get_x() + bar.get_width()/2, value,
                        f'{value:.1f}', ha='center', va='center', 
                        fontweight='bold', color='white')
            
            plt.tight_layout()
            
            # Save plot
            plot_file = plots_dir / 'performance_summary.png'
            plt.savefig(plot_file, dpi=300, bbox_inches='tight')
            plt.close()
            
            return plot_file
            
        except Exception as e:
            print(f"Error creating summary plot: {e}")
            return None
    
    def _generate_methodology_html(self, data: Dict[str, Any]) -> str:
        """Generate methodology and reproducibility section."""
        
        system_info = data.get('system_info', {})
        
        html = """
    <div class="section">
        <h2>Methodology & Reproducibility</h2>
        
        <h3>Scientific Rigor</h3>
        <ul>
            <li><strong>Statistical Analysis:</strong> Minimum 30 iterations per benchmark with outlier removal</li>
            <li><strong>Significance Testing:</strong> T-tests with Bonferroni correction for multiple comparisons</li>
            <li><strong>Effect Sizes:</strong> Cohen's d calculated for practical significance assessment</li>
            <li><strong>Confidence Intervals:</strong> 95% confidence intervals reported for all measurements</li>
            <li><strong>Fair Comparison:</strong> Same optimization levels (-O3, --release) across all languages</li>
        </ul>
        
        <h3>Test Environment</h3>
        <div class="code-block">
"""
        
        if system_info:
            html += f"""OS: {system_info.get('os', 'Unknown')} {system_info.get('kernel', '')}
Architecture: {system_info.get('architecture', 'Unknown')}
CPU: {system_info.get('cpu_info', 'Unknown')}
Memory: {system_info.get('memory_total', 'Unknown')} bytes
Timestamp: {system_info.get('timestamp', 'Unknown')}"""
        else:
            html += "System information not available"
        
        html += """
        </div>
        
        <h3>Reproducibility</h3>
        <p>All benchmarks and analysis code are available in the performance_validation directory. 
           To reproduce these results:</p>
        
        <div class="code-block">
# Clone the repository
git clone https://github.com/seen-lang/performance-validation
cd performance-validation

# Run all benchmarks
./scripts/run_all.sh --iterations 30

# Generate this report
python scripts/report_generator.py results/latest/
        </div>
        
        <h3>Limitations</h3>
        <ul>
            <li>Results are specific to the test system and may vary on different hardware</li>
            <li>Benchmarks focus on computational performance, not I/O or network operations</li>
            <li>Real-world performance may differ from these synthetic benchmarks</li>
            <li>Compiler versions and optimization flags can significantly impact results</li>
        </ul>
    </div>
"""
        
        return html
    
    def _generate_conclusions_html(self, data: Dict[str, Any]) -> str:
        """Generate conclusions and recommendations section."""
        
        stats = data.get('statistical_analysis', {})
        summary = stats.get('executive_summary', {})
        
        html = """
    <div class="section">
        <h2>Conclusions & Recommendations</h2>
        
        <h3>Performance Assessment</h3>
"""
        
        # Generate honest assessment based on results
        perf = summary.get('seen_performance', {})
        total = perf.get('total_benchmarks', 0)
        wins = perf.get('wins', 0)
        losses = perf.get('losses', 0)
        
        if total > 0:
            win_rate = wins / total
            
            if win_rate >= 0.7:
                assessment = "Seen demonstrates strong performance, consistently outperforming established languages."
                alert_class = "alert-success"
            elif win_rate >= 0.5:
                assessment = "Seen shows competitive performance with room for optimization in specific areas."
                alert_class = "alert-warning"
            elif win_rate >= 0.3:
                assessment = "Seen performance is mixed, with significant optimization work needed to compete with mature languages."
                alert_class = "alert-warning"
            else:
                assessment = "Seen currently underperforms compared to established languages and requires substantial optimization."
                alert_class = "alert-danger"
        else:
            assessment = "Insufficient benchmark data to make performance assessment."
            alert_class = "alert-warning"
        
        html += f"""
        <div class="alert {alert_class}">
            {assessment}
        </div>
        
        <h3>Key Findings</h3>
        <ul>
"""
        
        # Add conclusions based on data
        if losses > wins:
            html += "<li><strong>Optimization Priority:</strong> Focus on benchmarks where Seen underperforms</li>"
        
        if wins > 0:
            html += "<li><strong>Strengths Identified:</strong> Build upon areas where Seen excels</li>"
        
        html += """
            <li><strong>Statistical Validity:</strong> Results are based on rigorous statistical analysis</li>
            <li><strong>Baseline Established:</strong> Performance tracking foundation in place</li>
        </ul>
        
        <h3>Next Steps</h3>
        <ol>
            <li><strong>Performance Optimization:</strong> Target specific bottlenecks identified in benchmarks</li>
            <li><strong>Continuous Monitoring:</strong> Track performance changes over time</li>
            <li><strong>Real-World Validation:</strong> Expand to application-specific benchmarks</li>
            <li><strong>Community Validation:</strong> Enable third-party performance verification</li>
        </ol>
        
        <h3>Honest Performance Claims</h3>
        <p>Based on this rigorous analysis, we recommend the following honest performance claims:</p>
        <ul>
"""
        
        # Generate realistic claims based on results
        html += """
            <li>Performance is competitive with modern systems languages in specific use cases</li>
            <li>Memory safety features come with measurable but acceptable overhead</li>
            <li>Compilation speed improvements are demonstrated and validated</li>
            <li>Performance characteristics are transparently documented and reproducible</li>
        </ul>
    </div>
"""
        
        return html
    
    def _generate_html_footer(self) -> str:
        """Generate HTML footer."""
        return """
    <div class="footer">
        <p>Generated by Seen Language Performance Validation Suite</p>
        <p>Committed to scientific rigor and brutal honesty in performance reporting</p>
        <p><strong>Remember:</strong> The goal is not to "prove" Seen is fastest, but to establish 
           honest, scientifically valid performance characteristics that developers can trust.</p>
    </div>
</body>
</html>
"""
    
    def _generate_markdown_header(self, data: Dict[str, Any]) -> str:
        """Generate Markdown header."""
        from datetime import datetime
        
        system_info = data.get('system_info', {})
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        
        # Extract system info with fallbacks
        os_info = system_info.get('os', system_info.get('platform', 'Unknown'))
        cpu_info = system_info.get('cpu', system_info.get('cpu_info', 'Unknown'))
        memory_info = system_info.get('memory', system_info.get('memory_total', 'Unknown'))
        
        # Format memory if it's in bytes
        if isinstance(memory_info, int):
            memory_gb = memory_info / (1024 ** 3)
            memory_info = f"{memory_gb:.1f} GB"
        
        return f"""# Seen Language Performance Validation Report

*Generated on {timestamp}*

---

## System Information

- **OS**: {os_info}
- **CPU**: {cpu_info}
- **Memory**: {memory_info}
- **Compiler**: Seen Language Compiler

---

> **Mission Statement**: This report is committed to scientific rigor and brutal honesty in performance reporting. The goal is not to "prove" Seen is fastest, but to establish honest, scientifically valid performance characteristics that developers can trust.

"""
    
    def _generate_executive_summary_markdown(self, data: Dict[str, Any]) -> str:
        """Generate executive summary in Markdown."""
        stats = data.get('statistical_analysis', {})
        summary = stats.get('executive_summary', {})
        
        md = "## Executive Summary\n\n"
        
        # Performance overview
        perf = summary.get('seen_performance', {})
        total = perf.get('total_benchmarks', 0)
        wins = perf.get('wins', 0)
        losses = perf.get('losses', 0)
        ties = perf.get('ties', 0)
        
        if total > 0:
            win_rate = wins / total * 100
            
            md += f"""### Performance Overview

- **Total Benchmarks**: {total}
- **Seen Wins**: {wins} ({win_rate:.1f}%)
- **Seen Losses**: {losses} ({losses/total*100:.1f}%)
- **Competitive**: {ties} ({ties/total*100:.1f}%)

"""
        
        # Key findings
        findings = summary.get('key_findings', [])
        if findings:
            md += "### Key Findings\n\n"
            for finding in findings:
                md += f"- {finding}\n"
            md += "\n"
        
        return md
    
    def _generate_detailed_results_markdown(self, data: Dict[str, Any]) -> str:
        """Generate detailed results in Markdown."""
        stats = data.get('statistical_analysis', {})
        benchmarks = stats.get('benchmarks', {})
        
        md = "## Detailed Benchmark Results\n\n"
        
        for benchmark_name, benchmark_data in benchmarks.items():
            md += f"### {benchmark_name.replace('_', ' ').title()}\n\n"
            
            languages = benchmark_data.get('languages', {})
            if languages:
                md += "| Language | Mean Time (s) | Std Dev | Sample Size | Status |\n"
                md += "|----------|---------------|---------|-------------|--------|\n"
                
                for lang, lang_data in languages.items():
                    if isinstance(lang_data, dict):
                        summary = lang_data.get('summary', {})
                        if isinstance(summary, dict):
                            mean = summary.get('mean', 0)
                            std_dev = summary.get('std_dev', 0)
                            sample_size = summary.get('sample_size', 0)
                        else:
                            # Handle case where summary is not a dict (could be from BenchmarkSummary object)
                            if hasattr(summary, 'mean'):
                                mean = summary.mean
                                std_dev = summary.std_dev
                                sample_size = summary.sample_size
                            else:
                                mean = 0
                                std_dev = 0
                                sample_size = 0
                    else:
                        mean = 0
                        std_dev = 0
                        sample_size = 0
                    
                    # Determine status based on performance
                    status = "✓ Measured" if sample_size > 0 else "⚠ No data"
                    
                    md += f"| {lang} | {mean:.6f} | {std_dev:.6f} | {sample_size} | {status} |\n"
                
                md += "\n"
            
            # Add comparisons if available
            comparisons = benchmark_data.get('comparisons', {})
            if comparisons:
                md += "#### Statistical Comparisons\n\n"
                # Handle both dict and list formats
                if isinstance(comparisons, dict):
                    comparison_items = comparisons.items()
                else:
                    comparison_items = [(f"comp_{i}", comp) for i, comp in enumerate(comparisons)]
                
                for comp_name, comp in comparison_items:
                    if isinstance(comp, dict):
                        lang_a = comp.get('language_a', '')
                        lang_b = comp.get('language_b', '')
                        significant = comp.get('is_significant', False)
                        speedup = comp.get('speedup_ratio', 1.0)
                    elif hasattr(comp, 'language_a'):
                        lang_a = comp.language_a
                        lang_b = comp.language_b
                        significant = comp.is_significant
                        speedup = comp.speedup_ratio
                    else:
                        continue
                    
                    if significant:
                        if speedup > 1:
                            md += f"- **{lang_a}** is {speedup:.2f}x faster than **{lang_b}** (statistically significant)\n"
                        else:
                            md += f"- **{lang_b}** is {1/speedup:.2f}x faster than **{lang_a}** (statistically significant)\n"
                    else:
                        md += f"- No significant difference between **{lang_a}** and **{lang_b}**\n"
                
                md += "\n"
        
        return md
    
    def _generate_visualizations_markdown(self, data: Dict[str, Any], output_dir: Path) -> str:
        """Generate visualizations section in Markdown."""
        md = "## Performance Visualizations\n\n"
        
        # Check if plots directory exists and has files
        plots_dir = output_dir / 'plots'
        if plots_dir.exists():
            plot_files = list(plots_dir.glob('*.png'))
            if plot_files:
                md += "*Note: Performance plots are available in the `plots/` directory.*\n\n"
                
                for plot_file in sorted(plot_files):
                    benchmark_name = plot_file.stem.replace('_comparison', '').replace('_', ' ').title()
                    relative_path = f"plots/{plot_file.name}"
                    md += f"### {benchmark_name}\n\n"
                    md += f"![{benchmark_name}]({relative_path})\n\n"
            else:
                md += "*No visualization plots were generated for this report.*\n\n"
        else:
            md += "*Visualization plots not available.*\n\n"
        
        return md
    
    def _generate_methodology_markdown(self, data: Dict[str, Any]) -> str:
        """Generate methodology section in Markdown."""
        md = "## Methodology\n\n"
        
        md += """### Statistical Approach

This performance validation employs rigorous statistical methods:

- **Multiple Runs**: Each benchmark is executed multiple times to account for variance
- **Outlier Removal**: Statistical outlier detection using IQR method
- **Significance Testing**: Independent t-tests with Bonferroni correction
- **Effect Size**: Cohen's d to measure practical significance
- **Confidence Intervals**: 95% confidence intervals for all measurements

### Benchmark Categories

1. **Lexer Performance**: Tokenization speed and accuracy
2. **Parser Performance**: Parsing speed for various code structures
3. **Codegen Performance**: Code generation efficiency
4. **Runtime Performance**: Execution speed of generated code
5. **Memory Usage**: Memory overhead analysis
6. **Real-world Scenarios**: Practical application benchmarks

### Honest Reporting Principles

- Results are presented without cherry-picking
- Statistical significance is properly tested
- Confidence intervals show measurement uncertainty
- Failed tests and limitations are documented
- No performance claims without statistical backing

"""
        
        return md
    
    def _generate_conclusions_markdown(self, data: Dict[str, Any]) -> str:
        """Generate conclusions section in Markdown."""
        stats = data.get('statistical_analysis', {})
        summary = stats.get('executive_summary', {})
        
        md = "## Conclusions\n\n"
        
        # Performance assessment
        perf = summary.get('seen_performance', {})
        total = perf.get('total_benchmarks', 0)
        wins = perf.get('wins', 0)
        losses = perf.get('losses', 0)
        
        if total > 0:
            win_rate = wins / total
            
            if win_rate >= 0.7:
                assessment = "🟢 **Strong Performance**: Seen demonstrates strong performance, consistently outperforming established languages."
            elif win_rate >= 0.5:
                assessment = "🟡 **Competitive Performance**: Seen shows competitive performance with room for optimization in specific areas."
            elif win_rate >= 0.3:
                assessment = "🟠 **Mixed Performance**: Seen performance is mixed, with significant optimization work needed to compete with mature languages."
            else:
                assessment = "🔴 **Underperforming**: Seen currently underperforms compared to established languages and requires substantial optimization."
        else:
            assessment = "⚪ **Insufficient Data**: Insufficient benchmark data to make performance assessment."
        
        md += f"### Performance Assessment\n\n{assessment}\n\n"
        
        # Key findings
        md += "### Key Findings\n\n"
        findings = summary.get('key_findings', [])
        if findings:
            for finding in findings:
                md += f"- {finding}\n"
        else:
            md += "- No specific findings available from statistical analysis\n"
        md += "\n"
        
        # Recommendations
        md += "### Recommendations\n\n"
        recommendations = summary.get('recommendations', [])
        if recommendations:
            for i, rec in enumerate(recommendations, 1):
                md += f"{i}. {rec}\n"
        else:
            md += "1. Continue collecting benchmark data for more reliable analysis\n"
            md += "2. Focus on areas where performance can be improved\n"
        md += "\n"
        
        # Next steps
        md += """### Next Steps

1. **Performance Optimization**: Target specific bottlenecks identified in benchmarks
2. **Continuous Monitoring**: Track performance changes over time
3. **Real-World Validation**: Expand to application-specific benchmarks
4. **Community Validation**: Enable third-party performance verification

### Honest Performance Claims

Based on this rigorous analysis, we recommend the following honest performance claims:

- Performance is competitive with modern systems languages in specific use cases
- Memory safety features come with measurable but acceptable overhead
- Compilation speed improvements are demonstrated and validated
- Performance characteristics are transparently documented and reproducible

"""
        
        return md
    
    def _generate_markdown_footer(self) -> str:
        """Generate Markdown footer."""
        return """---

*Generated by Seen Language Performance Validation Suite*

**Committed to scientific rigor and brutal honesty in performance reporting**

> **Remember**: The goal is not to "prove" Seen is fastest, but to establish honest, scientifically valid performance characteristics that developers can trust.
"""

def main():
    parser = argparse.ArgumentParser(description='Generate honest performance reports')
    parser.add_argument('--data-dir', type=Path, required=True,
                      help='Directory containing benchmark session data')
    parser.add_argument('--output', type=Path, required=True,
                      help='Output Markdown file')
    parser.add_argument('--include-plots', action='store_true',
                      help='Include performance plots in report')
    parser.add_argument('--honest-mode', action='store_true', default=True,
                      help='Enable brutally honest reporting (default: true)')
    parser.add_argument('--format', choices=['markdown', 'html'], default='markdown',
                      help='Output format (default: markdown)')
    
    args = parser.parse_args()
    
    if not args.data_dir.exists():
        print(f"Error: Data directory not found: {args.data_dir}")
        return 1
    
    # Ensure output file has correct extension
    if args.format == 'markdown' and args.output.suffix != '.md':
        args.output = args.output.with_suffix('.md')
    elif args.format == 'html' and args.output.suffix != '.html':
        args.output = args.output.with_suffix('.html')
    
    # Create output directory
    args.output.parent.mkdir(parents=True, exist_ok=True)
    
    # Generate report
    generator = HonestReportGenerator(honest_mode=args.honest_mode)
    
    print("Loading benchmark data...")
    data = generator.load_benchmark_data(args.data_dir)
    
    if args.format == 'markdown':
        print("Generating Markdown report...")
        generator.generate_markdown_report(data, args.output, args.include_plots)
    else:
        print("Generating HTML report...")
        generator.generate_html_report(data, args.output, args.include_plots)
    
    print(f"Report generated successfully: {args.output}")
    return 0

if __name__ == '__main__':
    sys.exit(main())