//! Tests for async/await parsing

use crate::{Parser, Expression};
use seen_lexer::{Lexer, KeywordManager};
use std::sync::Arc;

fn parse_top_level_item(input: &str) -> Result<Expression, crate::ParseError> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_top_level_item()
}

fn parse_expression(input: &str) -> Result<Expression, crate::ParseError> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_expression()
}

// Async/Await Tests (following Syntax Design spec)

#[test]
fn test_parse_async_function() {
    let expr = parse_top_level_item(r#"
        async fun FetchUser(id: UserID): User {
            let response = await Http.Get("/users/" + id)
            return User.FromJson(response.body)
        }
    "#).unwrap();
    
    match expr {
        Expression::Function { name, is_async, params, return_type, .. } => {
            assert_eq!(name, "FetchUser");
            assert!(is_async);
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "id");
            assert!(return_type.is_some());
            let ret_type = return_type.unwrap();
            assert_eq!(ret_type.name, "User");
        }
        _ => panic!("Expected async function"),
    }
}

#[test]
fn test_parse_private_async_function() {
    let expr = parse_top_level_item(r#"
        async fun processInternal(): Result {
            return Success()
        }
    "#).unwrap();
    
    match expr {
        Expression::Function { name, is_async, .. } => {
            assert_eq!(name, "processInternal");
            assert!(is_async);
            // Verify privacy by lowercase name
            assert!(name.chars().next().unwrap().is_lowercase());
        }
        _ => panic!("Expected async function"),
    }
}

#[test]
fn test_parse_await_expression() {
    let expr = parse_expression("await Http.Get(\"/api/data\")").unwrap();
    
    match expr {
        Expression::Await { expr, .. } => {
            match expr.as_ref() {
                Expression::Call { callee, .. } => {
                    match callee.as_ref() {
                        Expression::MemberAccess { object, member, .. } => {
                            match object.as_ref() {
                                Expression::Identifier { name, .. } => assert_eq!(name, "Http"),
                                _ => panic!("Expected Http identifier"),
                            }
                            assert_eq!(member, "Get");
                        }
                        _ => panic!("Expected member access"),
                    }
                }
                _ => panic!("Expected function call in await"),
            }
        }
        _ => panic!("Expected await expression"),
    }
}

#[test] 
fn test_parse_await_variable() {
    let expr = parse_expression("await userFuture").unwrap();
    
    match expr {
        Expression::Await { expr, .. } => {
            match expr.as_ref() {
                Expression::Identifier { name, .. } => {
                    assert_eq!(name, "userFuture");
                }
                _ => panic!("Expected identifier in await"),
            }
        }
        _ => panic!("Expected await expression"),
    }
}

#[test]
fn test_parse_async_block() {
    let expr = parse_expression(r#"
        async {
            let user = spawn { FetchUser(123) }
            let posts = spawn { FetchPosts(123) }
            Display(await user, await posts)
        }
    "#).unwrap();
    
    match expr {
        Expression::AsyncBlock { body, .. } => {
            match body.as_ref() {
                Expression::Block { expressions, .. } => {
                    assert!(expressions.len() > 0);
                }
                _ => panic!("Expected block in async block"),
            }
        }
        _ => panic!("Expected async block"),
    }
}

#[test]
fn test_parse_spawn_expression() {
    let expr = parse_expression("spawn { FetchUser(123) }").unwrap();
    
    match expr {
        Expression::Spawn { expr, .. } => {
            match expr.as_ref() {
                Expression::Block { expressions, .. } => {
                    assert_eq!(expressions.len(), 1);
                    match &expressions[0] {
                        Expression::Call { .. } => {
                            // Expected - call within spawn block
                        }
                        other => panic!("Expected call in block, got: {:?}", other),
                    }
                }
                other => panic!("Expected block in spawn, got: {:?}", other),
            }
        }
        other => panic!("Expected spawn expression, got: {:?}", other),
    }
}

#[test]
fn test_parse_async_lambda() {
    // Let's first test if a simple lambda works
    let simple = parse_expression("{ x -> x + 1 }").unwrap();
    match simple {
        Expression::Lambda { .. } => {
            println!("Simple lambda parsed correctly");
        }
        other => panic!("Expected simple lambda, got: {:?}", other),
    }
    
    // Now test lambda with await
    let await_lambda = parse_expression("{ x -> await process(x) }").unwrap();
    match await_lambda {
        Expression::Lambda { .. } => {
            println!("Await lambda parsed correctly");
        }
        other => panic!("Expected await lambda, got: {:?}", other),
    }
    
    // Finally test async block with lambda
    let expr = parse_expression("async { x -> await process(x) }").unwrap();
    
    match expr {
        Expression::AsyncBlock { body, .. } => {
            match body.as_ref() {
                Expression::Lambda { .. } => {
                    println!("Async lambda parsed correctly");
                }
                other => panic!("Expected lambda in async block, got: {:?}", other),
            }
        }
        other => panic!("Expected async block with lambda, got: {:?}", other),
    }
}

#[test]
fn test_parse_nested_await() {
    let expr = parse_expression("await (await getUser()).getProfile()").unwrap();
    
    match expr {
        Expression::Await { expr, .. } => {
            // Outer await
            match expr.as_ref() {
                Expression::Call { callee, .. } => {
                    match callee.as_ref() {
                        Expression::MemberAccess { object, .. } => {
                            match object.as_ref() {
                                Expression::Await { .. } => {
                                    // Inner await found
                                }
                                _ => panic!("Expected inner await"),
                            }
                        }
                        _ => panic!("Expected member access"),
                    }
                }
                _ => panic!("Expected call expression"),
            }
        }
        _ => panic!("Expected outer await expression"),
    }
}