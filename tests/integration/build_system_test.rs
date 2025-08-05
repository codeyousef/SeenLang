//! Step 1 TDD Tests: Repository Structure & Build System
//! These tests MUST fail initially, then implementation makes them pass

use std::process::Command;
use std::path::Path;
use tempfile::TempDir;
use std::fs;

/// FAILING TEST: `seen build` compiles simple programs successfully
#[test]
fn test_seen_build_compiles_simple_programs() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_path = temp_dir.path();
    
    // Create a simple Seen project
    create_test_project(project_path, "simple_test", "en");
    
    // Write a simple program
    let main_seen = r#"
func main() {
    let x = 42;
    println("Hello, Seen!");
}
"#;
    
    fs::write(project_path.join("src/main.seen"), main_seen)
        .expect("Failed to write main.seen");
    
    // Test: `seen build` should succeed
    let output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "build"])
        .current_dir(project_path)
        .output()
        .expect("Failed to execute seen build");
    
    assert!(output.status.success(), 
        "seen build failed: {}", String::from_utf8_lossy(&output.stderr));
    
    // Verify output executable exists
    let target_dir = project_path.join("target");
    assert!(target_dir.exists(), "Target directory should be created");
    
    // Check for executable (platform-specific)
    let executable_name = if cfg!(windows) { "simple_test.exe" } else { "simple_test" };
    let executable_path = target_dir.join("native/debug").join(executable_name);
    assert!(executable_path.exists(), 
        "Executable should be created at: {}", executable_path.display());
}

/// FAILING TEST: `seen clean` removes all build artifacts  
#[test]
fn test_seen_clean_removes_artifacts() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_path = temp_dir.path();
    
    create_test_project(project_path, "clean_test", "en");
    
    // Build first to create artifacts
    let build_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "build"])
        .current_dir(project_path)
        .output()
        .expect("Failed to execute seen build");
    
    // Assume build creates target directory
    let target_dir = project_path.join("target");
    fs::create_dir_all(&target_dir).expect("Failed to create target dir");
    fs::write(target_dir.join("test_artifact.txt"), "test")
        .expect("Failed to create test artifact");
    
    // Test: `seen clean` should remove artifacts
    let clean_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "clean"])
        .current_dir(project_path)
        .output()
        .expect("Failed to execute seen clean");
    
    assert!(clean_output.status.success(),
        "seen clean failed: {}", String::from_utf8_lossy(&clean_output.stderr));
    
    // Verify artifacts are removed
    assert!(!target_dir.exists() || target_dir.read_dir().unwrap().count() == 0,
        "Target directory should be empty or removed");
}

/// FAILING TEST: `seen check` validates syntax without building
#[test]
fn test_seen_check_validates_syntax() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_path = temp_dir.path();
    
    create_test_project(project_path, "check_test", "en");
    
    // Write valid Seen code
    let valid_code = r#"
func main() {
    let x = 42;
    return x;
}
"#;
    fs::write(project_path.join("src/main.seen"), valid_code)
        .expect("Failed to write valid code");
    
    // Test: `seen check` should pass for valid code
    let check_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "check"])
        .current_dir(project_path)
        .output()
        .expect("Failed to execute seen check");
    
    assert!(check_output.status.success(),
        "seen check failed on valid code: {}", String::from_utf8_lossy(&check_output.stderr));
    
    // Verify no build artifacts created (check-only)
    let target_dir = project_path.join("target");
    assert!(!target_dir.exists() || target_dir.read_dir().unwrap().count() == 0,
        "seen check should not create build artifacts");
    
    // Test invalid code
    let invalid_code = r#"
func main() {
    let x = ; // Syntax error
    invalid_token @#$
}
"#;
    fs::write(project_path.join("src/main.seen"), invalid_code)
        .expect("Failed to write invalid code");
    
    let check_invalid = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "check"])
        .current_dir(project_path)
        .output()
        .expect("Failed to execute seen check on invalid code");
    
    assert!(!check_invalid.status.success(),
        "seen check should fail on invalid code");
}

/// FAILING TEST: Language files load from TOML configuration
#[test]
fn test_language_files_load_from_toml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_path = temp_dir.path();
    
    // Test English language loading
    create_test_project(project_path, "lang_test", "en");
    
    let english_code = r#"
