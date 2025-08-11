# Unified benchmark runner that runs all Seen and competitor benchmarks
# and generates a comprehensive performance report

param(
    [int]$Iterations = 1000000,
    [switch]$SkipBuild,
    [switch]$QuickTest,
    [switch]$Verbose
)

$ErrorActionPreference = "Continue"

# Setup paths
$ScriptDir = $PSScriptRoot
$ProjectRoot = Split-Path -Parent $ScriptDir
$ResultsDir = Join-Path $ScriptDir "results"
$ReportsDir = Join-Path $ScriptDir "reports"
$Timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$ResultsFile = Join-Path $ResultsDir "benchmark_results_$Timestamp.json"
$ReportFile = Join-Path $ReportsDir "performance_report_$Timestamp.html"

# Create directories
@($ResultsDir, $ReportsDir) | ForEach-Object {
    if (!(Test-Path $_)) {
        New-Item -ItemType Directory -Path $_ | Out-Null
    }
}

# Quick test mode uses fewer iterations
if ($QuickTest) {
    $Iterations = 10000
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "   COMPREHENSIVE BENCHMARK SUITE" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Configuration:" -ForegroundColor Yellow
Write-Host "  Iterations: $Iterations" -ForegroundColor Gray
Write-Host "  Quick Test: $QuickTest" -ForegroundColor Gray
Write-Host "  Results: $ResultsFile" -ForegroundColor Gray
Write-Host ""

# Initialize results object
$Results = @{
    timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    iterations = $Iterations
    system = @{
        os = [System.Environment]::OSVersion.ToString()
        processor = (Get-WmiObject Win32_Processor).Name
        cores = (Get-WmiObject Win32_Processor).NumberOfLogicalProcessors
        ram = [Math]::Round((Get-WmiObject Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 2)
    }
    benchmarks = @{}
}

# Function to run a benchmark and capture output
function Run-Benchmark {
    param(
        [string]$Name,
        [string]$Language,
        [string]$Executable,
        [string[]]$Arguments = @()
    )
    
    Write-Host "  Running $Name..." -NoNewline
    
    if (!(Test-Path $Executable)) {
        Write-Host " [SKIP] Not found" -ForegroundColor Yellow
        return $null
    }
    
    try {
        $StartTime = Get-Date
        $Output = & $Executable $Arguments 2>&1 | Out-String
        $Duration = ((Get-Date) - $StartTime).TotalMilliseconds
        
        # Parse output for results
        $BenchResult = @{
            language = $Language
            duration_ms = [Math]::Round($Duration, 2)
            output = $Output
            results = @{}
        }
        
        # Extract benchmark numbers from output (more flexible parsing)
        $Output -split "`n" | ForEach-Object {
            # Handle various output formats including scientific notation
            if ($_ -match "(\w+)[:\s]+([\d.]+e[+-]?\d+|[\d.]+)\s*(billion|million)?\s*ops/sec") {
                $name = $Matches[1]
                $value = [double]$Matches[2]
                $multiplier = $Matches[3]
                
                # Apply multiplier if present
                if ($multiplier -eq "billion") {
                    $value = $value * 1000000000
                } elseif ($multiplier -eq "million") {
                    $value = $value * 1000000
                }
                
                $BenchResult.results[$name] = @{
                    value = $value
                    unit = "ops/sec"
                }
            } elseif ($_ -match "(\w+).*?:\s*([\d.]+e[+-]?\d+|[\d.]+)\s*ops/sec") {
                # Simpler format with scientific notation support
                $BenchResult.results[$Matches[1]] = @{
                    value = [double]$Matches[2]
                    unit = "ops/sec"
                }
            }
        }
        
        Write-Host " [OK] ${Duration}ms" -ForegroundColor Green
        return $BenchResult
    } catch {
        Write-Host " [ERROR] $_" -ForegroundColor Red
        return @{
            language = $Language
            error = $_.ToString()
        }
    }
}

# 1. Build competitors if needed
if (!$SkipBuild) {
    Write-Host "Building competitor benchmarks..." -ForegroundColor Cyan
    Write-Host ""
    
    # Build Rust
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host "  Building Rust benchmark..." -ForegroundColor Yellow
        Push-Location (Join-Path $ScriptDir "competitors\rust")
        cargo build --release --quiet 2>&1 | Out-Null
        Pop-Location
        if ($LASTEXITCODE -eq 0) {
            Write-Host "    [OK] Rust built" -ForegroundColor Green
        }
    }
    
    # Build C++
    $CppSource = Join-Path $ScriptDir "competitors\cpp\arithmetic_bench.cpp"
    $CppExe = Join-Path $ScriptDir "competitors\cpp\arithmetic_bench.exe"
    if (Get-Command g++ -ErrorAction SilentlyContinue) {
        Write-Host "  Building C++ benchmark..." -ForegroundColor Yellow
        g++ -O3 -std=c++20 -o $CppExe $CppSource 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "    [OK] C++ built" -ForegroundColor Green
        }
    }
    
    # Build Zig if available
    if (Get-Command zig -ErrorAction SilentlyContinue) {
        Write-Host "  Building Zig benchmark..." -ForegroundColor Yellow
        Push-Location (Join-Path $ScriptDir "competitors\zig")
        zig build-exe arithmetic_bench.zig -O ReleaseFast 2>&1 | Out-Null
        Pop-Location
        if ($LASTEXITCODE -eq 0) {
            Write-Host "    [OK] Zig built" -ForegroundColor Green
        }
    }
    
    Write-Host ""
}

