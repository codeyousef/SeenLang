# Master benchmark runner for comprehensive Seen Language performance validation
# Executes all benchmarks with proper statistical rigor and honest reporting
# Windows PowerShell version of run_all.sh

param(
    [int]$Iterations = 30,
    [int]$WarmupIterations = 5,
    [int]$TimeoutSeconds = 300,
    [string]$Categories = "all",
    [string]$Competitors = "cpp,rust,zig",
    [string]$TestSize = "medium",
    [switch]$Verbose,
    [switch]$SkipSetup,
    [switch]$StatisticalOnly,
    [switch]$RealWorldOnly,
    [switch]$Clean,
    [switch]$Help
)

# Configuration
$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = (Get-Item "$SCRIPT_DIR\..\..").FullName
$PERF_ROOT = (Get-Item "$SCRIPT_DIR\..").FullName
$RESULTS_DIR = "$PERF_ROOT\results"
$TIMESTAMP = Get-Date -Format "yyyyMMdd_HHmmss"
$SESSION_DIR = "$RESULTS_DIR\$TIMESTAMP"

# Default parameters override from command line
$ITERATIONS = if ($PSBoundParameters.ContainsKey('Iterations')) { $Iterations } else { 30 }
$WARMUP_ITERATIONS = if ($PSBoundParameters.ContainsKey('WarmupIterations')) { $WarmupIterations } else { 5 }
$TIMEOUT_SECONDS = if ($PSBoundParameters.ContainsKey('TimeoutSeconds')) { $TimeoutSeconds } else { 300 }
$CATEGORIES = if ($PSBoundParameters.ContainsKey('Categories')) { $Categories } else { "all" }
$COMPETITORS = if ($PSBoundParameters.ContainsKey('Competitors')) { $Competitors } else { "cpp,rust,zig" }
$TEST_SIZE = if ($PSBoundParameters.ContainsKey('TestSize')) { $TestSize } else { "medium" }
$VERBOSE = if ($PSBoundParameters.ContainsKey('Verbose')) { $Verbose } else { $false }
$SKIP_SETUP = if ($PSBoundParameters.ContainsKey('SkipSetup')) { $SkipSetup } else { $false }
$STATISTICAL_ONLY = if ($PSBoundParameters.ContainsKey('StatisticalOnly')) { $StatisticalOnly } else { $false }
$REAL_WORLD_ONLY = if ($PSBoundParameters.ContainsKey('RealWorldOnly')) { $RealWorldOnly } else { $false }

# Colors for output (Windows PowerShell compatible)
$RED = "Red"
$GREEN = "Green"
$YELLOW = "Yellow"
$BLUE = "Blue"
$CYAN = "Cyan"
$WHITE = "White"

# Logging functions
function Log-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor $BLUE
}

function Log-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor $GREEN
}

function Log-Warning {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor $YELLOW
}

function Log-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor $RED
}

function Log-Header {
    param([string]$Message)
    Write-Host ""
    Write-Host "=====================================================" -ForegroundColor $CYAN
    Write-Host " $Message" -ForegroundColor $CYAN
    Write-Host "=====================================================" -ForegroundColor $CYAN
    Write-Host ""
}

# Show help information
function Show-Help {
    Write-Host @"
Seen Language Performance Validation Suite (Windows PowerShell)

Usage: .\run_all.ps1 [OPTIONS]

OPTIONS:
    -Iterations N          Number of benchmark iterations (default: 30)
    -WarmupIterations N    Number of warmup iterations (default: 5)  
    -TimeoutSeconds N      Timeout per benchmark in seconds (default: 300)
    -Categories LIST       Comma-separated categories to test (default: all)
                          Options: lexer,parser,codegen,runtime,memory,reactive,all
    -Competitors LIST      Comma-separated competitors to test (default: cpp,rust,zig)
                          Options: cpp,rust,zig,c
    -TestSize SIZE         Test data size (small,medium,large) (default: medium)
    -RealWorldOnly         Run only real-world application benchmarks
    -StatisticalOnly       Skip benchmarks, only run statistical analysis
    -SkipSetup            Skip environment setup and dependency checks
    -Verbose              Enable verbose output and debugging
    -Clean                Clean previous results and exit
    -Help                 Show this help message

CATEGORIES:
    lexer      - Lexical analysis performance vs competitors
    parser     - Parser speed and memory usage
    codegen    - Code generation quality and speed  
    runtime    - Runtime performance of generated code
    memory     - Memory management overhead analysis
    reactive   - Reactive programming abstractions cost
    real_world - Real-world application benchmarks

EXAMPLES:
    .\run_all.ps1                                    # Run all benchmarks with defaults
    .\run_all.ps1 -Iterations 50 -Verbose           # More iterations with verbose output
    .\run_all.ps1 -Categories "lexer,memory"        # Only lexer and memory benchmarks
    .\run_all.ps1 -RealWorldOnly -TestSize "large"  # Large real-world benchmarks only
    .\run_all.ps1 -StatisticalOnly                  # Only statistical analysis of existing data
    .\run_all.ps1 -Competitors "rust,zig"           # Compare only against Rust and Zig
"@
}

