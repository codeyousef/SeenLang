//! Tests for control flow expressions (if, match, loops)

use crate::{Parser, Expression, Pattern, ParseResult};
use seen_lexer::{Lexer, KeywordManager};

fn parse_expression(input: &str) -> ParseResult<Expression> {
    let keyword_manager = KeywordManager::new("en").unwrap();
    let lexer = Lexer::new(input, keyword_manager);
    let mut parser = Parser::new(lexer);
    parser.parse_expression()
}

#[test]
fn test_parse_if_expression_returns_value() {
    let expr = parse_expression("if x > 10 { \"big\" } else { \"small\" }").unwrap();
    match expr {
        Expression::If { condition, then_branch, else_branch, .. } => {
            assert!(else_branch.is_some());
            // Verify both branches are expressions
            match &**then_branch {
                Expression::StringLiteral { value, .. } => assert_eq!(value, "big"),
                _ => panic!("Expected string literal in then branch"),
            }
        }
        _ => panic!("Expected if expression"),
    }
}

#[test]
fn test_parse_if_without_else() {
    let expr = parse_expression("if condition { doSomething() }").unwrap();
    match expr {
        Expression::If { else_branch, .. } => {
            assert!(else_branch.is_none());
        }
        _ => panic!("Expected if expression"),
    }
}

#[test]
fn test_parse_if_with_word_operators() {
    let expr = parse_expression("if age >= 18 and hasPermission { \"allowed\" }").unwrap();
    match expr {
        Expression::If { condition, .. } => {
            // Should parse 'and' as a binary operator
            match &**condition {
                Expression::BinaryOp { .. } => {
                    // Success
                }
                _ => panic!("Expected binary operation with 'and'"),
            }
        }
        _ => panic!("Expected if expression"),
    }
}

#[test]
fn test_parse_match_expression() {
    let expr = parse_expression(r#"
        match value {
            0 -> "zero"
            1..10 -> "small"
            _ -> "large"
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { arms, .. } => {
            assert_eq!(arms.len(), 3);
            // Check first arm is literal pattern
            match &arms[0].pattern {
                Pattern::Literal(_) => {},
                _ => panic!("Expected literal pattern"),
            }
            // Check second arm is range pattern
            match &arms[1].pattern {
                Pattern::Range { .. } => {},
                _ => panic!("Expected range pattern"),
            }
            // Check third arm is wildcard
            match &arms[2].pattern {
                Pattern::Wildcard => {},
                _ => panic!("Expected wildcard pattern"),
            }
        }
        _ => panic!("Expected match expression"),
    }
}

#[test]
fn test_parse_match_with_guard() {
    let expr = parse_expression(r#"
        match response {
            Ok(data) if data.length > 0 -> processData(data)
            Ok(_) -> "empty"
            Err(e) -> handleError(e)
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { arms, .. } => {
            assert_eq!(arms.len(), 3);
            // First arm should have a guard
            assert!(arms[0].guard.is_some());
            // Other arms should not have guards
            assert!(arms[1].guard.is_none());
            assert!(arms[2].guard.is_none());
        }
        _ => panic!("Expected match expression"),
    }
}

#[test]
fn test_parse_while_loop() {
    let expr = parse_expression("while count < 10 { count = count + 1 }").unwrap();
    match expr {
        Expression::While { condition, body, .. } => {
            // Verify condition is a comparison
            match &**condition {
                Expression::BinaryOp { .. } => {
                    // Success
                }
                _ => panic!("Expected binary operation in condition"),
            }
        }
        _ => panic!("Expected while loop"),
    }
}

#[test]
fn test_parse_for_loop() {
    let expr = parse_expression("for item in items { process(item) }").unwrap();
    match expr {
        Expression::For { variable, iterable, body, .. } => {
            assert_eq!(variable, "item");
            match &**iterable {
                Expression::Identifier { name, .. } => assert_eq!(name, "items"),
                _ => panic!("Expected identifier as iterable"),
            }
        }
        _ => panic!("Expected for loop"),
    }
}

#[test]
fn test_parse_break_with_value() {
    let expr = parse_expression("break 42").unwrap();
    match expr {
        Expression::Break { value, .. } => {
            assert!(value.is_some());
            match &**value.as_ref().unwrap() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 42),
                _ => panic!("Expected integer literal"),
            }
        }
        _ => panic!("Expected break expression"),
    }
}

#[test]
fn test_parse_break_without_value() {
    let expr = parse_expression("break").unwrap();
    match expr {
        Expression::Break { value, .. } => {
            assert!(value.is_none());
        }
        _ => panic!("Expected break expression"),
    }
}

#[test]
fn test_parse_continue() {
    let expr = parse_expression("continue").unwrap();
    match expr {
        Expression::Continue { .. } => {
            // Success
        }
        _ => panic!("Expected continue expression"),
    }
}

#[test]
fn test_parse_return_with_value() {
    let expr = parse_expression("return result").unwrap();
    match expr {
        Expression::Return { value, .. } => {
            assert!(value.is_some());
        }
        _ => panic!("Expected return expression"),
    }
}

#[test]
fn test_parse_return_without_value() {
    let expr = parse_expression("return").unwrap();
    match expr {
        Expression::Return { value, .. } => {
            assert!(value.is_none());
        }
        _ => panic!("Expected return expression"),
    }
}