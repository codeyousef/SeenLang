# Run all real-world benchmarks
# This script runs each real-world benchmark and collects results

param(
    [int]$Iterations = 5,
    [switch]$Verbose
)

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = (Get-Item "$SCRIPT_DIR\..").FullName

Write-Host "Running Real-World Benchmarks with $Iterations iterations..." -ForegroundColor Cyan
Write-Host "=" * 60 -ForegroundColor DarkGray

$benchmarks = @(
    @{Name="Compression"; Path="$PROJECT_ROOT\real_world\compression"},
    @{Name="HTTP Server"; Path="$PROJECT_ROOT\real_world\http_server"},
    @{Name="Ray Tracer"; Path="$PROJECT_ROOT\real_world\ray_tracer"},
    @{Name="Regex Engine"; Path="$PROJECT_ROOT\real_world\regex_engine"}
)

$allResults = @{}
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"

foreach ($benchmark in $benchmarks) {
    Write-Host "`n[BENCHMARK] $($benchmark.Name)" -ForegroundColor Blue
    Write-Host "-" * 40 -ForegroundColor DarkGray
    
    $benchmarkDir = $benchmark.Path
    $runScript = "$benchmarkDir\run_benchmark.ps1"
    
    if (Test-Path $runScript) {
        Set-Location $benchmarkDir
        
        $outputFile = "$($benchmark.Name.ToLower() -replace ' ', '_')_results.json"
        
        Write-Host "Running benchmark script..." -ForegroundColor Yellow
        
        if ($Verbose) {
            & $runScript -Iterations $Iterations -Verbose -Output $outputFile
        } else {
            & $runScript -Iterations $Iterations -Output $outputFile
        }
        
        if (Test-Path $outputFile) {
            $results = Get-Content $outputFile | ConvertFrom-Json
            $allResults[$benchmark.Name] = $results
            
            # Display summary
            if ($results.benchmarks) {
                Write-Host "`nResults Summary:" -ForegroundColor Green
                foreach ($lang in $results.benchmarks.PSObject.Properties.Name) {
                    $langResults = $results.benchmarks.$lang
                    Write-Host "  $lang : Found $($langResults.PSObject.Properties.Count) metrics" -ForegroundColor White
                }
            }
        } else {
            Write-Host "[WARNING] No results file generated for $($benchmark.Name)" -ForegroundColor Yellow
        }
    } else {
        Write-Host "[ERROR] Benchmark script not found: $runScript" -ForegroundColor Red
    }
}

# Save combined results
$combinedResultsPath = "$PROJECT_ROOT\results\real_world_combined_$timestamp.json"
New-Item -ItemType Directory -Path "$PROJECT_ROOT\results" -Force | Out-Null
$allResults | ConvertTo-Json -Depth 10 | Out-File -FilePath $combinedResultsPath -Encoding UTF8

Write-Host "`n" + "=" * 60 -ForegroundColor DarkGray
Write-Host "All real-world benchmarks completed!" -ForegroundColor Green
Write-Host "Combined results saved to: $combinedResultsPath" -ForegroundColor Cyan

# Generate performance report
Write-Host "`nGenerating performance report..." -ForegroundColor Yellow
$reportScript = "$SCRIPT_DIR\report_generator.py"
if (Test-Path $reportScript) {
    python $reportScript $combinedResultsPath
} else {
    Write-Host "[WARNING] Report generator not found" -ForegroundColor Yellow
}

Write-Host "`nBenchmark run complete!" -ForegroundColor Green