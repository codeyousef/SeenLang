//! Tests for function and lambda parsing

use crate::{BinaryOperator, Expression, MemoryModifier, ParseError, ParseResult, Parser};
use seen_lexer::{KeywordManager, Lexer, LexerConfig, Position, VisibilityPolicy};
use std::sync::Arc;

fn parse_expression(input: &str) -> ParseResult<Expression> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_expression()
}

fn parse_expression_with_visibility(
    input: &str,
    policy: VisibilityPolicy,
) -> ParseResult<Expression> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::with_config(
        input.to_string(),
        Arc::new(keyword_manager),
        LexerConfig {
            visibility_policy: policy,
        },
    );
    let mut parser = Parser::new_with_visibility(lexer, policy);
    let program = parser.parse_program()?;
    program
        .expressions
        .into_iter()
        .next()
        .ok_or_else(|| ParseError::UnexpectedEof {
            pos: Position::new(1, 1, 0),
        })
}

#[test]
fn test_parse_simple_function() {
    let expr = parse_expression(
        r#"
        fun greet(name: String): String {
            return "Hello, {name}!"
        }
    "#,
    )
    .unwrap();

    match expr {
        Expression::Function {
            name,
            params,
            return_type,
            is_async,
            receiver,
            ..
        } => {
            assert_eq!(name, "greet");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "name");
            assert!(return_type.is_some());
            assert!(!is_async);
            assert!(receiver.is_none());
        }
        _ => panic!("Expected function expression"),
    }
}

#[test]
fn test_parse_async_function() {
    let expr = parse_expression(
        r#"
        async fun fetchData(): String {
            return await api.get()
        }
    "#,
    )
    .unwrap();

    match expr {
        Expression::Function { name, is_async, .. } => {
            assert_eq!(name, "fetchData");
            assert!(is_async);
        }
        _ => panic!("Expected async function"),
    }
}

#[test]
fn test_parse_method_receiver_syntax() {
    let expr = parse_expression(
        r#"
        fun (p: Person) getName(): String {
            return p.name
        }
    "#,
    )
    .unwrap();

    match expr {
        Expression::Function { receiver, .. } => {
            assert!(receiver.is_some());
            let recv = receiver.unwrap();
            assert_eq!(recv.name, "p");
            assert_eq!(recv.type_name, "Person");
            assert!(!recv.is_mutable);
        }
        _ => panic!("Expected function with receiver"),
    }
}

#[test]
fn test_parse_mutable_receiver() {
    let expr = parse_expression(
        r#"
        fun (p: inout Person) setAge(age: Int) {
            p.age = age
        }
    "#,
    )
    .unwrap();

    match expr {
        Expression::Function { receiver, .. } => {
            assert!(receiver.is_some());
            let recv = receiver.unwrap();
            assert!(recv.is_mutable);
        }
        _ => panic!("Expected function with mutable receiver"),
    }
}

#[test]
fn test_parse_memory_management_parameters() {
    // Test move parameter
    let expr = parse_expression(
        r#"
        fun process(move value: String) {
            println("processing value")
        }
    "#,
    )
    .unwrap();

    match expr {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "value");
            assert_eq!(params[0].memory_modifier, Some(MemoryModifier::Move));
        }
        _ => panic!("Expected function with move parameter"),
    }

    // Test borrow parameter
    let expr2 = parse_expression(
        r#"
        fun process(borrow item: String) {
            println("borrowing item")
        }
    "#,
    )
    .unwrap();

    match expr2 {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "item");
            assert_eq!(params[0].memory_modifier, Some(MemoryModifier::Borrow));
        }
        _ => panic!("Expected function with borrow parameter"),
    }

    // Test inout parameter
    let expr3 = parse_expression(
        r#"
        fun process(inout buffer: String) {
            println("modifying buffer")
        }
    "#,
    )
    .unwrap();

    match expr3 {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "buffer");
            assert_eq!(params[0].memory_modifier, Some(MemoryModifier::Inout));
        }
        _ => panic!("Expected function with inout parameter"),
    }

    // Test mut parameter
    let expr4 = parse_expression(
        r#"
        fun process(mut counter: Int) {
            counter = counter + 1
        }
    "#,
    )
    .unwrap();

    match expr4 {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "counter");
            assert_eq!(params[0].memory_modifier, Some(MemoryModifier::Mut));
        }
        _ => panic!("Expected function with mut parameter"),
    }
}

#[test]
fn test_parse_external_function_keyword() {
    let expr = parse_expression_with_visibility(
        r#"
        external fun puts(message: String) {
            // external shim
        }
    "#,
        VisibilityPolicy::Caps,
    )
        .unwrap();

    match expr {
        Expression::Function {
            name,
            is_external,
            params,
            ..
        } => {
            assert_eq!(name, "puts");
            assert!(is_external);
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "message");
        }
        other => panic!("expected external function, got {:?}", other),
    }
}

