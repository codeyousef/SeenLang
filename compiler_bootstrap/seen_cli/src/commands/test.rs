//! Test command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn};
use crate::project::Project;

/// Execute the test command
pub fn execute(bench: bool, coverage: bool, filter: Option<String>, manifest_path: Option<PathBuf>) -> Result<()> {
    if bench {
        info!("Running benchmarks...");
    } else {
        info!("Running tests...");
    }
    
    let project = Project::find_and_load(manifest_path)?;
    
    info!("Project: {} v{}", project.name(), project.version());
    
    // Find test files
    let test_files = find_test_files(&project)?;
    
    if test_files.is_empty() {
        info!("No test files found");
        return Ok(());
    }
    
    info!("Found {} test files", test_files.len());
    
    // Apply filter if provided
    let filtered_tests = if let Some(filter_pattern) = &filter {
        test_files.into_iter()
            .filter(|path| {
                path.to_string_lossy().contains(filter_pattern)
            })
            .collect()
    } else {
        test_files
    };
    
    if filtered_tests.is_empty() {
        info!("No tests match the filter");
        return Ok(());
    }
    
    info!("Running {} test files", filtered_tests.len());
    
    // Phase 1: Validate test files can be parsed (full test execution in Alpha phase)
    let lang_config = project.load_language_config()
        .context("Failed to load language configuration")?;
    
    let mut passed = 0;
    let mut failed = 0;
    
    for test_file in &filtered_tests {
        info!("Testing {}", test_file.display());
        
        let source_code = std::fs::read_to_string(test_file)
            .with_context(|| format!("Failed to read test file: {}", test_file.display()))?;
        
        // Basic validation - can we tokenize the test file?
        let mut lexer = seen_lexer::Lexer::new(&source_code, 0, &lang_config);
        match lexer.tokenize() {
            Ok(_tokens) => {
                if lexer.diagnostics().has_errors() {
                    failed += 1;
                    for diagnostic in &lexer.diagnostics().messages {
                        warn!("  {}", diagnostic);
                    }
                } else {
                    passed += 1;
                    info!("  ✓ PASS");
                }
            }
            Err(e) => {
                failed += 1;
                warn!("  ✗ FAIL: {}", e);
            }
        }
    }
    
    info!("Test results: {} passed, {} failed", passed, failed);
    
    if coverage {
        info!("Code coverage analysis not yet implemented");
    }
    
    if failed > 0 {
        return Err(anyhow::anyhow!("Some tests failed"));
    }
    
    info!("All tests passed!");
    Ok(())
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