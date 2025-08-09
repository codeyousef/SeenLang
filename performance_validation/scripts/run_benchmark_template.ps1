# Universal Benchmark Template Script
# This template can be used for any benchmark type with automatic competitive executable discovery
param(
    [int]$Iterations = 3,
    [string]$BenchmarkName = "",
    [switch]$Verbose,
    [switch]$SkipBuild
)

# Auto-detect benchmark name from directory
if ([string]::IsNullOrEmpty($BenchmarkName)) {
    $BenchmarkName = Split-Path -Leaf $PSScriptRoot
}

# Path configuration
$BENCHMARK_DIR = $PSScriptRoot
$SEEN_CLI = @(
    "$PSScriptRoot\..\..\..\target\release\seen.exe",
    "$PSScriptRoot\..\..\..\target\debug\seen.exe",
    "$PSScriptRoot\..\..\..\target-wsl\release\seen",
    "$PSScriptRoot\..\..\..\target-wsl\debug\seen"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

# Competitive executables discovery
$COMPETITIVE_DIR = "$PSScriptRoot\..\..\benchmarks\real_implementations"

# Logging functions
function Write-BenchmarkInfo { param([string]$Message) Write-Host "[INFO] $Message" -ForegroundColor Cyan }
function Write-BenchmarkSuccess { param([string]$Message) Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-BenchmarkWarning { param([string]$Message) Write-Host "[WARNING] $Message" -ForegroundColor Yellow }
function Write-BenchmarkError { param([string]$Message) Write-Host "[ERROR] $Message" -ForegroundColor Red }
function Write-BenchmarkDebug { param([string]$Message) if ($Verbose) { Write-Host "[DEBUG] $Message" -ForegroundColor Gray } }

Write-BenchmarkInfo "Running $BenchmarkName Benchmark (Iterations: $Iterations)..."

# Function to parse benchmark output with multiple strategies
function Parse-BenchmarkOutput {
    param(
        [string[]]$Output,
        [string]$Language
    )
    
    $outputStr = ($Output -join " ").Trim()
    Write-BenchmarkDebug "$Language raw output: '$outputStr'"
    
    # Strategy 1: Try JSON parsing first
    try {
        $jsonData = $outputStr | ConvertFrom-Json -ErrorAction Stop
        Write-BenchmarkDebug "$Language: Successfully parsed as JSON"
        return $jsonData
    }
    catch {
        Write-BenchmarkDebug "$Language: Not JSON format, trying other parsers"
    }
    
    # Strategy 2: Enhanced regex for scientific notation and decimals
    $scientificPattern = "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)"
    
    if ($outputStr -match $scientificPattern) {
        Write-BenchmarkDebug "$Language: Matched 4-value scientific notation pattern"
        
        # Convert scientific notation to regular numbers
        $values = @([double]$matches[1], [double]$matches[2], [double]$matches[3], [double]$matches[4])
        Write-BenchmarkDebug "$Language: Parsed values: $($values -join ', ')"
        
        # Return appropriate structure based on benchmark type
        switch ($BenchmarkName) {
            "json_parser" {
                return @{
                    "parse_time_ms" = $values[0]
                    "validation_time_ms" = $values[1] 
                    "tokens_per_sec" = $values[2]
                    "memory_kb" = $values[3]
                }
            }
            "http_server" {
                return @{
                    "requests_per_sec" = $values[0]
                    "response_time_ms" = $values[1]
                    "memory_mb" = $values[2]
                    "concurrent_connections" = $values[3]
                }
            }
            "ray_tracer" {
                return @{
                    "render_time_ms" = $values[0]
                    "pixels_per_sec" = $values[1]
                    "memory_mb" = $values[2]
                    "quality_score" = $values[3]
                }
            }
            "regex_engine" {
                return @{
                    "match_time_ms" = $values[0]
                    "matches_per_sec" = $values[1]
                    "memory_mb" = $values[2]
                    "pattern_complexity" = $values[3]
                }
            }
            default {
                return @{
                    "value1" = $values[0]
                    "value2" = $values[1]
                    "value3" = $values[2]
                    "value4" = $values[3]
                }
            }
        }
    }
    
    # Strategy 3: Try 3-value pattern
    $threeValuePattern = "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)"
    if ($outputStr -match $threeValuePattern) {
        Write-BenchmarkDebug "$Language: Matched 3-value pattern"
        return @{
            "value1" = [double]$matches[1]
            "value2" = [double]$matches[2]
            "value3" = [double]$matches[3]
        }
    }
    
    # Strategy 4: Try 2-value pattern
    $twoValuePattern = "([\d.]+(?:[eE][+-]?\d+)?)\s+([\d.]+(?:[eE][+-]?\d+)?)"
    if ($outputStr -match $twoValuePattern) {
        Write-BenchmarkDebug "$Language: Matched 2-value pattern"
        return @{
            "value1" = [double]$matches[1]
            "value2" = [double]$matches[2]
        }
    }
    
    # Strategy 5: Key-value parsing (Time: 123ms Memory: 456KB)
    $keyValueResults = @{}
    if ($outputStr -match "Time:\s*([\d.]+(?:[eE][+-]?\d+)?)") {
        $keyValueResults["time"] = [double]$matches[1]
    }
    if ($outputStr -match "Memory:\s*([\d.]+(?:[eE][+-]?\d+)?)") {
        $keyValueResults["memory"] = [double]$matches[1]
    }
    if ($outputStr -match "Throughput:\s*([\d.]+(?:[eE][+-]?\d+)?)") {
        $keyValueResults["throughput"] = [double]$matches[1]
    }
    
    if ($keyValueResults.Count -gt 0) {
        Write-BenchmarkDebug "$Language: Parsed key-value format: $($keyValueResults | ConvertTo-Json -Compress)"
        return $keyValueResults
    }
    
    Write-BenchmarkWarning "$Language: Failed to parse output: '$outputStr'"
    return $null
}

# Function to run competitive benchmarks
function Invoke-CompetitiveBenchmark {
    param(
        [string]$ExecutablePath,
        [string]$Language,
        [int]$Iterations
    )
    
    if (-not (Test-Path $ExecutablePath)) {
        Write-BenchmarkDebug "$Language executable not found: $ExecutablePath"
        return @()
    }
    
    Write-BenchmarkInfo "Running $Language benchmark: $(Split-Path -Leaf $ExecutablePath)"
    $results = @()
    
    for ($i = 0; $i -lt $Iterations; $i++) {
        try {
            Write-BenchmarkDebug "$Language: Running iteration $($i + 1)/$Iterations"
            
            # Execute with timeout protection
            $timeout = 30000  # 30 seconds
            $job = Start-Job -ScriptBlock {
                param($ExePath)
                & $ExePath 2>&1
            } -ArgumentList $ExecutablePath
            
            if (Wait-Job $job -Timeout 30) {
                $output = Receive-Job $job
                $exitCode = 0
            } else {
                Write-BenchmarkWarning "$Language: Benchmark timed out on iteration $($i + 1)"
                Remove-Job $job -Force
                continue
            }
            
            Remove-Job $job
            
            # Parse the output
            $parsedResult = Parse-BenchmarkOutput -Output $output -Language $Language
            
            if ($null -ne $parsedResult) {
                $results += $parsedResult
                Write-BenchmarkSuccess "$Language: Successfully parsed iteration $($i + 1)"
            } else {
                Write-BenchmarkWarning "$Language: Failed to parse output on iteration $($i + 1)"
            }
        }
        catch {
            Write-BenchmarkError "$Language: Error on iteration $($i + 1): $($_.Exception.Message)"
        }
    }
    
    if ($results.Count -gt 0) {
        # Calculate averages for display
        $firstResult = $results[0]
        foreach ($key in $firstResult.Keys) {
            $values = $results | ForEach-Object { $_.$key }
            $average = ($values | Measure-Object -Average).Average
            Write-BenchmarkSuccess "$Language Average $key`: $([math]::Round($average, 3))"
        }
    }
    
    return $results
}

# Function to run Seen benchmark
function Invoke-SeenBenchmark {
    param([int]$Iterations)
    
    # Try to find Seen executable
    $seenExecutable = "$BENCHMARK_DIR\${BenchmarkName}_benchmark\target\native\debug\${BenchmarkName}_benchmark"
    if (-not (Test-Path $seenExecutable)) {
        $seenExecutable = "$BENCHMARK_DIR\${BenchmarkName}_benchmark\target\native\release\${BenchmarkName}_benchmark"
    }
    
    # Try alternative naming patterns
    if (-not (Test-Path $seenExecutable)) {
        $seenExecutable = "$BENCHMARK_DIR\benchmark\target\native\debug\benchmark"
    }
    
    # Build if needed and not skipping
    if (-not (Test-Path $seenExecutable) -and -not $SkipBuild) {
        Write-BenchmarkInfo "Building Seen $BenchmarkName benchmark..."
        
        $buildDirs = @(
            "$BENCHMARK_DIR\${BenchmarkName}_benchmark",
            "$BENCHMARK_DIR\benchmark"
        ) | Where-Object { Test-Path $_ }
        
        if ($buildDirs.Count -gt 0) {
            Push-Location $buildDirs[0]
            try {
                $buildOutput = & $SEEN_CLI build 2>&1
                if ($LASTEXITCODE -ne 0) {
                    Write-BenchmarkWarning "Build failed: $($buildOutput -join ' ')"
                }
            } finally {
                Pop-Location
            }
        }
    }
    
    if (-not (Test-Path $seenExecutable)) {
        Write-BenchmarkWarning "Seen executable not found: $seenExecutable"
        return @()
    }
    
    Write-BenchmarkInfo "Running Seen benchmark: $(Split-Path -Leaf $seenExecutable)"
    $results = @()
    
    for ($i = 0; $i -lt $Iterations; $i++) {
        try {
            Write-BenchmarkDebug "Seen: Running iteration $($i + 1)/$Iterations"
            
            # Handle WSL execution for Linux binaries
            if ($seenExecutable -notlike "*.exe") {
                # Convert to WSL path
                $wslPath = $seenExecutable -replace '\\', '/' -replace '^([A-Za-z]):', '/mnt/$1'
                $wslPath = $wslPath.ToLower()
                Write-BenchmarkDebug "Executing via WSL: $wslPath"
                $output = wsl bash -c "`"$wslPath`"" 2>&1
                $exitCode = $LASTEXITCODE
            } else {
                $output = & $seenExecutable 2>&1
                $exitCode = $LASTEXITCODE
            }
            
            if ($exitCode -eq 0) {
                $parsedResult = Parse-BenchmarkOutput -Output $output -Language "Seen"
                
                if ($null -ne $parsedResult) {
                    $results += $parsedResult
                    Write-BenchmarkSuccess "Seen: Successfully parsed iteration $($i + 1)"
                } else {
                    Write-BenchmarkWarning "Seen: Failed to parse output on iteration $($i + 1)"
                }
            } else {
                Write-BenchmarkWarning "Seen: Non-zero exit code ($exitCode) on iteration $($i + 1)"
            }
        }
        catch {
            Write-BenchmarkError "Seen: Error on iteration $($i + 1): $($_.Exception.Message)"
        }
    }
    
    if ($results.Count -gt 0) {
        # Calculate averages for display
        $firstResult = $results[0]
        foreach ($key in $firstResult.Keys) {
            $values = $results | ForEach-Object { $_.$key }
            $average = ($values | Measure-Object -Average).Average
            Write-BenchmarkSuccess "Seen Average $key`: $([math]::Round($average, 3))"
        }
    }
    
    return $results
}

# Main execution
$allResults = @{}

# 1. Run Seen implementation
Write-BenchmarkInfo "=== Running Seen Implementation ==="
$seenResults = Invoke-SeenBenchmark -Iterations $Iterations
if ($seenResults.Count -gt 0) {
    $allResults["seen"] = $seenResults
}

# 2. Discover and run competitive benchmarks
Write-BenchmarkInfo "=== Discovering Competitive Benchmarks ==="

# Find matching executables with various naming patterns
$executablePatterns = @(
    "${BenchmarkName}_bench_rust.exe",
    "${BenchmarkName}_bench_cpp.exe", 
    "${BenchmarkName}_rust.exe",
    "${BenchmarkName}_cpp.exe",
    "*${BenchmarkName}*rust*.exe",
    "*${BenchmarkName}*cpp*.exe"
)

$foundExecutables = @{}
foreach ($pattern in $executablePatterns) {
    $matches = Get-ChildItem -Path $COMPETITIVE_DIR -Filter $pattern -ErrorAction SilentlyContinue
    foreach ($match in $matches) {
        $lang = if ($match.Name -match "rust") { "rust" } 
               elseif ($match.Name -match "cpp") { "cpp" }
               else { "unknown" }
        
        if ($lang -ne "unknown" -and -not $foundExecutables.ContainsKey($lang)) {
            $foundExecutables[$lang] = $match.FullName
            Write-BenchmarkSuccess "Found $lang executable: $($match.Name)"
        }
    }
}

# 3. Run competitive benchmarks
foreach ($lang in $foundExecutables.Keys) {
    Write-BenchmarkInfo "=== Running $lang Implementation ==="
    $competitiveResults = Invoke-CompetitiveBenchmark -ExecutablePath $foundExecutables[$lang] -Language $lang -Iterations $Iterations
    if ($competitiveResults.Count -gt 0) {
        $allResults[$lang] = $competitiveResults
    }
}

# 4. Generate standardized JSON output
$finalResults = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = $BenchmarkName
        iterations = $Iterations
        total_implementations = $allResults.Keys.Count
        implementations = @($allResults.Keys)
    }
    benchmarks = @{}
}

# Convert results to JSON-compatible format
foreach ($lang in $allResults.Keys) {
    $langResults = $allResults[$lang]
    
    if ($langResults.Count -gt 0) {
        # Get all metric names from first result
        $metricNames = $langResults[0].Keys
        $langData = @{}
        
        # Extract arrays of values for each metric
        foreach ($metric in $metricNames) {
            $langData[$metric] = $langResults | ForEach-Object { $_.$metric }
        }
        
        # Add metadata
        $langData["metadata"] = @{
            "language" = $lang
            "successful_iterations" = $langResults.Count
            "total_iterations" = $Iterations
        }
        
        $finalResults.benchmarks[$lang] = $langData
    }
}

# 5. Save results
$outputFile = "${BenchmarkName}_results.json"
$finalResults | ConvertTo-Json -Depth 10 | Out-File -FilePath $outputFile -Encoding UTF8

Write-BenchmarkSuccess "Results saved to $outputFile"
Write-BenchmarkInfo "Summary: $($allResults.Keys.Count) implementations completed successfully"

# 6. Console output for orchestration script
Write-Output "Benchmark complete: $BenchmarkName"
Write-Output "Implementations: $($allResults.Keys -join ', ')"
Write-Output "Results file: $outputFile"

# Return success/failure status
if ($allResults.Keys.Count -gt 0) {
    exit 0
} else {
    Write-BenchmarkError "No benchmark implementations completed successfully"
    exit 1
}