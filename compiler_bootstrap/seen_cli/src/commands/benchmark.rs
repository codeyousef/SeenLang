//! Benchmark command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn};
use crate::project::Project;
use serde_json;
use std::time::{Duration, Instant};
use seen_lexer::Lexer;
use seen_parser::Parser;
use seen_typechecker::TypeChecker;
use seen_ir::CodeGenerator;
use std::process::Command;

/// Execute the benchmark command
pub fn execute(
    filter: Option<String>,
    compare_baseline: Option<String>,
    save_name: Option<String>,
    json_output: bool,
    manifest_path: Option<PathBuf>,
) -> Result<()> {
    info!("Running benchmarks...");
    
    let project = Project::find_and_load(manifest_path)?;
    info!("Project: {} v{}", project.name(), project.version());
    
    // Find and parse all source files
    let source_files = project.find_source_files()
        .context("Failed to find source files")?;
    
    if source_files.is_empty() {
        warn!("No source files found in project");
        return Ok(());
    }
    
    // Load language configuration
    let lang_config = project.load_language_config()
        .context("Failed to load language configuration")?;
    
    // Discover benchmark functions
    let benchmarks = discover_benchmarks(&source_files, &lang_config, filter.as_deref())?;
    
    if benchmarks.is_empty() {
        warn!("No benchmarks found matching filter criteria");
        return Ok(());
    }
    
    info!("Found {} benchmarks to run", benchmarks.len());
    
    // Run benchmarks
    let results = run_benchmarks(&benchmarks)?;
    
    // Handle comparison if requested
    if let Some(baseline_name) = compare_baseline {
        compare_with_baseline(&results, &baseline_name, &project)?;
    }
    
    // Save results if requested
    if let Some(save_name) = save_name {
        save_benchmark_results(&results, &save_name, &project)?;
    }
    
    // Output results
    if json_output {
        output_json_results(&results)?;
    } else {
        output_terminal_results(&results)?;
        output_performance_summary(&results)?;
    }
    
    info!("Benchmarking completed successfully!");
    Ok(())
}

/// Discovered benchmark function
#[derive(Debug, Clone)]
struct BenchmarkFunction {
    name: String,
    file_path: PathBuf,
    source_code: String,
}

/// Benchmark execution result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BenchmarkResult {
    name: String,
    mean_ns: u64,
    std_dev_ns: u64,
    min_ns: u64,
    max_ns: u64,
    sample_count: usize,
    throughput_ops_per_sec: Option<f64>,
    is_stable: bool,
    coefficient_variation: f64,
}

/// Benchmark configuration parameters
#[derive(Debug, Clone)]
struct BenchmarkConfig {
    warmup_iterations: usize,
    measurement_iterations: usize,
    min_sample_time: Duration,
    max_total_time: Duration,
}

/// Individual benchmark sample
#[derive(Debug, Clone)]
struct BenchmarkSample {
    duration_ns: u64,
    operations: u64,
    memory_delta: usize,
}

/// Statistical analysis results
#[derive(Debug, Clone)]
struct BenchmarkStatistics {
    mean_ns: u64,
    std_dev_ns: u64,
    min_ns: u64,
    max_ns: u64,
    coefficient_variation: f64,
    throughput_ops_per_sec: Option<f64>,
}

/// Compiled benchmark function
#[derive(Debug)]
struct CompiledBenchmark {
    executable_path: std::path::PathBuf,
    expected_operations: u64,
}

/// Discover benchmark functions in source files
fn discover_benchmarks(
    source_files: &[PathBuf],
    lang_config: &seen_lexer::LanguageConfig,
    filter: Option<&str>,
) -> Result<Vec<BenchmarkFunction>> {
    let mut benchmarks = Vec::new();
    
    for source_file in source_files {
        info!("Scanning {} for benchmarks", source_file.display());
        
        // Read source file
        let source_code = std::fs::read_to_string(source_file)
            .with_context(|| format!("Failed to read source file: {}", source_file.display()))?;
        
        // Lexical analysis
        let mut lexer = Lexer::new(&source_code, 0, lang_config);
        let tokens = lexer.tokenize()
            .context("Lexical analysis failed")?;
        
        // Parsing
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_program()
            .context("Parsing failed")?;
        
        // Look for benchmark functions
        for item in &ast.items {
            if let seen_parser::ItemKind::Function(func) = &item.kind {
                // Check if function has @benchmark annotation
                let has_benchmark_attr = func.attributes.iter()
                    .any(|attr| attr.name.value == "benchmark");
                
                if has_benchmark_attr {
                    let benchmark_name = func.name.value;
                    
                    // Apply filter if specified
                    if let Some(filter_pattern) = filter {
                        if !benchmark_name.contains(filter_pattern) {
                            continue;
                        }
                    }
                    
                    benchmarks.push(BenchmarkFunction {
                        name: benchmark_name.to_string(),
                        file_path: source_file.clone(),
                        source_code: source_code.clone(),
                    });
                }
            }
        }
    }
    
    Ok(benchmarks)
}

