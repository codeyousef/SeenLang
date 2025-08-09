# Binary Trees Benchmark Script
param(
    [int]$Iterations = 5
)

$BINARY_TREES_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
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

Write-Host "Running Binary Trees Benchmark..." -ForegroundColor Cyan

# Define benchmark results
$cppResults = @()
$rustResults = @()
$seenResults = @()

# Run C++ version
$CPP_EXE = "$PSScriptRoot\..\..\benchmarks\real_implementations\binary_trees_bench_cpp.exe"
if (Test-Path $CPP_EXE) {
    Write-Host "Running C++ binary trees..." -ForegroundColor Blue
    
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
                "creation_time_ms" = [double]$matches[1]
                "allocations_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "checksum" = [double]$matches[4]
            }
        }
    }
    
    if ($cppResults.Count -gt 0) {
        $avgTime = ($cppResults | ForEach-Object { $_.creation_time_ms } | Measure-Object -Average).Average
        $avgAllocs = ($cppResults | ForEach-Object { $_.allocations_per_sec } | Measure-Object -Average).Average
        Write-Host "C++ Average: $([math]::Round($avgTime, 2)) ms, $([math]::Round($avgAllocs / 1000000, 1))M allocs/s" -ForegroundColor Green
    }
}

# Run Rust version
$RUST_EXE = "$PSScriptRoot\..\..\benchmarks\real_implementations\binary_trees_bench_rust.exe"
if (Test-Path $RUST_EXE) {
    Write-Host "Running Rust binary trees..." -ForegroundColor Blue
    
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
                "creation_time_ms" = [double]$matches[1]
                "allocations_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "checksum" = [double]$matches[4]
            }
        }
    }
    
    if ($rustResults.Count -gt 0) {
        $avgTime = ($rustResults | ForEach-Object { $_.creation_time_ms } | Measure-Object -Average).Average
        $avgAllocs = ($rustResults | ForEach-Object { $_.allocations_per_sec } | Measure-Object -Average).Average
        Write-Host "Rust Average: $([math]::Round($avgTime, 2)) ms, $([math]::Round($avgAllocs / 1000000, 1))M allocs/s" -ForegroundColor Green
    }
}

# Run Seen version
$SEEN_EXECUTABLE = "$BINARY_TREES_DIR\binary_trees_benchmark\target\native\debug\binary_trees_benchmark"

# Try to build if executable doesn't exist
if (-not (Test-Path $SEEN_EXECUTABLE)) {
    Write-Host "Building Seen binary trees benchmark..." -ForegroundColor Yellow
    Push-Location "$BINARY_TREES_DIR\binary_trees_benchmark"
    & $SEEN_CLI build 2>&1 | Out-Null
    Pop-Location
}

if (Test-Path $SEEN_EXECUTABLE) {
    Write-Host "Running Seen binary trees..." -ForegroundColor Blue
    
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
        
        # Extract only the numeric line from output (binary_trees outputs 4 values)
        $numericLine = $output | Where-Object { $_ -match "^\s*([\d.eE+-]+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s*$" }
        Write-Host "Found numeric line: '$numericLine'" -ForegroundColor Gray
        if ($exitCode -eq 0 -and $numericLine -and $numericLine -match "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)") {
            $seenResults += @{
                "creation_time_ms" = [double]$matches[1]
                "allocations_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "checksum" = [double]$matches[4]
            }
            Write-Host "Successfully parsed iteration $($i + 1): $([double]$matches[1])ms creation, $([double]$matches[2]) allocs/s" -ForegroundColor Green
        } else {
            Write-Host "Failed to parse output on iteration $($i + 1). Exit code: $exitCode, Output: '$outputStr'" -ForegroundColor Red
        }
    }
    
    if ($seenResults.Count -gt 0) {
        $avgTime = ($seenResults | ForEach-Object { $_.creation_time_ms } | Measure-Object -Average).Average
        $avgAllocs = ($seenResults | ForEach-Object { $_.allocations_per_sec } | Measure-Object -Average).Average
        Write-Host "Seen Average: $([math]::Round($avgTime, 2)) ms, $([math]::Round($avgAllocs / 1000000, 1))M allocs/s" -ForegroundColor Green
    }
}

# Save results
$results = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = "binary_trees"
        iterations = $Iterations
    }
    benchmarks = @{}
}

if ($cppResults.Count -gt 0) {
    $results.benchmarks.cpp = @{
        "creation_time_ms" = $cppResults | ForEach-Object { $_.creation_time_ms }
        "allocations_per_sec" = $cppResults | ForEach-Object { $_.allocations_per_sec }
        "memory_mb" = $cppResults | ForEach-Object { $_.memory_mb }
        "checksum" = $cppResults | ForEach-Object { $_.checksum }
    }
}

if ($rustResults.Count -gt 0) {
    $results.benchmarks.rust = @{
        "creation_time_ms" = $rustResults | ForEach-Object { $_.creation_time_ms }
        "allocations_per_sec" = $rustResults | ForEach-Object { $_.allocations_per_sec }
        "memory_mb" = $rustResults | ForEach-Object { $_.memory_mb }
        "checksum" = $rustResults | ForEach-Object { $_.checksum }
    }
}

if ($seenResults.Count -gt 0) {
    $results.benchmarks.seen = @{
        "creation_time_ms" = $seenResults | ForEach-Object { $_.creation_time_ms }
        "allocations_per_sec" = $seenResults | ForEach-Object { $_.allocations_per_sec }
        "memory_mb" = $seenResults | ForEach-Object { $_.memory_mb }
        "checksum" = $seenResults | ForEach-Object { $_.checksum }
    }
}

$results | ConvertTo-Json -Depth 10 | Out-File -FilePath "binary_trees_results.json" -Encoding UTF8
Write-Host "Results saved to binary_trees_results.json" -ForegroundColor Green
Write-Output "Results saved to binary_trees_results.json"  # For background job capture