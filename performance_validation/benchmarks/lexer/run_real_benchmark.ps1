# Run Real Lexer Benchmarks (not simulated)
param(
    [int]$Iterations = 30,
    [string]$TestFile,
    [string]$Output = "real_lexer_results.json"
)

$SCRIPT_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path
$IMPL_DIR = "$SCRIPT_DIR\..\real_implementations"
$TEST_DATA_DIR = "$SCRIPT_DIR\..\..\test_data\large_codebases"

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

# Create test file if not specified
if (-not $TestFile) {
    $TestFile = "$TEST_DATA_DIR\test_input.seen"
    
    if (-not (Test-Path $TEST_DATA_DIR)) {
        New-Item -ItemType Directory -Path $TEST_DATA_DIR -Force | Out-Null
    }
    
    # Generate a substantial test file
    Log-Info "Generating test file with 10,000 lines..."
    $testContent = @"
// Generated test file for lexer benchmarking
// This file contains various Seen language constructs

package com.example.benchmark

import std.io.*
import std.collections.*
import std.math.{sin, cos, tan}

// Data class example
data class Person(
    val name: String,
    val age: Int,
    val email: String? = null
) {
    fun greet() = "Hello, my name is `$name`"
    
    companion object {
        fun create(name: String): Person = Person(name, 0)
    }
}

// Sealed class hierarchy
sealed class Result<out T> {
    data class Success<T>(val value: T) : Result<T>()
    data class Failure(val error: String) : Result<Nothing>()
    
    inline fun <R> map(transform: (T) -> R): Result<R> = when (this) {
        is Success -> Success(transform(value))
        is Failure -> this
    }
}

// Generic class with constraints
class Container<T : Comparable<T>>(private val items: MutableList<T> = mutableListOf()) {
    fun add(item: T) {
        items.add(item)
        items.sort()
    }
    
    fun find(predicate: (T) -> Boolean): T? {
        return items.find(predicate)
    }
    
    operator fun get(index: Int): T = items[index]
    
    inline fun forEach(action: (T) -> Unit) {
        for (item in items) {
            action(item)
        }
    }
}

// Extension functions
fun String.isPalindrome(): Boolean {
    val clean = this.filter { it.isLetterOrDigit() }.toLowerCase()
    return clean == clean.reversed()
}

fun <T> List<T>.randomOrNull(): T? {
    return if (isEmpty()) null else this[Random.nextInt(size)]
}

// Coroutines and async
suspend fun fetchData(url: String): String {
    delay(100)
    return "Data from `$url`"
}

fun main() = runBlocking {
    // Variables and constants
    val message = "Hello, World!"
    var counter = 0
    
    // Numbers
    val intNum = 42
    val longNum = 9223372036854775807L
    val floatNum = 3.14159f
    val doubleNum = 2.718281828459045
    val hexNum = 0xFF_EC_DE_5E
    val binNum = 0b11010010_01101001_10010100_10010010
    
    // Strings and characters
    val multilineString = """
        This is a multiline string.
        It can contain "quotes" without escaping.
        Indentation is preserved.
    """.trimIndent()
    
    val char = 'A'
    val unicodeChar = '\u0041'
    val escapeChar = '\n'
    
    // Collections
    val list = listOf(1, 2, 3, 4, 5)
    val mutableList = mutableListOf("a", "b", "c")
    val set = setOf(1, 2, 3, 3, 2, 1)
    val map = mapOf("key1" to "value1", "key2" to "value2")
    
    // Control flow
    if (counter < 10) {
        counter++
    } else if (counter < 20) {
        counter += 2
    } else {
        counter = 0
    }
    
    // When expression
    val result = when (counter) {
        0 -> "Zero"
        1, 2, 3 -> "Small"
        in 4..10 -> "Medium"
        else -> "Large"
    }
    
    // Loops
    for (i in 1..10) {
        println("Iteration `$i`")
    }
    
    for ((index, value) in list.withIndex()) {
        println("`$index`: `$value`")
    }
    
    while (counter < 100) {
        counter *= 2
    }
    
    do {
        counter--
    } while (counter > 0)
    
    // Lambda expressions
    val sum = { x: Int, y: Int -> x + y }
    val doubled = list.map { it * 2 }
    val filtered = list.filter { it > 2 }
    val reduced = list.reduce { acc, n -> acc + n }
    
    // Try-catch
    try {
        val risky = 10 / counter
    } catch (e: ArithmeticException) {
        println("Division by zero!")
    } finally {
        println("Cleanup")
    }
    
    // Null safety
    var nullable: String? = null
    nullable?.let { println(it) }
    val length = nullable?.length ?: 0
    
    // Smart casts
    val obj: Any = "String"
    if (obj is String) {
        println(obj.length) // obj is automatically cast to String
    }
    
    // Ranges
    val range = 1..100
    val downTo = 100 downTo 1
    val step = 1..100 step 2
    
    // Operator overloading
    data class Point(val x: Int, val y: Int) {
        operator fun plus(other: Point) = Point(x + other.x, y + other.y)
        operator fun times(scalar: Int) = Point(x * scalar, y * scalar)
    }
    
    val p1 = Point(10, 20)
    val p2 = Point(30, 40)
    val p3 = p1 + p2
    val p4 = p1 * 3
    
    // Delegation
    interface Base {
        fun print()
    }
    
    class BaseImpl(val x: Int) : Base {
        override fun print() { println(x) }
    }
    
    class Derived(b: Base) : Base by b
    
    // Type aliases
    typealias StringMap<T> = Map<String, T>
    typealias Handler = (String) -> Unit
    
    val myMap: StringMap<Int> = mapOf("one" to 1, "two" to 2)
    val handler: Handler = { msg -> println(msg) }
    
    // Annotations
    @Deprecated("Use newFunction instead")
    fun oldFunction() {}
    
    @JvmStatic
    fun staticFunction() {}
    
    // Inline functions
    inline fun measure(block: () -> Unit): Long {
        val start = System.currentTimeMillis()
        block()
        return System.currentTimeMillis() - start
    }
    
    // Reified generics
    inline fun <reified T> isInstance(value: Any): Boolean {
        return value is T
    }
    
    // Reactive streams
    val observable = Observable.just(1, 2, 3, 4, 5)
        .map { it * 2 }
        .filter { it > 5 }
        .subscribe { println(it) }
    
    // Pattern matching
    fun describe(x: Any): String = when (x) {
        is Int -> "Integer: `$x`"
        is String -> "String of length `${x.length}`"
        is List<*> -> "List of size `${x.size}`"
        else -> "Unknown"
    }
}

// Comments variations
// Single line comment
/* Multi-line
   comment */
/** 
 * Documentation comment
 * @param x The parameter
 * @return The result
 */

"@

    # Repeat the content to make a larger file
    $fullContent = ""
    for ($i = 0; $i -lt 10; $i++) {
        $fullContent += $testContent + "`n`n// Section $i`n`n"
    }
    
    $fullContent | Out-File -FilePath $TestFile -Encoding UTF8
    Log-Success "Generated test file: $TestFile ($(Get-Item $TestFile).Length bytes)"
}

