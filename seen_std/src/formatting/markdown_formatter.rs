//! Markdown document formatter
//!
//! Formats Markdown files for consistent documentation

use super::{DocumentFormatter, FormatConfig, FormatResult, FileType};
use crate::string::String;
use crate::collections::Vec;

/// Formatter for Markdown documents
pub struct MarkdownFormatter {}

impl MarkdownFormatter {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Format Markdown content according to common conventions
    fn format_markdown_content(&self, content: &str, config: &FormatConfig) -> FormatResult {
        let lines: Vec<&str> = content.lines().collect();
        let mut formatted_lines: Vec<String> = Vec::new();
        let mut in_code_block = false;
        let _list_depth = 0;
        
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            // Handle code blocks - preserve formatting inside them
            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                formatted_lines.push(String::from(trimmed));
                continue;
            }
            
            if in_code_block {
                // Preserve code block formatting
                formatted_lines.push(String::from(line));
                continue;
            }
            
            // Empty lines
            if trimmed.is_empty() {
                // Preserve single blank lines, collapse multiple
                if config.preserve_blank_lines {
                    if i == 0 || !lines[i-1].trim().is_empty() {
                        formatted_lines.push(String::from(""));
                    }
                }
                continue;
            }
            
            // Headers - ensure proper spacing
            if trimmed.starts_with('#') {
                // Add blank line before headers (except at start)
                if !formatted_lines.is_empty() && !formatted_lines.last().unwrap().is_empty() {
                    formatted_lines.push(String::from(""));
                }
                formatted_lines.push(String::from(trimmed));
                continue;
            }
            
            // Lists - format indentation
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") || 
               trimmed.starts_with("+ ") || self.is_numbered_list(trimmed) {
                let indent_level = (line.len() - line.trim_start().len()) / 2; // Assume 2-space indent
                let indent = "  ".repeat(indent_level);
                formatted_lines.push(String::from(&format!("{}{}", indent, trimmed)));
                continue;
            }
            
            // Tables - ensure consistent spacing (basic formatting)
            if trimmed.contains('|') && self.is_table_row(trimmed) {
                let formatted_table_row = self.format_table_row(trimmed);
                formatted_lines.push(formatted_table_row);
                continue;
            }
            
            // Regular paragraphs - respect max line length
            if trimmed.len() > config.max_line_length as usize {
                // For now, preserve long lines (future: smart wrapping)
                formatted_lines.push(String::from(trimmed));
            } else {
                formatted_lines.push(String::from(trimmed));
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
    
    fn is_numbered_list(&self, line: &str) -> bool {
        // Check for patterns like "1. ", "10. ", etc.
        if let Some(pos) = line.find(". ") {
            let prefix = &line[..pos];
            prefix.chars().all(|c| c.is_ascii_digit())
        } else {
            false
        }
    }
    
    fn is_table_row(&self, line: &str) -> bool {
        // Basic table detection - starts and ends with |
        line.trim().starts_with('|') && line.trim().ends_with('|')
    }
    
    fn format_table_row(&self, line: &str) -> String {
        let cells: Vec<&str> = line.split('|').collect();
        let formatted_cells: Vec<String> = cells
            .iter()
            .map(|cell| String::from(&format!(" {} ", cell.trim())))
            .collect();
        
        String::from(&formatted_cells.join("|"))
    }
}

impl DocumentFormatter for MarkdownFormatter {
    fn format(&self, content: &str, config: &FormatConfig) -> FormatResult {
        if !config.format_markdown {
            return FormatResult::new(String::from(content), String::from(content));
        }
        
        self.format_markdown_content(content, config)
    }
    
    fn supports_file_type(&self, file_type: &FileType) -> bool {
        matches!(file_type, FileType::Markdown)
    }
    
    fn name(&self) -> &'static str {
        "markdown-formatter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_header_spacing() {
        let formatter = MarkdownFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"# Title
Some text
## Subtitle"#;
        
        let expected = r#"# Title
Some text

## Subtitle
"#;
        
        let result = formatter.format(input, &config);
        assert!(result.changed);
        assert_eq!(result.formatted_content, expected);
    }
    
    #[test]
    fn test_code_block_preservation() {
        let formatter = MarkdownFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"Some text
```rust
  let x = 1;
    let y = 2;
```
More text"#;
        
        let result = formatter.format(input, &config);
        // Code block formatting should be preserved
        assert!(result.formatted_content.contains("  let x = 1;"));
        assert!(result.formatted_content.contains("    let y = 2;"));
    }
    
    #[test]
    fn test_list_formatting() {
        let formatter = MarkdownFormatter::new();
        let config = FormatConfig::default();
        
        let input = r#"- Item 1
  - Nested item
- Item 2"#;
        
        let result = formatter.format(input, &config);
        // Should preserve list structure
        assert!(result.formatted_content.contains("- Item 1"));
        assert!(result.formatted_content.contains("  - Nested item"));
    }
    
    #[test]
    fn test_table_formatting() {
        let formatter = MarkdownFormatter::new();
        let config = FormatConfig::default();
        
        let input = "|Name|Age|City|";
        let result = formatter.format(input, &config);
        
        // Should format table with proper spacing
        assert!(result.formatted_content.contains("| Name | Age | City |"));
    }
    
    #[test]
    fn test_numbered_list_detection() {
        let formatter = MarkdownFormatter::new();
        
        assert!(formatter.is_numbered_list("1. First item"));
        assert!(formatter.is_numbered_list("10. Tenth item"));
        assert!(!formatter.is_numbered_list("- Bullet item"));
        assert!(!formatter.is_numbered_list("Not a list"));
    }
    
    #[test]
    fn test_no_formatting_when_disabled() {
        let formatter = MarkdownFormatter::new();
        let mut config = FormatConfig::default();
        config.format_markdown = false;
        
        let input = "# Title\nSome text\n## Subtitle";
        let result = formatter.format(input, &config);
        
        assert!(!result.changed);
        assert_eq!(result.formatted_content, input);
    }
}