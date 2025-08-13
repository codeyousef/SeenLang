//! Tests for generic type parsing

use crate::{Parser, Expression, Type};
use seen_lexer::{Lexer, KeywordManager};
use std::sync::Arc;

fn parse_expression(input: &str) -> Result<Expression, crate::ParseError> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_expression()
}

fn parse_type(input: &str) -> Result<Type, crate::ParseError> {
    // Parse a type by creating a dummy variable declaration with assignment
    let full_input = format!("let x: {} = null", input);
    match parse_expression(&full_input)? {
        Expression::Let { type_annotation: Some(t), .. } => Ok(t),
        _ => panic!("Failed to extract type annotation"),
    }
}

// Generic Type Tests (following Syntax Design spec)

#[test]
fn test_parse_simple_generic_type() {
    let type_result = parse_type("Array<Int>").unwrap();
    assert_eq!(type_result.name, "Array");
    assert_eq!(type_result.generics.len(), 1);
    assert_eq!(type_result.generics[0].name, "Int");
}

#[test]
fn test_parse_map_generic_type() {
    let type_result = parse_type("Map<String, Int>").unwrap();
    assert_eq!(type_result.name, "Map");
    assert_eq!(type_result.generics.len(), 2);
    assert_eq!(type_result.generics[0].name, "String");
    assert_eq!(type_result.generics[1].name, "Int");
}

#[test]
fn test_parse_hashmap_generic_type() {
    let type_result = parse_type("HashMap<String, Int>").unwrap();
    assert_eq!(type_result.name, "HashMap");
    assert_eq!(type_result.generics.len(), 2);
    assert_eq!(type_result.generics[0].name, "String");
    assert_eq!(type_result.generics[1].name, "Int");
}

#[test]
fn test_parse_nested_generic_type() {
    let type_result = parse_type("Array<Map<String, Int>>").unwrap();
    assert_eq!(type_result.name, "Array");
    assert_eq!(type_result.generics.len(), 1);
    
    let inner_type = &type_result.generics[0];
    assert_eq!(inner_type.name, "Map");
    assert_eq!(inner_type.generics.len(), 2);
    assert_eq!(inner_type.generics[0].name, "String");
    assert_eq!(inner_type.generics[1].name, "Int");
}

#[test]
fn test_parse_generic_variable_declaration() {
    let expr = parse_expression("let numbers: Array<Int> = [1, 2, 3]").unwrap();
    match expr {
        Expression::Let { type_annotation: Some(t), .. } => {
            assert_eq!(t.name, "Array");
            assert_eq!(t.generics.len(), 1);
            assert_eq!(t.generics[0].name, "Int");
        }
        _ => panic!("Expected let expression with generic type"),
    }
}

#[test]
fn test_parse_generic_function_parameter() {
    let expr = parse_expression("fun Process(items: List<String>): Bool { return true }").unwrap();
    match expr {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "items");
            
            match &params[0].type_annotation {
                Some(param_type) => {
                    assert_eq!(param_type.name, "List");
                    assert_eq!(param_type.generics.len(), 1);
                    assert_eq!(param_type.generics[0].name, "String");
                }
                None => panic!("Expected type annotation on parameter"),
            }
        }
        _ => panic!("Expected function expression"),
    }
}

#[test]
fn test_parse_generic_function_return_type() {
    let expr = parse_expression("fun GetData(): Array<String> { return [] }").unwrap();
    match expr {
        Expression::Function { return_type: Some(ret_type), .. } => {
            assert_eq!(ret_type.name, "Array");
            assert_eq!(ret_type.generics.len(), 1);
            assert_eq!(ret_type.generics[0].name, "String");
        }
        _ => panic!("Expected function with generic return type"),
    }
}

#[test]
fn test_parse_generic_constructor_call() {
    // NOTE: Current implementation limitation - generic constructor calls 
    // like HashMap<String, Int>() are parsed as binary operations
    // This needs to be enhanced in a future update to support
    // generic type expressions in expression contexts
    let expr = parse_expression("HashMap<String, Int>()").unwrap();
    
    // For now, just verify it parses to something (binary op due to < >)
    // This test documents current behavior until we implement proper
    // generic type expressions in expression contexts
    match expr {
        Expression::BinaryOp { left, op: crate::BinaryOperator::Less, .. } => {
            match left.as_ref() {
                Expression::Identifier { name, .. } => {
                    assert_eq!(name, "HashMap");
                }
                _ => panic!("Expected HashMap identifier"),
            }
        }
        _ => panic!("Expected binary operation (current parsing limitation)"),
    }
}

#[test]
fn test_parse_multiple_generic_parameters() {
    let type_result = parse_type("Result<User, Error>").unwrap();
    assert_eq!(type_result.name, "Result");
    assert_eq!(type_result.generics.len(), 2);
    assert_eq!(type_result.generics[0].name, "User");
    assert_eq!(type_result.generics[1].name, "Error");
}

#[test]
fn test_parse_generic_with_nullable() {
    let type_result = parse_type("Array<String?>").unwrap();
    assert_eq!(type_result.name, "Array");
    assert_eq!(type_result.generics.len(), 1);
    assert_eq!(type_result.generics[0].name, "String");
    assert!(type_result.generics[0].is_nullable);
}