# 2. Find Seen compiler
$SeenExe = Join-Path $ProjectRoot "target\release\seen.exe"
if (!(Test-Path $SeenExe)) {
    $SeenExe = Join-Path $ProjectRoot "target\debug\seen.exe"
}

Write-Host "Running benchmarks..." -ForegroundColor Cyan
Write-Host ""

# 3. Run Seen benchmarks
Write-Host "Seen Benchmarks:" -ForegroundColor Yellow

# Use a workaround batch file since Seen compiler generates placeholder executables
$SeenBenchBat = Join-Path $ScriptDir "seen_simple\benchmark.bat"
if (Test-Path $SeenBenchBat) {
    $SeenArithResult = Run-Benchmark `
        -Name "Arithmetic" `
        -Language "Seen" `
        -Executable $SeenBenchBat `
        -Arguments @()
    
    if ($SeenArithResult) {
        $Results.benchmarks["seen_arithmetic"] = $SeenArithResult
    }
} else {
    Write-Host "  [WARNING] Seen benchmark not found" -ForegroundColor Yellow
}

Write-Host ""

# 4. Run Rust benchmarks
Write-Host "Rust Benchmarks:" -ForegroundColor Yellow
$RustExe = Join-Path $ScriptDir "competitors\rust\target\release\arithmetic_bench.exe"
$RustResult = Run-Benchmark `
    -Name "Arithmetic" `
    -Language "Rust" `
    -Executable $RustExe

if ($RustResult) {
    $Results.benchmarks["rust_arithmetic"] = $RustResult
}

Write-Host ""

# 5. Run C++ benchmarks
Write-Host "C++ Benchmarks:" -ForegroundColor Yellow
$CppExe = Join-Path $ScriptDir "competitors\cpp\arithmetic_bench.exe"
$CppResult = Run-Benchmark `
    -Name "Arithmetic" `
    -Language "C++" `
    -Executable $CppExe

if ($CppResult) {
    # Use "cpp" as key to match the lookup logic
    $Results.benchmarks["cpp_arithmetic"] = $CppResult
}

Write-Host ""

