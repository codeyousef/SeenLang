# Quick benchmark runner for Windows - simplified version
# Run a quick performance test with minimal configuration

param(
    [string]$Test = "arithmetic",
    [switch]$CompareOnly
)

$ErrorActionPreference = "Stop"

# Setup paths
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$ResultsDir = Join-Path $ScriptDir "results"

# Ensure results directory exists
if (!(Test-Path $ResultsDir)) {
    New-Item -ItemType Directory -Path $ResultsDir | Out-Null
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "     SEEN QUICK BENCHMARK v1.0" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

# Function to measure execution time
function Measure-Execution {
    param(
        [string]$Name,
        [string]$Command,
        [int]$Iterations = 1000
    )
    
    Write-Host "Testing: $Name" -ForegroundColor Yellow
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    
    for ($i = 1; $i -le $Iterations; $i++) {
        Invoke-Expression $Command | Out-Null
    }
    
    $stopwatch.Stop()
    $msPerOp = $stopwatch.ElapsedMilliseconds / $Iterations
    $opsPerSec = 1000 / $msPerOp
    
    return @{
        Name = $Name
        TotalMs = $stopwatch.ElapsedMilliseconds
        MsPerOp = [Math]::Round($msPerOp, 3)
        OpsPerSec = [Math]::Round($opsPerSec, 0)
        Iterations = $Iterations
    }
}

if (!$CompareOnly) {
    Write-Host "Running quick benchmarks for: $Test" -ForegroundColor Green
    Write-Host "=================================" -ForegroundColor Gray
    Write-Host ""
    
    $results = @()
    
    switch ($Test.ToLower()) {
        "arithmetic" {
            # Test 1: Simple arithmetic in PowerShell (baseline)
            $result1 = Measure-Execution -Name "PowerShell Arithmetic" -Iterations 10000 -Command {
                $sum = 0
                for ($i = 0; $i -lt 1000; $i++) {
                    $sum += $i * 2
                }
            }
            $results += $result1
            
            # Test 2: Array operations
            $result2 = Measure-Execution -Name "PowerShell Array Ops" -Iterations 1000 -Command {
                $arr = @(1..1000)
                $sum = ($arr | Measure-Object -Sum).Sum
            }
            $results += $result2
        }
        
        "string" {
            # Test 1: String concatenation
            $result1 = Measure-Execution -Name "String Concatenation" -Iterations 1000 -Command {
                $str = ""
                for ($i = 0; $i -lt 100; $i++) {
                    $str += "test"
                }
            }
            $results += $result1
            
            # Test 2: String builder
            $result2 = Measure-Execution -Name "StringBuilder" -Iterations 1000 -Command {
                $sb = [System.Text.StringBuilder]::new()
                for ($i = 0; $i -lt 100; $i++) {
                    [void]$sb.Append("test")
                }
                $str = $sb.ToString()
            }
            $results += $result2
        }
        
        "file" {
            # Test 1: File write
            $tempFile = Join-Path $env:TEMP "bench_test.txt"
            $result1 = Measure-Execution -Name "File Write" -Iterations 100 -Command {
                "Test data for benchmark" | Out-File $tempFile -Force
            }
            $results += $result1
            
            # Test 2: File read
            $result2 = Measure-Execution -Name "File Read" -Iterations 100 -Command {
                $content = Get-Content $tempFile
            }
            $results += $result2
            
            # Cleanup
            Remove-Item $tempFile -Force -ErrorAction SilentlyContinue
        }
        
        default {
            Write-Host "Unknown test: $Test" -ForegroundColor Red
            Write-Host "Available tests: arithmetic, string, file" -ForegroundColor Yellow
            exit 1
        }
    }
    
    # Display results
    Write-Host ""
    Write-Host "RESULTS:" -ForegroundColor Green
    Write-Host "--------" -ForegroundColor Gray
    
    $results | ForEach-Object {
        Write-Host ""
        Write-Host "$($_.Name):" -ForegroundColor Yellow
        Write-Host "  Total time:     $($_.TotalMs) ms" -ForegroundColor White
        Write-Host "  Iterations:     $($_.Iterations)" -ForegroundColor White
        Write-Host "  Time per op:    $($_.MsPerOp) ms" -ForegroundColor Cyan
        Write-Host "  Ops per second: $($_.OpsPerSec)" -ForegroundColor Green
    }
    
    # Save results to JSON
    $jsonFile = Join-Path $ResultsDir "quick_bench_$(Get-Date -Format 'yyyyMMdd_HHmmss').json"
    $results | ConvertTo-Json -Depth 10 | Out-File $jsonFile
    
    Write-Host ""
    Write-Host "Results saved to: $jsonFile" -ForegroundColor Gray
}

# Run competitor comparison if available
Write-Host ""
Write-Host "COMPETITOR COMPARISON:" -ForegroundColor Green
Write-Host "---------------------" -ForegroundColor Gray

$competitors = @()

# Check for Rust benchmark
$rustExe = Join-Path $ScriptDir "competitors\rust\target\release\arithmetic_bench.exe"
if (Test-Path $rustExe) {
    Write-Host "[OK] Rust benchmark found" -ForegroundColor Green
    $rustOutput = & $rustExe 2>&1
    $competitors += @{
        Language = "Rust"
        Output = $rustOutput
    }
} else {
    Write-Host "[ERROR] Rust benchmark not found" -ForegroundColor Yellow
}

# Check for C++ benchmark
$cppExe = Join-Path $ScriptDir "competitors\cpp\arithmetic_bench.exe"
if (Test-Path $cppExe) {
    Write-Host "[OK] C++ benchmark found" -ForegroundColor Green
    $cppOutput = & $cppExe 2>&1
    $competitors += @{
        Language = "C++"
        Output = $cppOutput
    }
} else {
    Write-Host "[ERROR] C++ benchmark not found" -ForegroundColor Yellow
}

# Check for Zig benchmark
$zigExe = Join-Path $ScriptDir "competitors\zig\arithmetic_bench.exe"
if (Test-Path $zigExe) {
    Write-Host "[OK] Zig benchmark found" -ForegroundColor Green
    $zigOutput = & $zigExe 2>&1
    $competitors += @{
        Language = "Zig"
        Output = $zigOutput
    }
} else {
    Write-Host "[ERROR] Zig benchmark not found" -ForegroundColor Yellow
}

if ($competitors.Count -gt 0) {
    Write-Host ""
    Write-Host "Competitor Results:" -ForegroundColor Cyan
    foreach ($comp in $competitors) {
        Write-Host ""
        Write-Host "[$($comp.Language)]" -ForegroundColor Yellow
        Write-Host $comp.Output
    }
} else {
    Write-Host ""
    Write-Host "No competitor benchmarks available." -ForegroundColor Yellow
    Write-Host "Run '.\build_competitors.ps1' to build them." -ForegroundColor Gray
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "        BENCHMARK COMPLETE" -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""