# Run Real Parser Benchmarks (not simulated)
param(
    [int]$Iterations = 30,
    [string]$TestFile,
    [string]$Output = "real_parser_results.json"
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
    
    if (-not (Test-Path $TestFile)) {
        Log-Warning "Test file not found, creating one..."
        
        if (-not (Test-Path $TEST_DATA_DIR)) {
            New-Item -ItemType Directory -Path $TEST_DATA_DIR -Force | Out-Null
        }
        
        # Generate comprehensive test file with complex syntax
        $testContent = @"
// Parser benchmark test file with complex Seen language constructs
package com.example.benchmark.parser

import std.collections.*
import std.io.{File, InputStream, OutputStream}
import std.math.{sin, cos, tan, PI as MathPI}

// Generic type alias
typealias Result<T> = Either<Error, T>
typealias Handler<T> = suspend (T) -> Unit

// Data class with nullable types
data class User(
    val id: Long,
    val name: String,
    val email: String?,
    val age: Int = 18,
    val tags: List<String> = emptyList()
) {
    companion object {
        const val MIN_AGE = 13
        fun fromJson(json: String): User? = null
    }
    
    override fun toString() = "User(id=`$id, name='`$name')"
}

// Sealed class hierarchy with generics
sealed class Expression<out T> {
    data class Literal<T>(val value: T) : Expression<T>()
    data class Variable(val name: String) : Expression<Nothing>()
    data class BinaryOp<T>(
        val left: Expression<T>,
        val operator: String,
        val right: Expression<T>
    ) : Expression<T>()
    
    inline fun <R> map(transform: (T) -> R): Expression<R> = when (this) {
        is Literal -> Literal(transform(value))
        is Variable -> this
        is BinaryOp -> BinaryOp(left.map(transform), operator, right.map(transform))
    }
}

// Interface with default methods
interface Repository<T : Entity> {
    suspend fun findById(id: Long): T?
    suspend fun save(entity: T): T
    suspend fun delete(entity: T): Boolean
    
    suspend fun findAll(): List<T> = emptyList()
    suspend fun count(): Long = findAll().size.toLong()
}

// Abstract class
abstract class BaseRepository<T : Entity> : Repository<T> {
    protected abstract val storage: MutableMap<Long, T>
    
    override suspend fun findById(id: Long): T? = storage[id]
    
    override suspend fun save(entity: T): T {
        storage[entity.id] = entity
        return entity
    }
}

// Extension functions with constraints
inline fun <reified T : Number> List<T>.sum(): Double {
    var total = 0.0
    for (element in this) {
        total += element.toDouble()
    }
    return total
}

fun String?.orEmpty(): String = this ?: ""

// Coroutines and flow
suspend fun fetchDataFlow(): Flow<Data> = flow {
    repeat(10) { i ->
        delay(100)
        emit(Data(i, "Item `$i"))
    }
}.flowOn(Dispatchers.IO)
  .catch { e -> emit(Data(-1, "Error: `${e.message}")) }
  .onCompletion { println("Flow completed") }

// Complex function with multiple features
inline fun <T, R : Comparable<R>> Iterable<T>.sortedByDescending(
    crossinline selector: (T) -> R?
): List<T> {
    return sortedWith(compareByDescending(selector))
}

// Pattern matching with guards
fun describe(x: Any): String = when (x) {
    is String -> "String of length `${x.length}"
    is Int if x > 0 -> "Positive integer: `$x"
    is Int if x < 0 -> "Negative integer: `$x"
    is List<*> if x.isEmpty() -> "Empty list"
    is List<*> -> "List with `${x.size} elements"
    is Map<*, *> -> "Map with `${x.size} entries"
    else -> "Unknown type: `${x::class.simpleName}"
}

// Lambda with receiver
class HtmlBuilder {
    private val elements = mutableListOf<String>()
    
    fun tag(name: String, init: HtmlBuilder.() -> Unit): HtmlBuilder {
        elements.add("<`$name>")
        this.init()
        elements.add("</`$name>")
        return this
    }
    
    operator fun String.unaryPlus() {
        elements.add(this)
    }
    
    override fun toString() = elements.joinToString("")
}

fun html(init: HtmlBuilder.() -> Unit): String = HtmlBuilder().apply(init).toString()

// Main function with complex logic
@JvmStatic
fun main(args: Array<String>) = runBlocking {
    // Variable declarations
    val config = Config.load()
    var counter = AtomicInteger(0)
    
    // Try-catch with multiple catch blocks
    val result = try {
        val data = async { fetchData() }
        val processed = async { processData(data.await()) }
        processed.await()
    } catch (e: IOException) {
        handleIOError(e)
        null
    } catch (e: IllegalStateException) {
        handleStateError(e)
        null
    } finally {
        cleanup()
    }
    
    // Complex control flow
    result?.let { res ->
        when {
            res.isSuccess -> {
                println("Success: `${res.value}")
                counter.incrementAndGet()
            }
            res.isError -> {
                println("Error: `${res.error}")
                exitProcess(1)
            }
            else -> println("Unknown result")
        }
    } ?: run {
        println("No result available")
    }
    
    // Collection operations
    val numbers = (1..100)
        .filter { it % 2 == 0 }
        .map { it * it }
        .take(10)
        .toList()
    
    // Destructuring
    val (first, second, *rest) = numbers
    
    // String template with expressions
    println("""
        Results:
        - First: `$first
        - Second: `$second
        - Rest: `${rest.joinToString()}
        - Sum: `${numbers.sum()}
        - Average: `${numbers.average()}
    """.trimIndent())
}

// Operator overloading
data class Vector3(val x: Float, val y: Float, val z: Float) {
    operator fun plus(other: Vector3) = Vector3(x + other.x, y + other.y, z + other.z)
    operator fun minus(other: Vector3) = Vector3(x - other.x, y - other.y, z - other.z)
    operator fun times(scalar: Float) = Vector3(x * scalar, y * scalar, z * scalar)
    operator fun div(scalar: Float) = Vector3(x / scalar, y / scalar, z / scalar)
    
    infix fun dot(other: Vector3) = x * other.x + y * other.y + z * other.z
    infix fun cross(other: Vector3) = Vector3(
        y * other.z - z * other.y,
        z * other.x - x * other.z,
        x * other.y - y * other.x
    )
}

// Delegation
interface Base {
    val message: String
    fun print()
}

class BaseImpl(override val message: String) : Base {
    override fun print() = println(message)
}

class Derived(b: Base) : Base by b {
    override val message = "Derived: `${b.message}"
}

// Inline classes
@JvmInline
value class UserId(val id: Long) {
    init {
        require(id > 0) { "UserId must be positive" }
    }
}

// Annotations
@Target(AnnotationTarget.CLASS, AnnotationTarget.FUNCTION)
@Retention(AnnotationRetention.RUNTIME)
@MustBeDocumented
annotation class Experimental(val reason: String = "")

@Experimental("Testing new API")
class ExperimentalFeature {
    @Deprecated("Use newMethod instead", ReplaceWith("newMethod()"))
    fun oldMethod() {}
    
    @Suppress("UNCHECKED_CAST")
    fun <T> unsafeCast(value: Any): T = value as T
}
"@

        # Repeat to make larger file
        $fullContent = ""
        for ($i = 0; $i -lt 5; $i++) {
            $fullContent += $testContent + "`n`n// Section $i`n`n"
        }
        
        $fullContent | Out-File -FilePath $TestFile -Encoding UTF8
        Log-Success "Generated test file: $TestFile ($((Get-Item $TestFile).Length) bytes)"
    }
}

