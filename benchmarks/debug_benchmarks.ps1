# Debug script to diagnose why Seen and Zig benchmarks aren't showing up
param(
    [switch]$TestSeenOnly,
    [switch]$TestZigOnly
)

$ErrorActionPreference = "Continue"

Write-Host ""
Write-Host "=== BENCHMARK DIAGNOSTICS ===" -ForegroundColor Cyan
Write-Host ""

$ScriptDir = $PSScriptRoot
$ProjectRoot = Split-Path -Parent $ScriptDir

# 1. Check Seen compiler
Write-Host "1. Checking Seen Compiler:" -ForegroundColor Yellow
$SeenExe = Join-Path $ProjectRoot "target\release\seen.exe"
if (!(Test-Path $SeenExe)) {
    $SeenExe = Join-Path $ProjectRoot "target\debug\seen.exe"
}

if (Test-Path $SeenExe) {
    Write-Host "  [OK] Found: $SeenExe" -ForegroundColor Green
    
    # Test if it runs
    Write-Host "  Testing execution..." -ForegroundColor Gray
    & $SeenExe --version 2>&1 | Out-String | Write-Host
    
    # Test arithmetic benchmark
    $ArithPath = Join-Path $ScriptDir "seen_benchmarks\arithmetic.seen"
    if (Test-Path $ArithPath) {
        Write-Host "  [OK] Found arithmetic.seen" -ForegroundColor Green
        Write-Host "  Running test with 1000 iterations..." -ForegroundColor Gray
        
        $output = & $SeenExe run $ArithPath -- "1000" 2>&1 | Out-String
        Write-Host $output
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  [OK] Seen benchmark executed successfully" -ForegroundColor Green
        } else {
            Write-Host "  [ERROR] Seen benchmark failed with exit code: $LASTEXITCODE" -ForegroundColor Red
        }
    } else {
        Write-Host "  [ERROR] arithmetic.seen not found at: $ArithPath" -ForegroundColor Red
    }
} else {
    Write-Host "  [ERROR] Seen compiler not found" -ForegroundColor Red
    Write-Host "  Expected at: $SeenExe" -ForegroundColor Yellow
}

Write-Host ""

# 2. Check Zig
Write-Host "2. Checking Zig:" -ForegroundColor Yellow
if (Get-Command zig -ErrorAction SilentlyContinue) {
    $zigVersion = zig version
    Write-Host "  [OK] Zig found: $zigVersion" -ForegroundColor Green
    
    # Check if Zig benchmark exists
    $ZigSource = Join-Path $ScriptDir "competitors\zig\arithmetic_bench.zig"
    $ZigExe = Join-Path $ScriptDir "competitors\zig\arithmetic_bench.exe"
    
    if (Test-Path $ZigSource) {
        Write-Host "  [OK] Found arithmetic_bench.zig" -ForegroundColor Green
        
        # Try to build
        Write-Host "  Building Zig benchmark..." -ForegroundColor Gray
        Push-Location (Join-Path $ScriptDir "competitors\zig")
        $buildOutput = zig build-exe arithmetic_bench.zig -O ReleaseFast 2>&1 | Out-String
        Pop-Location
        
        if (Test-Path $ZigExe) {
            Write-Host "  [OK] Zig benchmark built successfully" -ForegroundColor Green
            
            # Try to run it
            Write-Host "  Running Zig benchmark..." -ForegroundColor Gray
            & $ZigExe
        } else {
            Write-Host "  [ERROR] Failed to build Zig benchmark" -ForegroundColor Red
            Write-Host "  Build output:" -ForegroundColor Yellow
            Write-Host $buildOutput
        }
    } else {
        Write-Host "  [ERROR] arithmetic_bench.zig not found" -ForegroundColor Red
    }
} else {
    Write-Host "  [WARNING] Zig not installed" -ForegroundColor Yellow
    Write-Host "  Install from: https://ziglang.org/download/" -ForegroundColor Gray
}

Write-Host ""

# 3. Test the main benchmark script's parsing
Write-Host "3. Testing Result Parsing:" -ForegroundColor Yellow

# Create a test output and see if it parses correctly
$testOutput = @"
=== Seen Arithmetic Benchmarks ===
Iterations: 1000

i32_addition: 2500000000.5 ops/sec
i32_multiplication: 2100000000.3 ops/sec
f64_operations: 3200000000.7 billion ops/sec
bitwise_operations: 6800000000.2 ops/sec
"@

Write-Host "  Test output:" -ForegroundColor Gray
Write-Host $testOutput

Write-Host ""
Write-Host "  Parsing test output..." -ForegroundColor Gray

$results = @{}
$testOutput -split "`n" | ForEach-Object {
    if ($_ -match "(\w+):\s*([\d.]+)\s*(.*)ops/sec") {
        $name = $Matches[1]
        $value = [double]$Matches[2]
        $unit = $Matches[3]
        Write-Host "    Parsed: $name = $value $unit" -ForegroundColor Green
        $results[$name] = @{
            value = $value
            unit = $unit + "ops/sec"
        }
    }
}

if ($results.Count -eq 0) {
    Write-Host "  [ERROR] No results parsed from test output" -ForegroundColor Red
} else {
    Write-Host "  [OK] Parsed $($results.Count) results" -ForegroundColor Green
}

Write-Host ""

# 4. Check existing results
Write-Host "4. Checking Existing Results:" -ForegroundColor Yellow
$ResultsDir = Join-Path $ScriptDir "results"
if (Test-Path $ResultsDir) {
    $latestResult = Get-ChildItem $ResultsDir -Filter "*.json" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($latestResult) {
        Write-Host "  Found latest result: $($latestResult.Name)" -ForegroundColor Green
        $json = Get-Content $latestResult.FullName | ConvertFrom-Json
        
        Write-Host "  Languages found in results:" -ForegroundColor Gray
        foreach ($key in $json.benchmarks.PSObject.Properties.Name) {
            $lang = $json.benchmarks.$key.language
            Write-Host "    - $lang ($key)" -ForegroundColor Gray
        }
    } else {
        Write-Host "  No results found" -ForegroundColor Yellow
    }
} else {
    Write-Host "  Results directory doesn't exist" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== DIAGNOSTICS COMPLETE ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Common Issues:" -ForegroundColor Yellow
Write-Host "1. Seen benchmarks may have syntax errors - check compiler output above" -ForegroundColor Gray
Write-Host "2. Zig needs to be installed separately from https://ziglang.org/" -ForegroundColor Gray
Write-Host "3. The benchmark script may not be capturing output correctly" -ForegroundColor Gray
Write-Host ""