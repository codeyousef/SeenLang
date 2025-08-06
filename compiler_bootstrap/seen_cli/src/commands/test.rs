//! Test command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn, debug};
use crate::project::Project;
use seen_std::testing::{TestRunner, TestConfig, TestStats};
use seen_std::testing::bench::{BenchRunner, BenchConfig};
use std::time::Instant;

/// Execute the test command
pub fn execute(bench: bool, coverage: bool, filter: Option<String>, manifest_path: Option<PathBuf>) -> Result<()> {
    let project = Project::find_and_load(manifest_path)?;
    
    info!("Project: {} v{}", project.name(), project.version());
    
    if bench {
        run_benchmarks(&project, filter, coverage)
    } else {
        run_tests(&project, filter, coverage)
    }
}

/// Run tests using the Seen testing framework
fn run_tests(project: &Project, filter: Option<String>, coverage: bool) -> Result<()> {
    info!("Running tests...");
    
    // Find test files
    let test_files = find_test_files(project)?;
    
    if test_files.is_empty() {
        info!("No test files found");
        return Ok(());
    }
    
    info!("Found {} test files", test_files.len());
    
    // Configure test runner
    let test_config = TestConfig {
        parallel: true,
        fail_fast: false,
        filter: filter.clone().map(|s| seen_std::string::String::from(&s)),
        exact_match: false,
        list_only: false,
        quiet: false,
        show_output: true,
    };
    
    let mut overall_stats = TestStats::new();
    let lang_config = project.load_language_config()
        .context("Failed to load language configuration")?;
    
    // Process each test file
    for test_file in test_files {
        if let Some(ref filter_pattern) = filter {
            if !test_file.to_string_lossy().contains(filter_pattern) {
                continue;
            }
        }
        
        debug!("Processing test file: {}", test_file.display());
        let file_stats = execute_test_file(&test_file, &lang_config)?;
        
        // Merge stats
        overall_stats.passed += file_stats.passed;
        overall_stats.failed += file_stats.failed;
        overall_stats.skipped += file_stats.skipped;
        overall_stats.ignored += file_stats.ignored;
        overall_stats.total_duration_ns += file_stats.total_duration_ns;
    }
    
    // Print final results
    print_test_results(&overall_stats);
    
    if coverage {
        info!("Code coverage analysis not yet implemented");
    }
    
    if overall_stats.failed > 0 {
        return Err(anyhow::anyhow!("{} tests failed", overall_stats.failed));
    }
    
    Ok(())
}

/// Run benchmarks using the Seen benchmarking framework
fn run_benchmarks(project: &Project, filter: Option<String>, _coverage: bool) -> Result<()> {
    info!("Running benchmarks...");
    
    let bench_files = find_bench_files(project)?;
    
    if bench_files.is_empty() {
        info!("No benchmark files found");
        return Ok(());
    }
    
    info!("Found {} benchmark files", bench_files.len());
    
    let bench_config = BenchConfig::default();
    let bench_runner = BenchRunner::new(bench_config);
    
    let lang_config = project.load_language_config()
        .context("Failed to load language configuration")?;
    
    // Process each benchmark file
    for bench_file in bench_files {
        if let Some(ref filter_pattern) = filter {
            if !bench_file.to_string_lossy().contains(filter_pattern) {
                continue;
            }
        }
        
        info!("Benchmarking {}", bench_file.display());
        execute_bench_file(&bench_file, &bench_runner, &lang_config)?;
    }
    
    Ok(())
}

