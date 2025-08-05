//! Step 1 TDD Tests: Self-Hosting Infrastructure
//! Tests for process spawning, pipe communication, and environment variables

use std::process::{Command, Stdio};
use std::io::{Write, Read};
use std::env;
use std::path::PathBuf;
use std::time::Duration;

/// FAILING TEST: Process spawning and management works
#[test]
fn test_process_spawning_and_management() {
    // Test: Spawn a child process and verify it runs
    let output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "spawn", "echo", "hello"])
        .output()
        .expect("Failed to spawn process");
    
    assert!(output.status.success(), 
        "Process spawning failed: {}", String::from_utf8_lossy(&output.stderr));
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"), "Expected 'hello' in output, got: {}", stdout);
    
    // Test: Get process exit code
    let exit_code_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "spawn", "--exit-code", "exit", "42"])
        .output()
        .expect("Failed to spawn process");
    
    assert_eq!(exit_code_output.status.code(), Some(42), 
        "Expected exit code 42");
}

/// FAILING TEST: Pipe communication between processes works
#[test]
fn test_pipe_communication() {
    // Test: Create a pipe between two processes
    let mut child = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "pipe", "producer"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn producer process");
    
    let producer_stdout = child.stdout.take().expect("Failed to capture stdout");
    
    let consumer = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "pipe", "consumer"])
        .stdin(producer_stdout)
        .output()
        .expect("Failed to spawn consumer process");
    
    child.wait().expect("Producer process failed");
    
    assert!(consumer.status.success(), 
        "Pipe communication failed: {}", String::from_utf8_lossy(&consumer.stderr));
    
    let output = String::from_utf8_lossy(&consumer.stdout);
    assert!(output.contains("Received from producer"), 
        "Expected piped data, got: {}", output);
}

/// FAILING TEST: Environment variable access and manipulation works
#[test]
fn test_environment_variables() {
    // Test: Read environment variables
    let read_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "env", "get", "PATH"])
        .output()
        .expect("Failed to read environment variable");
    
    assert!(read_output.status.success(), 
        "Failed to read env var: {}", String::from_utf8_lossy(&read_output.stderr));
    
    let path_value = String::from_utf8_lossy(&read_output.stdout);
    assert!(!path_value.is_empty(), "PATH should not be empty");
    
    // Test: Set environment variables
    let set_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "env", "set", "SEEN_TEST_VAR", "test_value"])
        .env("SEEN_TEST_VAR", "test_value")
        .output()
        .expect("Failed to set environment variable");
    
    assert!(set_output.status.success(), 
        "Failed to set env var: {}", String::from_utf8_lossy(&set_output.stderr));
    
    // Test: Verify environment variable was set
    let verify_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "env", "get", "SEEN_TEST_VAR"])
        .env("SEEN_TEST_VAR", "test_value")
        .output()
        .expect("Failed to verify environment variable");
    
    let var_value = String::from_utf8_lossy(&verify_output.stdout);
    assert!(var_value.contains("test_value"), 
        "Expected 'test_value', got: {}", var_value);
}

/// FAILING TEST: Working directory management works
#[test]
fn test_working_directory_management() {
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();
    
    // Test: Get current working directory
    let pwd_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "pwd"])
        .current_dir(temp_path)
        .output()
        .expect("Failed to get working directory");
    
    assert!(pwd_output.status.success(), 
        "Failed to get pwd: {}", String::from_utf8_lossy(&pwd_output.stderr));
    
    let pwd = PathBuf::from(String::from_utf8_lossy(&pwd_output.stdout).trim());
    assert_eq!(pwd.canonicalize().unwrap(), temp_path.canonicalize().unwrap(), 
        "Working directory mismatch");
    
    // Test: Change working directory
    let subdir = temp_path.join("subdir");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");
    
    let cd_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "cd", "subdir"])
        .current_dir(temp_path)
        .output()
        .expect("Failed to change directory");
    
    assert!(cd_output.status.success(), 
        "Failed to change directory: {}", String::from_utf8_lossy(&cd_output.stderr));
}

/// FAILING TEST: Exit code handling works correctly
#[test]
fn test_exit_code_handling() {
    // Test: Exit with code 0 (success)
    let success_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "exit", "0"])
        .output()
        .expect("Failed to exit with code 0");
    
    assert_eq!(success_output.status.code(), Some(0), 
        "Expected exit code 0");
    
    // Test: Exit with non-zero code (failure)
    let failure_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "exit", "1"])
        .output()
        .expect("Failed to exit with code 1");
    
    assert_eq!(failure_output.status.code(), Some(1), 
        "Expected exit code 1");
    
    // Test: Exit with custom code
    let custom_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "exit", "123"])
        .output()
        .expect("Failed to exit with code 123");
    
    assert_eq!(custom_output.status.code(), Some(123), 
        "Expected exit code 123");
}

/// FAILING TEST: Process spawning with timeout works
#[test]
fn test_process_timeout() {
    use std::time::Instant;
    
    let start = Instant::now();
    
    // Test: Spawn process with timeout
    let timeout_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "spawn", "--timeout", "100", "sleep", "5"])
        .output()
        .expect("Failed to spawn process with timeout");
    
    let elapsed = start.elapsed();
    
    // Process should have been killed after 100ms timeout
    assert!(elapsed < Duration::from_secs(1), 
        "Process should have timed out after 100ms, took {:?}", elapsed);
    
    assert!(!timeout_output.status.success(), 
        "Process should have failed due to timeout");
    
    let stderr = String::from_utf8_lossy(&timeout_output.stderr);
    assert!(stderr.contains("timeout") || stderr.contains("Timeout"), 
        "Expected timeout error, got: {}", stderr);
}

/// FAILING TEST: Process output capture works
#[test]
fn test_process_output_capture() {
    // Test: Capture stdout
    let stdout_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "capture", "stdout", "echo", "stdout message"])
        .output()
        .expect("Failed to capture stdout");
    
    assert!(stdout_output.status.success());
    let stdout = String::from_utf8_lossy(&stdout_output.stdout);
    assert!(stdout.contains("stdout message"), 
        "Expected stdout capture, got: {}", stdout);
    
    // Test: Capture stderr
    let stderr_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "capture", "stderr", "echo-stderr", "stderr message"])
        .output()
        .expect("Failed to capture stderr");
    
    let captured_stderr = String::from_utf8_lossy(&stderr_output.stdout);
    assert!(captured_stderr.contains("stderr message"), 
        "Expected stderr capture in stdout, got: {}", captured_stderr);
    
    // Test: Capture both stdout and stderr
    let both_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "capture", "both", "echo-both"])
        .output()
        .expect("Failed to capture both streams");
    
    let output = String::from_utf8_lossy(&both_output.stdout);
    assert!(output.contains("stdout") && output.contains("stderr"), 
        "Expected both stdout and stderr capture, got: {}", output);
}