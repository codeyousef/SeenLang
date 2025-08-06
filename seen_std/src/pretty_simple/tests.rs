//! Tests for simple pretty printer
//!
//! Focused tests for essential pretty printing functionality

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pretty_config_default() {
        let config = PrettyConfig::default();
        assert_eq!(config.indent.as_str(), "  ");
        assert_eq!(config.max_width, 80);
        assert!(!config.use_colors);
    }

    #[test] 
    fn test_pretty_printer_creation() {
        let printer = PrettyPrinter::new();
        assert_eq!(printer.config.indent.as_str(), "  ");
        
        let mut config = PrettyConfig::default();
        config.indent = String::from("    ");
        let printer2 = PrettyPrinter::with_config(config);
        assert_eq!(printer2.config.indent.as_str(), "    ");
    }

    #[test]
    fn test_json_object_empty() {
        let printer = PrettyPrinter::new();
        let fields = Vec::new();
        let result = printer.json_object(&fields);
        assert_eq!(result.as_str(), "{}");
    }

    #[test] 
    fn test_json_object_single_field() {
        let printer = PrettyPrinter::new();
        let mut fields = Vec::new();
        fields.push((String::from("name"), String::from("\"Seen\"")));
        let result = printer.json_object(&fields);
        
        assert!(result.contains("\"name\": \"Seen\""));
        assert!(result.contains("{"));
        assert!(result.contains("}"));
    }

    #[test]
    fn test_json_object_multiple_fields() {
        let printer = PrettyPrinter::new();
        let mut fields = Vec::new();
        fields.push((String::from("name"), String::from("\"Seen\"")));
        fields.push((String::from("version"), String::from("\"1.0\"")));
        let result = printer.json_object(&fields);
        
        assert!(result.contains("\"name\": \"Seen\""));
        assert!(result.contains("\"version\": \"1.0\""));
        assert!(result.contains(","));
    }

    #[test]
    fn test_array_empty() {
        let printer = PrettyPrinter::new();
        let items = Vec::new();
        let result = printer.array(&items);
        assert_eq!(result.as_str(), "[]");
    }

    #[test]
    fn test_array_with_items() {
        let printer = PrettyPrinter::new();
        let mut items = Vec::new();
        items.push(String::from("item1"));
        items.push(String::from("item2"));
        items.push(String::from("item3"));
        let result = printer.array(&items);
        
        assert!(result.contains("["));
        assert!(result.contains("]"));
        assert!(result.contains("item1"));
        assert!(result.contains("item2"));
        assert!(result.contains("item3"));
    }

    #[test]
    fn test_code_block() {
        let printer = PrettyPrinter::new();
        let mut lines = Vec::new();
        lines.push(String::from("fn main() {"));
        lines.push(String::from("    println!(\"Hello\");"));
        lines.push(String::from("}"));
        let result = printer.code_block(&lines);
        
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
    }

    #[test] 
    fn test_error_message_basic() {
        let printer = PrettyPrinter::new();
        let result = printer.error_message(
            "Type mismatch",
            "Expected I32, found String",
            None
        );
        
        assert!(result.contains("Error: Type mismatch"));
        assert!(result.contains("Expected I32, found String"));
    }

    #[test]
    fn test_error_message_with_context() {
        let printer = PrettyPrinter::new();
        let result = printer.error_message(
            "Parse error", 
            "Unexpected token '}'",
            Some("In function 'calculate' at line 42")
        );
        
        assert!(result.contains("Error: Parse error"));
        assert!(result.contains("Unexpected token"));
        assert!(result.contains("Context:"));
        assert!(result.contains("function 'calculate'"));
    }

    #[test]
    fn test_diagnostic_formatting() {
        let printer = PrettyPrinter::new();
        let result = printer.diagnostic(
            "error",
            "undefined variable 'x'",
            "src/main.seen",
            42,
            15
        );
        
        assert!(result.contains("error: undefined variable"));
        assert!(result.contains("src/main.seen:42:15"));
        assert!(result.contains("-->"));
    }

    #[test]
    fn test_compiler_diagnostic() {
        let printer = PrettyPrinter::new();
        
        // Test realistic compiler diagnostic
        let result = printer.diagnostic(
            "error",
            "type mismatch: expected I32, found String",
            "examples/hello.seen", 
            10,
            5
        );
        
        assert!(result.contains("error: type mismatch"));
        assert!(result.contains("examples/hello.seen:10:5"));
    }

    #[test]
    fn test_custom_indentation() {
        let mut config = PrettyConfig::default();
        config.indent = String::from("\t");
        let printer = PrettyPrinter::with_config(config);
        
        let mut fields = Vec::new();
        fields.push((String::from("key"), String::from("\"value\"")));
        let result = printer.json_object(&fields);
        
        assert!(result.contains("\t\"key\""));
    }

    #[test]
    fn test_performance_large_object() {
        let printer = PrettyPrinter::new();
        let mut fields = Vec::new();
        
        for i in 0..1000 {
            let key = String::from("key");
            let value = String::from("\"value\"");
            fields.push((key, value));
        }
        
        let start = std::time::Instant::now();
        let result = printer.json_object(&fields);
        let duration = start.elapsed();
        
        assert!(result.contains("key"));
        assert!(duration.as_millis() < 50); // Should be fast
    }

    #[test]
    fn test_warning_diagnostic() {
        let printer = PrettyPrinter::new();
        let result = printer.diagnostic(
            "warning",
            "unused variable 'x'",
            "src/lib.seen",
            25,
            9
        );
        
        assert!(result.contains("warning: unused variable"));
        assert!(result.contains("src/lib.seen:25:9"));
    }

    #[test]
    fn test_nested_structures() {
        let printer = PrettyPrinter::new();
        
        // Test nested array within object representation
        let mut outer_fields = Vec::new();
        outer_fields.push((String::from("items"), String::from("[1, 2, 3]")));
        let result = printer.json_object(&outer_fields);
        
        assert!(result.contains("\"items\": [1, 2, 3]"));
    }
}