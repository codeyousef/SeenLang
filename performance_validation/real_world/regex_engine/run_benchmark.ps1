# Regex Engine Benchmark Script
param(
    [int]$Iterations = 5
)

$REGEX_ENGINE_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$SEEN_CLI = "$PSScriptRoot\..\..\..\target\debug\seen.exe"
if (-not (Test-Path $SEEN_CLI)) {
    $SEEN_CLI = "$PSScriptRoot\..\..\..\target\release\seen.exe"
}
if (-not (Test-Path $SEEN_CLI)) {
    $SEEN_CLI = "$PSScriptRoot\..\..\..\target-wsl\debug\seen"
}
if (-not (Test-Path $SEEN_CLI)) {
    $SEEN_CLI = "$PSScriptRoot\..\..\..\target-wsl\release\seen"
}

Write-Host "Running Regex Engine Benchmark..." -ForegroundColor Cyan

# Define benchmark results
$cppResults = @()
$rustResults = @()
$seenResults = @()

# Run C++ version
$CPP_EXE = "$PSScriptRoot\..\..\benchmarks\real_implementations\regex_bench_cpp.exe"
if (Test-Path $CPP_EXE) {
    Write-Host "Running C++ regex engine..." -ForegroundColor Blue
    
    for ($i = 0; $i -lt $Iterations; $i++) {
        # Always run C++ executable via WSL
        $wslPath = $CPP_EXE -replace '\\', '/' -replace '^([A-Za-z]):', '/mnt/$1'
        $wslPath = $wslPath.ToLower()
        Write-Host "Executing C++ via WSL: $wslPath" -ForegroundColor Gray
        $output = wsl bash -c "`"$wslPath`"" 2>&1
        $outputStr = ($output -join " ").Trim()
        Write-Host "C++ output: '$outputStr'" -ForegroundColor Gray
        
        if ($outputStr -match "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)") {
            $cppResults += @{
                "match_time_ms" = [double]$matches[1]
                "matches_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "compile_time_ms" = [double]$matches[4]
            }
        }
    }
    
    if ($cppResults.Count -gt 0) {
        $avgTime = ($cppResults | ForEach-Object { $_.match_time_ms } | Measure-Object -Average).Average
        $avgMps = ($cppResults | ForEach-Object { $_.matches_per_sec } | Measure-Object -Average).Average
        Write-Host "C++ Average: $([math]::Round($avgTime, 1)) ms, $([math]::Round($avgMps / 1000, 1))K matches/s" -ForegroundColor Green
    }
}

# Run Rust version
$RUST_EXE = "$PSScriptRoot\..\..\benchmarks\real_implementations\regex_bench_rust.exe"
if (Test-Path $RUST_EXE) {
    Write-Host "Running Rust regex engine..." -ForegroundColor Blue
    
    for ($i = 0; $i -lt $Iterations; $i++) {
        # Always run Rust executable via WSL
        $wslPath = $RUST_EXE -replace '\\', '/' -replace '^([A-Za-z]):', '/mnt/$1'
        $wslPath = $wslPath.ToLower()
        Write-Host "Executing Rust via WSL: $wslPath" -ForegroundColor Gray
        $output = wsl bash -c "`"$wslPath`"" 2>&1
        $outputStr = ($output -join " ").Trim()
        Write-Host "Rust output: '$outputStr'" -ForegroundColor Gray
        
        if ($outputStr -match "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)") {
            $rustResults += @{
                "match_time_ms" = [double]$matches[1]
                "matches_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "compile_time_ms" = [double]$matches[4]
            }
        }
    }
    
    if ($rustResults.Count -gt 0) {
        $avgTime = ($rustResults | ForEach-Object { $_.match_time_ms } | Measure-Object -Average).Average
        $avgMps = ($rustResults | ForEach-Object { $_.matches_per_sec } | Measure-Object -Average).Average
        Write-Host "Rust Average: $([math]::Round($avgTime, 1)) ms, $([math]::Round($avgMps / 1000, 1))K matches/s" -ForegroundColor Green
    }
}

# Run Seen version
$SEEN_EXECUTABLE = "$REGEX_ENGINE_DIR\regex_engine_benchmark\target\native\debug\regex_engine_benchmark"

