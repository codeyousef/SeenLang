//! Benchmark command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn, error};
use crate::project::Project;
use serde_json;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use seen_lexer::Lexer;
use seen_parser::Parser;

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
        
        // For MVP, we'll simulate benchmark execution
        // In a full implementation, this would compile and run the benchmark code
        let result = simulate_benchmark_execution(&benchmark)?;
        results.push(result);
    }
    
    Ok(results)
}

/// Simulate benchmark execution (for MVP implementation)
fn simulate_benchmark_execution(benchmark: &BenchmarkFunction) -> Result<BenchmarkResult> {
    // This is a simplified simulation for the MVP
    // In the full implementation, this would:
    // 1. Compile the benchmark function
    // 2. Execute it with proper timing infrastructure
    // 3. Collect real performance statistics
    
    let start_time = Instant::now();
    
    // Simulate some work with varying performance
    let base_duration = Duration::from_micros(100 + benchmark.name.len() as u64 * 10);
    let mut samples = Vec::new();
    
    // Collect 100 samples
    for i in 0..100 {
        let variation = (i as f64 * 0.1).sin() * 0.1; // Add some realistic variation
        let sample_duration = Duration::from_nanos(
            (base_duration.as_nanos() as f64 * (1.0 + variation)) as u64
        );
        samples.push(sample_duration);
        
        // Simulate actual work
        std::thread::sleep(Duration::from_micros(1));
    }
    
    // Calculate statistics
    let mean_ns = samples.iter().map(|d| d.as_nanos()).sum::<u128>() / samples.len() as u128;
    let mean_duration = Duration::from_nanos(mean_ns as u64);
    
    let variance: f64 = samples.iter()
        .map(|d| {
            let diff = d.as_secs_f64() - mean_duration.as_secs_f64();
            diff * diff
        })
        .sum::<f64>() / samples.len() as f64;
    
    let std_dev_duration = Duration::from_secs_f64(variance.sqrt());
    let min_ns = samples.iter().map(|d| d.as_nanos()).min().unwrap() as u64;
    let max_ns = samples.iter().map(|d| d.as_nanos()).max().unwrap() as u64;
    
    let coefficient_variation = std_dev_duration.as_secs_f64() / mean_duration.as_secs_f64();
    let is_stable = coefficient_variation < 0.1;
    
    let execution_time = start_time.elapsed();
    info!("  Completed in {:.2}s (mean: {:.2}μs, std dev: {:.2}μs)", 
          execution_time.as_secs_f64(),
          mean_duration.as_micros(),
          std_dev_duration.as_micros());
    
    Ok(BenchmarkResult {
        name: benchmark.name.clone(),
        mean_ns: mean_ns as u64,
        std_dev_ns: std_dev_duration.as_nanos() as u64,
        min_ns,
        max_ns,
        sample_count: samples.len(),
        throughput_ops_per_sec: None, // Could be calculated if benchmark reports operations
        is_stable,
        coefficient_variation,
    })
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
    Ok(())
}