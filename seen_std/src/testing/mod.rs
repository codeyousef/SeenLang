//! Testing framework for Seen language
//!
//! Provides assertion macros, test runners, and benchmark infrastructure
//! optimized for compiler and systems programming workloads

use crate::string::String;
use crate::collections::Vec;

pub mod bench;

/// Test result tracking
#[derive(Debug, Clone, PartialEq)]
pub enum TestResult {
    Passed,
    Failed(String),
    Skipped(String),
    Ignored,
}

/// Test execution statistics
#[derive(Debug, Clone, Default)]
pub struct TestStats {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub ignored: usize,
    pub total_duration_ns: u64,
}

impl TestStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_result(&mut self, result: &TestResult, duration_ns: u64) {
        self.total_duration_ns += duration_ns;
        match result {
            TestResult::Passed => self.passed += 1,
            TestResult::Failed(_) => self.failed += 1,
            TestResult::Skipped(_) => self.skipped += 1,
            TestResult::Ignored => self.ignored += 1,
        }
    }
    
    pub fn total_tests(&self) -> usize {
        self.passed + self.failed + self.skipped + self.ignored
    }
    
    pub fn success_rate(&self) -> f64 {
        if self.total_tests() == 0 {
            1.0
        } else {
            self.passed as f64 / self.total_tests() as f64
        }
    }
}

/// Test function metadata
#[derive(Debug, Clone)]
pub struct TestInfo {
    pub name: String,
    pub module: String,
    pub file: String,
    pub line: u32,
    pub ignored: bool,
    pub should_panic: bool,
}

impl TestInfo {
    pub fn new(name: &str, module: &str, file: &str, line: u32) -> Self {
        Self {
            name: String::from(name),
            module: String::from(module),
            file: String::from(file),
            line,
            ignored: false,
            should_panic: false,
        }
    }
    
    pub fn ignore(mut self) -> Self {
        self.ignored = true;
        self
    }
    
    pub fn should_panic(mut self) -> Self {
        self.should_panic = true;
        self
    }
}

/// Test runner configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub parallel: bool,
    pub fail_fast: bool,
    pub filter: Option<String>,
    pub exact_match: bool,
    pub list_only: bool,
    pub quiet: bool,
    pub show_output: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            parallel: true,
            fail_fast: false,
            filter: None,
            exact_match: false,
            list_only: false,
            quiet: false,
            show_output: false,
        }
    }
}

/// Core assertion functions
pub mod assertions {
    use super::*;
    
    /// Assert that a condition is true
    pub fn assert(condition: bool, message: &str) -> TestResult {
        if condition {
            TestResult::Passed
        } else {
            TestResult::Failed(String::from(message))
        }
    }
    
    /// Assert that two values are equal
    pub fn assert_eq<T: PartialEq + std::fmt::Debug>(left: T, right: T) -> TestResult {
        if left == right {
            TestResult::Passed
        } else {
            TestResult::Failed(String::from(&format!(
                "assertion failed: `(left == right)`\n  left: `{:?}`,\n right: `{:?}`", 
                left, right
            )))
        }
    }
    
    /// Assert that two values are not equal
    pub fn assert_ne<T: PartialEq + std::fmt::Debug>(left: T, right: T) -> TestResult {
        if left != right {
            TestResult::Passed
        } else {
            TestResult::Failed(String::from(&format!(
                "assertion failed: `(left != right)`\n  left: `{:?}`,\n right: `{:?}`", 
                left, right
            )))
        }
    }
    
    /// Assert that a condition is true with custom message
    pub fn assert_with_msg(condition: bool, msg: &str) -> TestResult {
        if condition {
            TestResult::Passed
        } else {
            TestResult::Failed(String::from(msg))
        }
    }
    
    /// Assert that an operation panics
    pub fn assert_panics<F: FnOnce()>(f: F) -> TestResult {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        match result {
            Ok(_) => TestResult::Failed(String::from("assertion failed: expected panic but function completed normally")),
            Err(_) => TestResult::Passed,
        }
    }
}

/// Test runner implementation
pub struct TestRunner {
    config: TestConfig,
    tests: Vec<(TestInfo, fn() -> TestResult)>,
}

impl TestRunner {
    pub fn new(config: TestConfig) -> Self {
        Self {
            config,
            tests: Vec::new(),
        }
    }
    
    /// Register a test function
    pub fn register_test(&mut self, info: TestInfo, test_fn: fn() -> TestResult) {
        self.tests.push((info, test_fn));
    }
    
