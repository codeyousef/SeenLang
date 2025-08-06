//! Simple pretty printing utilities for code and data formatting
//!
//! A simplified pretty printer that works with our current codebase

use crate::string::String;
use crate::collections::Vec;

/// Simple pretty printer configuration
#[derive(Debug, Clone)]
pub struct PrettyConfig {
    /// Indentation string (spaces or tabs)
    pub indent: String,
    /// Maximum line width
    pub max_width: usize,
    /// Use colors
    pub use_colors: bool,
}

impl Default for PrettyConfig {
    fn default() -> Self {
        PrettyConfig {
            indent: String::from("  "),
            max_width: 80,
            use_colors: false,
        }
    }
}

/// Simple pretty printer
pub struct PrettyPrinter {
    config: PrettyConfig,
}

impl PrettyPrinter {
    /// Create new pretty printer
    pub fn new() -> Self {
        PrettyPrinter {
            config: PrettyConfig::default(),
        }
    }

    /// Create with config
    pub fn with_config(config: PrettyConfig) -> Self {
        PrettyPrinter { config }
    }

    /// Pretty print JSON-like object
    pub fn json_object(&self, fields: &Vec<(String, String)>) -> String {
        if fields.is_empty() {
            return String::from("{}");
        }

        let mut result = String::from("{\n");
        for (i, (key, value)) in fields.iter().enumerate() {
            result.push_str(self.config.indent.as_str());
            result.push_str("\"");
            result.push_str(key.as_str());
            result.push_str("\": ");
            result.push_str(value.as_str());
            if i < fields.len() - 1 {
                result.push_str(",");
            }
            result.push_str("\n");
        }
        result.push_str("}");
        result
    }

    /// Pretty print array
    pub fn array(&self, items: &Vec<String>) -> String {
        if items.is_empty() {
            return String::from("[]");
        }

        let mut result = String::from("[\n");
        for (i, item) in items.iter().enumerate() {
            result.push_str(self.config.indent.as_str());
            result.push_str(item.as_str());
            if i < items.len() - 1 {
                result.push_str(",");
            }
            result.push_str("\n");
        }
        result.push_str("]");
        result
    }

    /// Pretty print code with basic indentation
    pub fn code_block(&self, lines: &Vec<String>) -> String {
        let mut result = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                result.push_str("\n");
            }
            result.push_str(line.as_str());
        }
        result
    }

    /// Pretty print error message
    pub fn error_message(&self, title: &str, message: &str, context: Option<&str>) -> String {
        let mut result = String::from("Error: ");
        result.push_str(title);
        result.push_str("\n");
        result.push_str(message);
        
        if let Some(ctx) = context {
            result.push_str("\n\nContext:\n");
            result.push_str(self.config.indent.as_str());
            result.push_str(ctx);
        }
        result
    }

    /// Format diagnostic message with location
    pub fn diagnostic(&self, level: &str, message: &str, file: &str, line: usize, col: usize) -> String {
        let mut result = String::new();
        result.push_str(level);
        result.push_str(": ");
        result.push_str(message);
        result.push_str("\n");
        result.push_str("  --> ");
        result.push_str(file);
        result.push_str(":");
        
        // Convert numbers to string manually since we don't have format!
        let line_str = line.to_string();
        result.push_str(&line_str);
        result.push_str(":");
        
        let col_str = col.to_string();
        result.push_str(&col_str);
        
        result
    }
}

impl Default for PrettyPrinter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;