# Check system requirements and dependencies
function Test-Environment {
    if ($SKIP_SETUP) {
        Log-Info "Skipping environment setup"
        return $true
    }
    
    Log-Header "Environment Setup and Validation"
    
    # Check required tools
    $required_tools = @("python", "git", "cmake")
    $missing_tools = @()
    
    foreach ($tool in $required_tools) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            $missing_tools += $tool
        }
    }
    
    if ($missing_tools.Count -gt 0) {
        Log-Error "Missing required tools: $($missing_tools -join ', ')"
        Log-Error "Please install missing tools and try again"
        return $false
    }
    
    # Check Python dependencies
    try {
        python -c "import numpy, scipy, matplotlib, pandas, seaborn" 2>$null
        if ($LASTEXITCODE -ne 0) {
            Log-Warning "Installing required Python packages..."
            pip install numpy scipy matplotlib pandas seaborn
        }
    } catch {
        Log-Warning "Installing required Python packages..."
        pip install numpy scipy matplotlib pandas seaborn
    }
    
    # Verify Seen compiler
    $seen_exe = "$PROJECT_ROOT\target\release\seen.exe"
    $seen_debug = "$PROJECT_ROOT\target\debug\seen.exe"
    
    if (-not (Test-Path $seen_exe) -and -not (Test-Path $seen_debug)) {
        Log-Warning "Seen compiler not found, building..."
        Set-Location $PROJECT_ROOT
        cargo build --release --bin seen
    }
    
    # Check competitor languages if requested
    $comp_array = $COMPETITORS -split ','
    foreach ($comp in $comp_array) {
        switch ($comp.Trim()) {
            "cpp" {
                if (-not (Get-Command clang++ -ErrorAction SilentlyContinue) -and 
                    -not (Get-Command g++ -ErrorAction SilentlyContinue)) {
                    Log-Warning "C++ compiler not found, some benchmarks will be skipped"
                }
            }
            "rust" {
                if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
                    Log-Warning "Rust compiler not found, installing..."
                    # Note: On Windows, Rust installation typically requires manual intervention
                    Log-Warning "Please install Rust from https://rustup.rs/"
                }
            }
            "zig" {
                if (-not (Get-Command zig -ErrorAction SilentlyContinue)) {
                    Log-Warning "Zig compiler not found, some benchmarks will be skipped"
                }
            }
            "c" {
                if (-not (Get-Command clang -ErrorAction SilentlyContinue) -and 
                    -not (Get-Command gcc -ErrorAction SilentlyContinue)) {
                    Log-Warning "C compiler not found, some benchmarks will be skipped"
                }
            }
        }
    }
    
    Log-Success "Environment validation completed"
    return $true
}

# Setup benchmark session
function Initialize-Session {
    Log-Header "Setting Up Benchmark Session"
    
    # Create session directory
    New-Item -ItemType Directory -Path $SESSION_DIR -Force | Out-Null
    New-Item -ItemType Directory -Path "$SESSION_DIR\raw_data" -Force | Out-Null
    New-Item -ItemType Directory -Path "$SESSION_DIR\logs" -Force | Out-Null
    New-Item -ItemType Directory -Path "$SESSION_DIR\metadata" -Force | Out-Null
    
    # Record system information
    $system_info = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        hostname = $env:COMPUTERNAME
        os = (Get-CimInstance Win32_OperatingSystem).Caption
        version = (Get-CimInstance Win32_OperatingSystem).Version
        architecture = (Get-CimInstance Win32_ComputerSystem).SystemType
        cpu_info = (Get-CimInstance Win32_Processor).Name
        memory_total = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory
        benchmark_config = @{
            iterations = $ITERATIONS
            warmup_iterations = $WARMUP_ITERATIONS
            timeout_seconds = $TIMEOUT_SECONDS
            categories = $CATEGORIES
            competitors = $COMPETITORS
            test_size = $TEST_SIZE
        }
    } | ConvertTo-Json -Depth 3
    
    $system_info | Out-File "$SESSION_DIR\metadata\system_info.json" -Encoding UTF8
    
    # Record compiler versions
    $versions = @{}
    
    # Seen version
    $seen_exe = "$PROJECT_ROOT\target\release\seen.exe"
    if (Test-Path $seen_exe) {
        try {
            $versions.seen = & $seen_exe --version 2>$null
        } catch {
            $versions.seen = "Unknown"
        }
    } else {
        $versions.seen = "Not found"
    }
    
    # Other compilers
    try { $versions.rust = (rustc --version 2>$null) } catch { $versions.rust = "Not installed" }
    try { $versions.clang = (clang --version 2>$null | Select-Object -First 1) } catch { $versions.clang = "Not installed" }
    try { $versions.gcc = (gcc --version 2>$null | Select-Object -First 1) } catch { $versions.gcc = "Not installed" }
    try { $versions.zig = (zig version 2>$null) } catch { $versions.zig = "Not installed" }
    
    $versions | ConvertTo-Json | Out-File "$SESSION_DIR\metadata\compiler_versions.json" -Encoding UTF8
    
    Log-Success "Session setup completed: $SESSION_DIR"
}

