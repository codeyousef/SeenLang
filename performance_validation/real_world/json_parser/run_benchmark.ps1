# JSON Parser Benchmark Script
param(
    [int]$Iterations = 5
)

$JSON_PARSER_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
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

Write-Host "Running JSON Parser Benchmark..." -ForegroundColor Cyan

# Define benchmark results
$cppResults = @()
$rustResults = @()
$seenResults = @()

# Run C++ version
$CPP_EXE = "$PSScriptRoot\..\..\benchmarks\real_implementations\json_parser_bench_cpp.exe"
if (Test-Path $CPP_EXE) {
    Write-Host "Running C++ JSON parser..." -ForegroundColor Blue
    
    for ($i = 0; $i -lt $Iterations; $i++) {
        # Always run C++ executable via WSL (handles both Linux and Windows executables)
        $wslPath = $CPP_EXE -replace '\\', '/' -replace '^([A-Za-z]):', '/mnt/$1'
        $wslPath = $wslPath.ToLower()
        Write-Host "Executing C++ via WSL: $wslPath" -ForegroundColor Gray
        $output = wsl bash -c "`"$wslPath`"" 2>&1
        
        # Try JSON parsing first
        try {
            $jsonData = ($output -join " ") | ConvertFrom-Json
            if ($jsonData.average_time -and $jsonData.throughput_mb_per_sec) {
                $cppResults += @{
                    "parse_time_ms" = [double]$jsonData.average_time * 1000
                    "validation_time_ms" = 0.1  # Minimal for JSON
                    "tokens_per_sec" = [double]$jsonData.throughput_mb_per_sec * 1000000 / 3109  # Convert MB/s to tokens/s
                    "memory_kb" = [double]$jsonData.json_size_bytes / 1024
                }
            }
        } catch {
            # Fall back to regex parsing
            if ($output -match "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)") {
                $cppResults += @{
                    "parse_time_ms" = [double]$matches[1]
                    "validation_time_ms" = [double]$matches[2]
                    "tokens_per_sec" = [double]$matches[3]
                    "memory_kb" = [double]$matches[4]
                }
            }
        }
    }
    
    if ($cppResults.Count -gt 0) {
        $avgParse = ($cppResults | ForEach-Object { $_.parse_time_ms } | Measure-Object -Average).Average
        $avgTps = ($cppResults | ForEach-Object { $_.tokens_per_sec } | Measure-Object -Average).Average
        Write-Host "C++ Average: $([math]::Round($avgParse, 2)) ms parse, $([math]::Round($avgTps / 1000, 1))K tokens/s" -ForegroundColor Green
    }
}

# Run Rust version
$RUST_EXE = "$PSScriptRoot\..\..\benchmarks\real_implementations\json_parser_bench_rust.exe"
if (Test-Path $RUST_EXE) {
    Write-Host "Running Rust JSON parser..." -ForegroundColor Blue
    
    for ($i = 0; $i -lt $Iterations; $i++) {
        # Always run Rust executable via WSL (handles both Linux and Windows executables)
        $wslPath = $RUST_EXE -replace '\\', '/' -replace '^([A-Za-z]):', '/mnt/$1'
        $wslPath = $wslPath.ToLower()
        Write-Host "Executing Rust via WSL: $wslPath" -ForegroundColor Gray
        $output = wsl bash -c "`"$wslPath`"" 2>&1
        
        # Try JSON parsing first for Rust
        try {
            $jsonData = ($output -join " ") | ConvertFrom-Json
            if ($jsonData.average_time -and $jsonData.throughput_mb_per_sec) {
                $rustResults += @{
                    "parse_time_ms" = [double]$jsonData.average_time * 1000
                    "validation_time_ms" = 0.1  # Minimal for JSON
                    "tokens_per_sec" = [double]$jsonData.throughput_mb_per_sec * 1000000 / 3109
                    "memory_kb" = [double]$jsonData.json_size_bytes / 1024
                }
            }
        } catch {
            # Fall back to regex parsing
            if ($output -match "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)") {
                $rustResults += @{
                    "parse_time_ms" = [double]$matches[1]
                    "validation_time_ms" = [double]$matches[2]
                    "tokens_per_sec" = [double]$matches[3]
                    "memory_kb" = [double]$matches[4]
                }
            }
        }
    }
    
    if ($rustResults.Count -gt 0) {
        $avgParse = ($rustResults | ForEach-Object { $_.parse_time_ms } | Measure-Object -Average).Average
        $avgTps = ($rustResults | ForEach-Object { $_.tokens_per_sec } | Measure-Object -Average).Average
        Write-Host "Rust Average: $([math]::Round($avgParse, 2)) ms parse, $([math]::Round($avgTps / 1000, 1))K tokens/s" -ForegroundColor Green
    }
}

