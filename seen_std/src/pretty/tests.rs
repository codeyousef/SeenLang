//! Comprehensive tests for pretty printing utilities
//!
//! Tests cover all critical pretty printing functionality needed for compiler output

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pretty_config_default() {
        let config = PrettyConfig::default();
        assert_eq!(config.indent.as_str(), "  ");
        assert_eq!(config.max_width, 80);
        assert!(!config.compact);
        assert!(!config.show_types);
        assert!(!config.use_colors);
    }

    #[test]
    fn test_doc_text_creation() {
        let doc = Doc::text("hello world");
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "hello world");
    }

    #[test]
    fn test_doc_line_break() {
        let mut docs = Vec::new();
        docs.push(Doc::text("first line"));
        docs.push(Doc::line());
        docs.push(Doc::text("second line"));
        let doc = Doc::concat(docs);
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "first line\n  second line");
    }

    #[test]
    fn test_doc_nesting() {
        let mut inner_docs = Vec::new();
        inner_docs.push(Doc::text("nested"));
        inner_docs.push(Doc::line());
        inner_docs.push(Doc::text("content"));
        let inner = Doc::concat(inner_docs);
        
        let mut docs = Vec::new();
        docs.push(Doc::text("outer"));
        docs.push(Doc::line());
        docs.push(Doc::nest(2, inner));
        let doc = Doc::concat(docs);

        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "outer\n      nested\n      content");
    }

    #[test]
    fn test_doc_grouping_fits_on_line() {
        let mut docs = Vec::new();
        docs.push(Doc::text("short"));
        docs.push(Doc::line());
        docs.push(Doc::text("text"));
        let doc = Doc::group(Doc::concat(docs));

        let mut config = PrettyConfig::default();
        config.max_width = 20;
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        // Should flatten to single line since it fits
        assert_eq!(result.as_str(), "short text");
    }

    #[test]
    fn test_doc_grouping_too_wide() {
        let mut docs = Vec::new();
        docs.push(Doc::text("this is a very long line that exceeds width"));
        docs.push(Doc::line());
        docs.push(Doc::text("continuation"));
        let doc = Doc::group(Doc::concat(docs));

        let mut config = PrettyConfig::default();
        config.max_width = 20;
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        // Should use line breaks since it doesn't fit
        assert!(result.contains('\n'));
        assert!(result.contains("  continuation"));
    }

    #[test]
    fn test_doc_alternatives() {
        let mut expanded_docs = Vec::new();
        expanded_docs.push(Doc::text("expanded"));
        expanded_docs.push(Doc::line());
        expanded_docs.push(Doc::text("version"));
        let expanded = Doc::concat(expanded_docs);
        
        let doc = Doc::alt(
            Doc::text("compact"),
            expanded,
        );

        // Test with wide limit - should use primary (compact)
        let mut config = PrettyConfig::default();
        config.max_width = 50;
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "compact");

        // Test with narrow limit - should use fallback (expanded)
        config.max_width = 5;
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        assert!(result.contains("expanded"));
        assert!(result.contains("version"));
    }

    #[test]
    fn test_color_rendering_enabled() {
        let doc = Doc::color(Color::Red, Doc::text("error text"));
        
        let mut config = PrettyConfig::default();
        config.use_colors = true;
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        assert!(result.contains("\x1b[31m")); // Red color code
        assert!(result.contains("error text"));
        assert!(result.contains("\x1b[0m"));  // Reset code
    }

    #[test]
    fn test_color_rendering_disabled() {
        let doc = Doc::color(Color::Red, Doc::text("error text"));
        
        let mut config = PrettyConfig::default();
        config.use_colors = false;
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        assert_eq!(result.as_str(), "error text");
        assert!(!result.contains("\x1b["));
    }

    #[test]
    fn test_doc_join_empty() {
        let docs = Vec::new();
        let result = Doc::join(docs, Doc::text(", "));
        
        let printer = PrettyPrinter::new();
        let output = printer.render(&result);
        assert_eq!(output.as_str(), "");
    }

    #[test]
    fn test_doc_join_multiple() {
        let mut docs = Vec::new();
        docs.push(Doc::text("first"));
        docs.push(Doc::text("second"));
        docs.push(Doc::text("third"));
        let result = Doc::join(docs, Doc::text(", "));
        
        let printer = PrettyPrinter::new();
        let output = printer.render(&result);
        assert_eq!(output.as_str(), "first, second, third");
    }

    #[test]
    fn test_doc_brackets() {
        let doc = Doc::brackets(Doc::text("content"));
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "[content]");
    }

    #[test]
    fn test_doc_braces() {
        let doc = Doc::braces(Doc::text("content"));
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "{content}");
    }

    #[test]
    fn test_doc_parens() {
        let doc = Doc::parens(Doc::text("content"));
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "(content)");
    }

    #[test]
    fn test_json_object_empty() {
        let fields = Vec::new();
        let doc = pretty::json_object(&fields);
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "{}");
    }

    #[test]
    fn test_json_object_single_field() {
        let mut fields = Vec::new();
        fields.push((String::from("name"), String::from("\"Seen\"")));
        let doc = pretty::json_object(&fields);
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert!(result.contains("\"name\": \"Seen\""));
    }

    #[test]
    fn test_json_object_multiple_fields_compact() {
        let mut fields = Vec::new();
        fields.push((String::from("name"), String::from("\"Seen\"")));
        fields.push((String::from("version"), String::from("\"1.0\"")));
        let doc = pretty::json_object(&fields);
        
        let mut config = PrettyConfig::default();
        config.max_width = 100; // Wide enough for compact layout
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        // Should be compact format
        assert!(result.contains("\"name\": \"Seen\", \"version\": \"1.0\""));
    }

    #[test]
    fn test_json_object_multiple_fields_expanded() {
        let fields = vec![
            (String::from("name"), String::from("\"SeenLanguageWithVeryLongName\"")),
            (String::from("version"), String::from("\"1.0.0-alpha\"")),
            (String::from("description"), String::from("\"A very long description\"")),
        ];
        let doc = pretty::json_object(&fields);
        
        let mut config = PrettyConfig::default();
        config.max_width = 30; // Narrow enough to force expansion
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        // Should be expanded format with line breaks
        assert!(result.contains('\n'));
        assert!(result.contains("\"name\""));
        assert!(result.contains("\"version\""));
    }

    #[test]
    fn test_array_empty() {
        let items = vec![];
        let doc = pretty::array(&items);
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "[]");
    }

    #[test]
    fn test_array_single_item() {
        let items = vec![String::from("item1")];
        let doc = pretty::array(&items);
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        assert_eq!(result.as_str(), "[item1]");
    }

    #[test]
    fn test_array_multiple_items_compact() {
        let items = vec![
            String::from("item1"),
            String::from("item2"),
            String::from("item3"),
        ];
        let doc = pretty::array(&items);
        
        let mut config = PrettyConfig::default();
        config.max_width = 50;
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        assert_eq!(result.as_str(), "[item1, item2, item3]");
    }

    #[test]
    fn test_array_multiple_items_expanded() {
        let items = vec![
            String::from("very_long_item_name_1"),
            String::from("very_long_item_name_2"),
            String::from("very_long_item_name_3"),
        ];
        let doc = pretty::array(&items);
        
        let mut config = PrettyConfig::default();
        config.max_width = 20; // Force expansion
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        // Should be expanded format
        assert!(result.contains('\n'));
        assert!(result.contains("very_long_item_name_1"));
    }

    #[test]
    fn test_code_block_basic() {
        let lines = vec![
            String::from("fn main() {"),
            String::from("    println!(\"Hello\");"),
            String::from("}"),
        ];
        let doc = pretty::code_block(&lines, Some("rust"));
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        
        assert!(result.contains("// rust"));
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
    }

    #[test]
    fn test_code_block_no_language() {
        let lines = vec![
            String::from("line 1"),
            String::from("line 2"),
        ];
        let doc = pretty::code_block(&lines, None);
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        
        assert!(!result.contains("//"));
        assert!(result.contains("line 1"));
        assert!(result.contains("line 2"));
    }

    #[test]
    fn test_error_message_basic() {
        let doc = pretty::error_message(
            "Type mismatch",
            "Expected I32, found String",
            None,
        );
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        
        assert!(result.contains("Error: Type mismatch"));
        assert!(result.contains("Expected I32, found String"));
    }

    #[test]
    fn test_error_message_with_context() {
        let doc = pretty::error_message(
            "Parse error",
            "Unexpected token '}'",
            Some("In function 'calculate' at line 42"),
        );
        
        let printer = PrettyPrinter::new();
        let result = printer.render(&doc);
        
        assert!(result.contains("Error: Parse error"));
        assert!(result.contains("Unexpected token"));
        assert!(result.contains("Context:"));
        assert!(result.contains("function 'calculate'"));
    }

    #[test]
    fn test_error_message_with_colors() {
        let doc = pretty::error_message(
            "Fatal error",
            "Compilation failed",
            None,
        );
        
        let mut config = PrettyConfig::default();
        config.use_colors = true;
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);
        
        assert!(result.contains("\x1b[31m")); // Red color
        assert!(result.contains("\x1b[1m"));  // Bold
        assert!(result.contains("Fatal error"));
    }

    #[test]
    fn test_pretty_printer_fits() {
        let doc = Doc::text("short text");
        let printer = PrettyPrinter::new();
        
        assert!(printer.fits(&doc, 20));
        assert!(!printer.fits(&doc, 5));
    }

    #[test]
    fn test_pretty_printer_render_with_width() {
        let doc = Doc::group(Doc::concat(vec![
            Doc::text("word1"),
            Doc::line(),
            Doc::text("word2"),
        ]));
        
        let printer = PrettyPrinter::new();
        
        // Wide width - should be compact
        let result1 = printer.render_with_width(&doc, 50);
        assert_eq!(result1.as_str(), "word1 word2");
        
        // Narrow width - should use line breaks
        let result2 = printer.render_with_width(&doc, 5);
        assert!(result2.contains('\n'));
    }

    #[test]
    fn test_compiler_output_formatting() {
        // Test realistic compiler output scenario
        let diagnostic = Doc::concat(vec![
            Doc::color(Color::Red, Doc::text("error")),
            Doc::text(": "),
            Doc::text("type mismatch"),
            Doc::line(),
            Doc::text("  --> src/main.seen:10:5"),
            Doc::line(),
            Doc::text("   |"),
            Doc::line(),
            Doc::text("10 | let x: I32 = \"hello\";"),
            Doc::line(),
            Doc::text("   |               ^^^^^^^ expected I32, found String"),
        ]);

        let mut config = PrettyConfig::default();
        config.use_colors = false; // For predictable testing
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&diagnostic);

        assert!(result.contains("error: type mismatch"));
        assert!(result.contains("src/main.seen:10:5"));
        assert!(result.contains("expected I32, found String"));
    }

    #[test]
    fn test_ast_pretty_printing() {
        // Test AST-like structure formatting
        let ast_node = Doc::concat(vec![
            Doc::text("FunctionDecl {"),
            Doc::line(),
            Doc::nest(1, Doc::concat(vec![
                Doc::text("name: \"main\","),
                Doc::line(),
                Doc::text("params: [],"),
                Doc::line(),
                Doc::text("body: Block {"),
                Doc::line(),
                Doc::nest(1, Doc::concat(vec![
                    Doc::text("statements: ["),
                    Doc::line(),
                    Doc::nest(1, Doc::text("ExprStmt(Call(\"println\", [\"Hello\"]))")),
                    Doc::line(),
                    Doc::text("]"),
                ])),
                Doc::line(),
                Doc::text("}"),
            ])),
            Doc::line(),
            Doc::text("}"),
        ]);

        let printer = PrettyPrinter::new();
        let result = printer.render(&ast_node);

        assert!(result.contains("FunctionDecl {"));
        assert!(result.contains("name: \"main\""));
        assert!(result.contains("ExprStmt(Call"));
        // Check proper indentation
        assert!(result.contains("  name:"));
        assert!(result.contains("    statements:"));
    }

    #[test]
    fn test_custom_indentation() {
        let doc = Doc::concat(vec![
            Doc::text("level0"),
            Doc::line(),
            Doc::nest(1, Doc::concat(vec![
                Doc::text("level1"),
                Doc::line(),
                Doc::nest(1, Doc::text("level2")),
            ])),
        ]);

        // Test with tab indentation
        let mut config = PrettyConfig::default();
        config.indent = String::from("\t");
        let printer = PrettyPrinter::with_config(config);
        let result = printer.render(&doc);

        assert!(result.contains("\tlevel1"));
        assert!(result.contains("\t\tlevel2"));
    }

    #[test]
    fn test_performance_large_document() {
        // Create a large document to test performance
        let mut items = Vec::new();
        for i in 0..1000 {
            items.push(format!("item_{}", i));
        }
        
        let doc = pretty::array(&items);
        let printer = PrettyPrinter::new();
        
        let start = std::time::Instant::now();
        let result = printer.render(&doc);
        let duration = start.elapsed();
        
        assert!(result.contains("item_0"));
        assert!(result.contains("item_999"));
        assert!(duration.as_millis() < 100); // Should complete quickly
    }
}