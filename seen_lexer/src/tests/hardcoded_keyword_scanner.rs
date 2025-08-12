//! Tests to scan codebase for hardcoded keywords
//! 
//! This module implements comprehensive tests to detect and eliminate
//! all hardcoded keywords from the Seen language implementation.

use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;
use std::collections::HashSet;

/// Test to scan the entire codebase for hardcoded keywords
#[test]
fn test_scan_codebase_for_hardcoded_keywords() {
    let hardcoded_keywords = scan_for_hardcoded_keywords();
    
    if !hardcoded_keywords.is_empty() {
        panic!(
            "Found {} hardcoded keywords in codebase:\n{}",
            hardcoded_keywords.len(),
            hardcoded_keywords.join("\n")
        );
    }
}

/// Test to verify zero hardcoded keywords remain after cleanup
#[test]
fn test_verify_zero_hardcoded_keywords() {
    let hardcoded_keywords = scan_for_hardcoded_keywords();
    assert_eq!(
        hardcoded_keywords.len(),
        0,
        "Expected zero hardcoded keywords, but found: {:?}",
        hardcoded_keywords
    );
}

/// Test to validate keyword consistency across all language files
#[test]
fn test_keyword_validation_across_languages() {
    let validation_errors = validate_keyword_consistency();
    
    if !validation_errors.is_empty() {
        panic!(
            "Found {} keyword validation errors:\n{}",
            validation_errors.len(),
            validation_errors.join("\n")
        );
    }
}

/// Scan the codebase for hardcoded keywords
fn scan_for_hardcoded_keywords() -> Vec<String> {
    let mut hardcoded_keywords = Vec::new();
    
    // Define patterns that indicate hardcoded keywords in production code
    let keyword_patterns = vec![
        // Assert statements with hardcoded keywords
        r#"assert_eq!\s*\([^,]*\.is_keyword\s*\(\s*"(fun|if|else|while|for|match|and|or|not|move|borrow|inout|is|as|by)""#,
        // HashMap get operations with hardcoded keywords
        r#"\.get\s*\(\s*"(fun|if|else|while|for|match|and|or|not|move|borrow|inout|is|as|by)""#,
        // Direct string comparisons with keywords
        r#"==\s*"(fun|if|else|while|for|match|and|or|not|move|borrow|inout|is|as|by)""#,
        // Function calls with hardcoded keyword strings
        r#"check_keyword\s*\(\s*"(fun|if|else|while|for|match|and|or|not|move|borrow|inout|is|as|by)""#,
    ];
    
    let compiled_patterns: Vec<Regex> = keyword_patterns
        .iter()
        .map(|pattern| Regex::new(pattern).unwrap())
        .collect();
    
    // Scan all Rust source files
    let rust_files = find_rust_files();
    
    for file_path in rust_files {
        // Skip test files that contain legitimate test data
        if is_test_file(&file_path) {
            continue;
        }
        
        if let Ok(content) = fs::read_to_string(&file_path) {
            // Skip lines that are clearly test data or TOML content
            let lines: Vec<&str> = content.lines().collect();
            
            for (line_num, line) in lines.iter().enumerate() {
                // Skip lines that are part of TOML content creation or test data
                if is_toml_content_line(line) || is_test_data_line(line) || is_comment_line(line) {
                    continue;
                }
                
                for (pattern_idx, pattern) in compiled_patterns.iter().enumerate() {
                    for capture in pattern.captures_iter(line) {
                        if let Some(keyword) = capture.get(1) {
                            hardcoded_keywords.push(format!(
                                "File: {:?}, Line: {}, Pattern: {}, Keyword: '{}', Context: '{}'",
                                file_path,
                                line_num + 1,
                                pattern_idx,
                                keyword.as_str(),
                                line.trim()
                            ));
                        }
                    }
                }
            }
        }
    }
    
    hardcoded_keywords
}

/// Check if a file is a test file
fn is_test_file(path: &std::path::Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        name.starts_with("test_") || name.contains("_test") || name == "tests.rs"
    } else {
        false
    }
}

/// Check if a line contains TOML content creation
fn is_toml_content_line(line: &str) -> bool {
    line.contains("r#\"") || 
    line.contains("name = ") || 
    line.contains("description = ") || 
    line.contains("[keywords]") ||
    line.contains("KeywordFun") ||
    line.contains("KeywordIf") ||
    line.trim().starts_with("\"") && line.trim().ends_with("\"")
}

/// Check if a line contains test data
fn is_test_data_line(line: &str) -> bool {
    line.contains("let source = r#\"") ||
    line.contains("func main()") ||
    line.contains("println(") ||
    line.contains("val x =") ||
    line.contains("// Test") ||
    line.contains("// Arabic") ||
    line.contains("// English") ||
    line.contains("get_keyword_text") ||
    line.contains("get_logical_") ||
    line.contains("&KeywordType::")
}