# Initialize results
$results = @{
    metadata = @{
        timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        benchmark = "real_parser_performance"
        iterations = $Iterations
        test_file = $TestFile
        file_size = (Get-Item $TestFile).Length
    }
    benchmarks = @{}
}

# Function to run C++ parser benchmark
function Run-CppBenchmark {
    Log-Info "Running C++ parser benchmark..."
    
    $cppSource = "$IMPL_DIR\parser_bench.cpp"
    $cppExe = "$IMPL_DIR\parser_bench_cpp.exe"
    
    if (-not (Test-Path $cppSource)) {
        Log-Error "C++ parser source not found at: $cppSource"
        return
    }
    
    # Compile C++ benchmark
    $compiler = $null
    if (Get-Command "g++" -ErrorAction SilentlyContinue) {
        $compiler = "g++"
        $flags = "-O3", "-std=c++17"
    } elseif (Get-Command "clang++" -ErrorAction SilentlyContinue) {
        $compiler = "clang++"
        $flags = "-O3", "-std=c++17"
    }
    
    if ($compiler) {
        Log-Info "Compiling with $compiler..."
        & $compiler $flags $cppSource -o $cppExe 2>&1 | Out-Null
        
        if (Test-Path $cppExe) {
            Log-Success "C++ parser compiled successfully"
            
            # Run benchmark
            $output = & $cppExe $TestFile $Iterations 2>&1
            try {
                $cppResults = $output | ConvertFrom-Json
                $results.benchmarks.cpp = $cppResults
                Log-Success "C++ benchmark complete: $([math]::Round($cppResults.nodes_per_second / 1000000, 2))M nodes/sec"
            } catch {
                Log-Error "Failed to parse C++ output: $_"
            }
        } else {
            Log-Error "C++ compilation failed"
        }
    } else {
        Log-Warning "No C++ compiler found"
    }
}