# Run benchmarks for a specific category
function Invoke-CategoryBenchmarks {
    param([string]$Category)
    
    $category_dir = "$PERF_ROOT\benchmarks\$Category"
    $output_dir = "$SESSION_DIR\raw_data\$Category"
    
    if (-not (Test-Path $category_dir)) {
        Log-Warning "Category directory not found: $category_dir"
        return
    }
    
    Log-Header "Running $Category Benchmarks"
    New-Item -ItemType Directory -Path $output_dir -Force | Out-Null
    
    # Find all benchmark executables or scripts
    $benchmarks = @()
    $benchmarks += Get-ChildItem -Path $category_dir -Filter "*.ps1" -Recurse | Where-Object { $_.Name -like "benchmark_*" -or $_.Name -like "*_test*" }
    $benchmarks += Get-ChildItem -Path $category_dir -Filter "*.bat" -Recurse | Where-Object { $_.Name -like "benchmark_*" -or $_.Name -like "*_test*" }
    $benchmarks += Get-ChildItem -Path $category_dir -Filter "*.exe" -Recurse | Where-Object { $_.Name -like "benchmark_*" -or $_.Name -like "*_test*" }
    
    if ($benchmarks.Count -eq 0) {
        Log-Warning "No benchmarks found in $category_dir"
        return
    }
    
    foreach ($benchmark in $benchmarks) {
        $benchmark_name = [System.IO.Path]::GetFileNameWithoutExtension($benchmark.Name)
        Log-Info "Running benchmark: $benchmark_name"
        
        # Create benchmark-specific output file
        $bench_output = "$output_dir\${benchmark_name}_results.json"
        $bench_log = "$SESSION_DIR\logs\${Category}_${benchmark_name}.log"
        
        # Run benchmark with timeout and capture output
        $job = Start-Job -ScriptBlock {
            param($BenchmarkPath, $Iterations, $WarmupIterations, $BenchOutput, $Competitors, $TestSize)
            
            $args = @(
                "--iterations", $Iterations,
                "--warmup", $WarmupIterations,
                "--output", $BenchOutput,
                "--competitors", $Competitors,
                "--test-size", $TestSize,
                "--format", "json"
            )
            
            if ($BenchmarkPath.EndsWith(".ps1")) {
                & powershell.exe -File $BenchmarkPath @args
            } elseif ($BenchmarkPath.EndsWith(".bat")) {
                & cmd.exe /c $BenchmarkPath @args
            } else {
                & $BenchmarkPath @args
            }
        } -ArgumentList $benchmark.FullName, $ITERATIONS, $WARMUP_ITERATIONS, $bench_output, $COMPETITORS, $TEST_SIZE
        
        $completed = Wait-Job -Job $job -Timeout $TIMEOUT_SECONDS
        
        if ($completed) {
            $result = Receive-Job -Job $job
            Remove-Job -Job $job
            
            Log-Success "Completed: $benchmark_name"
            
            if ($VERBOSE) {
                Log-Info "Results written to: $bench_output"
            }
            
            # Save job output to log file
            $result | Out-File $bench_log -Encoding UTF8
        } else {
            Stop-Job -Job $job
            Remove-Job -Job $job
            Log-Error "Failed or timed out: $benchmark_name (see $bench_log)"
            "Benchmark timed out after $TIMEOUT_SECONDS seconds" | Out-File $bench_log -Encoding UTF8
        }
    }
}

