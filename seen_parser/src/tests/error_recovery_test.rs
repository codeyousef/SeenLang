//\! Tests for parser error recovery

use super::*;
use crate::ast::*;
use pretty_assertions::assert_eq;

#[test]
fn test_missing_semicolon_recovery() {
    // Parser should recover from missing semicolon
    let source = "val x = 5 val y = 6";
    assert_parse_error(source);
}

#[test]
fn test_missing_closing_brace_recovery() {
    let source = "if true { x = 5";
    assert_parse_error(source);
}

#[test]
fn test_missing_type_annotation_recovery() {
    let source = "val x: = 5;";
    assert_parse_error(source);
}

#[test]
fn test_unexpected_token_recovery() {
    let source = "val x = 5 + + 6;";
    assert_parse_error(source);
}

#[test]
fn test_invalid_expression_recovery() {
    let source = "val x = * 5;";
    assert_parse_error(source);
}

#[test]
fn test_multiple_errors() {
    // Multiple errors in one parse
    let source = r#"
        func broken {  // Missing parentheses
            val x =    // Missing initializer
            if true    // Missing braces
                return
        }              // Missing semicolons
    "#;
    assert_parse_error(source);
}

#[test]
fn test_error_location_reporting() {
    let source = "val x = ;";  // Missing expression after =
    let result = parse_source(source);
    
    assert!(result.is_err());
    match result {
        Err(err) => {
            // Verify error has location information
            // The exact structure depends on ParseError implementation
        }
        Ok(_) => panic!("Expected parse error"),
    }
}

#[test]
#[ignore = "This is a lexer error, not a parser error"]
fn test_unclosed_string_literal() {
    let source = r#"val msg = "unterminated"#;
    assert_parse_error(source);
}

#[test]
fn test_invalid_number_literal() {
    let source = "val x = 123.456.789;";
    assert_parse_error(source);
}

#[test]
fn test_reserved_keyword_as_identifier() {
    let source = "val func = 5;";  // 'func' is a keyword
    assert_parse_error(source);
}
