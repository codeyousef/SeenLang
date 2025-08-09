# Spectral Norm Benchmark Script
param(
    [int]$Iterations = 5
)

$SPECTRAL_NORM_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
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

Write-Host "Running Spectral Norm Benchmark..." -ForegroundColor Cyan

# Define benchmark results
$cppResults = @()
$rustResults = @()
$seenResults = @()

# Run C++ version
$CPP_EXE = "$PSScriptRoot\..\..\benchmarks\real_implementations\spectral_norm_bench_cpp.exe"
if (Test-Path $CPP_EXE) {
    Write-Host "Running C++ spectral norm..." -ForegroundColor Blue
    
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
                "computation_time_ms" = [double]$matches[1]
                "flops_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "spectral_norm" = [double]$matches[4]
            }
        }
    }
    
    if ($cppResults.Count -gt 0) {
        $avgTime = ($cppResults | ForEach-Object { $_.computation_time_ms } | Measure-Object -Average).Average
        $avgFlops = ($cppResults | ForEach-Object { $_.flops_per_sec } | Measure-Object -Average).Average
        Write-Host "C++ Average: $([math]::Round($avgTime, 2)) ms, $([math]::Round($avgFlops / 1000000, 1))M flops/s" -ForegroundColor Green
    }
}

# Run Rust version
$RUST_EXE = "$PSScriptRoot\..\..\benchmarks\real_implementations\spectral_norm_bench_rust.exe"
if (Test-Path $RUST_EXE) {
    Write-Host "Running Rust spectral norm..." -ForegroundColor Blue
    
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
                "computation_time_ms" = [double]$matches[1]
                "flops_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "spectral_norm" = [double]$matches[4]
            }
        }
    }
    
    if ($rustResults.Count -gt 0) {
        $avgTime = ($rustResults | ForEach-Object { $_.computation_time_ms } | Measure-Object -Average).Average
        $avgFlops = ($rustResults | ForEach-Object { $_.flops_per_sec } | Measure-Object -Average).Average
        Write-Host "Rust Average: $([math]::Round($avgTime, 2)) ms, $([math]::Round($avgFlops / 1000000, 1))M flops/s" -ForegroundColor Green
    }
}

# Run Seen version
$SEEN_EXECUTABLE = "$SPECTRAL_NORM_DIR\spectral_norm_benchmark\target\native\debug\spectral_norm_benchmark"

# Try to build if executable doesn't exist
if (-not (Test-Path $SEEN_EXECUTABLE)) {
    Write-Host "Building Seen spectral norm benchmark..." -ForegroundColor Yellow
    Push-Location "$SPECTRAL_NORM_DIR\spectral_norm_benchmark"
    & $SEEN_CLI build 2>&1 | Out-Null
    Pop-Location
}

if (Test-Path $SEEN_EXECUTABLE) {
    Write-Host "Running Seen spectral norm..." -ForegroundColor Blue
    
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
        
        # Extract only the numeric line from output (spectral_norm outputs 4 values)
        $numericLine = $output | Where-Object { $_ -match "^\s*([\d.eE+-]+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s*$" }
        Write-Host "Found numeric line: '$numericLine'" -ForegroundColor Gray
        if ($exitCode -eq 0 -and $numericLine -and $numericLine -match "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)") {
            $seenResults += @{
                "computation_time_ms" = [double]$matches[1]
                "flops_per_sec" = [double]$matches[2]
                "memory_mb" = [double]$matches[3]
                "spectral_norm" = [double]$matches[4]
            }
            Write-Host "Successfully parsed iteration $($i + 1): $([double]$matches[1])ms computation, $([double]$matches[2]) flops/s" -ForegroundColor Green
        } else {
            Write-Host "Failed to parse output on iteration $($i + 1). Exit code: $exitCode, Output: '$outputStr'" -ForegroundColor Red
        }
    }
    
    if ($seenResults.Count -gt 0) {
        $avgTime = ($seenResults | ForEach-Object { $_.computation_time_ms } | Measure-Object -Average).Average
        $avgFlops = ($seenResults | ForEach-Object { $_.flops_per_sec } | Measure-Object -Average).Average
        Write-Host "Seen Average: $([math]::Round($avgTime, 2)) ms, $([math]::Round($avgFlops / 1000000, 1))M flops/s" -ForegroundColor Green
    }
}

# Save results
$results = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = "spectral_norm"
        iterations = $Iterations
    }
    benchmarks = @{}
}

if ($cppResults.Count -gt 0) {
    $results.benchmarks.cpp = @{
        "computation_time_ms" = $cppResults | ForEach-Object { $_.computation_time_ms }
        "flops_per_sec" = $cppResults | ForEach-Object { $_.flops_per_sec }
        "memory_mb" = $cppResults | ForEach-Object { $_.memory_mb }
        "spectral_norm" = $cppResults | ForEach-Object { $_.spectral_norm }
    }
}

if ($rustResults.Count -gt 0) {
    $results.benchmarks.rust = @{
        "computation_time_ms" = $rustResults | ForEach-Object { $_.computation_time_ms }
        "flops_per_sec" = $rustResults | ForEach-Object { $_.flops_per_sec }
        "memory_mb" = $rustResults | ForEach-Object { $_.memory_mb }
        "spectral_norm" = $rustResults | ForEach-Object { $_.spectral_norm }
    }
}

if ($seenResults.Count -gt 0) {
    $results.benchmarks.seen = @{
        "computation_time_ms" = $seenResults | ForEach-Object { $_.computation_time_ms }
        "flops_per_sec" = $seenResults | ForEach-Object { $_.flops_per_sec }
        "memory_mb" = $seenResults | ForEach-Object { $_.memory_mb }
        "spectral_norm" = $seenResults | ForEach-Object { $_.spectral_norm }
    }
}

$results | ConvertTo-Json -Depth 10 | Out-File -FilePath "spectral_norm_results.json" -Encoding UTF8
Write-Host "Results saved to spectral_norm_results.json" -ForegroundColor Green
Write-Output "Results saved to spectral_norm_results.json"  # For background job capture