# Run real-world application benchmarks
function Invoke-RealWorldBenchmarks {
    $real_world_dir = "$PERF_ROOT\real_world"
    $output_dir = "$SESSION_DIR\raw_data\real_world"
    
    Log-Header "Running Real-World Application Benchmarks"
    New-Item -ItemType Directory -Path $output_dir -Force | Out-Null
    
    $applications = @("json_parser", "http_server", "ray_tracer", "compression", "regex_engine")
    
    foreach ($app in $applications) {
        $app_dir = "$real_world_dir\$app"
        
        if (-not (Test-Path $app_dir)) {
            Log-Warning "Real-world benchmark not found: $app"
            continue
        }
        
        Log-Info "Running real-world benchmark: $app"
        
        $app_output = "$output_dir\${app}_results.json"
        $app_log = "$SESSION_DIR\logs\real_world_${app}.log"
        
        # Look for run_benchmark script (PowerShell or batch)
        $run_script = $null
        if (Test-Path "$app_dir\run_benchmark.ps1") {
            $run_script = "$app_dir\run_benchmark.ps1"
        } elseif (Test-Path "$app_dir\run_benchmark.bat") {
            $run_script = "$app_dir\run_benchmark.bat"
        }
        
        if ($run_script) {
            $job = Start-Job -ScriptBlock {
                param($ScriptPath, $Iterations, $AppOutput, $Competitors, $TestSize)
                
                $args = @(
                    "--iterations", $Iterations,
                    "--output", $AppOutput,
                    "--competitors", $Competitors,
                    "--test-size", $TestSize,
                    "--format", "json"
                )
                
                if ($ScriptPath.EndsWith(".ps1")) {
                    & powershell.exe -File $ScriptPath @args
                } else {
                    & cmd.exe /c $ScriptPath @args
                }
            } -ArgumentList $run_script, $ITERATIONS, $app_output, $COMPETITORS, $TEST_SIZE
            
            $completed = Wait-Job -Job $job -Timeout $TIMEOUT_SECONDS
            
            if ($completed) {
                $result = Receive-Job -Job $job
                Remove-Job -Job $job
                Log-Success "Completed real-world benchmark: $app"
                $result | Out-File $app_log -Encoding UTF8
            } else {
                Stop-Job -Job $job
                Remove-Job -Job $job
                Log-Error "Failed real-world benchmark: $app (see $app_log)"
                "Real-world benchmark timed out after $TIMEOUT_SECONDS seconds" | Out-File $app_log -Encoding UTF8
            }
        } else {
            Log-Warning "No run_benchmark script found for $app"
        }
    }
}

# Perform statistical analysis on collected data
function Invoke-StatisticalAnalysis {
    Log-Header "Statistical Analysis"
    
    $analysis_output = "$SESSION_DIR\analysis"
    New-Item -ItemType Directory -Path $analysis_output -Force | Out-Null
    
    # Run comprehensive statistical analysis
    Log-Info "Performing rigorous statistical analysis..."
    
    $analysis_log = "$SESSION_DIR\logs\statistical_analysis.log"
    
    try {
        $result = python "$SCRIPT_DIR\statistical_analysis.py" "$SESSION_DIR\raw_data" --output $analysis_output --min-samples 25 --plot 2>&1
        $result | Out-File $analysis_log -Encoding UTF8
        
        if ($LASTEXITCODE -eq 0) {
            Log-Success "Statistical analysis completed"
            
            # Copy analysis summary to main results
            $analysis_json = "$analysis_output\statistical_analysis.json"
            if (Test-Path $analysis_json) {
                Copy-Item $analysis_json "$SESSION_DIR\"
            }
            return $true
        } else {
            Log-Error "Statistical analysis failed (see logs\statistical_analysis.log)"
            return $false
        }
    } catch {
        Log-Error "Statistical analysis failed: $_"
        return $false
    }
}

# Generate comprehensive performance report
function New-PerformanceReport {
    Log-Header "Generating Performance Report"
    
    $report_output = "$SESSION_DIR\performance_report.md"
    $report_log = "$SESSION_DIR\logs\report_generation.log"
    
    Log-Info "Generating comprehensive Markdown report..."
    
    try {
        $result = python "$SCRIPT_DIR\report_generator.py" --data-dir $SESSION_DIR --output $report_output --format markdown --include-plots --honest-mode 2>&1
        $result | Out-File $report_log -Encoding UTF8
        
        if ($LASTEXITCODE -eq 0) {
            Log-Success "Performance report generated: $report_output"
            
            # Also generate HTML version for web viewing
            $html_output = "$SESSION_DIR\performance_report.html"
            $html_result = python "$SCRIPT_DIR\report_generator.py" --data-dir $SESSION_DIR --output $html_output --format html --include-plots --honest-mode 2>&1
            if ($LASTEXITCODE -eq 0) {
                Log-Success "HTML report also generated: $html_output"
            }
        } else {
            Log-Error "Report generation failed (see logs\report_generation.log)"
        }
    } catch {
        Log-Error "Report generation failed: $_"
    }
}