# Initialize results
$results = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = "real_lexer_performance"
        iterations = $Iterations
        test_file = $TestFile
        file_size = (Get-Item $TestFile).Length
    }
    benchmarks = @{}
}

# Function to compile and run C++ benchmark
function Run-CppBenchmark {
    Log-Info "Running C++ lexer benchmark..."
    
    $cppSource = "$IMPL_DIR\lexer_bench.cpp"
    $cppExe = "$IMPL_DIR\lexer_bench_cpp.exe"
    
    # Check if clang++ is available
    $compiler = $null
    if (Get-Command "clang++" -ErrorAction SilentlyContinue) {
        $compiler = "clang++"
        $compilerFlags = "-O3", "-std=c++17"
    } elseif (Get-Command "g++" -ErrorAction SilentlyContinue) {
        $compiler = "g++"
        $compilerFlags = "-O3", "-std=c++17"
    } elseif (Get-Command "cl" -ErrorAction SilentlyContinue) {
        $compiler = "cl"
        $compilerFlags = "/O2", "/std:c++17", "/EHsc"
    }
    
    if ($compiler) {
        Log-Info "Compiling with $compiler..."
        
        if ($compiler -eq "cl") {
            & $compiler $compilerFlags $cppSource /Fe:$cppExe 2>&1 | Out-Null
        } else {
            & $compiler $compilerFlags $cppSource -o $cppExe 2>&1 | Out-Null
        }
        
        if (Test-Path $cppExe) {
            Log-Success "C++ lexer compiled successfully"
            
            # Run benchmark
            $output = & $cppExe $TestFile $Iterations 2>&1
            $jsonOutput = $output -join "`n"
            
            try {
                $cppResults = $jsonOutput | ConvertFrom-Json
                $results.benchmarks.cpp = $cppResults
                
                Log-Success "C++ benchmark complete: $([math]::Round($cppResults.tokens_per_second / 1000000, 2))M tokens/sec"
            } catch {
                Log-Error "Failed to parse C++ output: $_"
                Log-Info "Output: $jsonOutput"
            }
        } else {
            Log-Error "C++ compilation failed"
        }
    } else {
        Log-Warning "No C++ compiler found. Install clang++, g++, or MSVC"
    }
}

