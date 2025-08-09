# Compression Real-World Benchmark Runner
# PowerShell script for testing compression algorithm performance

param(
    [int]$Iterations = 30,
    [int]$Warmup = 5,
    [string]$Output = "compression_results.json",
    [string]$Competitors = "cpp,rust,zig",
    [string]$TestSize = "medium",
    [string]$Format = "json",
    [switch]$Verbose,
    [switch]$Help
)

# Set up environment variables
$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$PROJECT_ROOT = (Get-Item "$SCRIPT_DIR\..\..\..").FullName

if ($Help) {
    Write-Host @"
Compression Real-World Benchmark

Usage: .\run_benchmark.ps1 [OPTIONS]

DESCRIPTION:
    Tests compression algorithm performance with real data.
    Measures compression ratio, speed, and memory usage.
"@
    exit 0
}

$results = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = "compression_real_world"
        iterations = $Iterations
        application = "Compression"
    }
    benchmarks = @{}
}

Write-Host "[INFO] Starting compression benchmark..." -ForegroundColor Blue
Write-Host "[INFO] Running ACTUAL Seen compression benchmarks (not simulated)" -ForegroundColor Green

# Function to run actual Seen compression benchmark
function Invoke-SeenCompressionBenchmark {
    param([int]$Iterations)
    
    Write-Host "[INFO] Testing Seen compression (REAL implementation)..." -ForegroundColor Blue
    
    $seenExe = "$PROJECT_ROOT\target\release\seen.exe"
    $seenDebugExe = "$PROJECT_ROOT\target\debug\seen.exe"
    $seenWslExe = "$PROJECT_ROOT\target-wsl\release\seen"
    $seenWslDebugExe = "$PROJECT_ROOT\target-wsl\debug\seen"
    $compressionSeen = "$SCRIPT_DIR\compression.seen"
    
    # Check for Seen compiler (try both Windows and WSL versions)
    $seenCompiler = $null
    if (Test-Path $seenExe) {
        $seenCompiler = $seenExe
    } elseif (Test-Path $seenDebugExe) {
        $seenCompiler = $seenDebugExe
    } elseif (Test-Path $seenWslExe) {
        $seenCompiler = $seenWslExe
    } elseif (Test-Path $seenWslDebugExe) {
        $seenCompiler = $seenWslDebugExe
    } else {
        Write-Host "[WARNING] Seen compiler not found, building..." -ForegroundColor Yellow
        Set-Location $PROJECT_ROOT
        cargo build --release --bin seen
        if (Test-Path $seenExe) {
            $seenCompiler = $seenExe
        } else {
            Write-Host "[ERROR] Failed to build Seen compiler" -ForegroundColor Red
            return $null
        }
    }
    
    $seenResults = @{
        compression_times = @()
        decompression_times = @()
        compression_ratios = @()
        throughput_mb_per_sec = @()
        metadata = @{
            language = "seen"
            algorithm = "lz77"
            compiler_path = $seenCompiler
        }
    }
    
    # Create temporary directory for benchmark
    $tempDir = "$env:TEMP\seen_compression_benchmark"
    New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
    
    # Check if built executable exists, otherwise build it
    $seenExecutable = Join-Path $PSScriptRoot "target\native\debug\compression_benchmark.exe"
    if (-not (Test-Path $seenExecutable)) {
        $seenExecutable = Join-Path $PSScriptRoot "target\native\debug\compression_benchmark"  # Try without .exe extension
    }
    if (-not (Test-Path $seenExecutable)) {
        Write-Host "  Building Seen benchmark executable..." -ForegroundColor Yellow
        & $seenCompiler "build" 2>&1 | Out-Null
        if (-not (Test-Path $seenExecutable)) {
            Write-Warning "Failed to build Seen executable, skipping Seen benchmarks"
            $seenResults.success = $false
            continue
        }
    }
    
    # Run actual Seen benchmarks
    for ($i = 0; $i -lt $Iterations; $i++) {
        if ($Verbose) {
            Write-Host "  Iteration $($i + 1)/$Iterations" -ForegroundColor Gray
        }
        
        # Skip the benchmark script creation - use the pre-built executable instead
        $benchmarkSeen = $compressionSeen
        $benchmarkCode = Get-Content $compressionSeen -Raw
        
        # Modify main function to output metrics
        $benchmarkMain = @"

fun benchmarkMain() {
    let compressor = SimpleCompressor.new();
    let mut totalCompressTime = 0.0;
    let mut totalDecompressTime = 0.0;
    let mut totalCompressionRatio = 0.0;
    let mut totalThroughput = 0.0;
    let testCount = 4;
    
    // Test different data types
    let testData = [
        generateTextData(1024 * 1024),      // 1MB text
        generateBinaryData(2 * 1024 * 1024),// 2MB binary
        generateSourceCode(512 * 1024),     // 512KB source
        generateMixedData(5 * 1024 * 1024), // 5MB mixed
    ];
    
    for (i, data) in testData.enumerate() {
        let result = compressor.compress(&data);
        
        let startDecompress = std.time.now();
        let decompressed = compressor.decompress(&result);
        let endDecompress = std.time.now();
        let decompressTime = (endDecompress - startDecompress).toSecondsF64();
        
        let originalSizeMB = data.len() as f64 / (1024.0 * 1024.0);
        let throughput = originalSizeMB / result.compression_time;
        
        totalCompressTime += result.compression_time;
        totalDecompressTime += decompressTime;
        totalCompressionRatio += result.compression_ratio;
        totalThroughput += throughput;
    }
    
    let avgCompressTime = totalCompressTime / testCount as f64;
    let avgDecompressTime = totalDecompressTime / testCount as f64;
    let avgCompressionRatio = totalCompressionRatio / testCount as f64;
    let avgThroughput = totalThroughput / testCount as f64;
    
    println("{}", avgCompressTime);
    println("{}", avgDecompressTime);
    println("{}", avgCompressionRatio);
    println("{}", avgThroughput);
}

fun main() {
    benchmarkMain();
}
"@
        
        # Replace the main function
        $modifiedCode = $benchmarkCode -replace 'fun main\(\).*?\n}$', $benchmarkMain
        $modifiedCode | Out-File -FilePath $benchmarkSeen -Encoding UTF8
        
        try {
            # Run the built Seen executable directly
            # Use WSL to execute the Linux binary if it doesn't have .exe extension
            if ($seenExecutable -notlike "*.exe") {
                # Manual path conversion from Windows to WSL format
                $wslPath = $seenExecutable -replace '\\', '/' -replace '^([A-Za-z]):', '/mnt/$1'
                $wslPath = $wslPath.ToLower()
                Write-Host "Executing via WSL: $wslPath" -ForegroundColor Gray
                $seenOutput = wsl bash -c "`"$wslPath`"" 2>&1
                Write-Host "WSL output: $($seenOutput -join ' ')" -ForegroundColor Gray
            } else {
                $seenOutput = & $seenExecutable 2>&1
            }
            
            if ($LASTEXITCODE -eq 0) {
                $outputText = $seenOutput -join " "
                
                # Look for the line with space-separated numbers
                if ($outputText -match "(\d+\.?\d*)\s+(\d+\.?\d*)\s+(\d+\.?\d*)\s+(\d+\.?\d*)") {
                    $compressTime = [double]$matches[1]
                    $decompressTime = [double]$matches[2]
                    $compressionRatio = [double]$matches[3]
                    $throughput = [double]$matches[4]
                    
                    $seenResults.compression_times += $compressTime
                    $seenResults.decompression_times += $decompressTime
                    $seenResults.compression_ratios += $compressionRatio
                    $seenResults.throughput_mb_per_sec += $throughput
                } else {
                    Write-Host "  [FAILED] Unexpected output format on iteration $($i + 1)" -ForegroundColor Red
                    # No fallback data - this iteration failed
                }
            } else {
                Write-Host "  [FAILED] Seen benchmark failed on iteration $($i + 1)" -ForegroundColor Red
                # No fallback data - this iteration failed
            }
        } catch {
            Write-Host "  [FAILED] Exception running Seen benchmark: $_" -ForegroundColor Red
            # No fallback data - this iteration failed
        }
    }
    
    # Cleanup
    Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    
    return $seenResults
}

# Execute Seen compression benchmark
$seenResults = Invoke-SeenCompressionBenchmark -Iterations $Iterations

$results.benchmarks.seen = $seenResults

# Test actual competitor implementations (no simulated data)
$competitorList = $Competitors -split ',' | ForEach-Object { $_.Trim() }
foreach ($competitor in $competitorList) {
    Write-Host "[INFO] Looking for real $competitor compression implementation..." -ForegroundColor Blue
    
    # Use correct file extensions for each language
    $extension = switch ($competitor) {
        "cpp" { ".cpp" }
        "rust" { ".rs" }
        "zig" { ".zig" }
        "c" { ".c" }
        default { ".$competitor" }
    }
    
    $competitorImpl = "$PROJECT_ROOT\performance_validation\benchmarks\real_implementations\compression_bench$extension"
    $competitorExe = "$PROJECT_ROOT\performance_validation\benchmarks\real_implementations\compression_bench_$($competitor).exe"
    
    if (Test-Path $competitorImpl) {
        Write-Host "[INFO] Found $competitor implementation, compiling..." -ForegroundColor Blue
        
        # Try to compile the competitor implementation
        $compiled = $false
        switch ($competitor) {
            "cpp" {
                if (Get-Command "g++" -ErrorAction SilentlyContinue) {
                    g++ -O3 -std=c++17 $competitorImpl -o $competitorExe 2>&1 | Out-Null
                    if (Test-Path $competitorExe) { $compiled = $true }
                }
            }
            "rust" {
                if (Get-Command "rustc" -ErrorAction SilentlyContinue) {
                    rustc -O $competitorImpl -o $competitorExe 2>&1 | Out-Null
                    if (Test-Path $competitorExe) { $compiled = $true }
                }
            }
        }
        
        if ($compiled) {
            Write-Host "[SUCCESS] $competitor implementation compiled" -ForegroundColor Green
            
            # Run actual competitor benchmark
            $competitorResults = @{
                compression_times = @()
                decompression_times = @()
                compression_ratios = @()
                throughput_mb_per_sec = @()
                metadata = @{ language = $competitor; algorithm = "actual_implementation" }
            }
            
            for ($i = 0; $i -lt $Iterations; $i++) {
                try {
                    $benchmarkOutput = & $competitorExe $i 2>&1
                    if ($LASTEXITCODE -eq 0) {
                        $lines = $benchmarkOutput -split '\n' | Where-Object { $_.Trim() -ne "" }
                        if ($lines.Count -ge 4) {
                            $competitorResults.compression_times += [double]$lines[-4]
                            $competitorResults.decompression_times += [double]$lines[-3]  
                            $competitorResults.compression_ratios += [double]$lines[-2]
                            $competitorResults.throughput_mb_per_sec += [double]$lines[-1]
                        }
                    } else {
                        Write-Host "  [FAILED] $competitor benchmark failed on iteration $($i + 1)" -ForegroundColor Red
                    }
                } catch {
                    Write-Host "  [FAILED] $competitor exception on iteration $($i + 1): $_" -ForegroundColor Red
                }
            }
            
            if ($competitorResults.compression_times.Count -gt 0) {
                $results.benchmarks.$competitor = $competitorResults
                Write-Host "[SUCCESS] $competitor benchmark completed with $($competitorResults.compression_times.Count) successful iterations" -ForegroundColor Green
            }
        } else {
            Write-Host "[SKIPPED] Failed to compile $competitor implementation" -ForegroundColor Yellow
        }
    } else {
        Write-Host "[SKIPPED] No real $competitor implementation found at: $competitorImpl" -ForegroundColor Yellow
    }
}

# Display results
$seenAvgTime = ($seenResults.compression_times | Measure-Object -Average).Average
$seenAvgRatio = ($seenResults.compression_ratios | Measure-Object -Average).Average
$seenAvgThroughput = ($seenResults.throughput_mb_per_sec | Measure-Object -Average).Average

Write-Host "[INFO] Compression Results:" -ForegroundColor Blue
Write-Host "Seen: $([math]::Round($seenAvgTime, 3))s, $([math]::Round($seenAvgRatio * 100, 1))% ratio, $([math]::Round($seenAvgThroughput, 1)) MB/s" -ForegroundColor White

# Write results
Write-Host "[DEBUG] Output parameter value: '$Output'" -ForegroundColor Magenta
Write-Host "[DEBUG] Results has these languages: $($results.benchmarks.PSObject.Properties.Name -join ', ')" -ForegroundColor Magenta
$jsonContent = $results | ConvertTo-Json -Depth 10
$jsonContent | Out-File -FilePath $Output -Encoding UTF8
Write-Host "[SUCCESS] Results written to: $Output" -ForegroundColor Green
Write-Host "[DEBUG] File exists: $(Test-Path $Output)" -ForegroundColor Magenta
Write-Host "[INFO] Compression benchmark completed" -ForegroundColor Blue
Write-Output "Results written to: $Output"  # For background job capture