/// Check if a line is a comment
fn is_comment_line(line: &str) -> bool {
    line.trim().starts_with("//")
}

/// Find all Rust source files in the project
fn find_rust_files() -> Vec<PathBuf> {
    let mut rust_files = Vec::new();
    
    // Start from current directory and scan recursively
    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !should_skip_directory(&path) {
                rust_files.extend(find_rust_files_recursive(&path));
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                rust_files.push(path);
            }
        }
    }
    
    rust_files
}

/// Recursively find Rust files in a directory
fn find_rust_files_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut rust_files = Vec::new();
    
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !should_skip_directory(&path) {
                rust_files.extend(find_rust_files_recursive(&path));
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                rust_files.push(path);
            }
        }
    }
    
    rust_files
}

/// Check if a directory should be skipped during scanning
fn should_skip_directory(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        matches!(name, "target" | ".git" | "node_modules" | ".vscode" | ".idea")
    } else {
        false
    }
}

/// Validate keyword consistency across all language files
fn validate_keyword_consistency() -> Vec<String> {
    let mut errors = Vec::new();
    
    // Load all language files
    let language_files = find_language_files();
    
    if language_files.is_empty() {
        errors.push("No language files found".to_string());
        return errors;
    }
    
    // Define required keywords that must exist in all languages
    let required_keywords = vec![
        "KeywordFun", "KeywordIf", "KeywordElse", "KeywordAnd", "KeywordOr", "KeywordNot",
        "KeywordMove", "KeywordBorrow", "KeywordInout", "KeywordIs", "KeywordAs", "KeywordBy",
        "KeywordWhile", "KeywordFor", "KeywordMatch", "KeywordLet", "KeywordMut", "KeywordConst",
    ];
    
    for lang_file in language_files {
        if let Ok(content) = fs::read_to_string(&lang_file) {
            // Parse TOML content to check for required keywords
            match toml::from_str::<toml::Value>(&content) {
                Ok(toml_value) => {
                    if let Some(keywords) = toml_value.get("keywords").and_then(|k| k.as_table()) {
                        let available_keyword_types: HashSet<String> = keywords
                            .values()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect();
                        
                        for required_keyword in &required_keywords {
                            if !available_keyword_types.contains(*required_keyword) {
                                errors.push(format!(
                                    "Language file {:?} is missing required keyword: {}",
                                    lang_file,
                                    required_keyword
                                ));
                            }
                        }
                    } else {
                        errors.push(format!(
                            "Language file {:?} does not have a [keywords] section",
                            lang_file
                        ));
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to parse language file {:?}: {}",
                        lang_file,
                        e
                    ));
                }
            }
        } else {
            errors.push(format!(
                "Failed to read language file: {:?}",
                lang_file
            ));
        }
    }
    
    errors
}

/// Find all language TOML files
fn find_language_files() -> Vec<PathBuf> {
    let mut language_files = Vec::new();
    
    // Look for languages directory in common locations
    let possible_dirs = vec![
        PathBuf::from("languages"),
        PathBuf::from("../languages"),
        PathBuf::from("../../languages"),
    ];
    
    for dir in possible_dirs {
        if dir.exists() && dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "toml") {
                        language_files.push(path);
                    }
                }
            }
            break; // Use the first valid directory found
        }
    }
    
    language_files
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_find_rust_files() {
        let rust_files = find_rust_files();
        assert!(!rust_files.is_empty(), "Should find at least some Rust files");
        
        // Verify all found files have .rs extension
        for file in &rust_files {
            assert_eq!(
                file.extension().and_then(|ext| ext.to_str()),
                Some("rs"),
                "File {:?} should have .rs extension",
                file
            );
        }
    }
    
    #[test]
    fn test_find_language_files() {
        let language_files = find_language_files();
        
        // Should find at least the minimum 10 languages
        assert!(
            language_files.len() >= 10,
            "Should find at least 10 language files, found: {}",
            language_files.len()
        );
        
        // Verify all found files have .toml extension
        for file in &language_files {
            assert_eq!(
                file.extension().and_then(|ext| ext.to_str()),
                Some("toml"),
                "File {:?} should have .toml extension",
                file
            );
        }
    }
    
    #[test]
    fn test_should_skip_directory() {
        assert!(should_skip_directory(&PathBuf::from("target")));
        assert!(should_skip_directory(&PathBuf::from(".git")));
        assert!(should_skip_directory(&PathBuf::from("node_modules")));
        assert!(!should_skip_directory(&PathBuf::from("src")));
        assert!(!should_skip_directory(&PathBuf::from("tests")));
    }
}