    /// Run all registered tests
    pub fn run_tests(&self) -> TestStats {
        let mut stats = TestStats::new();
        
        // Filter tests if needed
        let filtered_tests: Vec<_> = self.tests.iter()
            .filter(|(info, _)| self.should_run_test(info))
            .collect();
        
        if self.config.list_only {
            for (info, _) in &filtered_tests {
                println!("{}: test", info.name);
            }
            return stats;
        }
        
        if !self.config.quiet {
            println!("running {} tests", filtered_tests.len());
        }
        
        // Execute tests
        for (info, test_fn) in filtered_tests {
            let start_time = std::time::Instant::now();
            
            let result = if info.ignored {
                TestResult::Ignored
            } else {
                // Execute the test function
                let test_result = test_fn();
                
                // Handle should_panic expectation
                if info.should_panic {
                    match test_result {
                        TestResult::Passed => TestResult::Failed(String::from("test did not panic as expected")),
                        TestResult::Failed(_) => TestResult::Passed, // Assume failure was from expected panic
                        other => other,
                    }
                } else {
                    test_result
                }
            };
            
            let duration_ns = start_time.elapsed().as_nanos() as u64;
            stats.add_result(&result, duration_ns);
            
            if !self.config.quiet {
                match &result {
                    TestResult::Passed => println!("test {} ... ok", info.name),
                    TestResult::Failed(msg) => {
                        println!("test {} ... FAILED", info.name);
                        if self.config.show_output {
                            println!("  {}", msg);
                        }
                    },
                    TestResult::Skipped(msg) => println!("test {} ... skipped: {}", info.name, msg),
                    TestResult::Ignored => println!("test {} ... ignored", info.name),
                }
            }
            
            // Fail fast if enabled
            if self.config.fail_fast && matches!(result, TestResult::Failed(_)) {
                break;
            }
        }
        
        stats
    }
    
    /// Check if a test should be run based on filter and configuration
    fn should_run_test(&self, info: &TestInfo) -> bool {
        if let Some(ref filter) = self.config.filter {
            if self.config.exact_match {
                info.name == *filter
            } else {
                info.name.contains(filter)
            }
        } else {
            true
        }
    }
    
    /// Print final test results
    pub fn print_results(&self, stats: &TestStats) {
        if self.config.quiet {
            return;
        }
        
        println!("\ntest result: {}. {} passed; {} failed; {} ignored; {} filtered out; finished in {:.2}s",
            if stats.failed == 0 { "ok" } else { "FAILED" },
            stats.passed,
            stats.failed,
            stats.ignored,
            0, // TODO: track filtered count
            stats.total_duration_ns as f64 / 1_000_000_000.0
        );
        
        if stats.failed > 0 {
            println!("\nfailures:");
            // TODO: collect and display failure details
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::assertions::*;
    
    #[test]
    fn test_assert_basic() {
        let result = assert(true, "should pass");
        assert_eq!(result, TestResult::Passed);
        
        let result = assert(false, "should fail");
        match result {
            TestResult::Failed(msg) => assert_eq!(msg, String::from("should fail")),
            _ => panic!("Expected failure"),
        }
    }
    
    #[test]
    fn test_assert_eq_basic() {
        let result = assert_eq(42, 42);
        assert_eq!(result, TestResult::Passed);
        
        let result = assert_eq(42, 24);
        match result {
            TestResult::Failed(_) => {}, // Expected
            _ => panic!("Expected failure"),
        }
    }
    
    #[test]
    fn test_stats_tracking() {
        let mut stats = TestStats::new();
        stats.add_result(&TestResult::Passed, 1000);
        stats.add_result(&TestResult::Failed(String::from("error")), 2000);
        stats.add_result(&TestResult::Ignored, 0);
        
        assert_eq!(stats.passed, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.ignored, 1);
        assert_eq!(stats.total_tests(), 3);
        assert_eq!(stats.total_duration_ns, 3000);
    }
    
    #[test]
    fn test_info_creation() {
        let info = TestInfo::new("test_func", "my_module", "test.rs", 42);
        assert_eq!(info.name, "test_func");
        assert_eq!(info.module, "my_module");
        assert_eq!(info.file, "test.rs");
        assert_eq!(info.line, 42);
        assert_eq!(info.ignored, false);
        assert_eq!(info.should_panic, false);
        
        let info = info.ignore().should_panic();
        assert_eq!(info.ignored, true);
        assert_eq!(info.should_panic, true);
    }
}