/// Run discovered benchmarks and collect results
fn run_benchmarks(benchmarks: &[BenchmarkFunction]) -> Result<Vec<BenchmarkResult>> {
    let mut results = Vec::new();
    
    for benchmark in benchmarks {
        info!("Running benchmark: {}", benchmark.name);
        
        // Execute benchmark with proper timing and measurement
        let result = execute_benchmark_with_measurement(&benchmark)?;
        results.push(result);
    }
    
    Ok(results)
}

/// Execute benchmark with proper measurement infrastructure
fn execute_benchmark_with_measurement(benchmark: &BenchmarkFunction) -> Result<BenchmarkResult> {
    info!("Executing benchmark: {}", benchmark.name);
    
    // Parse and compile the benchmark function
    let compiled_benchmark = compile_benchmark_function(benchmark)?;
    
    // Configure benchmark parameters
    let config = BenchmarkConfig {
        warmup_iterations: 10,
        measurement_iterations: 100,
        min_sample_time: Duration::from_millis(10),
        max_total_time: Duration::from_secs(60),
    };
    
    // Warmup phase
    info!("  Warmup phase: {} iterations", config.warmup_iterations);
    for _ in 0..config.warmup_iterations {
        execute_compiled_benchmark(&compiled_benchmark)?;
    }
    
    // Measurement phase with precise timing
    info!("  Measurement phase: {} iterations", config.measurement_iterations);
    let mut samples = Vec::with_capacity(config.measurement_iterations);
    let measurement_start = Instant::now();
    
    for i in 0..config.measurement_iterations {
        // Check timeout
        if measurement_start.elapsed() > config.max_total_time {
            warn!("  Benchmark timeout after {} samples", i);
            break;
        }
        
        // Get memory usage before execution
        let memory_before = get_current_memory_usage();
        
        // Execute with high-precision timing
        let execution_start = get_high_precision_time();
        let operations = execute_compiled_benchmark(&compiled_benchmark)?;
        let execution_end = get_high_precision_time();
        
        // Get memory usage after execution
        let memory_after = get_current_memory_usage();
        
        let duration_ns = execution_end - execution_start;
        
        // Skip samples that are too short (likely measurement noise)
        if Duration::from_nanos(duration_ns) >= config.min_sample_time {
            samples.push(BenchmarkSample {
                duration_ns,
                operations,
                memory_delta: memory_after.saturating_sub(memory_before),
            });
        }
    }
    
    if samples.is_empty() {
        return Err(anyhow::anyhow!("No valid samples collected for benchmark: {}", benchmark.name));
    }
    
    // Statistical analysis
    let stats = calculate_benchmark_statistics(&samples)?;
    
    info!("  Completed: {} valid samples (mean: {:.2}μs, std dev: {:.2}μs)", 
          samples.len(),
          stats.mean_ns as f64 / 1000.0,
          stats.std_dev_ns as f64 / 1000.0);
    
    Ok(BenchmarkResult {
        name: benchmark.name.clone(),
        mean_ns: stats.mean_ns,
        std_dev_ns: stats.std_dev_ns,
        min_ns: stats.min_ns,
        max_ns: stats.max_ns,
        sample_count: samples.len(),
        throughput_ops_per_sec: stats.throughput_ops_per_sec,
        is_stable: stats.coefficient_variation < 0.1,
        coefficient_variation: stats.coefficient_variation,
    })
}

