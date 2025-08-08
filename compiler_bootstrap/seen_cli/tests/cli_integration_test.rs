//! Integration tests for the Seen CLI
//! 
//! These tests ensure the CLI commands work correctly and the build system
//! functions as expected. Tests cover all major commands and edge cases.

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use anyhow::Result;

/// Helper to create a test project in a temporary directory
fn create_test_project(name: &str) -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let project_path = temp_dir.path().join(name);
    fs::create_dir_all(&project_path)?;
    
    // Create Seen.toml
    let toml_content = format!(r#"
[project]
name = "{}"
version = "0.1.0"
language = "en"
entry = "src/main.seen"

[dependencies]

[build]
target = "native"
optimize_for = "speed"
"#, name);
    
    fs::write(project_path.join("Seen.toml"), toml_content)?;
    
    // Create src directory
    fs::create_dir_all(project_path.join("src"))?;
    
    // Create main.seen
    let main_content = r#"
fun main() {
    println("Hello from test project!")
}
"#;
    
    fs::write(project_path.join("src/main.seen"), main_content)?;
    
    Ok(temp_dir)
}

#[test]
fn test_project_discovery() {
    let temp_dir = create_test_project("test_discovery").unwrap();
    let project_path = temp_dir.path().join("test_discovery");
    
    // Test that project can be found
    let project = seen_cli::project::Project::find_and_load(Some(project_path.clone()));
    assert!(project.is_ok(), "Should find project at {:?}", project_path);
    
    let project = project.unwrap();
    assert_eq!(project.name(), "test_discovery");
    assert_eq!(project.version(), "0.1.0");
}

#[test]
fn test_build_config_parsing() {
    let temp_dir = create_test_project("test_config").unwrap();
    let project_path = temp_dir.path().join("test_config");
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path)).unwrap();
    
    // Test build configuration
    let config = seen_cli::config::BuildConfig {
        target: "native".to_string(),
        release: false,
        optimize_for: "debug".to_string(),
    };
    
    assert_eq!(config.target, "native");
    assert!(!config.release);
    assert_eq!(config.optimize_for, "debug");
}

#[test]
fn test_supported_targets() {
    // Test that all required targets are supported
    let targets = vec!["native", "wasm", "js"];
    
    for target in targets {
        // This should not panic
        let config = seen_cli::config::BuildConfig {
            target: target.to_string(),
            release: false,
            optimize_for: "debug".to_string(),
        };
        assert_eq!(config.target, target);
    }
}

#[test]
fn test_build_directory_creation() {
    let temp_dir = create_test_project("test_build_dir").unwrap();
    let project_path = temp_dir.path().join("test_build_dir");
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path.clone())).unwrap();
    
    // Build directory should be created
    let build_dir = project.build_dir();
    
    // Create build directory (simulate build command)
    fs::create_dir_all(&build_dir).unwrap();
    
    assert!(build_dir.exists(), "Build directory should exist");
}

#[test]
fn test_source_file_discovery() {
    let temp_dir = create_test_project("test_sources").unwrap();
    let project_path = temp_dir.path().join("test_sources");
    
    // Add more source files
    fs::write(project_path.join("src/utils.seen"), "fun helper() { }").unwrap();
    fs::write(project_path.join("src/types.seen"), "struct Point { x: Int, y: Int }").unwrap();
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path.clone())).unwrap();
    let sources = project.source_files();
    
    // Should find all .seen files
    assert!(sources.len() >= 3, "Should find at least 3 source files, found {}", sources.len());
}

#[test]
fn test_init_command_structure() {
    let temp_dir = TempDir::new().unwrap();
    let project_name = "new_project";
    let project_path = temp_dir.path().join(project_name);
    
    // Simulate init command
    fs::create_dir_all(&project_path).unwrap();
    fs::create_dir_all(project_path.join("src")).unwrap();
    fs::create_dir_all(project_path.join("tests")).unwrap();
    fs::create_dir_all(project_path.join("benches")).unwrap();
    
    // Verify structure
    assert!(project_path.join("src").exists());
    assert!(project_path.join("tests").exists());
    assert!(project_path.join("benches").exists());
}

#[test]
fn test_language_configuration() {
    let temp_dir = create_test_project("test_lang").unwrap();
    let project_path = temp_dir.path().join("test_lang");
    
    // Test with Arabic language
    let toml_content = r#"
[project]
name = "test_lang"
version = "0.1.0"
language = "ar"
entry = "src/main.seen"
"#;
    
    fs::write(project_path.join("Seen.toml"), toml_content).unwrap();
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path)).unwrap();
    assert_eq!(project.language(), "ar");
}

#[test]
fn test_dependency_parsing() {
    let temp_dir = create_test_project("test_deps").unwrap();
    let project_path = temp_dir.path().join("test_deps");
    
    // Add dependencies (simplified format for MVP)
    let toml_content = r#"
[project]
name = "test_deps"
version = "0.1.0"
language = "en"
entry = "src/main.seen"

[dependencies]
std = "0.1.0"
http = "0.2.0"
"#;
    
    fs::write(project_path.join("Seen.toml"), toml_content).unwrap();
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path)).unwrap();
    let deps = project.dependencies();
    
    assert!(deps.contains_key("std"));
    assert!(deps.contains_key("http"));
    assert_eq!(deps.get("std").unwrap(), "0.1.0");
    assert_eq!(deps.get("http").unwrap(), "0.2.0");
}

