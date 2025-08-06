//! Seen language code formatter
//!
//! Formats Seen source code according to style guidelines

use super::{DocumentFormatter, FormatConfig, FormatResult, FileType};
use crate::string::String;
use crate::collections::Vec;

/// Formatter for Seen language source code
pub struct SeenFormatter {
    // Future: Add AST-based formatting capabilities
}

impl SeenFormatter {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Format Seen code with basic rules (syntax-aware formatting will come later)
    fn format_seen_basic(&self, content: &str, config: &FormatConfig) -> FormatResult {
        let lines: Vec<&str> = content.lines().collect();
        let mut formatted_lines: Vec<String> = Vec::new();
        let mut indent_level = 0u32;
        let _in_string = false;
        let _in_comment = false;
        
        for line in lines {
            let trimmed = line.trim();
            
            // Skip empty lines or preserve them based on config
            if trimmed.is_empty() {
                if config.preserve_blank_lines {
                    formatted_lines.push(String::from(""));
                }
                continue;
            }
            
            // Handle comments
            if trimmed.starts_with("//") {
                if config.preserve_comments {
                    let indent = " ".repeat((indent_level * config.indent_size) as usize);
                    formatted_lines.push(String::from(&format!("{}{}", indent, trimmed)));
                }
                continue;
            }
            
            // Adjust indent level based on braces
            let _opening_braces = trimmed.matches('{').count() as u32;
            let _closing_braces = trimmed.matches('}').count() as u32;
            
            // Decrease indent for closing braces at start of line
            if trimmed.starts_with('}') {
                indent_level = indent_level.saturating_sub(1);
            }
            
            // Apply indentation
            let indent = " ".repeat((indent_level * config.indent_size) as usize);
            let formatted_line = String::from(&format!("{}{}", indent, trimmed));
            
            // Check line length
            if formatted_line.len() > config.max_line_length as usize {
                // For now, just preserve long lines (future: smart line breaking)
            }
            
            formatted_lines.push(formatted_line);
            
            // Increase indent for opening braces
            if trimmed.ends_with('{') {
                indent_level += 1;
            }
            
            // Handle other indentation cases
            if trimmed.starts_with("if ") || trimmed.starts_with("while ") || 
               trimmed.starts_with("for ") || trimmed.starts_with("match ") {
                // These will be handled by brace counting
            }
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
}

impl DocumentFormatter for SeenFormatter {
    fn format(&self, content: &str, config: &FormatConfig) -> FormatResult {
        if !config.format_seen {
            return FormatResult::new(String::from(content), String::from(content));
        }
        
        self.format_seen_basic(content, config)
    }
    
    fn supports_file_type(&self, file_type: &FileType) -> bool {
        matches!(file_type, FileType::Seen)
    }
    
    fn name(&self) -> &'static str {
        "seen-formatter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_indentation() {
        let formatter = SeenFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"func main() {
println("Hello");
if true {
println("World");
}
}"#;
        
        let expected = r#"func main() {
    println("Hello");
    if true {
        println("World");
    }
}
"#;
        
        let result = formatter.format(input, &config);
        assert!(result.changed);
        assert_eq!(result.formatted_content, expected);
    }
    
    #[test]
    fn test_comment_preservation() {
        let formatter = SeenFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"// This is a comment
func main() {
// Another comment
println("Hello");
}"#;
        
        let result = formatter.format(input, &config);
        assert!(result.formatted_content.contains("// This is a comment"));
        assert!(result.formatted_content.contains("// Another comment"));
    }
    
    #[test]
    fn test_blank_line_preservation() {
        let formatter = SeenFormatter::new();
        let mut config = FormatConfig::default();
        config.preserve_blank_lines = true;
        
        let input = r#"func main() {

    println("Hello");

}
"#;
        
        let result = formatter.format(input, &config);
        // Should preserve blank lines
        assert!(result.formatted_content.contains("\n\n"));
    }
    
    #[test]
    fn test_no_formatting_when_disabled() {
        let formatter = SeenFormatter::new();
        let mut config = FormatConfig::default();
        config.format_seen = false;
        
        let input = "func main(){println(\"Hello\");}";
        
        let result = formatter.format(input, &config);
        assert!(!result.changed);
        assert_eq!(result.formatted_content, input);
    }
    
    #[test]
    fn test_supports_seen_files() {
        let formatter = SeenFormatter::new();
        assert!(formatter.supports_file_type(&FileType::Seen));
        assert!(!formatter.supports_file_type(&FileType::Markdown));
        assert!(!formatter.supports_file_type(&FileType::Toml));
    }
}