#[test]
fn test_parse_cstyle_extern_function() {
    let expr = parse_expression_with_visibility(
        r#"
        extern "C" fun printf(format: String, value: Int): Int
    "#,
        VisibilityPolicy::Caps,
    )
        .unwrap();

    match expr {
        Expression::Function {
            name,
            is_external,
            params,
            return_type,
            body,
            ..
        } => {
            assert_eq!(name, "printf");
            assert!(is_external);
            assert_eq!(params.len(), 2);
            assert!(return_type.is_some());
            if let Expression::NullLiteral { .. } = *body {} else {
                panic!("expected c-style extern body placeholder");
            }
        }
        other => panic!("expected c-style extern function, got {:?}", other),
    }
}

#[test]
fn test_parse_simple_lambda() {
    let expr = parse_expression("{ x -> x * 2 }").unwrap();

    match expr {
        Expression::Lambda {
            params,
            body: _,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert!(params[0].type_annotation.is_none());
            assert!(return_type.is_none());
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_function_visibility_caps_policy() {
    let exported = parse_expression_with_visibility("fun ExportMe() {}", VisibilityPolicy::Caps)
        .expect("should parse exported function");
    let internal = parse_expression_with_visibility("fun internal() {}", VisibilityPolicy::Caps)
        .expect("should parse internal function");

    match exported {
        Expression::Function { is_public, .. } => assert!(is_public, "ExportMe should be public"),
        other => panic!("Expected function expression, got {:?}", other),
    }

    match internal {
        Expression::Function { is_public, .. } => {
            assert!(
                !is_public,
                "lowercase should remain private under caps policy"
            )
        }
        other => panic!("Expected function expression, got {:?}", other),
    }
}

#[test]
fn test_function_visibility_explicit_policy() {
    let exported =
        parse_expression_with_visibility("pub fun ExportMe() {}", VisibilityPolicy::Explicit)
            .expect("should parse exported function with explicit visibility");
    let internal =
        parse_expression_with_visibility("fun ExportMe() {}", VisibilityPolicy::Explicit)
            .expect("should parse internal function without pub");

    match exported {
        Expression::Function { is_public, .. } => {
            assert!(is_public, "pub should mark function public")
        }
        other => panic!("Expected function expression, got {:?}", other),
    }

    match internal {
        Expression::Function { is_public, .. } => {
            assert!(!is_public, "explicit policy requires pub keyword")
        }
        other => panic!("Expected function expression, got {:?}", other),
    }
}

#[test]
fn test_parse_lambda_with_multiple_params() {
    let expr = parse_expression("{ x, y -> x + y }").unwrap();

    match expr {
        Expression::Lambda { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[1].name, "y");
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_lambda_with_types() {
    // Test lambda with explicit parameter types (no return type, inferred)
    let expr = parse_expression("{ x: Int, y: Int -> x + y }").unwrap();

    match expr {
        Expression::Lambda {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[1].name, "y");
            assert!(params[0].type_annotation.is_some());
            assert!(params[1].type_annotation.is_some());
            assert!(return_type.is_none()); // Return type inferred, not specified
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_lambda_with_block_body() {
    let expr = parse_expression(
        r#"{ x -> 
        let doubled = x * 2
        return doubled + 10
    }"#,
    )
    .unwrap();

    match expr {
        Expression::Lambda { body, .. } => match body.as_ref() {
            Expression::Block { expressions, .. } => {
                assert!(expressions.len() > 1);
            }
            _ => panic!("Expected block body"),
        },
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_function_call() {
    let expr = parse_expression("calculate(10, 20)").unwrap();

    match expr {
        Expression::Call { callee, args, .. } => {
            match callee.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "calculate"),
                _ => panic!("Expected identifier as callee"),
            }
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected function call"),
    }
}

#[test]
fn test_parse_method_call() {
    let expr = parse_expression("person.getName()").unwrap();

    match expr {
        Expression::Call { callee, args, .. } => {
            match callee.as_ref() {
                Expression::MemberAccess { member, .. } => {
                    assert_eq!(member, "getName");
                }
                _ => panic!("Expected member access as callee"),
            }
            assert_eq!(args.len(), 0);
        }
        _ => panic!("Expected method call"),
    }
}

#[test]
fn test_parse_chained_calls() {
    let expr = parse_expression("list.filter(isEven).map(double).sum()").unwrap();

    match expr {
        Expression::Call { callee, args, .. } => {
            // The outermost call should be sum()
            assert_eq!(args.len(), 0);
            match callee.as_ref() {
                Expression::MemberAccess { member, .. } => {
                    assert_eq!(member, "sum");
                }
                _ => panic!("Expected member access"),
            }
        }
        _ => panic!("Expected chained method calls"),
    }
}

#[test]
fn test_parse_await_expression() {
    let expr = parse_expression("await fetchData()").unwrap();

    match expr {
        Expression::Await { expr, .. } => {
            match expr.as_ref() {
                Expression::Call { .. } => {
                    // Success
                }
                _ => panic!("Expected function call in await"),
            }
        }
        _ => panic!("Expected await expression"),
    }
}

#[test]
fn test_parse_function_with_default_params() {
    let expr = parse_expression(
        r#"
        fun greet(name: String, greeting: String = "Hello"): String {
            return "{greeting}, {name}!"
        }
    "#,
    )
    .unwrap();

    match expr {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 2);
            assert!(params[0].default_value.is_none());
            assert!(params[1].default_value.is_some());
        }
        _ => panic!("Expected function with default parameter"),
    }
}

#[test]
fn test_function_body_let_initializer_allows_trailing_lambda() {
    let expr = parse_expression(
        r#"
        fun pipeline(numbers: Sequence<Int>): Sequence<Int> {
            let doubled = numbers.Map { it * 2 }
            doubled
        }
    "#,
    )
        .expect("function parses");

    match expr {
        Expression::Function { body, .. } => match body.as_ref() {
            Expression::Block { expressions, .. } => {
                assert_eq!(expressions.len(), 2);
                match &expressions[0] {
                    Expression::Let { value, .. } => match value.as_ref() {
                        Expression::Call { args, .. } => {
                            assert_eq!(args.len(), 1);
                            match &args[0] {
                                Expression::Lambda { params, .. } => {
                                    assert_eq!(params.len(), 1);
                                }
                                other => panic!("expected lambda argument, got {:?}", other),
                            }
                        }
                        other => panic!("expected call on let initializer, got {:?}", other),
                    },
                    other => panic!("expected let binding as first statement, got {:?}", other),
                }
            }
            other => panic!("expected block body, got {:?}", other),
        },
        other => panic!("expected function expression, got {:?}", other),
    }
}

// Additional Default Parameter Tests (following Syntax Design spec)

#[test]
fn test_parse_function_with_multiple_default_params_comprehensive() {
    let expr = parse_expression(
        r#"fun Connect(
        host: String = "localhost",
        port: Int = 8080,
        secure: Bool = false
    ): Connection {
        return Connection(host, port, secure)
    }"#,
    )
    .unwrap();

    match expr {
        Expression::Function { name, params, .. } => {
            assert_eq!(name, "Connect");
            assert_eq!(params.len(), 3);

            // Check first parameter (host)
            assert_eq!(params[0].name, "host");
            assert!(params[0].default_value.is_some());
            match params[0].default_value.as_ref().unwrap() {
                Expression::StringLiteral { value, .. } => assert_eq!(value, "localhost"),
                _ => panic!("Expected string literal"),
            }

            // Check second parameter (port)
            assert_eq!(params[1].name, "port");
            assert!(params[1].default_value.is_some());
            match params[1].default_value.as_ref().unwrap() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 8080),
                _ => panic!("Expected integer literal"),
            }

            // Check third parameter (secure)
            assert_eq!(params[2].name, "secure");
            assert!(params[2].default_value.is_some());
            match params[2].default_value.as_ref().unwrap() {
                Expression::BooleanLiteral { value, .. } => assert_eq!(*value, false),
                _ => panic!("Expected boolean literal"),
            }
        }
        _ => panic!("Expected function expression"),
    }
}

#[test]
fn test_parse_function_mixed_params_some_defaults() {
    let expr = parse_expression(
        "fun Process(input: String, timeout: Int = 5000): Result { return Success(input) }",
    )
    .unwrap();
    match expr {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 2);

            // First param has no default
            assert_eq!(params[0].name, "input");
            assert!(params[0].default_value.is_none());

            // Second param has default
            assert_eq!(params[1].name, "timeout");
            assert!(params[1].default_value.is_some());
            match params[1].default_value.as_ref().unwrap() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 5000),
                _ => panic!("Expected integer literal"),
            }
        }
        _ => panic!("Expected function expression"),
    }
}

#[test]
fn test_parse_function_default_param_complex_expression() {
    let expr =
        parse_expression("fun CreateBuffer(size: Int = 1024 * 8): Buffer { return Buffer(size) }")
            .unwrap();
    match expr {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "size");
            assert!(params[0].default_value.is_some());

            // Default should be a binary operation (1024 * 8)
            match params[0].default_value.as_ref().unwrap() {
                Expression::BinaryOp {
                    op, left, right, ..
                } => {
                    assert_eq!(*op, BinaryOperator::Multiply);
                    match (left.as_ref(), right.as_ref()) {
                        (
                            Expression::IntegerLiteral {
                                value: left_val, ..
                            },
                            Expression::IntegerLiteral {
                                value: right_val, ..
                            },
                        ) => {
                            assert_eq!(*left_val, 1024);
                            assert_eq!(*right_val, 8);
                        }
                        _ => panic!("Expected integer literals in multiplication"),
                    }
                }
                _ => panic!("Expected binary operation as default value"),
            }
        }
        _ => panic!("Expected function expression"),
    }
}
