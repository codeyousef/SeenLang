//! Tests for function and lambda parsing

use crate::{Parser, Expression, Parameter, Receiver, ParseResult};
use seen_lexer::{Lexer, KeywordManager};

fn parse_expression(input: &str) -> ParseResult<Expression> {
    let keyword_manager = KeywordManager::new("en").unwrap();
    let lexer = Lexer::new(input, keyword_manager);
    let mut parser = Parser::new(lexer);
    parser.parse_expression()
}

#[test]
fn test_parse_simple_function() {
    let expr = parse_expression(r#"
        fun greet(name: String): String {
            return "Hello, {name}!"
        }
    "#).unwrap();
    
    match expr {
        Expression::Function { name, params, return_type, is_async, receiver, .. } => {
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
    let expr = parse_expression(r#"
        async fun fetchData(): Data {
            return await api.get()
        }
    "#).unwrap();
    
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
    let expr = parse_expression(r#"
        fun (p: Person) getName(): String {
            return p.name
        }
    "#).unwrap();
    
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
    let expr = parse_expression(r#"
        fun (p: inout Person) setAge(age: Int) {
            p.age = age
        }
    "#).unwrap();
    
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
fn test_parse_simple_lambda() {
    let expr = parse_expression("{ x -> x * 2 }").unwrap();
    
    match expr {
        Expression::Lambda { params, body, return_type, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert!(params[0].type_annotation.is_none());
            assert!(return_type.is_none());
        }
        _ => panic!("Expected lambda expression"),
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
    let expr = parse_expression("{ (x: Int, y: Int) -> Int in x + y }").unwrap();
    
    match expr {
        Expression::Lambda { params, return_type, .. } => {
            assert_eq!(params.len(), 2);
            assert!(params[0].type_annotation.is_some());
            assert!(params[1].type_annotation.is_some());
            assert!(return_type.is_some());
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_lambda_with_block_body() {
    let expr = parse_expression(r#"{ x -> 
        let doubled = x * 2
        return doubled + 10
    }"#).unwrap();
    
    match expr {
        Expression::Lambda { body, .. } => {
            match &**body {
                Expression::Block { expressions, .. } => {
                    assert!(expressions.len() > 1);
                }
                _ => panic!("Expected block body"),
            }
        }
        _ => panic!("Expected lambda expression"),
    }
}

#[test]
fn test_parse_function_call() {
    let expr = parse_expression("calculate(10, 20)").unwrap();
    
    match expr {
        Expression::Call { callee, args, .. } => {
            match &**callee {
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
            match &**callee {
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
            match &**callee {
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
            match &**expr {
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
    let expr = parse_expression(r#"
        fun greet(name: String, greeting: String = "Hello"): String {
            return "{greeting}, {name}!"
        }
    "#).unwrap();
    
    match expr {
        Expression::Function { params, .. } => {
            assert_eq!(params.len(), 2);
            assert!(params[0].default_value.is_none());
            assert!(params[1].default_value.is_some());
        }
        _ => panic!("Expected function with default parameter"),
    }
}