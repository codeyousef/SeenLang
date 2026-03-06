# Seen Language Benchmark Suite

## 🚀 Quick Start

Run the complete benchmark suite with one command:

```powershell
.\run_all_benchmarks.ps1
```

This will:
1. Build all competitor benchmarks (Rust, C++, Zig)
2. Run Seen benchmarks (arithmetic, memory, strings)
3. Run competitor benchmarks
4. Generate an HTML performance report
5. Show performance rankings

## 📊 Features

### Comprehensive Testing
- **Arithmetic operations**: Addition, multiplication, floating-point, bitwise
- **Memory operations**: Allocation, sequential access, random access
- **String operations**: Concatenation, parsing, searching
- **Cross-language comparison**: Seen vs Rust vs C++ vs Zig

### Automated Reporting
- HTML reports with charts and tables
- JSON results for further analysis
- Performance rankings
- System information capture

## 📁 Directory Structure

```
benchmarks/
├── run_all_benchmarks.ps1    # Main benchmark runner
├── seen_benchmarks/           # Seen implementations
│   ├── arithmetic.seen        # Math operations
│   ├── memory.seen           # Memory benchmarks
│   └── strings.seen          # String operations
├── competitors/              # Other language implementations
│   ├── rust/                 # Rust benchmarks
│   ├── cpp/                  # C++ benchmarks
│   └── zig/                  # Zig benchmarks
├── results/                  # Benchmark results (JSON)
└── reports/                  # HTML reports
```

## 🎯 Usage Examples

### Basic Run
```powershell
# Run with default settings (1M iterations)
.\run_all_benchmarks.ps1
```

### Quick Test
```powershell
# Run with fewer iterations for quick testing
.\run_all_benchmarks.ps1 -QuickTest
```

### Custom Iterations
```powershell
# Run with specific iteration count
.\run_all_benchmarks.ps1 -Iterations 5000000
```

### Skip Building
```powershell
# Skip rebuilding competitors (use existing binaries)
.\run_all_benchmarks.ps1 -SkipBuild
```

### Verbose Output
```powershell
# Show detailed output
.\run_all_benchmarks.ps1 -Verbose
```

## 🔧 Requirements

### Required
- **Windows 10/11** or Windows Server
- **PowerShell 5.1+** or PowerShell Core 7+
- **Seen compiler** (built from this repository)

### Optional (for competitors)
- **Rust**: Install from https://rustup.rs/
- **C++ Compiler**: One of:
  - Visual Studio with C++ workload
  - MinGW-w64
  - LLVM/Clang
- **Zig**: Install from https://ziglang.org/

## 📈 Benchmark Categories

### 1. Arithmetic Operations
- 32-bit integer addition/multiplication
- 64-bit floating-point operations
- Bitwise operations (XOR, AND, OR, shifts)

### 2. Memory Operations
- Heap allocation/deallocation
- Sequential memory access
- Random memory access

### 3. String Operations
- String concatenation
- String building
- Parsing (string to integer)
- Pattern searching

## 📊 Output Files

### Results (JSON)
Location: `results/benchmark_results_YYYYMMDD_HHMMSS.json`

Contains:
- Raw benchmark data
- System information
- Execution times
- Operations per second

### Report (HTML)
Location: `reports/performance_report_YYYYMMDD_HHMMSS.html`

Contains:
- Visual performance comparison
- Rankings by operation type
- System specifications
- Relative performance metrics

## 🏆 Performance Metrics

The benchmarks measure:
- **Operations per second**: Higher is better
- **Execution time**: Lower is better
- **Relative performance**: Percentage compared to C++

## 🛠️ Troubleshooting

### "Seen compiler not found"
Build the Seen compiler first:
```powershell
cargo build --release
```

### "Parse errors in Seen files"
The Seen benchmarks use simplified syntax. If you encounter errors:
1. Check that the Seen compiler is up to date
2. Verify the `.seen` files have valid syntax
3. Use `-QuickTest` mode for debugging

### "Competitor benchmark not found"
Build competitors manually:
```powershell
.\build_competitors.ps1
```

### "Access denied" errors
Run PowerShell as Administrator or check file permissions

## 📝 Interpreting Results

### Performance Rankings
Languages are ranked by average relative performance across all benchmarks:
- **100%**: Baseline performance (usually C++)
- **>100%**: Faster than baseline
- **<100%**: Slower than baseline

### Example Output
```
Performance Rankings:
  1. Rust: 98.5% average performance
  2. C++: 97.2% average performance
  3. Seen: 85.3% average performance
  4. Zig: 94.1% average performance
```

## 🔄 Continuous Benchmarking

For continuous performance monitoring:

```powershell
# Run benchmarks daily
while ($true) {
    .\run_all_benchmarks.ps1
    Start-Sleep -Seconds 86400  # Wait 24 hours
}
```

## 📚 Additional Scripts

- `build_competitors.ps1` - Build only competitor benchmarks
- `simple_test.ps1` - Basic functionality test
- `demo_benchmark.ps1` - Demonstration with examples

## 🤝 Contributing

To add new benchmarks:
1. Create a `.seen` file in `seen_benchmarks/`
2. Add equivalent implementations in `competitors/`
3. Update `run_all_benchmarks.ps1` to include new tests
4. Submit a pull request with results

## 📄 License

MIT License - See repository root for details