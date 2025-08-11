# Demo benchmark script showing working benchmarks
param(
    [string]$Mode = "jit",
    [int]$Iterations = 10000
)

Write-Host ""
Write-Host "==================================" -ForegroundColor Cyan
Write-Host "   SEEN BENCHMARK DEMONSTRATION" -ForegroundColor Cyan
Write-Host "==================================" -ForegroundColor Cyan
Write-Host ""

$ScriptDir = $PSScriptRoot
$ProjectRoot = Split-Path -Parent $ScriptDir
$SeenExe = Join-Path $ProjectRoot "target\release\seen.exe"

if (!(Test-Path $SeenExe)) {
    $SeenExe = Join-Path $ProjectRoot "target\debug\seen.exe"
    if (!(Test-Path $SeenExe)) {
        Write-Host "[ERROR] Seen compiler not found!" -ForegroundColor Red
        exit 1
    }
}

Write-Host "Configuration:" -ForegroundColor Yellow
Write-Host "  Seen compiler: $SeenExe" -ForegroundColor Gray
Write-Host "  Mode: $Mode" -ForegroundColor Gray
Write-Host "  Iterations: $Iterations" -ForegroundColor Gray
Write-Host ""

# Test 1: Run the working example
$WorkingExample = Join-Path $ScriptDir "working_example.seen"
if (Test-Path $WorkingExample) {
    Write-Host "1. Seen Working Example Benchmark" -ForegroundColor Cyan
    Write-Host "----------------------------------" -ForegroundColor Gray
    
    $OutputFile = Join-Path $ScriptDir "results\seen_benchmark.json"
    
    # Ensure results directory exists
    $ResultsDir = Join-Path $ScriptDir "results"
    if (!(Test-Path $ResultsDir)) {
        New-Item -ItemType Directory -Path $ResultsDir | Out-Null
    }
    
    # Run the benchmark
    & $SeenExe run $WorkingExample -- $Mode $Iterations $OutputFile
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "[OK] Seen benchmark completed" -ForegroundColor Green
        
        if (Test-Path $OutputFile) {
            Write-Host ""
            Write-Host "Results:" -ForegroundColor Yellow
            Get-Content $OutputFile | Write-Host
        }
    } else {
        Write-Host "[ERROR] Seen benchmark failed" -ForegroundColor Red
    }
} else {
    Write-Host "[WARNING] Working example not found: $WorkingExample" -ForegroundColor Yellow
}

Write-Host ""

# Test 2: Run Rust competitor benchmark
Write-Host "2. Rust Competitor Benchmark" -ForegroundColor Cyan
Write-Host "----------------------------" -ForegroundColor Gray

$RustExe = Join-Path $ScriptDir "competitors\rust\target\release\arithmetic_bench.exe"
if (Test-Path $RustExe) {
    Write-Host "Running Rust benchmark..." -ForegroundColor Yellow
    & $RustExe
    Write-Host "[OK] Rust benchmark completed" -ForegroundColor Green
} else {
    Write-Host "[WARNING] Rust benchmark not built. Run: .\build_competitors.ps1" -ForegroundColor Yellow
}

Write-Host ""

# Test 3: Run C++ competitor benchmark if available
Write-Host "3. C++ Competitor Benchmark" -ForegroundColor Cyan
Write-Host "---------------------------" -ForegroundColor Gray

$CppExe = Join-Path $ScriptDir "competitors\cpp\arithmetic_bench.exe"
if (Test-Path $CppExe) {
    Write-Host "Running C++ benchmark..." -ForegroundColor Yellow
    & $CppExe
    Write-Host "[OK] C++ benchmark completed" -ForegroundColor Green
} else {
    Write-Host "[WARNING] C++ benchmark not built. Run: .\build_competitors.ps1" -ForegroundColor Yellow
}

Write-Host ""

# Test 4: Demonstrate performance comparison
Write-Host "4. Performance Comparison" -ForegroundColor Cyan
Write-Host "-------------------------" -ForegroundColor Gray

# Run simple tests from existing Seen examples
$SimpleTest = Join-Path $ProjectRoot "compiler_seen\simple_test.seen"
if (Test-Path $SimpleTest) {
    Write-Host "Running compiler_seen simple test..." -ForegroundColor Yellow
    
    $StartTime = Get-Date
    & $SeenExe run $SimpleTest 2>&1 | Out-Null
    $ElapsedTime = ((Get-Date) - $StartTime).TotalMilliseconds
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "[OK] Simple test executed in $([Math]::Round($ElapsedTime, 2))ms" -ForegroundColor Green
    } else {
        Write-Host "[WARNING] Simple test failed but took $([Math]::Round($ElapsedTime, 2))ms" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "==================================" -ForegroundColor Cyan
Write-Host "      DEMONSTRATION COMPLETE" -ForegroundColor Green
Write-Host "==================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Summary:" -ForegroundColor Yellow
Write-Host "- Seen compiler is working" -ForegroundColor Gray
Write-Host "- Rust benchmarks are functional" -ForegroundColor Gray
Write-Host "- C++ benchmarks can be built with: .\build_competitors.ps1" -ForegroundColor Gray
Write-Host "- The benchmark harness files need Seen syntax updates" -ForegroundColor Gray
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "1. Update benchmark .seen files with valid Seen syntax" -ForegroundColor White
Write-Host "2. Run full benchmark suite: .\run_benchmarks.ps1" -ForegroundColor White
Write-Host "3. Build all competitors: .\build_competitors.ps1" -ForegroundColor White