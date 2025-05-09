use std::path::PathBuf;
use std::env;
use std::fs;

use crate::test_harness::{TestCase, TestHarness, TestStatus};

/// Create test cases for Hello World examples
fn create_hello_world_test_cases() -> Vec<TestCase> {
    vec![
        // English Hello World test
        TestCase {
            name: "hello_world_english".to_string(),
            source_file: PathBuf::from("examples/hello_world/hello_world_english.seen"),
            config_file: PathBuf::from("examples/hello_world/hello_world_english.seen.toml"),
            expected_output: "Hello, World!\n".to_string(),
            expected_exit_code: 0,
        },
        // Arabic Hello World test
        TestCase {
            name: "hello_world_arabic".to_string(),
            source_file: PathBuf::from("examples/hello_world/hello_world_arabic.seen"),
            config_file: PathBuf::from("examples/hello_world/hello_world_arabic.seen.toml"),
            expected_output: "مرحبا بالعالم!\n".to_string(),
            expected_exit_code: 0,
        },
    ]
}

/// Run the Hello World tests
pub fn run_hello_world_tests() -> bool {
    println!("Running Hello World tests...");
    
    // Get the path to the compiler
    let compiler_path = env::current_dir()
        .expect("Failed to get current directory")
        .join("target/debug/seen");
    
    if !compiler_path.exists() {
        println!("Error: Compiler not found at {}", compiler_path.display());
        println!("Please build the compiler first with 'cargo build'");
        return false;
    }
    
    // Create a temporary directory for test artifacts
    let temp_dir = env::temp_dir().join("seen_tests");
    fs::create_dir_all(&temp_dir).expect("Failed to create temporary directory");
    
    // Create the test harness
    let mut harness = TestHarness::new(compiler_path, temp_dir);
    
    // Add the test cases
    for test_case in create_hello_world_test_cases() {
        harness.add_test_case(test_case);
    }
    
    // Run the tests
    let results = harness.run_all_tests();
    
    // Report the results
    harness.report_results(&results);
    
    // Return true if all tests passed
    results.iter().all(|r| r.status == TestStatus::Pass)
}