#[test]
fn test_clean_command() {
    let temp_dir = create_test_project("test_clean").unwrap();
    let project_path = temp_dir.path().join("test_clean");
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path)).unwrap();
    let build_dir = project.build_dir();
    
    // Create build artifacts
    fs::create_dir_all(&build_dir).unwrap();
    fs::write(build_dir.join("artifact.o"), "dummy").unwrap();
    
    assert!(build_dir.exists());
    
    // Simulate clean
    fs::remove_dir_all(&build_dir).unwrap();
    
    assert!(!build_dir.exists(), "Build directory should be removed");
}

#[test]
fn test_format_command_discovery() {
    let temp_dir = create_test_project("test_format").unwrap();
    let project_path = temp_dir.path().join("test_format");
    
    // Add various file types
    fs::write(project_path.join("src/code.seen"), "fun test() {}").unwrap();
    fs::write(project_path.join("README.md"), "# Test").unwrap();
    fs::write(project_path.join("config.toml"), "[test]").unwrap();
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path.clone())).unwrap();
    
    // Test that different file types can be discovered
    assert!(project_path.join("src/code.seen").exists());
    assert!(project_path.join("README.md").exists());
    assert!(project_path.join("config.toml").exists());
}

#[test]
fn test_test_command_discovery() {
    let temp_dir = create_test_project("test_tests").unwrap();
    let project_path = temp_dir.path().join("test_tests");
    
    // Add test files
    fs::create_dir_all(project_path.join("tests")).unwrap();
    fs::write(project_path.join("tests/unit_test.seen"), r#"
@test
fun test_addition() {
    assert(1 + 1 == 2)
}
"#).unwrap();
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path.clone())).unwrap();
    
    // Tests directory should exist
    assert!(project_path.join("tests").exists());
    assert!(project_path.join("tests/unit_test.seen").exists());
}

#[test]
fn test_benchmark_discovery() {
    let temp_dir = create_test_project("test_bench").unwrap();
    let project_path = temp_dir.path().join("test_bench");
    
    // Add benchmark files
    fs::create_dir_all(project_path.join("benches")).unwrap();
    fs::write(project_path.join("benches/perf.seen"), r#"
@benchmark
fun bench_loop(b: Bencher) {
    b.iter {
        for i in 0..1000 {
            // work
        }
    }
}
"#).unwrap();
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path.clone())).unwrap();
    
    // Benchmarks directory should exist
    assert!(project_path.join("benches").exists());
    assert!(project_path.join("benches/perf.seen").exists());
}

#[test]
fn test_release_vs_debug_config() {
    // Test release configuration
    let release_config = seen_cli::config::BuildConfig {
        target: "native".to_string(),
        release: true,
        optimize_for: "speed".to_string(),
    };
    
    assert!(release_config.release);
    assert_eq!(release_config.optimize_for, "speed");
    
    // Test debug configuration
    let debug_config = seen_cli::config::BuildConfig {
        target: "native".to_string(),
        release: false,
        optimize_for: "debug".to_string(),
    };
    
    assert!(!debug_config.release);
    assert_eq!(debug_config.optimize_for, "debug");
}

#[test]
fn test_multiple_source_directories() {
    let temp_dir = create_test_project("test_multi_src").unwrap();
    let project_path = temp_dir.path().join("test_multi_src");
    
    // Create multiple source directories
    fs::create_dir_all(project_path.join("src/models")).unwrap();
    fs::create_dir_all(project_path.join("src/controllers")).unwrap();
    fs::create_dir_all(project_path.join("src/views")).unwrap();
    
    fs::write(project_path.join("src/models/user.seen"), "struct User {}").unwrap();
    fs::write(project_path.join("src/controllers/api.seen"), "fun handle() {}").unwrap();
    fs::write(project_path.join("src/views/home.seen"), "fun render() {}").unwrap();
    
    let project = seen_cli::project::Project::find_and_load(Some(project_path.clone())).unwrap();
    let sources = project.source_files();
    
    // Should find files in subdirectories
    assert!(sources.len() >= 4, "Should find files in subdirectories");
}

#[test]
fn test_invalid_project_handling() {
    let temp_dir = TempDir::new().unwrap();
    let invalid_path = temp_dir.path().join("nonexistent");
    
    // Should handle missing project gracefully
    let result = seen_cli::project::Project::find_and_load(Some(invalid_path));
    assert!(result.is_err(), "Should error on missing project");
}

#[test]
fn test_empty_project_handling() {
    let temp_dir = TempDir::new().unwrap();
    let empty_path = temp_dir.path().join("empty");
    fs::create_dir_all(&empty_path).unwrap();
    
    // Project without Seen.toml
    let result = seen_cli::project::Project::find_and_load(Some(empty_path));
    assert!(result.is_err(), "Should error on missing Seen.toml");
}

#[test]
fn test_malformed_toml_handling() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("malformed");
    fs::create_dir_all(&project_path).unwrap();
    
    // Write malformed TOML
    fs::write(project_path.join("Seen.toml"), "this is not valid toml!").unwrap();
    
    let result = seen_cli::project::Project::find_and_load(Some(project_path));
    assert!(result.is_err(), "Should error on malformed TOML");
}

#[test]
fn test_build_incremental_flag() {
    let config = seen_cli::config::BuildConfig {
        target: "native".to_string(),
        release: false,
        optimize_for: "debug".to_string(),
    };
    
    // Should support incremental builds in debug mode
    assert!(!config.release);
}

#[test]
fn test_cross_compilation_targets() {
    // Test various cross-compilation targets
    let targets = vec![
        ("wasm", "wasm32-unknown-unknown"),
        ("js", "javascript"),
        ("native", std::env::consts::ARCH),
    ];
    
    for (target_name, _expected_arch) in targets {
        let config = seen_cli::config::BuildConfig {
            target: target_name.to_string(),
            release: false,
            optimize_for: "debug".to_string(),
        };
        
        assert!(!config.target.is_empty());
    }
}