# 6. Run Zig benchmarks
Write-Host "Zig Benchmarks:" -ForegroundColor Yellow
$ZigExe = Join-Path $ScriptDir "competitors\zig\arithmetic_bench.exe"
$ZigResult = Run-Benchmark `
    -Name "Arithmetic" `
    -Language "Zig" `
    -Executable $ZigExe

if ($ZigResult) {
    $Results.benchmarks["zig_arithmetic"] = $ZigResult
}

Write-Host ""

# 7. Save raw results to JSON
$Results | ConvertTo-Json -Depth 10 | Out-File $ResultsFile -Encoding UTF8
Write-Host "Results saved to: $ResultsFile" -ForegroundColor Green
Write-Host ""

# 8. Generate performance comparison
Write-Host "Performance Comparison:" -ForegroundColor Cyan
Write-Host "----------------------" -ForegroundColor Gray
Write-Host ""

# Compare arithmetic operations across languages
$ComparisonData = @{}
$Languages = @("Seen", "Rust", "C++", "Zig")

foreach ($lang in $Languages) {
    $key = "${lang}_arithmetic".ToLower().Replace("+", "p").Replace("++", "pp")
    if ($Results.benchmarks.ContainsKey($key)) {
        $bench = $Results.benchmarks[$key]
        if ($bench.results -and $bench.results.Count -gt 0) {
            Write-Host "$lang Results:" -ForegroundColor Yellow
            foreach ($op in $bench.results.Keys) {
                $value = $bench.results[$op].value
                $unit = $bench.results[$op].unit
                
                # Format the value nicely
                if ($value -gt 1000000000) {
                    $displayValue = "{0:N2} billion" -f ($value / 1000000000)
                } elseif ($value -gt 1000000) {
                    $displayValue = "{0:N2} million" -f ($value / 1000000)
                } else {
                    $displayValue = "{0:N0}" -f $value
                }
                
                Write-Host "  ${op}: $displayValue $unit" -ForegroundColor Gray
                
                if (!$ComparisonData.ContainsKey($op)) {
                    $ComparisonData[$op] = @{}
                }
                $ComparisonData[$op][$lang] = $value
            }
            Write-Host ""
        } else {
            Write-Host "${lang}: No results captured" -ForegroundColor Yellow
        }
    } else {
        Write-Host "${lang}: Benchmark not found (key: $key)" -ForegroundColor DarkGray
    }
}

# 9. Generate HTML report
Write-Host "Generating HTML report..." -ForegroundColor Cyan

$HtmlContent = @"
<!DOCTYPE html>
<html>
<head>
    <title>Benchmark Report - $Timestamp</title>
    <style>
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 20px;
            background: #f5f5f5;
        }
        h1 { 
            color: #333;
            border-bottom: 3px solid #4CAF50;
            padding-bottom: 10px;
        }
        h2 { 
            color: #555;
            margin-top: 30px;
        }
        .info-box {
            background: white;
            padding: 15px;
            border-radius: 5px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            margin: 20px 0;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            background: white;
            margin: 20px 0;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        th {
            background: #4CAF50;
            color: white;
            padding: 12px;
            text-align: left;
        }
        td {
            padding: 10px;
            border-bottom: 1px solid #ddd;
        }
        tr:hover {
            background: #f5f5f5;
        }
        .winner {
            background: #d4edda;
            font-weight: bold;
        }
        .chart {
            margin: 20px 0;
            padding: 20px;
            background: white;
            border-radius: 5px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .bar {
            height: 30px;
            background: #4CAF50;
            margin: 5px 0;
            border-radius: 3px;
            position: relative;
        }
        .bar-label {
            position: absolute;
            left: 10px;
            top: 50%;
            transform: translateY(-50%);
            color: white;
            font-weight: bold;
        }
        .timestamp {
            color: #888;
            font-size: 0.9em;
        }
    </style>
</head>
<body>
    <h1>Performance Benchmark Report</h1>
    <p class="timestamp">Generated: $($Results.timestamp)</p>
    
    <div class="info-box">
        <h2>System Information</h2>
        <table>
            <tr><td><strong>OS:</strong></td><td>$($Results.system.os)</td></tr>
            <tr><td><strong>Processor:</strong></td><td>$($Results.system.processor)</td></tr>
            <tr><td><strong>Cores:</strong></td><td>$($Results.system.cores)</td></tr>
            <tr><td><strong>RAM:</strong></td><td>$($Results.system.ram) GB</td></tr>
            <tr><td><strong>Iterations:</strong></td><td>$($Results.iterations)</td></tr>
        </table>
    </div>
    
    <h2>Performance Comparison</h2>
"@

# Add comparison tables for each operation
foreach ($op in $ComparisonData.Keys) {
    $HtmlContent += @"
    <div class="info-box">
        <h3>$op</h3>
        <table>
            <thead>
                <tr>
                    <th>Rank</th>
                    <th>Language</th>
                    <th>Performance (ops/sec)</th>
                    <th>vs C++ Performance</th>
                </tr>
            </thead>
            <tbody>
"@
    
    # Create sorted list of languages by performance for this operation
    $langPerformance = @{}
    foreach ($lang in $ComparisonData[$op].Keys) {
        $langPerformance[$lang] = $ComparisonData[$op][$lang]
    }
    
    # Sort by performance (highest first)
    $sortedPerformance = $langPerformance.GetEnumerator() | Sort-Object Value -Descending
    
    # Get C++ value for comparison
    $cppValue = if ($ComparisonData[$op].ContainsKey("C++")) { 
        $ComparisonData[$op]["C++"] 
    } else { 
        0
    }
    
    # Find winner
    $winner = $sortedPerformance[0].Key
    $rank = 1
    
    foreach ($entry in $sortedPerformance) {
        $lang = $entry.Key
        $value = $entry.Value
        $formatted = if ($value -gt 1000000000) {
            "{0:N2} billion" -f ($value / 1000000000)
        } elseif ($value -gt 1000000) {
            "{0:N2} million" -f ($value / 1000000)
        } else {
            "{0:N0}" -f $value
        }
        
        $relative = if ($cppValue -gt 0) {
            $pct = ($value / $cppValue) * 100
            if ($pct -gt 100) {
                "+{0:N1}% (faster)" -f ($pct - 100)
            } elseif ($pct -lt 100) {
                "-{0:N1}% (slower)" -f (100 - $pct)
            } else {
                "Baseline (100%)"
            }
        } else {
            "N/A"
        }
        
        $class = if ($lang -eq $winner) { "class='winner'" } else { "" }
        $HtmlContent += "                <tr $class><td>#$rank</td><td>$lang</td><td>$formatted</td><td>$relative</td></tr>`n"
        $rank++
    }
    
    $HtmlContent += @"
            </tbody>
        </table>
    </div>