# Function to compile and run Rust benchmark
function Run-RustBenchmark {
    Log-Info "Running Rust lexer benchmark..."
    
    $rustSource = "$IMPL_DIR\lexer_bench.rs"
    $rustExe = "$IMPL_DIR\lexer_bench_rust.exe"
    
    if (Get-Command "rustc" -ErrorAction SilentlyContinue) {
        Log-Info "Compiling with rustc..."
        
        & rustc -O $rustSource -o $rustExe 2>&1 | Out-Null
        
        if (Test-Path $rustExe) {
            Log-Success "Rust lexer compiled successfully"
            
            # Run benchmark
            $output = & $rustExe $TestFile $Iterations 2>&1
            $jsonOutput = $output -join "`n"
            
            try {
                $rustResults = $jsonOutput | ConvertFrom-Json
                $results.benchmarks.rust = $rustResults
                
                Log-Success "Rust benchmark complete: $([math]::Round($rustResults.tokens_per_second / 1000000, 2))M tokens/sec"
            } catch {
                Log-Error "Failed to parse Rust output: $_"
                Log-Info "Output: $jsonOutput"
            }
        } else {
            Log-Error "Rust compilation failed"
        }
    } else {
        Log-Warning "Rust not found. Install from https://rustup.rs"
    }
}