# Try to build if executable doesn't exist
if (-not (Test-Path $SEEN_EXECUTABLE)) {
    Write-Host "Building Seen regex engine benchmark..." -ForegroundColor Yellow
    Push-Location "$REGEX_ENGINE_DIR\regex_engine_benchmark"
    & $SEEN_CLI build 2>&1 | Out-Null
    Pop-Location
}

if (Test-Path $SEEN_EXECUTABLE) {
    Write-Host "Running Seen regex engine..." -ForegroundColor Blue
    
    for ($i = 0; $i -lt $Iterations; $i++) {
        # Use WSL to execute the Linux binary if it doesn't have .exe extension
        if ($SEEN_EXECUTABLE -notlike "*.exe") {
            # Manual path conversion from Windows to WSL format
            $wslPath = $SEEN_EXECUTABLE -replace '\\', '/' -replace '^([A-Za-z]):', '/mnt/$1'
            $wslPath = $wslPath.ToLower()
            Write-Host "Executing via WSL: $wslPath" -ForegroundColor Gray
            $output = wsl bash -c "`"$wslPath`"" 2>&1
            $exitCode = $LASTEXITCODE
            Write-Host "WSL output: $($output -join ' ')" -ForegroundColor Gray
            Write-Host "WSL exit code: $exitCode" -ForegroundColor Gray
        } else {
            $output = & $SEEN_EXECUTABLE 2>&1
            $exitCode = $LASTEXITCODE
            Write-Host "Exit code: $exitCode" -ForegroundColor Gray
        }
        
        $outputStr = $output -join " "
        Write-Host "Raw output string: '$outputStr'" -ForegroundColor Gray
        
        if ($exitCode -eq 0 -and $outputStr -match "(\d+\.?\d*)\s+(\d+\.?\d*)\s+(\d+\.?\d*)\s+(\d+\.?\d*)") {
            $seenResults += @{
                "match_time_ms" = [double]$matches[1]
                "matches_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "compile_time_ms" = [double]$matches[4]
            }
            Write-Host "Successfully parsed iteration $($i + 1): $([double]$matches[1])ms match, $([double]$matches[2]) matches/s" -ForegroundColor Green
        } else {
            Write-Host "Failed to parse output on iteration $($i + 1). Exit code: $exitCode, Output: '$outputStr'" -ForegroundColor Red
        }
    }
    
    if ($seenResults.Count -gt 0) {
        $avgTime = ($seenResults | ForEach-Object { $_.match_time_ms } | Measure-Object -Average).Average
        $avgMps = ($seenResults | ForEach-Object { $_.matches_per_sec } | Measure-Object -Average).Average
        Write-Host "Seen Average: $([math]::Round($avgTime, 1)) ms, $([math]::Round($avgMps / 1000, 1))K matches/s" -ForegroundColor Green
    }
}

# Save results
$results = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = "regex_engine"
        iterations = $Iterations
    }
    benchmarks = @{}
}

if ($cppResults.Count -gt 0) {
    $results.benchmarks.cpp = @{
        "match_time_ms" = $cppResults | ForEach-Object { $_.match_time_ms }
        "matches_per_sec" = $cppResults | ForEach-Object { $_.matches_per_sec }
        "memory_mb" = $cppResults | ForEach-Object { $_.memory_mb }
        "compile_time_ms" = $cppResults | ForEach-Object { $_.compile_time_ms }
    }
}

if ($rustResults.Count -gt 0) {
    $results.benchmarks.rust = @{
        "match_time_ms" = $rustResults | ForEach-Object { $_.match_time_ms }
        "matches_per_sec" = $rustResults | ForEach-Object { $_.matches_per_sec }
        "memory_mb" = $rustResults | ForEach-Object { $_.memory_mb }
        "compile_time_ms" = $rustResults | ForEach-Object { $_.compile_time_ms }
    }
}

if ($seenResults.Count -gt 0) {
    $results.benchmarks.seen = @{
        "match_time_ms" = $seenResults | ForEach-Object { $_.match_time_ms }
        "matches_per_sec" = $seenResults | ForEach-Object { $_.matches_per_sec }
        "memory_mb" = $seenResults | ForEach-Object { $_.memory_mb }
        "compile_time_ms" = $seenResults | ForEach-Object { $_.compile_time_ms }
    }
}

$results | ConvertTo-Json -Depth 10 | Out-File -FilePath "regex_engine_results.json" -Encoding UTF8
Write-Host "Results saved to regex_engine_results.json" -ForegroundColor Green
Write-Output "Results saved to regex_engine_results.json"  # For background job capture