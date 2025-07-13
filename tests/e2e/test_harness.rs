use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Represents a test case for the Seen language
pub struct TestCase {
    /// Name of the test case
    pub name: String,
    /// Path to the source file
    pub source_file: PathBuf,
    /// Path to the configuration file
    pub config_file: PathBuf,
    /// Expected output from running the program
    pub expected_output: String,
    /// Expected exit code (usually 0 for success)
    pub expected_exit_code: i32,
}

/// Status of a test result
#[derive(Debug, PartialEq)]
pub enum TestStatus {
    Pass,
    CompileFailed,
    ExecutionFailed,
    OutputMismatch,
    ExitCodeMismatch,
}

/// Result of running a test case
pub struct TestResult {
    /// The test case that was run
    pub test_case: TestCase,
    /// Status of the test (pass/fail and reason)
    pub status: TestStatus,
    /// Actual output from the program
    pub actual_output: Option<String>,
    /// Actual exit code from the program
    pub actual_exit_code: Option<i32>,
    /// Time taken to compile
    pub compile_time_ms: u64,
    /// Time taken to execute
    pub execution_time_ms: u64,
    /// Error message if any
    pub error_message: Option<String>,
}

/// Test runner for executing Seen language tests
pub struct TestRunner {
    /// Path to the compiler executable
    compiler_path: PathBuf,
    /// Temporary directory for test artifacts
    temp_dir: PathBuf,
}

impl TestRunner {
    /// Create a new test runner
    pub fn new(compiler_path: PathBuf, temp_dir: PathBuf) -> Self {
        Self {
            compiler_path,
            temp_dir,
        }
    }

    /// Run a test case
    pub fn run_test(&self, test_case: TestCase) -> TestResult {
        println!("Running test: {}", test_case.name);

        // Compile the program
        let compile_start = Instant::now();
        let compile_result = self.compile(&test_case.source_file, &test_case.config_file);
        let compile_time = compile_start.elapsed();

        match compile_result {
            Ok(executable) => {
                // Run the program
                let execution_start = Instant::now();
                let execution_result = self.execute(&executable);
                let execution_time = execution_start.elapsed();

                match execution_result {
                    Ok((output, exit_code)) => {
                        // Check if the output and exit code match expectations
                        if output != test_case.expected_output {
                            TestResult {
                                test_case,
                                status: TestStatus::OutputMismatch,
                                actual_output: Some(output),
                                actual_exit_code: Some(exit_code),
                                compile_time_ms: compile_time.as_millis() as u64,
                                execution_time_ms: execution_time.as_millis() as u64,
                                error_message: Some(format!(
                                    "Expected output: {:?}, got: {:?}",
                                    test_case.expected_output, output
                                )),
                            }
                        } else if exit_code != test_case.expected_exit_code {
                            TestResult {
                                test_case,
                                status: TestStatus::ExitCodeMismatch,
                                actual_output: Some(output),
                                actual_exit_code: Some(exit_code),
                                compile_time_ms: compile_time.as_millis() as u64,
                                execution_time_ms: execution_time.as_millis() as u64,
                                error_message: Some(format!(
                                    "Expected exit code: {}, got: {}",
                                    test_case.expected_exit_code, exit_code
                                )),
                            }
                        } else {
                            // Test passed
                            TestResult {
                                test_case,
                                status: TestStatus::Pass,
                                actual_output: Some(output),
                                actual_exit_code: Some(exit_code),
                                compile_time_ms: compile_time.as_millis() as u64,
                                execution_time_ms: execution_time.as_millis() as u64,
                                error_message: None,
                            }
                        }
                    }
                    Err(e) => {
                        // Execution failed
                        TestResult {
                            test_case,
                            status: TestStatus::ExecutionFailed,
                            actual_output: None,
                            actual_exit_code: None,
                            compile_time_ms: compile_time.as_millis() as u64,
                            execution_time_ms: execution_time.as_millis() as u64,
                            error_message: Some(e.to_string()),
                        }
                    }
                }
            }
            Err(e) => {
                // Compilation failed
                TestResult {
                    test_case,
                    status: TestStatus::CompileFailed,
                    actual_output: None,
                    actual_exit_code: None,
                    compile_time_ms: compile_time.as_millis() as u64,
                    execution_time_ms: 0,
                    error_message: Some(e.to_string()),
                }
            }
        }
    }

