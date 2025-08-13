//! Tests for basic expression parsing

use crate::{Parser, Expression, ParseResult, InterpolationPart, InterpolationKind};
use seen_lexer::{Lexer, KeywordManager};
use std::sync::Arc;

fn parse_expression(input: &str) -> ParseResult<Expression> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_expression()
}

#[test]
fn test_parse_integer_literal() {
    let expr = parse_expression("42").unwrap();
    match expr {
        Expression::IntegerLiteral { value, .. } => {
            assert_eq!(value, 42);
        }
        _ => panic!("Expected integer literal"),
    }
}

#[test]
fn test_parse_float_literal() {
    let expr = parse_expression("3.14").unwrap();
    match expr {
        Expression::FloatLiteral { value, .. } => {
            assert_eq!(value, 3.14);
        }
        _ => panic!("Expected float literal"),
    }
}

#[test]
fn test_parse_string_literal() {
    let expr = parse_expression("\"hello world\"").unwrap();
    match expr {
        Expression::StringLiteral { value, .. } => {
            assert_eq!(value, "hello world");
        }
        _ => panic!("Expected string literal"),
    }
}

#[test]
fn test_parse_boolean_true() {
    let expr = parse_expression("true");
    if let Err(e) = &expr {
        eprintln!("Parse error: {:?}", e);
        // Try to debug what tokens the lexer produces
        let mut keyword_manager = KeywordManager::new();
        if let Err(load_err) = keyword_manager.load_from_toml("en") {
            eprintln!("Failed to load keywords: {:?}", load_err);
        } else {
            keyword_manager.switch_language("en").unwrap();
            let mut lexer = Lexer::new("true".to_string(), Arc::new(keyword_manager));
            if let Ok(token) = lexer.next_token() {
                eprintln!("Token for 'true': {:?}", token);
            }
        }
    }
    let expr = expr.unwrap();
    match expr {
        Expression::BooleanLiteral { value, .. } => {
            assert_eq!(value, true);
        }
        _ => panic!("Expected boolean literal"),
    }
}

#[test]
fn test_parse_boolean_false() {
    let expr = parse_expression("false").unwrap();
    match expr {
        Expression::BooleanLiteral { value, .. } => {
            assert_eq!(value, false);
        }
        _ => panic!("Expected boolean literal"),
    }
}

#[test]
fn test_parse_null_literal() {
    let expr = parse_expression("null").unwrap();
    match expr {
        Expression::NullLiteral { .. } => {
            // Success
        }
        _ => panic!("Expected null literal"),
    }
}

#[test]
fn test_parse_identifier() {
    let expr = parse_expression("myVariable").unwrap();
    match expr {
        Expression::Identifier { name, is_public, .. } => {
            assert_eq!(name, "myVariable");
            assert_eq!(is_public, false); // lowercase = private
        }
        _ => panic!("Expected identifier"),
    }
}

#[test]
fn test_parse_public_identifier() {
    let expr = parse_expression("PublicVariable").unwrap();
    match expr {
        Expression::Identifier { name, is_public, .. } => {
            assert_eq!(name, "PublicVariable");
            assert_eq!(is_public, true); // uppercase = public
        }
        _ => panic!("Expected identifier"),
    }
}

#[test]
fn test_parse_interpolated_string() {
    let expr = parse_expression("\"Hello, {name}!\"").unwrap();
    match expr {
        Expression::InterpolatedString { parts, .. } => {
            assert_eq!(parts.len(), 3); // "Hello, " + {name} + "!"
        }
        _ => panic!("Expected interpolated string"),
    }
}

#[test]
fn test_parse_complex_interpolated_string() {
    // Test complex expression in interpolation
    let expr = parse_expression("\"Result: {compute(x + y)}\"").unwrap();
    match expr {
        Expression::InterpolatedString { parts, .. } => {
            assert_eq!(parts.len(), 2); // "Result: " + {compute(x + y)}
            if let InterpolationPart { kind: InterpolationKind::Expression(expr), .. } = &parts[1] {
                // Verify it parsed as a function call
                match expr.as_ref() {
                    Expression::Call { .. } => {
                        // Success! The complex expression was parsed correctly
                    }
                    _ => panic!("Expected function call expression in interpolation, got: {:?}", expr),
                }
            } else {
                panic!("Expected expression part in interpolation");
            }
        }
        _ => panic!("Expected interpolated string"),
    }
}

#[test]
fn test_parse_array_literal() {
    let expr = parse_expression("[1, 2, 3]").unwrap();
    match expr {
        Expression::ArrayLiteral { elements, .. } => {
            assert_eq!(elements.len(), 3);
        }
        _ => panic!("Expected array literal"),
    }
}

#[test]
fn test_parse_empty_array() {
    let expr = parse_expression("[]").unwrap();
    match expr {
        Expression::ArrayLiteral { elements, .. } => {
            assert_eq!(elements.len(), 0);
        }
        _ => panic!("Expected array literal"),
    }
}

#[test]
fn test_parse_struct_literal() {
    let expr = parse_expression("Person { name: \"Alice\", age: 30 }").unwrap();
    match expr {
        Expression::StructLiteral { name, fields, .. } => {
            assert_eq!(name, "Person");
            assert_eq!(fields.len(), 2);
        }
        _ => panic!("Expected struct literal"),
    }
}

#[test]
fn test_parse_block_expression() {
    // Test simple single-expression block
    let expr = parse_expression("{ 42 }").unwrap();
    match expr {
        Expression::IntegerLiteral { value, .. } => {
            assert_eq!(value, 42);
        }
        _ => panic!("Expected integer literal (blocks with single expression return that expression)"),
    }
    
    // Test multi-expression block
    // Seen doesn't use semicolons - statements are separated by newlines
    let expr2 = parse_expression("{ let x = 10 \n x + 5 }").unwrap();
    match expr2 {
        Expression::Block { expressions, .. } => {
            assert_eq!(expressions.len(), 2);
        }
        _ => panic!("Expected block expression"),
    }
}

#[test]
fn test_parse_parenthesized_expression() {
    let expr = parse_expression("(42)").unwrap();
    match expr {
        Expression::IntegerLiteral { value, .. } => {
            assert_eq!(value, 42);
        }
        _ => panic!("Expected integer literal"),
    }
}