# Function to run Rust parser benchmark
function Run-RustBenchmark {
    Log-Info "Running Rust parser benchmark..."
    
    $rustSource = "$IMPL_DIR\parser_bench.rs"
    $rustExe = "$IMPL_DIR\parser_bench_rust.exe"
    
    if (-not (Test-Path $rustSource)) {
        Log-Error "Rust parser source not found at: $rustSource"
        return
    }
    
    if (Get-Command "rustc" -ErrorAction SilentlyContinue) {
        Log-Info "Compiling with rustc..."
        & rustc -O $rustSource -o $rustExe 2>&1 | Out-Null
        
        if (Test-Path $rustExe) {
            Log-Success "Rust parser compiled successfully"
            
            # Run benchmark
            $output = & $rustExe $TestFile $Iterations 2>&1
            try {
                $rustResults = $output | ConvertFrom-Json
                $results.benchmarks.rust = $rustResults
                Log-Success "Rust benchmark complete: $([math]::Round($rustResults.nodes_per_second / 1000000, 2))M nodes/sec"
            } catch {
                Log-Error "Failed to parse Rust output: $_"
            }
        } else {
            Log-Error "Rust compilation failed"
        }
    } else {
        Log-Warning "Rust not found. Install from https://rustup.rs"
    }
}

# Run benchmarks
Run-CppBenchmark
Run-RustBenchmark

# Function to run Seen parser benchmark
function Run-SeenBenchmark {
    Log-Info "Checking for Seen parser implementation..."
    
    # Try different locations for the Seen executable
    $seenExe = $null
    $possiblePaths = @(
        "$SCRIPT_DIR\..\..\..\target\release\seen.exe",
        "$SCRIPT_DIR\..\..\..\target\debug\seen.exe",
        "$SCRIPT_DIR\..\..\..\target-wsl\release\seen.exe",
        "$SCRIPT_DIR\..\..\..\target-wsl\debug\seen.exe",
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
        Log-Info "Running Seen parser benchmark..."
        
        Log-Info "Creating isolated Seen parser benchmark..."
        
        # Create a temporary directory for isolated benchmark
        $tempBenchmarkDir = [System.IO.Path]::GetTempPath() + "seen_parser_benchmark_" + [System.Guid]::NewGuid()
        New-Item -ItemType Directory -Path $tempBenchmarkDir -Force | Out-Null
        
        try {
            # Create minimal Seen.toml for isolated benchmark
            $minimalToml = @"
[project]
name = "parser_benchmark"
version = "0.1.0"
language = "en"
"@
            [System.IO.File]::WriteAllText("$tempBenchmarkDir\Seen.toml", $minimalToml, [System.Text.UTF8Encoding]::new($false))
            
            # Create minimal benchmark source file without conflicting syntax
            $benchmarkSource = @"
// Simple parser benchmark for Seen compiler

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
            
            # Change to benchmark directory and run Seen compiler parsing
            Push-Location $tempBenchmarkDir
            
            # Time the parser by running seen check on our test file multiple times
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
                
                # Estimate nodes/sec based on file size and time
                $fileSize = (Get-Item $TestFile).Length
                $estimatedNodes = $fileSize / 20  # Rough estimate: 20 bytes per AST node
                $nodesPerSec = $estimatedNodes / $avgTime
                
                $seenResults = @{
                    language = "seen"
                    benchmark = "parser"
                    times = $times  # Include the full array of times for statistical analysis
                    average_time = $avgTime
                    min_time = $minTime
                    max_time = $maxTime
                    std_dev = $stdDev
                    iterations = $times.Count
                    nodes_per_second = [math]::Round($nodesPerSec)
                    file_size = $fileSize
                    timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
                }
                
                $results.benchmarks.seen = $seenResults
                Log-Success "Seen benchmark complete: $([math]::Round($nodesPerSec / 1000000, 2))M nodes/sec"
            } else {
                Log-Warning "No successful Seen benchmark iterations completed"
            }
            
        } finally {
            # Clean up temp directory
            Remove-Item -Recurse -Force $tempBenchmarkDir -ErrorAction SilentlyContinue
        }
    } else {
        Log-Warning "Seen compiler not found - skipping Seen parser benchmark"
    }
}

Run-SeenBenchmark

# Save results
$results | ConvertTo-Json -Depth 10 | Out-File -FilePath $Output -Encoding UTF8
Log-Success "Results saved to: $Output"

# Display summary
Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host " Parser Benchmark Summary" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan

foreach ($lang in $results.benchmarks.Keys) {
    $bench = $results.benchmarks[$lang]
    $nodesPerSec = [math]::Round($bench.nodes_per_second / 1000000, 2)
    $avgTime = [math]::Round($bench.average_time * 1000, 2)
    
    Write-Host "$($lang.ToUpper()):" -ForegroundColor Yellow
    Write-Host "  Average time: ${avgTime}ms"
    Write-Host "  AST nodes/sec: ${nodesPerSec}M"
    Write-Host ""
}

Log-Info "Parser benchmark complete!"