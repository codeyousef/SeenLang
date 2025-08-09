# Performance Report Data Issues - Explained

## The Problem
The performance reports are showing missing data because:

1. **Insufficient Iterations**: Running benchmarks with only 1 iteration doesn't provide enough data for statistical analysis
2. **Statistical Analysis Requirements**: The analysis script requires at least 2 data points (preferably 30+) to calculate statistics
3. **Report Generator Issues**: The report generator expects properly formatted statistical summaries

## Root Causes

### 1. Minimum Sample Size Issue
- Statistical analysis REQUIRES at least 2 data points to calculate standard deviation
- With `-Iterations 1`, only 1 data point is collected
- This causes the statistical analysis to skip those benchmarks
- The report then shows empty sections

### 2. Data Format Mismatch
- Statistical analysis outputs `StatisticalSummary` objects as string representations
- Report generator tries to parse these strings but sometimes fails
- This results in missing data even when statistics are calculated

### 3. Different Benchmark Structures
- Lexer benchmark: Uses `times` array with multiple measurements
- Reactive benchmark: Uses different structure with `results` object
- Parser/Codegen/etc: Only have placeholder data
- Report generator expects consistent structure

## Solutions

### Immediate Fix: Run with More Iterations
```powershell
# Minimum for basic statistics (2+ iterations)
.\scripts\run_all.ps1 -Categories lexer -Iterations 5 -SkipSetup

# Recommended for reliable statistics (30+ iterations)
.\scripts\run_all.ps1 -Categories lexer,reactive -Iterations 30 -SkipSetup

# Full benchmark suite with proper statistics
.\scripts\run_all.ps1 -Iterations 30 -AutoInstall
```

### Why You Need Multiple Iterations
- **Mean**: Average performance across runs
- **Standard Deviation**: Measure of variability
- **Confidence Intervals**: Statistical certainty of results
- **Outlier Detection**: Remove anomalous results
- **Statistical Significance**: Compare languages reliably

### What Each Iteration Count Provides

| Iterations | Statistical Analysis | Report Quality |
|------------|---------------------|----------------|
| 1 | ❌ No statistics possible | Empty sections |
| 2-4 | ⚠️ Basic mean/std dev | Limited data |
| 5-9 | ⚠️ Some statistics | Basic report |
| 10-29 | ✅ Most statistics | Good report |
| 30+ | ✅ Full statistics | Complete report |

## Current Benchmark Status

| Benchmark | Implementation | Data Available | Shows in Report |
|-----------|---------------|----------------|-----------------|
| Lexer | ✅ Real C++/Rust | Yes (with iterations > 1) | Yes |
| Reactive | ✅ Real C++ | Yes | Partial |
| Parser | ❌ Placeholder | No | No |
| Codegen | ❌ Placeholder | No | No |
| Runtime | ❌ Placeholder | No | No |
| Memory | ❌ Placeholder | No | No |
| Real-world | ⚠️ Simulated | Limited | Limited |

## How to Get Complete Reports

### Step 1: Run with Sufficient Iterations
```powershell
.\scripts\run_all.ps1 -Iterations 30 -SkipSetup
```

### Step 2: Wait for Completion
- Lexer: ~1-2 minutes for 30 iterations
- Reactive: ~30 seconds for 30 iterations
- Total: ~5-10 minutes for all benchmarks

### Step 3: Check the Report
Open: `results\<timestamp>\performance_report.md`

The report will now contain:
- Complete statistical tables
- Performance comparisons
- Effect sizes and p-values
- Confidence intervals
- Actual benchmark data

## Example of Good vs Bad Reports

### Bad (1 iteration):
```
### Real Lexer Performance

(empty - no data shown)
```

### Good (30 iterations):
```
### Real Lexer Performance

| Language | Mean Time (s) | Std Dev | Sample Size | Status |
|----------|---------------|---------|-------------|--------|
| rust | 0.000741 | 0.000010 | 27 | ✓ Measured |
| cpp | 0.000635 | 0.000063 | 30 | ✓ Measured |

#### Statistical Comparisons
- Rust vs C++: p=0.0023, Effect Size: 2.29 (Large)
- C++ is 14.3% faster
```

## Summary

The "missing data" issue is simply because:
1. You need to run benchmarks with more iterations (minimum 2, recommended 30)
2. Only lexer and reactive benchmarks have real implementations
3. Other benchmarks show "not implemented" as intended

**Solution**: Run `.\scripts\run_all.ps1 -Iterations 30` for complete reports with all available data.