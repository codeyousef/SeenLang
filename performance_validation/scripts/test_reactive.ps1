# Quick test for reactive benchmark
param(
    [int]$Iterations = 100,
    [int]$DataSize = 100
)

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PERF_ROOT = (Get-Item "$SCRIPT_DIR\..").FullName

Write-Host "Testing Reactive Benchmark Fix" -ForegroundColor Cyan
Write-Host ""

# Run the reactive benchmark with small data size for testing
& "$PERF_ROOT\benchmarks\reactive\run_real_benchmark.ps1" -Iterations $Iterations -DataSize $DataSize

# Check if results were generated
$resultFile = "$PERF_ROOT\benchmarks\reactive\reactive_results.json"
if (Test-Path $resultFile) {
    $results = Get-Content $resultFile -Raw | ConvertFrom-Json
    
    Write-Host ""
    Write-Host "Results Summary:" -ForegroundColor Yellow
    
    foreach ($lang in $results.benchmarks.PSObject.Properties.Name) {
        $bench = $results.benchmarks.$lang
        if ($bench.results -and $bench.results.overhead_percent) {
            $overhead = [math]::Round($bench.results.overhead_percent, 2)
            $status = if ([math]::Abs($overhead) -lt 5) { "ZERO-COST" } else { "HAS OVERHEAD" }
            $color = if ([math]::Abs($overhead) -lt 5) { "Green" } else { "Yellow" }
            
            Write-Host "$($lang.ToUpper()): $status ($overhead%)" -ForegroundColor $color
        }
    }
} else {
    Write-Host "No results file found!" -ForegroundColor Red
}