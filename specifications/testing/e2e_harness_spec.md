# End-to-End Test Harness Specification

## Overview

This document specifies a simple test harness for automating end-to-end (E2E) tests of the Seen programming language compiler and runtime. The initial focus is on validating "Hello World" examples in both English and Arabic, ensuring the compiler correctly handles bilingual keywords based on the language setting in the project configuration.

## Test Harness Structure

### Directory Structure

```
tests/
  ├── e2e/                  # End-to-end tests
  │   ├── harness.rs        # Main test harness
  │   ├── test_runner.rs    # Test runner implementation
  │   ├── test_case.rs      # Test case definition
  │   └── utils.rs          # Utility functions
  └── test_cases/           # Test case definitions
      ├── hello_world/      # Hello World test cases
      │   ├── english.rs    # English Hello World test
      │   └── arabic.rs     # Arabic Hello World test
      └── ...
```

### Core Components

1. **Test Case**: Represents a single test case with inputs and expected outputs.
2. **Test Runner**: Executes a test case and validates the results.
3. **Test Harness**: Discovers, runs, and reports on test cases.

## Test Case Definition

A test case consists of:

1. **Name**: A unique identifier for the test case.
2. **Source File**: Path to the Seen source file.
3. **Config File**: Path to the project configuration file (seen.toml).
4. **Expected Output**: The expected output of running the program.
5. **Expected Exit Code**: The expected exit code (typically 0 for success).

```rust
struct TestCase {
    name: String,
    source_file: PathBuf,
    config_file: PathBuf,
    expected_output: String,
    expected_exit_code: i32,
}
```

## Test Runner Implementation

The test runner is responsible for:

1. **Compilation**: Compiling the source file using the Seen compiler.
2. **Execution**: Running the compiled program and capturing the output.
3. **Validation**: Comparing the actual output and exit code with the expected values.

```rust
struct TestRunner {
    compiler_path: PathBuf,
    temp_dir: PathBuf,
}

impl TestRunner {
    fn new(compiler_path: PathBuf) -> Self { ... }
    
    fn run_test(&self, test_case: &TestCase) -> TestResult { ... }
    
    fn compile(&self, source_file: &Path, config_file: &Path) -> Result<PathBuf> { ... }
    
    fn execute(&self, executable: &Path) -> Result<(String, i32)> { ... }
}
```

## Test Harness Implementation

The test harness provides:

1. **Test Discovery**: Finding test cases in the test directory.
2. **Parallel Execution**: Running multiple tests in parallel.
3. **Reporting**: Generating a summary of test results.

```rust
struct TestHarness {
    test_cases: Vec<TestCase>,
    runner: TestRunner,
}

impl TestHarness {
    fn new(compiler_path: PathBuf) -> Self { ... }
    
    fn discover_tests(&mut self, test_dir: &Path) -> Result<()> { ... }
    
    fn run_all_tests(&self) -> Vec<TestResult> { ... }
    
    fn report_results(&self, results: &[TestResult]) { ... }
}
```

## Hello World Test Cases

### English Hello World Test

```rust
fn create_english_hello_world_test() -> TestCase {
    TestCase {
        name: "hello_world_english".to_string(),
        source_file: PathBuf::from("examples/hello_world/hello_world_english.seen"),
        config_file: PathBuf::from("examples/hello_world/hello_world_english.seen.toml"),
        expected_output: "Hello, World!\n".to_string(),
        expected_exit_code: 0,
    }
}
```

### Arabic Hello World Test

```rust
fn create_arabic_hello_world_test() -> TestCase {
    TestCase {
        name: "hello_world_arabic".to_string(),
        source_file: PathBuf::from("examples/hello_world/hello_world_arabic.seen"),
        config_file: PathBuf::from("examples/hello_world/hello_world_arabic.seen.toml"),
        expected_output: "مرحبا بالعالم!\n".to_string(),
        expected_exit_code: 0,
    }
}
```

## Command Line Interface

The test harness should be runnable from the command line:

```
USAGE:
    seen test [OPTIONS]

OPTIONS:
    -t, --test-name <NAME>     Run a specific test by name
    -d, --test-dir <DIR>       Directory containing test cases [default: tests/test_cases]
    -c, --compiler <PATH>      Path to the Seen compiler [default: target/debug/seen]
    -v, --verbose              Enable verbose output
    -h, --help                 Print help information
```

## Test Execution Process

1. **Initialization**:
   - Create a temporary directory for test artifacts.
   - Initialize the test runner with the compiler path.

2. **For each test case**:
   - Copy the source file and configuration to the temporary directory.
   - Compile the source file using the Seen compiler.
   - Execute the compiled program and capture the output.
   - Compare the actual output with the expected output.
   - Compare the actual exit code with the expected exit code.
   - Report the test result (pass/fail).

3. **Cleanup**:
   - Remove temporary files and directories.

## Test Result Format

```rust
struct TestResult {
    test_case: TestCase,
    status: TestStatus,
    actual_output: Option<String>,
    actual_exit_code: Option<i32>,
    compile_time_ms: u64,
    execution_time_ms: u64,
    error_message: Option<String>,
}

enum TestStatus {
    Pass,
    CompileFailed,
    ExecutionFailed,
    OutputMismatch,
    ExitCodeMismatch,
}
```

## Reporting

The test harness will generate a summary report of all test results:

```
Test Results:
-------------
Total tests:    10
Passed:         8
Failed:         2
Success rate:   80%

Failures:
---------
1. test_name_1:
   - Expected output: "Hello, World!"
   - Actual output:   "Hello World!"
   - Error: OutputMismatch

2. test_name_2:
   - Error: CompileFailed
   - Message: "Syntax error at line 5"
```

## Integration with Continuous Integration

The test harness should be easily integratable with CI systems:

1. **Exit Code**: The test harness should return a non-zero exit code if any tests fail.
2. **Machine-Readable Output**: The test harness should support outputting results in a machine-readable format (e.g., JSON).
3. **Configurability**: The test harness should be configurable via command-line arguments and environment variables.

## Implementation Roadmap

1. **Phase 1** (MVP):
   - Basic test case structure
   - Simple compilation and execution
   - Support for Hello World examples in English and Arabic

2. **Phase 2**:
   - Expanded test case coverage
   - Improved reporting
   - Performance metrics

3. **Phase 3**:
   - Integration with IDE
   - Visual test results
   - Automatic test generation