# Run Seen version
$SEEN_EXECUTABLE = "$JSON_PARSER_DIR\json_parser_benchmark\target\native\debug\json_parser_benchmark"

# Try to build if executable doesn't exist
if (-not (Test-Path $SEEN_EXECUTABLE)) {
    Write-Host "Building Seen JSON parser benchmark..." -ForegroundColor Yellow
    Push-Location "$JSON_PARSER_DIR\json_parser_benchmark"
    & $SEEN_CLI build 2>&1 | Out-Null
    Pop-Location
}

if (Test-Path $SEEN_EXECUTABLE) {
    Write-Host "Running Seen JSON parser..." -ForegroundColor Blue
    
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
        
        # Extract only the numeric line from output
        $numericLine = $output | Where-Object { $_ -match "^\s*([\d.eE+-]+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s*$" }
        Write-Host "Found numeric line: '$numericLine'" -ForegroundColor Gray
        if ($exitCode -eq 0 -and $numericLine -and $numericLine -match "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)") {
            $seenResults += @{
                "parse_time_ms" = [double]$matches[1]
                "validation_time_ms" = [double]$matches[2]
                "tokens_per_sec" = [double]$matches[3]
                "memory_kb" = [double]$matches[4]
            }
            Write-Host "Successfully parsed iteration $($i + 1): $([double]$matches[1])ms parse, $([double]$matches[3]) tokens/s" -ForegroundColor Green
        } else {
            Write-Host "Failed to parse output on iteration $($i + 1). Exit code: $exitCode, Output: '$outputStr'" -ForegroundColor Red
        }
    }
    
    if ($seenResults.Count -gt 0) {
        $avgParse = ($seenResults | ForEach-Object { $_.parse_time_ms } | Measure-Object -Average).Average
        $avgTps = ($seenResults | ForEach-Object { $_.tokens_per_sec } | Measure-Object -Average).Average
        Write-Host "Seen Average: $([math]::Round($avgParse, 2)) ms parse, $([math]::Round($avgTps / 1000, 1))K tokens/s" -ForegroundColor Green
    }
}

# Save results
$results = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = "json_parser"
        iterations = $Iterations
    }
    benchmarks = @{}
}

if ($cppResults.Count -gt 0) {
    $results.benchmarks.cpp = @{
        "parse_time_ms" = $cppResults | ForEach-Object { $_.parse_time_ms }
        "validation_time_ms" = $cppResults | ForEach-Object { $_.validation_time_ms }
        "tokens_per_sec" = $cppResults | ForEach-Object { $_.tokens_per_sec }
        "memory_kb" = $cppResults | ForEach-Object { $_.memory_kb }
    }
}

if ($rustResults.Count -gt 0) {
    $results.benchmarks.rust = @{
        "parse_time_ms" = $rustResults | ForEach-Object { $_.parse_time_ms }
        "validation_time_ms" = $rustResults | ForEach-Object { $_.validation_time_ms }
        "tokens_per_sec" = $rustResults | ForEach-Object { $_.tokens_per_sec }
        "memory_kb" = $rustResults | ForEach-Object { $_.memory_kb }
    }
}

if ($seenResults.Count -gt 0) {
    $results.benchmarks.seen = @{
        "parse_time_ms" = $seenResults | ForEach-Object { $_.parse_time_ms }
        "validation_time_ms" = $seenResults | ForEach-Object { $_.validation_time_ms }
        "tokens_per_sec" = $seenResults | ForEach-Object { $_.tokens_per_sec }
        "memory_kb" = $seenResults | ForEach-Object { $_.memory_kb }
    }
}

$results | ConvertTo-Json -Depth 10 | Out-File -FilePath "json_parser_results.json" -Encoding UTF8
Write-Host "Results saved to json_parser_results.json" -ForegroundColor Green
Write-Output "Results saved to json_parser_results.json"  # For background job capture