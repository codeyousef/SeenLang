//! Tests for generic type parsing

use crate::{Expression, Parser, Type};
use seen_lexer::{KeywordManager, Lexer};
use std::sync::Arc;

fn parse_expression(input: &str) -> Result<Expression, crate::ParseError> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program()?;
    program
        .expressions
        .into_iter()
        .next()
        .ok_or_else(|| crate::ParseError::UnexpectedEof {
            pos: seen_lexer::Position::new(1, 1, 0),
        })
}

fn parse_type(input: &str) -> Result<Type, crate::ParseError> {
    // Parse a type by creating a dummy variable declaration with assignment
    let full_input = format!("let x: {} = null", input);
    match parse_expression(&full_input)? {
        Expression::Let {
            type_annotation: Some(t),
            ..
        } => Ok(t),
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
    dbg!(&expr);
    match expr {
        Expression::Let {
            type_annotation: Some(t),
            ..
        } => {
            assert_eq!(t.name, "Array");
            assert_eq!(t.generics.len(), 1);
            assert_eq!(t.generics[0].name, "Int");
        }
        _ => panic!("Expected let expression with generic type"),
    }
}

#[test]
fn test_parse_generic_function_parameter() {
    let expr = parse_expression("fun Process(items: List<String>) r: Bool { return true }").unwrap();
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
    let expr = parse_expression("fun GetData() r: Array<String> { return [] }").unwrap();
    match expr {
        Expression::Function {
            return_type: Some(ret_type),
            ..
        } => {
            assert_eq!(ret_type.name, "Array");
            assert_eq!(ret_type.generics.len(), 1);
            assert_eq!(ret_type.generics[0].name, "String");
        }
        _ => panic!("Expected function with generic return type"),
    }
}

#[test]
fn test_parse_generic_constructor_call() {
    // NOTE: Generic arguments are currently parsed and discarded, so the
    // constructor shows up as a plain call expression. Once expression-level
    // generics are represented explicitly, this test can be tightened.
    let expr = parse_expression("HashMap<String, Int>()").unwrap();

    match expr {
        Expression::Call { callee, args, .. } => {
            match callee.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "HashMap"),
                other => panic!("Expected HashMap identifier, got {:?}", other),
            }
            assert!(
                args.is_empty(),
                "Constructor should have no positional args in this snippet"
            );
        }
        other => panic!("Expected call expression, got {:?}", other),
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
fn test_parse_struct_with_generics() {
    let expr = parse_expression("data CommandBuffer<S> { phantom: Phantom<S> }").unwrap();
    match expr {
        Expression::StructDefinition {
            name,
            generics,
            fields,
            ..
        } => {
            assert_eq!(name, "CommandBuffer");
            assert_eq!(generics, vec!["S".to_string()]);
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "phantom");
            assert_eq!(fields[0].field_type.name, "Phantom");
            assert_eq!(fields[0].field_type.generics.len(), 1);
            assert_eq!(fields[0].field_type.generics[0].name, "S");
        }
        other => panic!("Expected struct definition, got: {:?}", other),
    }
}

#[test]
fn test_parse_generic_with_nullable() {
    let type_result = parse_type("Array<String?>").unwrap();
    assert_eq!(type_result.name, "Array");
    assert_eq!(type_result.generics.len(), 1);
    assert_eq!(type_result.generics[0].name, "String");
    assert!(type_result.generics[0].is_nullable);
}

#[test]
fn test_parse_generic_class_definition_with_struct_literal() {
    let source = r#"
class Vec<T> {
    var data: Array<T>
    var length: Int

    fun new() r: Vec<T> {
        return Vec{ data: Array<T>(), length: 0 }
    }

    fun toArray() r: Array<T> {
        let out = Array<T>()
        for index in range(0, length) {
            out.push(data[index])
        }
        return out
    }
}
"#;

    let expr = parse_expression(source).expect("class parses");
    match expr {
        Expression::ClassDefinition {
            name,
            generics,
            fields,
            ..
        } => {
            assert_eq!(name, "Vec");
            assert_eq!(generics, vec!["T".to_string()]);
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "data");
            assert_eq!(fields[0].field_type.name, "Array");
            assert_eq!(fields[0].field_type.generics.len(), 1);
            assert_eq!(fields[0].field_type.generics[0].name, "T");
        }
        other => panic!("expected class definition, got {:?}", other),
    }
}

#[test]
fn test_parse_extension_function_with_generics() {
    let source = r#"
fun <T> List<T>.isEmpty() r: Bool {
    return this.size() == 0
}
"#;

    let expr = parse_expression(source).expect("parse extension function");
    match expr {
        Expression::Function {
            generics, receiver, ..
        } => {
            assert_eq!(generics, vec!["T".to_string()]);
            let recv = receiver.expect("expected receiver");
            assert_eq!(recv.type_name, "List");
        }
        other => panic!("Expected function expression, got {:?}", other),
    }
}