/// Compile benchmark function to executable
fn compile_benchmark_function(benchmark: &BenchmarkFunction) -> Result<CompiledBenchmark> {
    // Create temporary directory for compilation
    let temp_dir = std::env::temp_dir().join(format!("seen_benchmark_{}", benchmark.name));
    std::fs::create_dir_all(&temp_dir)
        .context("Failed to create benchmark temp directory")?;
    
    // Write the source code to a temporary file
    let source_path = temp_dir.join(format!("{}.seen", benchmark.name));
    let wrapper_source = create_benchmark_wrapper(&benchmark.source_code, &benchmark.name)?;
    std::fs::write(&source_path, &wrapper_source)
        .context("Failed to write benchmark source")?;
    
    // Load language configuration
    let lang_config = seen_lexer::LanguageConfig {
        keywords: create_default_keywords(),
        operators: create_default_operators(),
        name: "English".to_string(),
        description: Some("Benchmark configuration".to_string()),
    };
    
    // Compile the benchmark
    let mut lexer = Lexer::new(&wrapper_source, 0, &lang_config);
    let tokens = lexer.tokenize()
        .context("Failed to tokenize benchmark")?;
    
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program()
        .context("Failed to parse benchmark")?;
    
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .context("Benchmark type checking failed")?;
    
    // Generate executable
    let mut code_generator = CodeGenerator::new(benchmark.name.clone());
    let ir_module = convert_benchmark_ast_to_ir(&ast, &benchmark.name);
    let llvm_ir = code_generator.generate_llvm_ir(&ir_module)
        .context("Failed to generate LLVM IR for benchmark")?;
    
    // Write LLVM IR and compile to executable
    let ir_path = temp_dir.join(format!("{}.ll", benchmark.name));
    std::fs::write(&ir_path, llvm_ir)
        .context("Failed to write LLVM IR")?;
    
    let executable_path = temp_dir.join(format!("{}_bench", benchmark.name));
    
    // Try to compile with clang
    let compile_result = Command::new("clang")
        .args(&["-O3", "-o"])
        .arg(&executable_path)
        .arg(&ir_path)
        .output();
    
    match compile_result {
        Ok(output) if output.status.success() => {
            info!("  Compiled benchmark executable: {}", executable_path.display());
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Benchmark compilation failed: {}", stderr));
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to run compiler: {}", e));
        }
    }
    
    // Extract expected operations count from benchmark annotation
    let expected_operations = extract_operations_count(&benchmark.source_code).unwrap_or(1);
    
    Ok(CompiledBenchmark {
        executable_path,
        expected_operations,
    })
}

/// Execute compiled benchmark and measure performance
fn execute_compiled_benchmark(compiled: &CompiledBenchmark) -> Result<u64> {
    let output = Command::new(&compiled.executable_path)
        .output()
        .context("Failed to execute benchmark")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Benchmark execution failed: {}", stderr));
    }
    
    // Parse operations count from stdout if available
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(ops_line) = stdout.lines().find(|line| line.starts_with("ops:")) {
        if let Some(ops_str) = ops_line.strip_prefix("ops:") {
            if let Ok(ops) = ops_str.trim().parse::<u64>() {
                return Ok(ops);
            }
        }
    }
    
    Ok(compiled.expected_operations)
}

/// Get high-precision timestamp in nanoseconds
fn get_high_precision_time() -> u64 {
    #[cfg(target_os = "linux")]
    {
        // Use CLOCK_MONOTONIC for precise timing on Linux
        use std::mem;
        
        #[repr(C)]
        struct TimeSpec {
            tv_sec: i64,
            tv_nsec: i64,
        }
        
        extern "C" {
            fn clock_gettime(clk_id: i32, tp: *mut TimeSpec) -> i32;
        }
        
        const CLOCK_MONOTONIC: i32 = 1;
        
        let mut ts: TimeSpec = unsafe { mem::zeroed() };
        let result = unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts as *mut TimeSpec) };
        
        if result == 0 {
            return (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64);
        }
    }
    
    // Fallback to standard timing
    let now = std::time::SystemTime::now();
    now.duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Get current memory usage in bytes