    /// Compile a Seen program
    fn compile(&self, source_file: &Path, config_file: &Path) -> Result<PathBuf, String> {
        // Create a unique output directory for this test
        let test_dir = self.temp_dir.join(format!(
            "test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        fs::create_dir_all(&test_dir).map_err(|e| format!("Failed to create test directory: {}", e))?;

        // Copy the source and config files to the test directory
        let test_source = test_dir.join(source_file.file_name().unwrap());
        let test_config = test_dir.join(config_file.file_name().unwrap());

        fs::copy(source_file, &test_source)
            .map_err(|e| format!("Failed to copy source file: {}", e))?;
        fs::copy(config_file, &test_config)
            .map_err(|e| format!("Failed to copy config file: {}", e))?;

        // Build the program using the Seen CLI
        let output = Command::new(&self.compiler_path)
            .args(["build", "--project-path", test_dir.to_str().unwrap()])
            .output()
            .map_err(|e| format!("Failed to execute compiler: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Find the executable
        let mut executable_path = None;
        for entry in fs::read_dir(test_dir.join("target"))
            .map_err(|e| format!("Failed to read target directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_file() && path.extension().is_none() {
                executable_path = Some(path);
                break;
            }
        }

        executable_path.ok_or_else(|| "Failed to find executable".to_string())
    }

    /// Execute a compiled Seen program
    fn execute(&self, executable: &Path) -> Result<(String, i32), String> {
        let output = Command::new(executable)
            .output()
            .map_err(|e| format!("Failed to execute program: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((stdout, exit_code))
    }
}

/// Test harness for running multiple test cases
pub struct TestHarness {
    /// Test cases to run
    test_cases: Vec<TestCase>,
    /// Test runner
    runner: TestRunner,
}

impl TestHarness {
    /// Create a new test harness
    pub fn new(compiler_path: PathBuf, temp_dir: PathBuf) -> Self {
        Self {
            test_cases: Vec::new(),
            runner: TestRunner::new(compiler_path, temp_dir),
        }
    }

    /// Add a test case
    pub fn add_test_case(&mut self, test_case: TestCase) {
        self.test_cases.push(test_case);
    }

    /// Run all test cases
    pub fn run_all_tests(&self) -> Vec<TestResult> {
        let mut results = Vec::new();

        for test_case in &self.test_cases {
            let result = self.runner.run_test(test_case.clone());
            results.push(result);
        }

        results
    }

    /// Report test results
    pub fn report_results(&self, results: &[TestResult]) {
        let total = results.len();
        let passed = results.iter().filter(|r| r.status == TestStatus::Pass).count();
        let failed = total - passed;

        println!("\nTest Results:");
        println!("-------------");
        println!("Total tests:    {}", total);
        println!("Passed:         {}", passed);
        println!("Failed:         {}", failed);

        if total > 0 {
            let success_rate = (passed as f64 / total as f64) * 100.0;
            println!("Success rate:   {:.1}%", success_rate);
        }

        if failed > 0 {
            println!("\nFailures:");
            println!("---------");

            for (i, result) in results.iter().enumerate() {
                if result.status != TestStatus::Pass {
                    println!("{}. {}:", i + 1, result.test_case.name);

                    match result.status {
                        TestStatus::Pass => unreachable!(),
                        TestStatus::CompileFailed => {
                            println!("   - Error: CompileFailed");
                        }
                        TestStatus::ExecutionFailed => {
                            println!("   - Error: ExecutionFailed");
                        }
                        TestStatus::OutputMismatch => {
                            println!("   - Expected output: {:?}", result.test_case.expected_output);
                            println!("   - Actual output:   {:?}", result.actual_output.as_ref().unwrap());
                            println!("   - Error: OutputMismatch");
                        }
                        TestStatus::ExitCodeMismatch => {
                            println!("   - Expected exit code: {}", result.test_case.expected_exit_code);
                            println!("   - Actual exit code:   {}", result.actual_exit_code.unwrap());
                            println!("   - Error: ExitCodeMismatch");
                        }
                    }

                    if let Some(error_message) = &result.error_message {
                        println!("   - Message: {:?}", error_message);
                    }

                    println!();
                }
            }
        }
    }
}

impl Clone for TestCase {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            source_file: self.source_file.clone(),
            config_file: self.config_file.clone(),
            expected_output: self.expected_output.clone(),
            expected_exit_code: self.expected_exit_code,
        }
    }
}
