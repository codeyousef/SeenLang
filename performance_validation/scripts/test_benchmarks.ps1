# Quick test script to verify all benchmarks are working
param(
    [switch]$Quick
)

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PERF_ROOT = (Get-Item "$SCRIPT_DIR\..").FullName

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Testing Performance Validation Suite" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$iterations = if ($Quick) { 2 } else { 5 }
$benchmarks = @(
    @{Name="Lexer"; Path="$PERF_ROOT\benchmarks\lexer\run_real_benchmark.ps1"},
    @{Name="Parser"; Path="$PERF_ROOT\benchmarks\parser\run_real_benchmark.ps1"},
    @{Name="Codegen"; Path="$PERF_ROOT\benchmarks\codegen\run_real_benchmark.ps1"},
    @{Name="Runtime"; Path="$PERF_ROOT\benchmarks\runtime\run_real_benchmark.ps1"},
    @{Name="Memory"; Path="$PERF_ROOT\benchmarks\memory\run_real_benchmark.ps1"},
    @{Name="Reactive"; Path="$PERF_ROOT\benchmarks\reactive\run_real_benchmark.ps1"}
)

$passed = 0
$failed = 0

foreach ($bench in $benchmarks) {
    Write-Host "Testing $($bench.Name) benchmark..." -ForegroundColor Yellow
    
    if (Test-Path $bench.Path) {
        try {
            $output = & $bench.Path -Iterations $iterations 2>&1
            $jsonFile = Split-Path $bench.Path -Parent
            $resultFiles = @(
                "$jsonFile\real_$($bench.Name.ToLower())_results.json",
                "$jsonFile\$($bench.Name.ToLower())_results.json",
                "$jsonFile\reactive_results.json"
            )
            
            $foundResult = $false
            foreach ($resultFile in $resultFiles) {
                if (Test-Path $resultFile) {
                    $content = Get-Content $resultFile -Raw | ConvertFrom-Json
                    if ($content.benchmarks -and $content.benchmarks.Count -gt 0) {
                        Write-Host "  ✓ $($bench.Name) benchmark passed" -ForegroundColor Green
                        $passed++
                        $foundResult = $true
                        break
                    }
                }
            }
            
            if (-not $foundResult) {
                Write-Host "  ✗ $($bench.Name) benchmark failed - no valid results" -ForegroundColor Red
                $failed++
            }
        } catch {
            Write-Host "  ✗ $($bench.Name) benchmark failed - $_" -ForegroundColor Red
            $failed++
        }
    } else {
        Write-Host "  ✗ $($bench.Name) benchmark script not found" -ForegroundColor Red
        $failed++
    }
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Test Results" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Passed: $passed" -ForegroundColor Green
Write-Host "Failed: $failed" -ForegroundColor $(if ($failed -gt 0) { "Red" } else { "Green" })

if ($failed -eq 0) {
    Write-Host ""
    Write-Host "All benchmarks are working correctly!" -ForegroundColor Green
    Write-Host "You can now run: .\scripts\run_all.ps1" -ForegroundColor Yellow
} else {
    Write-Host ""
    Write-Host "Some benchmarks failed. Please check the output above." -ForegroundColor Red
    exit 1
}