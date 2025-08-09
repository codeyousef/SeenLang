# Run Real Runtime Benchmarks (not simulated)
param(
    [int]$Iterations = 30,
    [string]$TestFile,
    [string]$Output = "real_runtime_results.json"
)

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path

# Color codes
$GREEN = "Green"
$RED = "Red"
$YELLOW = "Yellow"
$BLUE = "Blue"

function Log-Info($Message) {
    Write-Host "[INFO] $Message" -ForegroundColor $BLUE
}

function Log-Success($Message) {
    Write-Host "[SUCCESS] $Message" -ForegroundColor $GREEN
}

function Log-Error($Message) {
    Write-Host "[ERROR] $Message" -ForegroundColor $RED
}

function Log-Warning($Message) {
    Write-Host "[WARNING] $Message" -ForegroundColor $YELLOW
}

# Initialize results
$results = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = "real_runtime_performance"
        iterations = $Iterations
    }
    benchmarks = @{}
}

$IMPL_DIR = "$SCRIPT_DIR\..\real_implementations"

# Function to run C++ runtime benchmark
function Run-CppBenchmark {
    Log-Info "Running C++ runtime benchmark..."
    
    $cppSource = "$IMPL_DIR\runtime_bench.cpp"
    $cppExe = "$IMPL_DIR\runtime_bench_cpp.exe"
    
    $compiler = $null
    if (Get-Command "g++" -ErrorAction SilentlyContinue) {
        $compiler = "g++"
        $flags = "-O3", "-std=c++17"
    }
    
    if ($compiler -and (Test-Path $cppSource)) {
        & $compiler $flags $cppSource -o $cppExe 2>&1 | Out-Null
        
        if (Test-Path $cppExe) {
            $output = & $cppExe $Iterations 2>&1
            try {
                $cppResults = $output | ConvertFrom-Json
                $results.benchmarks.cpp = $cppResults
                Log-Success "C++ runtime benchmark complete"
            } catch {
                Log-Error "Failed to parse C++ output"
            }
        }
    }
}

# Function to run Rust runtime benchmark
function Run-RustBenchmark {
    Log-Info "Running Rust runtime benchmark..."
    
    $rustSource = "$IMPL_DIR\runtime_bench.rs"
    $rustExe = "$IMPL_DIR\runtime_bench_rust.exe"
    
    if ((Get-Command "rustc" -ErrorAction SilentlyContinue) -and (Test-Path $rustSource)) {
        rustc -O $rustSource -o $rustExe 2>&1 | Out-Null
        
        if (Test-Path $rustExe) {
            $output = & $rustExe $Iterations 2>&1
            try {
                $rustResults = $output | ConvertFrom-Json
                $results.benchmarks.rust = $rustResults
                Log-Success "Rust runtime benchmark complete"
            } catch {
                Log-Error "Failed to parse Rust output"
            }
        }
    } else {
        Log-Warning "Rust compiler or source not found - skipping Rust runtime benchmark"
    }
}

# Function to run Seen runtime benchmark
function Run-SeenBenchmark {
    Log-Info "Checking for Seen runtime implementation..."
    
    # Try different locations for the Seen executable
    $seenExe = $null
    $projectRoot = (Get-Item "$SCRIPT_DIR\..\..\..").FullName
    $possiblePaths = @(
        "$projectRoot\target\release\seen.exe",
        "$projectRoot\target\debug\seen.exe", 
        "$projectRoot\target-wsl\release\seen.exe",
        "$projectRoot\target-wsl\debug\seen.exe",
        "seen.exe",  # Try PATH
        "seen"       # Try PATH (no extension)
    )
    
    foreach ($path in $possiblePaths) {
        if (Test-Path $path) {
            $seenExe = $path
            break
        } elseif (Get-Command $path -ErrorAction SilentlyContinue) {
            $seenExe = $path
            break
        }
    }
    
    if ($seenExe) {
        Log-Info "Found Seen compiler at: $seenExe"
        Log-Info "Creating isolated Seen runtime benchmark..."
        
        # Create a temporary directory for isolated benchmark
        $tempBenchmarkDir = [System.IO.Path]::GetTempPath() + "seen_runtime_benchmark_" + [System.Guid]::NewGuid()
        New-Item -ItemType Directory -Path $tempBenchmarkDir -Force | Out-Null
        
        try {
            # Create minimal Seen.toml for isolated benchmark
            $minimalToml = @"
[project]
name = "runtime_benchmark"
version = "0.1.0"
language = "en"
"@
            [System.IO.File]::WriteAllText("$tempBenchmarkDir\Seen.toml", $minimalToml, [System.Text.UTF8Encoding]::new($false))
            
            # Create runtime benchmark source
            $benchmarkSource = @"
// Runtime benchmark for Seen compiler
fun main() {
    val iterations = $Iterations
    var sum = 0
    var i = 1
    
    while (i <= iterations) {
        sum = sum + (i * i)
        i = i + 1
    }
    
    sum
}
"@
            [System.IO.File]::WriteAllText("$tempBenchmarkDir\main.seen", $benchmarkSource, [System.Text.UTF8Encoding]::new($false))
            
            # Change to benchmark directory and run Seen compiler
            Push-Location $tempBenchmarkDir
            
            # Time the runtime performance by running seen run multiple times
            $times = @()
            for ($i = 0; $i -lt $Iterations; $i++) {
                $start = [System.Diagnostics.Stopwatch]::StartNew()
                $output = & $seenExe run 2>$null
                $elapsed = $start.Elapsed.TotalSeconds
                $times += $elapsed
                
            }
            
            Pop-Location
            
            if ($times.Count -gt 0) {
                $avgTime = ($times | Measure-Object -Average).Average
                $minTime = ($times | Measure-Object -Minimum).Minimum
                $maxTime = ($times | Measure-Object -Maximum).Maximum
                $stdDev = [Math]::Sqrt(($times | ForEach-Object { ($_ - $avgTime) * ($_ - $avgTime) } | Measure-Object -Sum).Sum / $times.Count)
                
                # Estimate operations per second
                $runtimeOpsPerSec = $Iterations / $avgTime
                
                $seenResults = @{
                    language = "seen"
                    benchmark = "runtime"
                    times = $times  # Include the full array of times for statistical analysis
                    average_time = $avgTime
                    min_time = $minTime
                    max_time = $maxTime
                    std_dev = $stdDev
                    iterations = $times.Count
                    runtime_ops_per_second = [math]::Round($runtimeOpsPerSec)
                    timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
                }
                
                $results.benchmarks.seen = $seenResults
                Log-Success "Seen benchmark complete: $([math]::Round($runtimeOpsPerSec, 2)) ops/sec"
            } else {
                Log-Warning "No successful Seen benchmark iterations completed"
            }
            
        } finally {
            # Clean up temp directory
            Remove-Item -Recurse -Force $tempBenchmarkDir -ErrorAction SilentlyContinue
        }
    } else {
        Log-Warning "Seen compiler not found - skipping Seen runtime benchmark"
    }
}

# Run benchmarks
Run-CppBenchmark
Run-RustBenchmark
Run-SeenBenchmark

# Remove error message if we have results
if ($results.benchmarks.Count -gt 0) {
    $results.Remove("error")
}

# Save results
$results | ConvertTo-Json -Depth 10 | Out-File -FilePath $Output -Encoding UTF8
Log-Success "Results saved to: $Output"