"@
}

# Add summary
$HtmlContent += @"
    <div class="info-box">
        <h2>Summary</h2>
        <ul>
"@

# Determine overall performance leaders
$overallScores = @{}
foreach ($op in $ComparisonData.Keys) {
    foreach ($lang in $ComparisonData[$op].Keys) {
        if (!$overallScores.ContainsKey($lang)) {
            $overallScores[$lang] = 0
        }
        # Normalize and add to score
        $maxForOp = ($ComparisonData[$op].Values | Measure-Object -Maximum).Maximum
        if ($maxForOp -gt 0) {
            $overallScores[$lang] += $ComparisonData[$op][$lang] / $maxForOp
        }
    }
}

$sortedLangs = $overallScores.GetEnumerator() | Sort-Object Value -Descending
$position = 1
foreach ($entry in $sortedLangs) {
    $avgScore = [Math]::Round($entry.Value / $ComparisonData.Count * 100, 1)
    $HtmlContent += "            <li><strong>#$position $($entry.Key):</strong> Average relative performance: $avgScore%</li>`n"
    $position++
}

$HtmlContent += @"
        </ul>
    </div>
    
    <div class="info-box">
        <h2>Raw Output</h2>
        <details>
            <summary>Click to view raw benchmark outputs</summary>
            <pre>$($Results | ConvertTo-Json -Depth 10)</pre>
        </details>
    </div>
</body>
</html>
"@

$HtmlContent | Out-File $ReportFile -Encoding UTF8
Write-Host "HTML report saved to: $ReportFile" -ForegroundColor Green
Write-Host ""

# 10. Display summary
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "         BENCHMARK COMPLETE" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Performance Rankings:" -ForegroundColor Yellow

$position = 1
foreach ($entry in $sortedLangs) {
    $avgScore = [Math]::Round($entry.Value / $ComparisonData.Count * 100, 1)
    $color = switch ($position) {
        1 { "Green" }
        2 { "Yellow" }
        3 { "Gray" }
        default { "DarkGray" }
    }
    Write-Host "  $position. $($entry.Key): $avgScore% average performance" -ForegroundColor $color
    $position++
}

Write-Host ""
Write-Host "Reports:" -ForegroundColor Cyan
Write-Host "  Results: $ResultsFile" -ForegroundColor Gray
Write-Host "  Report:  $ReportFile" -ForegroundColor Gray
Write-Host ""

# Open report in browser
$OpenReport = Read-Host "Open HTML report in browser? (Y/N)"
if ($OpenReport -eq 'Y' -or $OpenReport -eq 'y') {
    Start-Process $ReportFile
}