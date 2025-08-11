# Windows Benchmark Scripts Documentation

## Overview
This directory contains Windows-specific scripts for running the Seen language benchmark suite on Windows systems. These scripts provide feature parity with the Unix/Linux shell scripts.

## Scripts

### 1. `run_benchmarks.ps1` - Main Benchmark Runner
Full-featured PowerShell script for running comprehensive benchmarks.

**Features:**
- Runs Seen benchmarks in JIT or AOT mode
- Builds and runs competitor benchmarks (Rust, C++, Zig)
- Statistical analysis and reporting
- Performance claim validation
- Baseline comparison for regression detection

**Usage:**
```powershell
# Run with defaults (JIT mode, 100 iterations)
.\run_benchmarks.ps1

# Run in AOT mode with 500 iterations
.\run_benchmarks.ps1 -Mode aot -Iterations 500

# Run specific category with validation
.\run_benchmarks.ps1 -Category microbenchmarks -Validate

# Compare against baseline
.\run_benchmarks.ps1 -Compare baseline.json
```

### 2. `run_benchmarks.bat` - Batch Wrapper
Simple batch file wrapper for users who prefer command prompt over PowerShell.

**Usage:**
```batch
# Run with defaults
run_benchmarks.bat

# Run with options
run_benchmarks.bat --mode aot --iterations 500

# Show help
run_benchmarks.bat --help
```

### 3. `build_competitors.ps1` - Competitor Build Script
Builds benchmark implementations for competitor languages.

**Features:**
- Auto-detects installed compilers (MSVC, GCC, Clang)
- Builds Rust benchmarks via Cargo
- Builds C++ benchmarks with optimal flags
- Builds Zig benchmarks
- Shows build status and binary sizes

**Usage:**
```powershell
# Build all competitors (release mode)
.\build_competitors.ps1

# Clean and rebuild
.\build_competitors.ps1 -Clean

# Build with verbose output
.\build_competitors.ps1 -Verbose

# Build debug versions
.\build_competitors.ps1 -Release:$false
```

### 4. `quick_bench.ps1` - Quick Benchmark Runner
Simplified script for quick performance tests.

**Features:**
- Fast execution for rapid testing
- Built-in PowerShell baseline measurements
- Automatic competitor detection and execution
- JSON result export

**Usage:**
```powershell
# Run arithmetic benchmarks
.\quick_bench.ps1 -Test arithmetic

# Run string benchmarks
.\quick_bench.ps1 -Test string

# Run file I/O benchmarks
.\quick_bench.ps1 -Test file

# Compare competitors only
.\quick_bench.ps1 -CompareOnly
```

## Requirements

### PowerShell
- Windows PowerShell 5.1+ (built into Windows 10/11)
- OR PowerShell Core 7+ (cross-platform)

### Compilers (Optional)
For building competitor benchmarks:
- **Rust**: Install from https://rustup.rs/
- **C++**: One of:
  - Visual Studio with C++ workload
  - MinGW-w64 from https://www.mingw-w64.org/
  - LLVM/Clang from https://releases.llvm.org/
- **Zig**: Install from https://ziglang.org/download/

### Python (for analysis)
- Python 3.8+ with scipy, numpy, matplotlib

## Installation

1. Ensure PowerShell execution policy allows scripts:
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

2. Install required compilers (see above)

3. Build competitor benchmarks:
```powershell
.\build_competitors.ps1
```

4. Run benchmarks:
```powershell
.\run_benchmarks.ps1
```

## Compiler Detection

The scripts automatically detect installed compilers in this order:

### C++ Compilers
1. **MSVC (cl.exe)**: Checked via PATH or vswhere
2. **GCC (g++.exe)**: Checked in PATH
3. **Clang (clang++.exe)**: Checked in PATH

### Visual Studio Detection
The `build_competitors.ps1` script uses multiple methods to find MSVC:
1. vswhere utility (most reliable)
2. Common installation paths for VS 2017/2019/2022
3. Environment variables

## Output

### Results Directory
All benchmark results are saved to `benchmarks/results/`:
- JSON files with raw data
- Text files with competitor outputs
- Statistical analysis results

### Reports Directory
Generated reports are saved to `benchmarks/reports/`:
- `benchmark_report.json` - Structured data
- `benchmark_report.md` - Markdown summary
- `benchmark_report.html` - Interactive HTML report
- `validation_report.json` - Performance claim validation
- `comparison_report.json` - Regression detection

## Troubleshooting

### "Script cannot be loaded" Error
Run PowerShell as Administrator and execute:
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### "Seen compiler not found"
The scripts look for Seen compiler at:
1. `<project_root>/target/release/seen.exe` (preferred)
2. `<project_root>/target/debug/seen.exe` (fallback)

Build the Seen compiler first if not found.

### "No compiler found" for C++
Install one of the supported C++ compilers. Visual Studio Community is recommended for Windows.

### Permission Issues
Ensure you have write permissions to the benchmarks directory and its subdirectories.

## Performance Notes

### Windows-Specific Considerations
- File I/O may be slower on Windows due to antivirus scanning
- Process creation is more expensive than on Unix systems
- Use AOT mode for best performance comparisons

### Optimization Flags
The scripts use appropriate optimization flags for each compiler:
- **MSVC**: `/O2` (maximize speed)
- **GCC/MinGW**: `-O3 -march=native`
- **Clang**: `-O3 -march=native`
- **Rust**: `--release` with default release profile
- **Zig**: `-O ReleaseFast`

## Integration with CI/CD

These scripts are designed to work with GitHub Actions Windows runners:
```yaml
- name: Run Benchmarks on Windows
  run: |
    .\benchmarks\build_competitors.ps1
    .\benchmarks\run_benchmarks.ps1 -Mode aot -Iterations 100 -Validate
```

## Contributing

When modifying these scripts:
1. Test on multiple Windows versions (10, 11, Server)
2. Test with different compiler configurations
3. Ensure backward compatibility with Windows PowerShell 5.1
4. Update this documentation for any new features

## License

These scripts are part of the Seen language project and follow the same MIT license.