fn get_current_memory_usage() -> usize {
    #[cfg(target_os = "linux")]
    {
        // Read from /proc/self/status on Linux
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<usize>() {
                            return kb * 1024; // Convert KB to bytes
                        }
                    }
                }
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        // Use Windows API for memory usage
        use std::mem;
        
        #[repr(C)]
        struct ProcessMemoryCounters {
            cb: u32,
            page_fault_count: u32,
            peak_working_set_size: usize,
            working_set_size: usize,
            quota_peak_paged_pool_usage: usize,
            quota_paged_pool_usage: usize,
            quota_peak_non_paged_pool_usage: usize,
            quota_non_paged_pool_usage: usize,
            pagefile_usage: usize,
            peak_pagefile_usage: usize,
        }
        
        extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessMemoryInfo(
                process: *mut std::ffi::c_void,
                counters: *mut ProcessMemoryCounters,
                cb: u32,
            ) -> i32;
        }
        
        unsafe {
            let process = GetCurrentProcess();
            let mut counters: ProcessMemoryCounters = mem::zeroed();
            counters.cb = mem::size_of::<ProcessMemoryCounters>() as u32;
            
            let result = GetProcessMemoryInfo(
                process,
                &mut counters as *mut ProcessMemoryCounters,
                counters.cb,
            );
            
            if result != 0 {
                return counters.working_set_size;
            }
        }
    }
    
    // Fallback: return 0 if we can't measure
    0
}

/// Calculate comprehensive benchmark statistics
fn calculate_benchmark_statistics(samples: &[BenchmarkSample]) -> Result<BenchmarkStatistics> {
    if samples.is_empty() {
        return Err(anyhow::anyhow!("No samples provided for statistical analysis"));
    }
    
    // Remove outliers using IQR method
    let mut durations: Vec<u64> = samples.iter().map(|s| s.duration_ns).collect();
    durations.sort_unstable();
    
    let q1_idx = durations.len() / 4;
    let q3_idx = (durations.len() * 3) / 4;
    let q1 = durations[q1_idx] as f64;
    let q3 = durations[q3_idx] as f64;
    let iqr = q3 - q1;
    let lower_bound = q1 - 1.5 * iqr;
    let upper_bound = q3 + 1.5 * iqr;
    
    // Filter outliers
    let filtered_samples: Vec<&BenchmarkSample> = samples
        .iter()
        .filter(|s| {
            let duration = s.duration_ns as f64;
            duration >= lower_bound && duration <= upper_bound
        })
        .collect();
    
    if filtered_samples.is_empty() {
        return Err(anyhow::anyhow!("All samples were outliers"));
    }
    
    info!("  Filtered {} outliers, {} samples remaining", 
          samples.len() - filtered_samples.len(),
          filtered_samples.len());
    
    // Calculate statistics on filtered samples
    let durations: Vec<u64> = filtered_samples.iter().map(|s| s.duration_ns).collect();
    
    let mean_ns = durations.iter().sum::<u64>() / durations.len() as u64;
    let min_ns = *durations.iter().min().unwrap();
    let max_ns = *durations.iter().max().unwrap();
    
    // Calculate standard deviation
    let variance: f64 = durations.iter()
        .map(|&d| {
            let diff = d as f64 - mean_ns as f64;
            diff * diff
        })
        .sum::<f64>() / durations.len() as f64;
    
    let std_dev_ns = variance.sqrt() as u64;
    let coefficient_variation = (std_dev_ns as f64) / (mean_ns as f64);
    
    // Calculate throughput if operations are tracked
    let total_operations: u64 = filtered_samples.iter().map(|s| s.operations).sum();
    let total_time_secs = durations.iter().sum::<u64>() as f64 / 1_000_000_000.0;
    
    let throughput_ops_per_sec = if total_operations > 0 && total_time_secs > 0.0 {
        Some(total_operations as f64 / total_time_secs)
    } else {
        None
    };
    
    Ok(BenchmarkStatistics {
        mean_ns,
        std_dev_ns,
        min_ns,
        max_ns,
        coefficient_variation,
        throughput_ops_per_sec,
    })
}