/// Execute a single test file and return statistics
fn execute_test_file(test_file: &PathBuf, lang_config: &seen_lexer::LanguageConfig) -> Result<TestStats> {
    let start_time = Instant::now();
    let mut stats = TestStats::new();
    
    let source_code = std::fs::read_to_string(test_file)
        .with_context(|| format!("Failed to read test file: {}", test_file.display()))?;
    
    info!("Testing {}", test_file.display());
    
    // Phase 1: Lexical analysis validation
    let mut lexer = seen_lexer::Lexer::new(&source_code, 0, lang_config);
    match lexer.tokenize() {
        Ok(tokens) => {
            if lexer.diagnostics().has_errors() {
                for diagnostic in &lexer.diagnostics().messages {
                    warn!("  Lexer error: {}", diagnostic);
                }
                stats.failed += 1;
                warn!("  ✗ LEXICAL ANALYSIS FAILED");
            } else {
                debug!("  ✓ Lexical analysis passed ({} tokens)", tokens.len());
                
                // Phase 2: Parsing validation  
                match parse_test_file(&tokens, lang_config) {
                    Ok(_) => {
                        stats.passed += 1;
                        info!("  ✓ PARSING PASSED");
                    }
                    Err(e) => {
                        stats.failed += 1;
                        warn!("  ✗ PARSING FAILED: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            stats.failed += 1;
            warn!("  ✗ LEXICAL ANALYSIS FAILED: {}", e);
        }
    }
    
    stats.total_duration_ns = start_time.elapsed().as_nanos() as u64;
    Ok(stats)
}

/// Parse tokens from a test file (simplified parsing for MVP)
fn parse_test_file(tokens: &[seen_lexer::Token], _lang_config: &seen_lexer::LanguageConfig) -> Result<()> {
    // For MVP: Just validate that we have valid tokens
    // In full implementation: Parse into AST and validate semantics
    
    if tokens.is_empty() {
        return Err(anyhow::anyhow!("No tokens found"));
    }
    
    // Basic validation: check for balanced braces, parentheses, etc.
    let mut brace_depth = 0;
    let mut paren_depth = 0;
    let mut bracket_depth = 0;
    
    for token in tokens {
        match token.value {
            seen_lexer::TokenType::LeftBrace => brace_depth += 1,
            seen_lexer::TokenType::RightBrace => brace_depth -= 1,
            seen_lexer::TokenType::LeftParen => paren_depth += 1,
            seen_lexer::TokenType::RightParen => paren_depth -= 1,
            seen_lexer::TokenType::LeftBracket => bracket_depth += 1,
            seen_lexer::TokenType::RightBracket => bracket_depth -= 1,
            _ => {}
        }
        
        // Check for negative depths (unmatched closing)
        if brace_depth < 0 || paren_depth < 0 || bracket_depth < 0 {
            return Err(anyhow::anyhow!("Unmatched closing delimiter"));
        }
    }
    
    // Check for unclosed delimiters
    if brace_depth > 0 || paren_depth > 0 || bracket_depth > 0 {
        return Err(anyhow::anyhow!("Unclosed delimiters"));
    }
    
    Ok(())
}

/// Execute benchmark file
fn execute_bench_file(
    bench_file: &PathBuf, 
    bench_runner: &BenchRunner, 
    lang_config: &seen_lexer::LanguageConfig
) -> Result<()> {
    let source_code = std::fs::read_to_string(bench_file)
        .with_context(|| format!("Failed to read benchmark file: {}", bench_file.display()))?;
    
    // Benchmark lexing performance
    let lexing_measurement = bench_runner.bench("lexing", || {
        let mut lexer = seen_lexer::Lexer::new(&source_code, 0, lang_config);
        let _ = lexer.tokenize();
    });
    
    info!("  Lexing: {:.2}ms (±{:.2}ms)", 
          lexing_measurement.mean.as_secs_f64() * 1000.0,
          lexing_measurement.std_dev.as_secs_f64() * 1000.0);
    
    // If lexing succeeds, benchmark parsing
    let mut lexer = seen_lexer::Lexer::new(&source_code, 0, lang_config);
    if let Ok(tokens) = lexer.tokenize() {
        let parsing_measurement = bench_runner.bench("parsing", || {
            let _ = parse_test_file(&tokens, lang_config);
        });
        
        info!("  Parsing: {:.2}ms (±{:.2}ms)", 
              parsing_measurement.mean.as_secs_f64() * 1000.0,
              parsing_measurement.std_dev.as_secs_f64() * 1000.0);
    }
    
    Ok(())
}

/// Print comprehensive test results
fn print_test_results(stats: &TestStats) {
    let total = stats.total_tests();
    let success_rate = stats.success_rate() * 100.0;
    let duration_secs = stats.total_duration_ns as f64 / 1_000_000_000.0;
    
    println!();
    println!("test result: {}. {} passed; {} failed; {} ignored; {} filtered out; finished in {:.2}s",
        if stats.failed == 0 { "ok" } else { "FAILED" },
        stats.passed,
        stats.failed,
        stats.ignored,
        stats.skipped,
        duration_secs
    );
    
    if total > 0 {
        println!("Success rate: {:.1}%", success_rate);
    }
    
    if stats.failed > 0 {
        println!("\nSome tests failed. Run with --verbose for more details.");
    }
}

/// Find benchmark files (similar to test files but in benches/ directory)
fn find_bench_files(project: &Project) -> Result<Vec<PathBuf>> {
    let mut bench_files = Vec::new();
    
    // Look for benchmark files in benches/ directory
    let benches_dir = project.root_dir().join("benches");
    if benches_dir.exists() {
        for entry in walkdir::WalkDir::new(&benches_dir) {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "seen") {
                bench_files.push(path.to_path_buf());
            }
        }
    }
    
    bench_files.sort();
    Ok(bench_files)
}

fn find_test_files(project: &Project) -> Result<Vec<PathBuf>> {
    let mut test_files = Vec::new();
    
    // Look for test files in tests/ directory
    let tests_dir = project.root_dir().join("tests");
    if tests_dir.exists() {
        for entry in walkdir::WalkDir::new(&tests_dir) {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "seen") {
                test_files.push(path.to_path_buf());
            }
        }
    }
    
    // Look for test files in src/ directory (files ending with _test.seen)
    let src_dir = project.src_dir();
    if src_dir.exists() {
        for entry in walkdir::WalkDir::new(&src_dir) {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            
            if path.is_file() 
                && path.extension().map_or(false, |ext| ext == "seen")
                && path.file_stem()
                    .and_then(|s| s.to_str())
                    .map_or(false, |s| s.ends_with("_test"))
            {
                test_files.push(path.to_path_buf());
            }
        }
    }
    
    test_files.sort();
    Ok(test_files)
}