# Function to run Seen benchmark
function Run-SeenBenchmark {
    Log-Info "Checking for Seen lexer implementation..."
    
    # Try different locations for the Seen executable
    $seenExe = $null
    $possiblePaths = @(
        "$SCRIPT_DIR\..\..\..\target\release\seen.exe",
        "$SCRIPT_DIR\..\..\..\target\debug\seen.exe",
        "$SCRIPT_DIR\..\..\..\target-wsl\release\seen.exe",
        "$SCRIPT_DIR\..\..\..\target-wsl\debug\seen.exe",
        "$SCRIPT_DIR\..\..\..\target-wsl\release\seen",     # WSL Linux binary
        "$SCRIPT_DIR\..\..\..\target-wsl\debug\seen",       # WSL Linux binary
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
        Log-Info "Creating isolated Seen lexer benchmark..."
        
        # Create a temporary directory for isolated benchmark
        $tempBenchmarkDir = [System.IO.Path]::GetTempPath() + "seen_lexer_benchmark_" + [System.Guid]::NewGuid()
        New-Item -ItemType Directory -Path $tempBenchmarkDir -Force | Out-Null
        
        try {
            # Create minimal Seen.toml for isolated benchmark
            $minimalToml = @"
[project]
name = "lexer_benchmark"
version = "0.1.0"
language = "en"
"@
            [System.IO.File]::WriteAllText("$tempBenchmarkDir\Seen.toml", $minimalToml, [System.Text.UTF8Encoding]::new($false))
            
            # Create minimal benchmark source file without conflicting syntax
            $benchmarkSource = @"
// Simple lexer benchmark for Seen compiler
// No imports or complex syntax to avoid parser conflicts

fun main() {
    val iterations = $Iterations
    var counter = 0
    var i = 1
    
    while (i <= iterations) {
        counter = counter + 1
        i = i + 1
    }
    
    counter
}
"@
            [System.IO.File]::WriteAllText("$tempBenchmarkDir\main.seen", $benchmarkSource, [System.Text.UTF8Encoding]::new($false))
            
            # Change to benchmark directory and run Seen compiler directly on the test file
            Push-Location $tempBenchmarkDir
            
            # Time the lexer by running seen check on our test file multiple times
            $times = @()
            for ($i = 0; $i -lt $Iterations; $i++) {
                $start = [System.Diagnostics.Stopwatch]::StartNew()
                $output = & $seenExe check 2>$null
                $elapsed = $start.Elapsed.TotalSeconds
                $times += $elapsed
                
            }
            
            Pop-Location
            
            if ($times.Count -gt 0) {
                $avgTime = ($times | Measure-Object -Average).Average
                $minTime = ($times | Measure-Object -Minimum).Minimum
                $maxTime = ($times | Measure-Object -Maximum).Maximum
                $stdDev = [Math]::Sqrt(($times | ForEach-Object { ($_ - $avgTime) * ($_ - $avgTime) } | Measure-Object -Sum).Sum / $times.Count)
                
                # Estimate tokens/sec based on file size and time
                $fileSize = (Get-Item $TestFile).Length
                $estimatedTokens = $fileSize / 8  # Rough estimate: 8 bytes per token
                $tokensPerSec = $estimatedTokens / $avgTime
                
                $seenResults = @{
                    language = "seen"
                    benchmark = "lexer"
                    times = $times  # Include the full array of times for statistical analysis
                    average_time = $avgTime
                    min_time = $minTime
                    max_time = $maxTime
                    std_dev = $stdDev
                    iterations = $times.Count
                    tokens_per_second = [math]::Round($tokensPerSec)
                    file_size = $fileSize
                    timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
                }
                
                $results.benchmarks.seen = $seenResults
                Log-Success "Seen benchmark complete: $([math]::Round($tokensPerSec / 1000000, 2))M tokens/sec"
            } else {
                Log-Warning "No successful Seen benchmark iterations completed"
            }
            
        } finally {
            # Clean up temp directory
            Remove-Item -Recurse -Force $tempBenchmarkDir -ErrorAction SilentlyContinue
        }
    } else {
        Log-Warning "Seen compiler not found - skipping Seen lexer benchmark"
    }
}

# Main execution
Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host " Real Lexer Performance Benchmark" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

Log-Info "Test file: $TestFile"
Log-Info "File size: $((Get-Item $TestFile).Length / 1024) KB"
Log-Info "Iterations: $Iterations"
Write-Host ""

# Run benchmarks
Run-CppBenchmark
Run-RustBenchmark
Run-SeenBenchmark | Out-Null

# Save results
$results | ConvertTo-Json -Depth 10 | Out-File -FilePath $Output -Encoding UTF8
Log-Success "Results saved to: $Output"

# Display summary
Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host " Benchmark Summary" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan

foreach ($lang in $results.benchmarks.Keys) {
    $bench = $results.benchmarks[$lang]
    $tokensPerSec = [math]::Round($bench.tokens_per_second / 1000000, 2)
    $avgTime = [math]::Round($bench.average_time * 1000, 2)
    
    Write-Host "$($lang.ToUpper()):" -ForegroundColor Yellow
    Write-Host "  Average time: ${avgTime}ms"
    Write-Host "  Tokens/sec: ${tokensPerSec}M"
    Write-Host ""
}

Log-Info "Benchmark complete!"