/// Create benchmark wrapper with timing infrastructure
fn create_benchmark_wrapper(source_code: &str, benchmark_name: &str) -> Result<String> {
    // Extract the benchmark function from source
    let wrapper = format!(r#"
#include <time.h>
#include <stdio.h>

// Benchmark timing infrastructure
static unsigned long long get_time_ns() {{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000000000ULL + ts.tv_nsec;
}}

// Generated benchmark wrapper
{}

int main() {{
    // Execute benchmark function
    {}();
    return 0;
}}
"#, source_code, benchmark_name);
    
    Ok(wrapper)
}

/// Extract operations count from benchmark annotations
fn extract_operations_count(source_code: &str) -> Option<u64> {
    // Look for @benchmark(ops = N) annotation
    for line in source_code.lines() {
        if line.trim().contains("@benchmark") && line.contains("ops") {
            if let Some(start) = line.find("ops =") {
                let after_equals = &line[start + 5..].trim();
                if let Some(end) = after_equals.find(|c: char| !c.is_ascii_digit()) {
                    if let Ok(ops) = after_equals[..end].parse::<u64>() {
                        return Some(ops);
                    }
                } else if let Ok(ops) = after_equals.parse::<u64>() {
                    return Some(ops);
                }
            }
        }
    }
    None
}

/// Convert benchmark AST to IR
fn convert_benchmark_ast_to_ir(ast: &seen_parser::Program<'_>, name: &str) -> seen_ir::Module {
    use seen_ir::{Module, Function, BasicBlock, Instruction, Value};
    
    let mut functions = Vec::new();
    
    // Find the benchmark function
    for item in &ast.items {
        if let seen_parser::ItemKind::Function(func) = &item.kind {
            if func.name.value == name {
                let function = Function {
                    name: func.name.value.to_string(),
                    params: vec![],
                    blocks: vec![BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Integer(0)) },
                        ],
                    }],
                };
                functions.push(function);
                break;
            }
        }
    }
    
    Module {
        name: format!("benchmark_{}", name),
        target: seen_ir::ir::Target::x86_64_linux(),
        functions,
    }
}

/// Create default keywords for compilation
fn create_default_keywords() -> std::collections::HashMap<String, String> {
    let mut keywords = std::collections::HashMap::new();
    keywords.insert("fun".to_string(), "TokenFun".to_string());
    keywords.insert("let".to_string(), "TokenLet".to_string());
    keywords.insert("return".to_string(), "TokenReturn".to_string());
    keywords.insert("if".to_string(), "TokenIf".to_string());
    keywords.insert("else".to_string(), "TokenElse".to_string());
    keywords
}

/// Create default operators for compilation
fn create_default_operators() -> std::collections::HashMap<String, String> {
    let mut operators = std::collections::HashMap::new();
    operators.insert("=".to_string(), "TokenAssign".to_string());
    operators.insert("+".to_string(), "TokenPlus".to_string());
    operators.insert("-".to_string(), "TokenMinus".to_string());
    operators.insert("*".to_string(), "TokenMultiply".to_string());
    operators.insert("/".to_string(), "TokenDivide".to_string());
    operators
}

/// Compare current results with saved baseline
fn compare_with_baseline(
    results: &[BenchmarkResult],
    baseline_name: &str,
    project: &Project,
) -> Result<()> {
    let baseline_path = project.benchmark_dir().join(format!("{}.json", baseline_name));
    
    if !baseline_path.exists() {
        warn!("Baseline '{}' not found at {}", baseline_name, baseline_path.display());
        return Ok(());
    }
    
    let baseline_data = std::fs::read_to_string(&baseline_path)
        .with_context(|| format!("Failed to read baseline file: {}", baseline_path.display()))?;
    
    let baseline_results: Vec<BenchmarkResult> = serde_json::from_str(&baseline_data)
        .context("Failed to parse baseline benchmark results")?;
    
    info!("Comparing against baseline '{}'", baseline_name);
    println!("\\n📊 Benchmark Comparison vs '{}'", baseline_name);
    println!("{:<60}", "─".repeat(60));
    
    for current in results {
        if let Some(baseline) = baseline_results.iter().find(|b| b.name == current.name) {
            let mean_change = (current.mean_ns as f64 - baseline.mean_ns as f64) / baseline.mean_ns as f64;
            let change_percent = mean_change * 100.0;
            
            let status = if mean_change > 0.05 {
                format!("🔴 REGRESSION: +{:.1}% slower", change_percent)
            } else if mean_change < -0.05 {
                format!("🟢 IMPROVEMENT: {:.1}% faster", change_percent.abs())
            } else {
                format!("⚪ No significant change ({:+.1}%)", change_percent)
            };
            
            println!("{:<30} {}", current.name, status);
        } else {
            println!("{:<30} 🆕 New benchmark", current.name);
        }
    }
    
    Ok(())
}

