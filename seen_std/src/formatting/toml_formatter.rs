//! TOML document formatter
//!
//! Formats TOML configuration files while preserving comments and structure

use super::{DocumentFormatter, FormatConfig, FormatResult, FileType};
use crate::string::String;
use crate::collections::Vec;

/// Formatter for TOML configuration files
pub struct TomlFormatter {}

impl TomlFormatter {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Format TOML content while preserving comments and logical structure
    fn format_toml_content(&self, content: &str, config: &FormatConfig) -> FormatResult {
        let lines: Vec<&str> = content.lines().collect();
        let mut formatted_lines: Vec<String> = Vec::new();
        let mut in_array = false;
        let mut _array_depth = 0;
        
        for (_i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            // Preserve empty lines based on config
            if trimmed.is_empty() {
                if config.preserve_blank_lines {
                    // Don't add multiple consecutive blank lines
                    if formatted_lines.is_empty() || 
                       !formatted_lines.last().unwrap().is_empty() {
                        formatted_lines.push(String::from(""));
                    }
                }
                continue;
            }
            
            // Preserve comments
            if trimmed.starts_with('#') {
                if config.preserve_comments {
                    formatted_lines.push(String::from(trimmed));
                }
                continue;
            }
            
            // Section headers [section]
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                // Add blank line before section headers (except at start)
                if !formatted_lines.is_empty() && !formatted_lines.last().unwrap().is_empty() {
                    formatted_lines.push(String::from(""));
                }
                formatted_lines.push(String::from(trimmed));
                continue;
            }
            
            // Key-value pairs
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();
                
                // Handle arrays
                if value.starts_with('[') {
                    if value.ends_with(']') {
                        // Single-line array
                        let formatted_array = self.format_array_value(value, config);
                        formatted_lines.push(String::from(&format!("{} = {}", key, formatted_array)));
                    } else {
                        // Multi-line array start
                        in_array = true;
                        _array_depth = 1;
                        formatted_lines.push(String::from(&format!("{} = [", key)));
                    }
                } else {
                    // Regular key-value pair
                    let formatted_value = self.format_value(value, config);
                    formatted_lines.push(String::from(&format!("{} = {}", key, formatted_value)));
                }
                continue;
            }
            
            // Handle multi-line array elements
            if in_array {
                let indent = " ".repeat(config.indent_size as usize);
                
                if trimmed.ends_with(']') {
                    // End of array
                    let element = trimmed.trim_end_matches(']').trim();
                    if !element.is_empty() {
                        let formatted_element = self.format_array_element(element, config);
                        formatted_lines.push(String::from(&format!("{}{},", indent, formatted_element)));
                    }
                    formatted_lines.push(String::from("]"));
                    in_array = false;
                    _array_depth = 0;
                } else {
                    // Array element
                    let formatted_element = self.format_array_element(trimmed, config);
                    let trailing_comma = if config.trailing_comma { "," } else { "" };
                    formatted_lines.push(String::from(&format!("{}{}{}", indent, formatted_element, trailing_comma)));
                }
                continue;
            }
            
            // Fallback - preserve the line as-is
            formatted_lines.push(String::from(trimmed));
        }
        
        let formatted_content = formatted_lines.join("\n");
        
        // Ensure file ends with newline
        let final_content = if !formatted_content.ends_with('\n') && !formatted_content.is_empty() {
            String::from(&format!("{}\n", formatted_content))
        } else {
            String::from(&formatted_content)
        };
        
        FormatResult::new(String::from(content), final_content)
    }
    
    fn format_value(&self, value: &str, _config: &FormatConfig) -> String {
        let trimmed = value.trim();
        
        // Handle strings - preserve quotes
        if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
           (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
            String::from(trimmed)
        } else {
            // Numbers, booleans, etc.
            String::from(trimmed)
        }
    }
    
    fn format_array_value(&self, value: &str, config: &FormatConfig) -> String {
        // Simple array formatting for single-line arrays
        let content = value.trim_start_matches('[').trim_end_matches(']');
        let elements: Vec<&str> = content.split(',').collect();
        
        if elements.len() <= 3 && value.len() <= config.max_line_length as usize {
            // Keep short arrays on one line
            let formatted_elements: Vec<String> = elements
                .iter()
                .map(|e| String::from(e.trim()))
                .collect();
            String::from(&format!("[{}]", formatted_elements.join(", ")))
        } else {
            // Long arrays should be multi-line (handled elsewhere)
            String::from(value)
        }
    }
    
    fn format_array_element(&self, element: &str, config: &FormatConfig) -> String {
        self.format_value(element, config)
    }
}

impl DocumentFormatter for TomlFormatter {
    fn format(&self, content: &str, config: &FormatConfig) -> FormatResult {
        if !config.format_toml {
            return FormatResult::new(String::from(content), String::from(content));
        }
        
        self.format_toml_content(content, config)
    }
    
    fn supports_file_type(&self, file_type: &FileType) -> bool {
        matches!(file_type, FileType::Toml)
    }
    
    fn name(&self) -> &'static str {
        "toml-formatter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_section_spacing() {
        let formatter = TomlFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"name = "test"
[section]
key = "value""#;
        
        let expected = r#"name = "test"

[section]
key = "value"
"#;
        
        let result = formatter.format(input, &config);
        assert!(result.changed);
        assert_eq!(result.formatted_content, expected);
    }
    
    #[test]
    fn test_comment_preservation() {
        let formatter = TomlFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"# Configuration file
name = "test"  # inline comment
# Another comment
key = "value""#;
        
        let result = formatter.format(input, &config);
        assert!(result.formatted_content.contains("# Configuration file"));
        assert!(result.formatted_content.contains("# Another comment"));
    }
    
    #[test]
    fn test_array_formatting() {
        let formatter = TomlFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"short = ["a","b","c"]
long = [
"item1",
"item2"
]"#;
        
        let result = formatter.format(input, &config);
        // Short arrays should be formatted with spaces
        assert!(result.formatted_content.contains("short = [\"a\", \"b\", \"c\"]"));
        // Multi-line arrays should be preserved with proper indentation
        assert!(result.formatted_content.contains("    \"item1\","));
    }
    
    #[test]
    fn test_key_value_formatting() {
        let formatter = TomlFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"name="test"
number =  42
boolean=true"#;
        
        let result = formatter.format(input, &config);
        // Should normalize spacing around equals
        assert!(result.formatted_content.contains("name = \"test\""));
        assert!(result.formatted_content.contains("number = 42"));
        assert!(result.formatted_content.contains("boolean = true"));
    }
    
    #[test]
    fn test_no_formatting_when_disabled() {
        let formatter = TomlFormatter::new();
        let mut config = FormatConfig::default();
        config.format_toml = false;
        
        let input = "name=\"test\"\nkey=\"value\"";
        let result = formatter.format(input, &config);
        
        assert!(!result.changed);
        assert_eq!(result.formatted_content, input);
    }
    
    #[test]
    fn test_supports_toml_files() {
        let formatter = TomlFormatter::new();
        assert!(formatter.supports_file_type(&FileType::Toml));
        assert!(!formatter.supports_file_type(&FileType::Seen));
        assert!(!formatter.supports_file_type(&FileType::Markdown));
    }
}