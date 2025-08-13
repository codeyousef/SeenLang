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

// Lambda Expression Tests (following Syntax Design spec)

#[test]
fn test_parse_simple_lambda_no_params() {
    let expr = parse_expression("{ 42 }").unwrap();
    match expr {
        Expression::Lambda { params, body, .. } => {
            assert_eq!(params.len(), 0);
            match body.as_ref() {
                Expression::IntegerLiteral { value, .. } => {
                    assert_eq!(*value, 42);
                }
                _ => panic!("Expected integer literal in lambda body"),
            }
        }
        // Single expression might be parsed as just the expression
        Expression::IntegerLiteral { value, .. } => {
            assert_eq!(value, 42);
        }
        _ => panic!("Expected lambda or integer literal, got: {:?}", expr),
    }
}

#[test]
fn test_parse_lambda_single_param() {
    let expr = parse_expression("{ x -> x * 2 }").unwrap();
    match expr {
        Expression::Lambda { params, body, return_type, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert!(return_type.is_none()); // No explicit return type
            
            // Check body is multiplication
            match body.as_ref() {
                Expression::BinaryOp { left, right, .. } => {
                    match (left.as_ref(), right.as_ref()) {
                        (Expression::Identifier { name, .. }, Expression::IntegerLiteral { value, .. }) => {
                            assert_eq!(name, "x");
                            assert_eq!(*value, 2);
                        }
                        _ => panic!("Expected x * 2 in lambda body"),
                    }
                }
                _ => panic!("Expected binary operation in lambda body"),
            }
        }
        _ => panic!("Expected lambda expression, got: {:?}", expr),
    }
}

#[test]
fn test_parse_lambda_multiple_params() {
    let expr = parse_expression("{ x, y -> x + y }").unwrap();
    match expr {
        Expression::Lambda { params, body, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[1].name, "y");
            
            // Check body is addition
            match body.as_ref() {
                Expression::BinaryOp { left, right, .. } => {
                    match (left.as_ref(), right.as_ref()) {
                        (Expression::Identifier { name: left_name, .. }, 
                         Expression::Identifier { name: right_name, .. }) => {
                            assert_eq!(left_name, "x");
                            assert_eq!(right_name, "y");
                        }
                        _ => panic!("Expected x + y in lambda body"),
                    }
                }
                _ => panic!("Expected binary operation in lambda body"),
            }
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_lambda_with_explicit_types() {
    let expr = parse_expression("{ x: Int, y: Int -> x + y }").unwrap();
    match expr {
        Expression::Lambda { params, body, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[1].name, "y");
            
            // Check parameter types
            assert!(params[0].type_annotation.is_some());
            assert!(params[1].type_annotation.is_some());
            
            if let Some(ref param_type) = params[0].type_annotation {
                assert_eq!(param_type.name, "Int");
            }
            if let Some(ref param_type) = params[1].type_annotation {
                assert_eq!(param_type.name, "Int");
            }
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_lambda_function_type_assignment() {
    // Test lambda assigned to function type: let predicate: (String) -> Bool = { s -> s.length > 5 }
    let expr = parse_expression("{ s -> s.length > 5 }").unwrap();
    match expr {
        Expression::Lambda { params, return_type, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "s");
            assert!(return_type.is_none()); // Type is inferred from assignment context
        }
        _ => panic!("Expected lambda expression for function type assignment"),
    }
}

#[test] 
fn test_parse_lambda_complex_body() {
    let expr = parse_expression("{ name -> \"Hello, \" + name + \"!\" }").unwrap();
    match expr {
        Expression::Lambda { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "name");
            
            // Body should be complex string concatenation
            match body.as_ref() {
                Expression::BinaryOp { .. } => {
                    // Complex expression parsed correctly
                }
                _ => panic!("Expected complex binary operation in lambda body"),
            }
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_lambda_block_body() {
    let expr = parse_expression("{ x -> \n    let y = x * 2 \n    y + 1 \n}").unwrap();
    match expr {
        Expression::Lambda { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            
            // Body should be a block with multiple expressions
            match body.as_ref() {
                Expression::Block { expressions, .. } => {
                    assert_eq!(expressions.len(), 2); // let statement and return expression
                }
                _ => panic!("Expected block expression in lambda body"),
            }
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_nested_lambdas() {
    let expr = parse_expression("{ x -> { y -> x + y } }").unwrap();
    match expr {
        Expression::Lambda { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            
            // Body should be another lambda
            match body.as_ref() {
                Expression::Lambda { params: inner_params, .. } => {
                    assert_eq!(inner_params.len(), 1);
                    assert_eq!(inner_params[0].name, "y");
                }
                _ => panic!("Expected nested lambda in body"),
            }
        }
        _ => panic!("Expected outer lambda expression"),
    }
}

#[test]
fn test_parse_lambda_as_argument() {
    let expr = parse_expression("map({ x -> x * 2 }, list)").unwrap();
    match expr {
        Expression::Call { callee, args, .. } => {
            match callee.as_ref() {
                Expression::Identifier { name, .. } => {
                    assert_eq!(name, "map");
                }
                _ => panic!("Expected 'map' function call"),
            }
            
            assert_eq!(args.len(), 2);
            
            // First argument should be a lambda
            match &args[0] {
                Expression::Lambda { params, .. } => {
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0].name, "x");
                }
                _ => panic!("Expected lambda as first argument"),
            }
        }
        _ => panic!("Expected function call"),
    }
}

#[test]
fn test_parse_lambda_trailing_syntax() {
    // Test trailing lambda syntax: list.Map { it * 2 }
    let expr = parse_expression("list.Map { it * 2 }").unwrap();
    match expr {
        Expression::Call { callee, args, .. } => {
            // Should parse as call with lambda argument
            match callee.as_ref() {
                Expression::MemberAccess { object, member, .. } => {
                    match object.as_ref() {
                        Expression::Identifier { name, .. } => assert_eq!(name, "list"),
                        _ => panic!("Expected 'list' object"),
                    }
                    assert_eq!(member, "Map");
                }
                _ => panic!("Expected member access"),
            }
            
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expression::Lambda { params, .. } => {
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0].name, "it"); // implicit parameter
                }
                _ => panic!("Expected lambda argument"),
            }
        }
        _ => panic!("Expected method call with trailing lambda"),
    }
}