//! Tests for control flow expressions (if, match, loops)

use crate::{Parser, Expression, Pattern, ParseResult, BinaryOperator};
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
fn test_parse_if_expression_returns_value() {
    let expr = parse_expression("if x > 10 { \"big\" } else { \"small\" }").unwrap();
    match expr {
        Expression::If { condition, then_branch, else_branch, .. } => {
            assert!(else_branch.is_some());
            // Verify both branches are expressions
            match then_branch.as_ref() {
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
            match condition.as_ref() {
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
    let expr = parse_expression(r#"match value {
            0 -> "zero"
            1..10 -> "small"
            _ -> "large"
        }"#).unwrap();
    
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
            data if true -> "processed"
            _ -> "other"
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // First arm should have a guard
            assert!(arms[0].guard.is_some());
            // Second arm should not have a guard
            assert!(arms[1].guard.is_none());
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
            match condition.as_ref() {
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
    let expr = parse_expression("for item in items { item }").unwrap();
    match expr {
        Expression::For { variable, iterable, body, .. } => {
            assert_eq!(variable, "item");
            match iterable.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "items"),
                _ => panic!("Expected identifier as iterable"),
            }
            match body.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "item"),
                _ => panic!("Expected identifier in body"),
            }
        }
        _ => panic!("Expected for loop"),
    }
}

#[test]
fn test_parse_match_with_complex_guards() {
    // Test complex guard expressions
    let expr = parse_expression(r#"
        match value {
            x if x > 0 and x < 100 -> "valid range"
            y if y == 0 or y == -1 -> "special cases"
            _ -> "other"
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { arms, .. } => {
            assert_eq!(arms.len(), 3);
            // First arm should have complex logical guard
            assert!(arms[0].guard.is_some());
            if let Some(guard) = &arms[0].guard {
                match guard {
                    Expression::BinaryOp { op: BinaryOperator::And, .. } => {
                        // Success - complex logical expression
                    }
                    _ => panic!("Expected logical AND in guard expression"),
                }
            }
            // Second arm should also have complex guard
            assert!(arms[1].guard.is_some());
            // Third arm (wildcard) should not have guard
            assert!(arms[2].guard.is_none());
        }
        _ => panic!("Expected match expression"),
    }
}

#[test]
fn test_parse_match_with_destructuring_patterns() {
    // Test destructuring patterns like Success(data), Failure(code, msg)
    let expr = parse_expression(r#"
        match result {
            Success { value: data } -> "Got: " + data  
            Failure { code: c, message: msg } if c >= 500 -> "Server error: " + msg
            Failure { message: msg } -> "Error: " + msg
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { arms, .. } => {
            assert_eq!(arms.len(), 3);
            // First arm: Success pattern
            match &arms[0].pattern {
                Pattern::Struct { name, fields } => {
                    assert_eq!(name, "Success");
                    assert_eq!(fields.len(), 1);
                    assert_eq!(fields[0].0, "value");
                }
                _ => panic!("Expected struct pattern for Success"),
            }
            // Second arm: Failure pattern with guard
            match &arms[1].pattern {
                Pattern::Struct { name, fields } => {
                    assert_eq!(name, "Failure");
                    assert_eq!(fields.len(), 2);
                }
                _ => panic!("Expected struct pattern for Failure"),
            }
            assert!(arms[1].guard.is_some()); // Should have guard
            // Third arm: Failure pattern without guard  
            assert!(arms[2].guard.is_none());
        }
        _ => panic!("Expected match expression"),
    }
}

#[test]
fn test_parse_match_with_nested_patterns() {
    // Test nested struct patterns
    let expr = parse_expression(r#"
        match response {
            Response { data: Success { value: x } } -> x
            Response { data: Failure { code: 404 } } -> "Not found"
            _ -> "Unknown"
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { arms, .. } => {
            assert_eq!(arms.len(), 3);
            // First arm should have nested struct pattern
            match &arms[0].pattern {
                Pattern::Struct { name, fields } => {
                    assert_eq!(name, "Response");
                    assert_eq!(fields.len(), 1);
                    // The data field should have nested Success pattern
                    match &*fields[0].1 {
                        Pattern::Struct { name: inner_name, .. } => {
                            assert_eq!(inner_name, "Success");
                        }
                        _ => panic!("Expected nested struct pattern"),
                    }
                }
                _ => panic!("Expected outer struct pattern"),
            }
        }
        _ => panic!("Expected match expression"),
    }
}

#[test]
fn test_parse_match_with_range_patterns() {
    // Test range patterns in match
    let expr = parse_expression(r#"
        match score {
            0..49 -> "F"
            50..69 -> "D" 
            70..79 -> "C"
            80..89 -> "B"
            90..100 -> "A"
            _ -> "Invalid"
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { arms, .. } => {
            assert_eq!(arms.len(), 6);
            // First few arms should have range patterns
            for i in 0..5 {
                match &arms[i].pattern {
                    Pattern::Range { inclusive, .. } => {
                        assert_eq!(*inclusive, true); // Should be inclusive ranges
                    }
                    _ => panic!("Expected range pattern at arm {}", i),
                }
            }
            // Last arm should be wildcard
            match &arms[5].pattern {
                Pattern::Wildcard => {
                    // Success
                }
                _ => panic!("Expected wildcard pattern for last arm"),
            }
        }
        _ => panic!("Expected match expression"),
    }
}

#[test]
fn test_parse_match_with_literal_patterns() {
    // Test literal patterns and mixed pattern types
    let expr = parse_expression(r#"
        match input {
            "hello" -> "greeting"
            42 -> "answer"
            true -> "boolean"
            null -> "empty"
            _ -> "unknown"
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { arms, .. } => {
            assert_eq!(arms.len(), 5);
            
            // String literal pattern
            match &arms[0].pattern {
                Pattern::Literal(expr) => {
                    match expr.as_ref() {
                        Expression::StringLiteral { value, .. } => {
                            assert_eq!(value, "hello");
                        }
                        _ => panic!("Expected string literal in pattern"),
                    }
                }
                _ => panic!("Expected literal pattern"),
            }
            
            // Integer literal pattern
            match &arms[1].pattern {
                Pattern::Literal(expr) => {
                    match expr.as_ref() {
                        Expression::IntegerLiteral { value, .. } => {
                            assert_eq!(*value, 42);
                        }
                        _ => panic!("Expected integer literal in pattern"),
                    }
                }
                _ => panic!("Expected literal pattern"),
            }
            
            // Boolean literal pattern
            match &arms[2].pattern {
                Pattern::Literal(expr) => {
                    match expr.as_ref() {
                        Expression::BooleanLiteral { value, .. } => {
                            assert_eq!(*value, true);
                        }
                        _ => panic!("Expected boolean literal in pattern"),
                    }
                }
                _ => panic!("Expected literal pattern"),
            }
        }
        _ => panic!("Expected match expression"),
    }
}

#[test]
fn test_parse_match_exhaustive_edge_cases() {
    // Test edge cases like multiple guards, empty body, complex expressions
    let expr = parse_expression(r#"
        match computation() {
            result if result > 0 and result % 2 == 0 -> {
                println("positive even")
                result / 2
            }
            result if result > 0 -> {
                println("positive odd") 
                result * 3 + 1
            }
            0 -> 0
            _ -> -1
        }
    "#).unwrap();
    
    match expr {
        Expression::Match { expr: matched_expr, arms, .. } => {
            // Verify the matched expression is a function call
            match matched_expr.as_ref() {
                Expression::Call { .. } => {
                    // Success - matched expression is function call
                }
                _ => panic!("Expected function call as matched expression"),
            }
            
            assert_eq!(arms.len(), 4);
            
            // First arm has complex guard with logical operators
            assert!(arms[0].guard.is_some());
            if let Some(guard) = &arms[0].guard {
                match guard {
                    Expression::BinaryOp { op: BinaryOperator::And, .. } => {
                        // Success - complex guard expression
                    }
                    _ => panic!("Expected AND in complex guard"),
                }
            }
            
            // Second arm also has guard
            assert!(arms[1].guard.is_some());
            
            // Third arm is literal without guard
            assert!(arms[2].guard.is_none());
            match &arms[2].pattern {
                Pattern::Literal(expr) => {
                    match expr.as_ref() {
                        Expression::IntegerLiteral { value, .. } => {
                            assert_eq!(*value, 0);
                        }
                        _ => panic!("Expected integer 0 literal"),
                    }
                }
                _ => panic!("Expected literal pattern for 0"),
            }
            
            // Fourth arm is wildcard
            assert!(arms[3].guard.is_none());
            match &arms[3].pattern {
                Pattern::Wildcard => {
                    // Success
                }
                _ => panic!("Expected wildcard pattern"),
            }
        }
        _ => panic!("Expected match expression"),
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