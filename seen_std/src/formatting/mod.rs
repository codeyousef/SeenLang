//! Document formatting infrastructure for Seen projects
//!
//! This module provides comprehensive document formatting capabilities for:
//! - Seen code files (.seen)
//! - Markdown documentation (.md)
//! - TOML configuration files (.toml)
//! - Project documentation and configuration

use crate::string::String;
// use crate::collections::Vec;  // Unused for now
use std::path::Path;

/// Formatting configuration options
#[derive(Debug, Clone)]
pub struct FormatConfig {
    pub indent_size: u32,
    pub max_line_length: u32,
    pub trailing_comma: bool,
    pub preserve_comments: bool,
    pub preserve_blank_lines: bool,
    pub format_markdown: bool,
    pub format_toml: bool,
    pub format_seen: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent_size: 4,
            max_line_length: 120,
            trailing_comma: true,
            preserve_comments: true,
            preserve_blank_lines: true,
            format_markdown: true,
            format_toml: true,
            format_seen: true,
        }
    }
}

/// Result of a formatting operation
#[derive(Debug, Clone, PartialEq)]
pub struct FormatResult {
    pub original_content: String,
    pub formatted_content: String,
    pub changed: bool,
    pub error: Option<String>,
}

impl FormatResult {
    pub fn new(original: String, formatted: String) -> Self {
        let changed = original != formatted;
        Self {
            original_content: original,
            formatted_content: formatted,
            changed,
            error: None,
        }
    }
    
    pub fn error(original: String, error_msg: String) -> Self {
        Self {
            original_content: original.clone(),
            formatted_content: original,
            changed: false,
            error: Some(error_msg),
        }
    }
}

/// File type detection for formatting
#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    Seen,
    Markdown,
    Toml,
    Unknown,
}

impl FileType {
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("seen") => FileType::Seen,
            Some("md") | Some("markdown") => FileType::Markdown,
            Some("toml") => FileType::Toml,
            _ => FileType::Unknown,
        }
    }
}

/// Main document formatter trait
pub trait DocumentFormatter {
    fn format(&self, content: &str, config: &FormatConfig) -> FormatResult;
    fn supports_file_type(&self, file_type: &FileType) -> bool;
    fn name(&self) -> &'static str;
}

pub mod seen_formatter;
pub mod markdown_formatter;
pub mod toml_formatter;

/// Formats a document based on file type
pub fn format_document(content: &str, file_type: &FileType, config: &FormatConfig) -> FormatResult {
    match file_type {
        FileType::Seen => {
            let formatter = seen_formatter::SeenFormatter::new();
            formatter.format(content, config)
        },
        FileType::Markdown => {
            let formatter = markdown_formatter::MarkdownFormatter::new();
            formatter.format(content, config)
        },
        FileType::Toml => {
            let formatter = toml_formatter::TomlFormatter::new();
            formatter.format(content, config)
        },
        FileType::Unknown => {
            FormatResult::error(
                String::from(content), 
                String::from("Unknown file type, cannot format")
            )
        }
    }
}

/// Format statistics for reporting
#[derive(Debug, Clone, Default)]
pub struct FormatStats {
    pub files_processed: u32,
    pub files_changed: u32,
    pub files_with_errors: u32,
    pub total_lines: u32,
    pub lines_changed: u32,
}

impl FormatStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_result(&mut self, result: &FormatResult) {
        self.files_processed += 1;
        
        if result.error.is_some() {
            self.files_with_errors += 1;
        } else if result.changed {
            self.files_changed += 1;
        }
        
        let original_lines = result.original_content.lines().count() as u32;
        let _formatted_lines = result.formatted_content.lines().count() as u32;
        
        self.total_lines += original_lines;
        if result.changed {
            self.lines_changed += original_lines;
        }
    }
    
    pub fn success_rate(&self) -> f64 {
        if self.files_processed == 0 {
            0.0
        } else {
            (self.files_processed - self.files_with_errors) as f64 / self.files_processed as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_config_default() {
        let config = FormatConfig::default();
        assert_eq!(config.indent_size, 4);
        assert_eq!(config.max_line_length, 120);
        assert!(config.trailing_comma);
        assert!(config.preserve_comments);
    }
    
    #[test]
    fn test_file_type_detection() {
        assert_eq!(FileType::from_path(Path::new("test.seen")), FileType::Seen);
        assert_eq!(FileType::from_path(Path::new("README.md")), FileType::Markdown);
        assert_eq!(FileType::from_path(Path::new("Cargo.toml")), FileType::Toml);
        assert_eq!(FileType::from_path(Path::new("unknown.txt")), FileType::Unknown);
    }
    
    #[test]
    fn test_format_result() {
        let original = String::from("original content");
        let formatted = String::from("formatted content");
        let result = FormatResult::new(original, formatted);
        
        assert!(result.changed);
        assert!(result.error.is_none());
    }
    
    #[test]
    fn test_format_stats() {
        let mut stats = FormatStats::new();
        
        let result1 = FormatResult::new(String::from("old"), String::from("new"));
        let result2 = FormatResult::error(String::from("bad"), String::from("error"));
        
        stats.add_result(&result1);
        stats.add_result(&result2);
        
        assert_eq!(stats.files_processed, 2);
        assert_eq!(stats.files_changed, 1);
        assert_eq!(stats.files_with_errors, 1);
        assert_eq!(stats.success_rate(), 0.5);
    }
}