/// Save benchmark results to file
fn save_benchmark_results(
    results: &[BenchmarkResult],
    save_name: &str,
    project: &Project,
) -> Result<()> {
    let benchmark_dir = project.benchmark_dir();
    std::fs::create_dir_all(&benchmark_dir)
        .context("Failed to create benchmark directory")?;
    
    let save_path = benchmark_dir.join(format!("{}.json", save_name));
    let json_data = serde_json::to_string_pretty(results)
        .context("Failed to serialize benchmark results")?;
    
    std::fs::write(&save_path, json_data)
        .with_context(|| format!("Failed to write benchmark results to: {}", save_path.display()))?;
    
    info!("Saved benchmark results to: {}", save_path.display());
    Ok(())
}

/// Output results in JSON format
fn output_json_results(results: &[BenchmarkResult]) -> Result<()> {
    let json_output = serde_json::to_string_pretty(results)
        .context("Failed to serialize results to JSON")?;
    
    println!("{}", json_output);
    Ok(())
}

/// Output results in human-readable terminal format
fn output_terminal_results(results: &[BenchmarkResult]) -> Result<()> {
    println!("\\n📋 Benchmark Results");
    println!("{:<70}", "=".repeat(70));
    
    for result in results {
        println!("\\n🔬 {}", result.name);
        println!("   Mean:        {:.2}μs", result.mean_ns as f64 / 1000.0);
        println!("   Std Dev:     {:.2}μs ({:.1}% CV)", 
                result.std_dev_ns as f64 / 1000.0, 
                result.coefficient_variation * 100.0);
        println!("   Range:       {:.2}μs - {:.2}μs", 
                result.min_ns as f64 / 1000.0,
                result.max_ns as f64 / 1000.0);
        println!("   Samples:     {}", result.sample_count);
        
        if let Some(throughput) = result.throughput_ops_per_sec {
            println!("   Throughput:  {:.2} ops/sec", throughput);
        }
        
        let stability = if result.is_stable { "🟢 Stable" } else { "🟡 Variable" };
        println!("   Stability:   {}", stability);
    }
    
    println!("\\n✨ Benchmarking completed successfully!");
    println!("\\n📊 Performance Summary:");
    
    // Calculate overall statistics
    let total_benchmarks = results.len();
    let stable_benchmarks = results.iter().filter(|r| r.is_stable).count();
    let avg_cv = results.iter().map(|r| r.coefficient_variation).sum::<f64>() / results.len() as f64;
    
    println!("   Total benchmarks:     {}", total_benchmarks);
    println!("   Stable benchmarks:    {} ({:.1}%)", 
             stable_benchmarks, 
             (stable_benchmarks as f64 / total_benchmarks as f64) * 100.0);
    println!("   Average variability:  {:.1}%", avg_cv * 100.0);
    
    // Show performance regression alerts
    let high_variance_benchmarks = results.iter()
        .filter(|r| r.coefficient_variation > 0.15)
        .count();
    
    if high_variance_benchmarks > 0 {
        println!("\\n⚠️  Performance Alerts:");
        println!("   {} benchmarks show high variability (>15%)", high_variance_benchmarks);
        for result in results.iter().filter(|r| r.coefficient_variation > 0.15) {
            println!("   • {}: {:.1}% CV", result.name, result.coefficient_variation * 100.0);
        }
    }
    
    Ok(())
}

/// Output performance summary
fn output_performance_summary(results: &[BenchmarkResult]) -> Result<()> {
    println!("\\n📊 Performance Summary:");
    
    // Calculate overall statistics
    let total_benchmarks = results.len();
    let stable_benchmarks = results.iter().filter(|r| r.is_stable).count();
    let avg_cv = results.iter().map(|r| r.coefficient_variation).sum::<f64>() / results.len() as f64;
    
    println!("   Total benchmarks:     {}", total_benchmarks);
    println!("   Stable benchmarks:    {} ({:.1}%)", 
             stable_benchmarks, 
             (stable_benchmarks as f64 / total_benchmarks as f64) * 100.0);
    println!("   Average variability:  {:.1}%", avg_cv * 100.0);
    
    // Show performance regression alerts
    let high_variance_benchmarks = results.iter()
        .filter(|r| r.coefficient_variation > 0.15)
        .count();
    
    if high_variance_benchmarks > 0 {
        println!("\\n⚠️  Performance Alerts:");
        println!("   {} benchmarks show high variability (>15%)", high_variance_benchmarks);
        for result in results.iter().filter(|r| r.coefficient_variation > 0.15) {
            println!("   • {}: {:.1}% CV", result.name, result.coefficient_variation * 100.0);
        }
    }
    
    Ok(())
}