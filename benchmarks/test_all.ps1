# Quick test script to verify all components work
param(
    [switch]$FixIssues
)

Write-Host ""
Write-Host "=== BENCHMARK SUITE TEST ===" -ForegroundColor Cyan
Write-Host ""

$ScriptDir = $PSScriptRoot
$Issues = @()

# Test 1: Check Seen benchmarks
Write-Host "1. Testing Seen benchmarks:" -ForegroundColor Yellow
$SeenFiles = @(
    "seen_benchmarks\simple_arithmetic.seen",
    "seen_benchmarks\arithmetic.seen",
    "seen_benchmarks\memory.seen",
    "seen_benchmarks\strings.seen"
)

foreach ($file in $SeenFiles) {
    $path = Join-Path $ScriptDir $file
    if (Test-Path $path) {
        Write-Host "  [OK] $file exists" -ForegroundColor Green
    } else {
        Write-Host "  [MISSING] $file" -ForegroundColor Red
        $Issues += "Missing: $file"
    }
}

# Test 2: Run diagnostic
Write-Host ""
Write-Host "2. Running diagnostics:" -ForegroundColor Yellow
& (Join-Path $ScriptDir "debug_benchmarks.ps1")

# Test 3: Try quick benchmark
Write-Host ""
Write-Host "3. Running quick benchmark test:" -ForegroundColor Yellow
Write-Host "  This will run with minimal iterations..." -ForegroundColor Gray
Write-Host ""

$TestResult = & (Join-Path $ScriptDir "run_all_benchmarks.ps1") -QuickTest -SkipBuild

# Check if results were generated
$ResultsDir = Join-Path $ScriptDir "results"
$LatestResult = Get-ChildItem $ResultsDir -Filter "*.json" | Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($LatestResult) {
    Write-Host ""
    Write-Host "4. Analyzing results:" -ForegroundColor Yellow
    
    $json = Get-Content $LatestResult.FullName | ConvertFrom-Json
    $languages = @{}
    
    foreach ($key in $json.benchmarks.PSObject.Properties.Name) {
        $lang = $json.benchmarks.$key.language
        if ($lang) {
            $languages[$lang] = $true
        }
    }
    
    Write-Host "  Languages found in results:" -ForegroundColor Gray
    foreach ($lang in $languages.Keys) {
        Write-Host "    [OK] $lang" -ForegroundColor Green
    }
    
    $missing = @("Seen", "Rust", "C++", "Zig") | Where-Object { -not $languages.ContainsKey($_) }
    if ($missing.Count -gt 0) {
        Write-Host "  Missing languages:" -ForegroundColor Yellow
        foreach ($lang in $missing) {
            Write-Host "    [MISSING] $lang" -ForegroundColor Red
            $Issues += "Missing results for: $lang"
        }
    }
}

Write-Host ""
Write-Host "=== TEST SUMMARY ===" -ForegroundColor Cyan

if ($Issues.Count -eq 0) {
    Write-Host "[OK] All tests passed!" -ForegroundColor Green
} else {
    Write-Host "[ISSUES] Found $($Issues.Count) issues:" -ForegroundColor Yellow
    foreach ($issue in $Issues) {
        Write-Host "  - $issue" -ForegroundColor Red
    }
    
    if ($FixIssues) {
        Write-Host ""
        Write-Host "Attempting to fix issues..." -ForegroundColor Cyan
        
        # Install Zig if missing
        if ($Issues -match "Zig") {
            Write-Host "  To install Zig:" -ForegroundColor Yellow
            Write-Host "  1. Download from https://ziglang.org/download/" -ForegroundColor Gray
            Write-Host "  2. Extract to C:\zig" -ForegroundColor Gray
            Write-Host "  3. Add C:\zig to PATH" -ForegroundColor Gray
        }
        
        # Build competitors
        Write-Host ""
        Write-Host "  Building competitors..." -ForegroundColor Yellow
        & (Join-Path $ScriptDir "build_competitors.ps1")
    }
}

Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "1. Fix any issues listed above" -ForegroundColor White
Write-Host "2. Run: .\run_all_benchmarks.ps1" -ForegroundColor White  
Write-Host "3. View the generated HTML report" -ForegroundColor White
Write-Host ""