func main() {
    if true {
        return 42;
    }
}
"#;
    fs::write(project_path.join("src/main.seen"), english_code)
        .expect("Failed to write English code");
    
    let check_en = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "check"])
        .current_dir(project_path)
        .output()
        .expect("Failed to check English code");
    
    assert!(check_en.status.success(),
        "English keywords should be recognized: {}", String::from_utf8_lossy(&check_en.stderr));
    
    // Test Arabic language loading
    create_test_project(project_path, "lang_test_ar", "ar");
    
    let arabic_code = r#"
دالة رئيسية() {
    إذا صحيح {
        ارجع 42;
    }
}
"#;
    fs::write(project_path.join("src/main.seen"), arabic_code)
        .expect("Failed to write Arabic code");
    
    // Update project config to use Arabic
    let seen_toml = format!(r#"
[project]
name = "lang_test_ar"
version = "0.1.0"
language = "ar"

[build]
targets = ["native"]
optimize = "speed"
"#);
    fs::write(project_path.join("Seen.toml"), seen_toml)
        .expect("Failed to write Arabic Seen.toml");
    
    let check_ar = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "check"])
        .current_dir(project_path)
        .output()
        .expect("Failed to check Arabic code");
    
    assert!(check_ar.status.success(),
        "Arabic keywords should be recognized: {}", String::from_utf8_lossy(&check_ar.stderr));
}

/// FAILING TEST: Hot reload completes in <50ms
#[test]
fn test_hot_reload_under_50ms() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_path = temp_dir.path();
    
    create_test_project(project_path, "hot_reload_test", "en");
    
    let simple_code = "func main() { let x = 1; }";
    fs::write(project_path.join("src/main.seen"), simple_code)
        .expect("Failed to write initial code");
    
    // First check (cold start)
    let _cold_check = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "check"])
        .current_dir(project_path)
        .output()
        .expect("Failed to execute cold check");
    
    // Modify the file slightly
    let modified_code = "func main() { let x = 2; }";
    fs::write(project_path.join("src/main.seen"), modified_code)
        .expect("Failed to write modified code");
    
    // Measure hot reload time
    let start = std::time::Instant::now();
    let hot_check = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "check"])
        .current_dir(project_path)
        .output()
        .expect("Failed to execute hot check");
    let hot_reload_time = start.elapsed();
    
    assert!(hot_check.status.success(),
        "Hot reload check should succeed");
    
    // HARD REQUIREMENT: <50ms hot reload for IDE responsiveness
    assert!(hot_reload_time < std::time::Duration::from_millis(50),
        "Hot reload took {:?}, must be <50ms for IDE responsiveness", hot_reload_time);
}

/// FAILING TEST: Build command startup must be <100ms
#[test]
fn test_build_command_startup_under_100ms() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_path = temp_dir.path();
    
    create_test_project(project_path, "startup_test", "en");
    
    let simple_code = "func main() { }";
    fs::write(project_path.join("src/main.seen"), simple_code)
        .expect("Failed to write simple code");
    
    // Measure build command startup time
    let start = std::time::Instant::now();
    let build_output = Command::new("cargo")
        .args(&["run", "--bin", "seen", "--", "build"])
        .current_dir(project_path)
        .output()
        .expect("Failed to execute seen build");
    let startup_time = start.elapsed();
    
    println!("Build startup time: {:?}", startup_time);
    
    // HARD REQUIREMENT: <100ms startup for developer experience
    assert!(startup_time < std::time::Duration::from_millis(100),
        "Build startup took {:?}, must be <100ms", startup_time);
}

/// Helper function to create test project structure
fn create_test_project(project_path: &Path, name: &str, language: &str) {
    // Create directory structure
    fs::create_dir_all(project_path.join("src")).expect("Failed to create src dir");
    
    // Create Seen.toml
    let seen_toml = format!(r#"
[project]
name = "{}"
version = "0.1.0"
language = "{}"

[build]
targets = ["native"]
optimize = "speed"

[format]
line-width = 100
indent = 4
trailing-comma = true
"#, name, language);
    
    fs::write(project_path.join("Seen.toml"), seen_toml)
        .expect("Failed to create Seen.toml");
    
    // Create empty main.seen (will be overwritten by tests)
    fs::write(project_path.join("src/main.seen"), "func main() { }")
        .expect("Failed to create main.seen");
}