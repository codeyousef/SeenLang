//! Tests for destructuring declarations (Kotlin feature)

use crate::parser::Parser;
use crate::ast::*;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_tuple_destructuring() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        val (x, y) = getPoint()
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse tuple destructuring");
}

#[test]
fn test_data_class_destructuring() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        val (name, age) = person
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse data class destructuring");
}

#[test]
fn test_nested_destructuring() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        val (first, (x, y)) = getNestedData()
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse nested destructuring");
}

#[test]
fn test_destructuring_with_underscore() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        val (name, _, age) = getPersonData()
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse destructuring with underscore");
}

#[test]
fn test_for_loop_destructuring() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        for ((key, value) in map) {
            println("$key: $value")
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse for loop destructuring");
}

#[test]
fn test_lambda_parameter_destructuring() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        map.forEach { (key, value) ->
            println("$key = $value")
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse lambda parameter destructuring");
}