# Validate performance claims against benchmark data
function Test-PerformanceClaims {
    Log-Header "Validating Performance Claims"
    
    Log-Info "Checking benchmark results against published claims..."
    
    $claims_output = "$SESSION_DIR\claims_validation.json"
    $claims_log = "$SESSION_DIR\logs\claims_validation.log"
    
    try {
        $result = python "$SCRIPT_DIR\validate_claims.py" --benchmark-data "$SESSION_DIR\statistical_analysis.json" --output $claims_output --verbose 2>&1
        $result | Out-File $claims_log -Encoding UTF8
        
        if ($LASTEXITCODE -eq 0) {
            Log-Success "Claims validation completed"
        } else {
            Log-Warning "Some performance claims could not be validated (see logs)"
        }
    } catch {
        Log-Warning "Claims validation failed: $_"
    }
}

# Main execution function
function Main {
    # Handle help and clean options
    if ($Help) {
        Show-Help
        exit 0
    }
    
    if ($Clean) {
        Log-Info "Cleaning previous results..."
        if (Test-Path $RESULTS_DIR) {
            Remove-Item -Path $RESULTS_DIR -Recurse -Force
        }
        Log-Success "Results directory cleaned"
        exit 0
    }
    
    Log-Header "Seen Language Performance Validation Suite"
    Log-Info "Starting comprehensive performance benchmarking..."
    Log-Info "Session: $TIMESTAMP"
    
    # Only run statistical analysis if requested
    if ($STATISTICAL_ONLY) {
        Log-Info "Running statistical analysis only"
        
        # Find latest results directory
        $latest_dir = Get-ChildItem -Path $RESULTS_DIR -Directory -Name "20*" | Sort-Object | Select-Object -Last 1
        if (-not $latest_dir) {
            Log-Error "No previous benchmark results found"
            exit 1
        }
        
        $script:SESSION_DIR = "$RESULTS_DIR\$latest_dir"
        if (Invoke-StatisticalAnalysis) {
            New-PerformanceReport
            Test-PerformanceClaims
        }
        exit 0
    }
    
    # Full benchmark run
    if (-not (Test-Environment)) {
        exit 1
    }
    
    Initialize-Session
    
    # Determine which categories to run
    if ($REAL_WORLD_ONLY) {
        Log-Info "Running real-world benchmarks only"
        Invoke-RealWorldBenchmarks
    } else {
        $categories_to_run = @()
        if ($CATEGORIES -eq "all") {
            $categories_to_run = @("lexer", "parser", "codegen", "runtime", "memory", "reactive")
        } else {
            $categories_to_run = $CATEGORIES -split ',' | ForEach-Object { $_.Trim() }
        }
        
        # Run category benchmarks
        foreach ($category in $categories_to_run) {
            Invoke-CategoryBenchmarks -Category $category
        }
        
        # Also run real-world benchmarks unless specifically excluded
        if ($CATEGORIES -eq "all") {
            Invoke-RealWorldBenchmarks
        }
    }
    
    # Analysis and reporting
    if (-not (Invoke-StatisticalAnalysis)) {
        exit 1
    }
    
    New-PerformanceReport
    Test-PerformanceClaims
    
    # Final summary
    Log-Header "Benchmark Session Complete"
    Log-Success "Results directory: $SESSION_DIR"
    Log-Success "Performance report: $SESSION_DIR\performance_report.md"
    Log-Success "Statistical analysis: $SESSION_DIR\statistical_analysis.json"
    
    # Show quick summary
    $summary_file = "$SESSION_DIR\PERFORMANCE_SUMMARY.md"
    if (Test-Path $summary_file) {
        Write-Host ""
        Write-Host "Quick Performance Summary:" -ForegroundColor $CYAN
        Get-Content $summary_file | Select-Object -First 20
        Write-Host ""
        Write-Host "See full report for detailed analysis: $SESSION_DIR\performance_report.md"
    }
    
    Log-Info "Benchmark validation completed successfully!"
}

# Execute main function
try {
    Main
} catch {
    Log-Error "Script execution failed: $_"
    exit 1
}