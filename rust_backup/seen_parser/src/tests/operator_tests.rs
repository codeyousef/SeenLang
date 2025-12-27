//! Tests for operator parsing (binary, unary, nullable)

use crate::{
    precedence, AssignmentOperator, BinaryOperator, Expression, ParseResult, Parser, UnaryOperator,
};
use seen_lexer::{KeywordManager, Lexer};
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
fn test_parse_binary_addition() {
    let expr = parse_expression("10 + 20").unwrap();
    match expr {
        Expression::BinaryOp {
            left, op, right, ..
        } => {
            assert_eq!(op, BinaryOperator::Add);
            match left.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 10),
                _ => panic!("Expected integer literal"),
            }
            match right.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 20),
                _ => panic!("Expected integer literal"),
            }
        }
        _ => panic!("Expected binary operation"),
    }
}

#[test]
fn test_parse_binary_precedence() {
    let expr = parse_expression("2 + 3 * 4").unwrap();
    match expr {
        Expression::BinaryOp { op, right, .. } => {
            assert_eq!(op, BinaryOperator::Add);
            // Right side should be 3 * 4
            match right.as_ref() {
                Expression::BinaryOp { op, .. } => {
                    assert_eq!(*op, BinaryOperator::Multiply);
                }
                _ => panic!("Expected multiplication on right side"),
            }
        }
        _ => panic!("Expected binary operation"),
    }
}

#[test]
fn test_parse_comparison_chain() {
    let expr = parse_expression("x >= 10 and x <= 20").unwrap();
    match expr {
        Expression::BinaryOp { op, .. } => {
            assert_eq!(op, BinaryOperator::And);
        }
        _ => panic!("Expected binary operation with 'and'"),
    }
}

#[test]
fn test_parse_word_operator_and() {
    let expr = parse_expression("isValid and isEnabled").unwrap();
    match expr {
        Expression::BinaryOp { op, .. } => {
            assert_eq!(op, BinaryOperator::And);
        }
        _ => panic!("Expected 'and' operator"),
    }
}

#[test]
fn test_parse_word_operator_or() {
    let expr = parse_expression("isAdmin or hasPermission").unwrap();
    match expr {
        Expression::BinaryOp { op, .. } => {
            assert_eq!(op, BinaryOperator::Or);
        }
        _ => panic!("Expected 'or' operator"),
    }
}

#[test]
fn test_parse_word_operator_not() {
    let expr = parse_expression("not isActive").unwrap();
    match expr {
        Expression::UnaryOp { op, .. } => {
            assert_eq!(op, UnaryOperator::Not);
        }
        _ => panic!("Expected 'not' operator"),
    }
}

#[test]
fn test_binary_operator_precedence_table() {
    assert!(BinaryOperator::And.precedence() > BinaryOperator::Or.precedence());
    assert!(BinaryOperator::Add.precedence() > BinaryOperator::And.precedence());
    assert!(BinaryOperator::Multiply.precedence() > BinaryOperator::Add.precedence());
    assert_eq!(
        BinaryOperator::InclusiveRange.precedence(),
        precedence::RANGE
    );
    assert_eq!(BinaryOperator::Or.precedence(), precedence::LOGICAL_OR);
}

#[test]
fn test_binary_operator_symbols() {
    assert_eq!(BinaryOperator::Add.symbol(), "+");
    assert_eq!(BinaryOperator::InclusiveRange.symbol(), "..");
    assert_eq!(BinaryOperator::ExclusiveRange.symbol(), "..<");
    assert_eq!(BinaryOperator::And.symbol(), "and");
}

#[test]
fn test_parse_complex_logical_expression() {
    let expr =
        parse_expression("age >= 18 and (hasLicense or isSupervised) and not isDrunk").unwrap();
    match expr {
        Expression::BinaryOp { op, .. } => {
            assert_eq!(op, BinaryOperator::And);
        }
        _ => panic!("Expected complex logical expression"),
    }
}

#[test]
fn test_parse_unary_negation() {
    let expr = parse_expression("-42").unwrap();
    match expr {
        Expression::UnaryOp { op, operand, .. } => {
            assert_eq!(op, UnaryOperator::Negate);
            match operand.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 42),
                _ => panic!("Expected integer literal"),
            }
        }
        _ => panic!("Expected unary negation"),
    }
}

#[test]
fn test_parse_safe_navigation() {
    let expr = parse_expression("user?.name").unwrap();
    match expr {
        Expression::MemberAccess {
            member, is_safe, ..
        } => {
            assert_eq!(member, "name");
            assert!(is_safe);
        }
        _ => panic!("Expected safe navigation"),
    }
}

#[test]
fn test_parse_elvis_operator() {
    let expr = parse_expression("userName ?: \"Guest\"").unwrap();
    match expr {
        Expression::Elvis {
            nullable, default, ..
        } => {
            match nullable.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "userName"),
                _ => panic!("Expected identifier"),
            }
            match default.as_ref() {
                Expression::StringLiteral { value, .. } => assert_eq!(value, "Guest"),
                _ => panic!("Expected string literal"),
            }
        }
        _ => panic!("Expected elvis operator"),
    }
}

#[test]
fn test_parse_force_unwrap() {
    let expr = parse_expression("maybeValue!!").unwrap();
    match expr {
        Expression::ForceUnwrap { nullable, .. } => match nullable.as_ref() {
            Expression::Identifier { name, .. } => assert_eq!(name, "maybeValue"),
            _ => panic!("Expected identifier"),
        },
        _ => panic!("Expected force unwrap"),
    }
}

#[test]
fn test_parse_chained_nullable_operators() {
    let expr = parse_expression("user?.profile?.name ?: \"Unknown\"").unwrap();
    match expr {
        Expression::Elvis { nullable, .. } => {
            // The nullable part should be a chain of safe navigations
            match nullable.as_ref() {
                Expression::MemberAccess { is_safe, .. } => {
                    assert!(is_safe);
                }
                _ => panic!("Expected safe navigation chain"),
            }
        }
        _ => panic!("Expected elvis operator with chained safe navigation"),
    }
}

#[test]
fn test_parse_is_operator() {
    let expr = parse_expression("value is FooType").unwrap();
    match expr {
        Expression::TypeCheck {
            expr, target_type, ..
        } => {
            match expr.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "value"),
                _ => panic!("expected identifier on lhs of is expression"),
            }
            assert_eq!(target_type.name, "FooType");
            assert!(!target_type.is_nullable);
        }
        _ => panic!("expected type-check expression"),
    }
}

#[test]
fn test_is_operator_precedence_with_and() {
    let expr = parse_expression("value is FooType and hasFoo").unwrap();
    match expr {
        Expression::BinaryOp { op, left, .. } => {
            assert_eq!(op, BinaryOperator::And);
            match left.as_ref() {
                Expression::TypeCheck { target_type, .. } => {
                    assert_eq!(target_type.name, "FooType")
                }
                _ => panic!("expected type-check on left side"),
            }
        }
        _ => panic!("expected binary and expression"),
    }
}

#[test]
fn test_parse_inclusive_range() {
    let expr = parse_expression("1..10").unwrap();
    match expr {
        Expression::BinaryOp {
            op, left, right, ..
        } => {
            assert_eq!(op, BinaryOperator::InclusiveRange);
            match left.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 1),
                _ => panic!("Expected integer literal"),
            }
            match right.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 10),
                _ => panic!("Expected integer literal"),
            }
        }
        _ => panic!("Expected inclusive range"),
    }
}

#[test]
fn test_parse_exclusive_range() {
    let expr = parse_expression("0..<10").unwrap();
    match expr {
        Expression::BinaryOp {
            op, left, right, ..
        } => {
            assert_eq!(op, BinaryOperator::ExclusiveRange);
            match left.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 0),
                _ => panic!("Expected integer literal"),
            }
            match right.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 10),
                _ => panic!("Expected integer literal"),
            }
        }
        _ => panic!("Expected exclusive range"),
    }
}

#[test]
fn test_parse_assignment() {
    let expr = parse_expression("x = 42").unwrap();
    match expr {
        Expression::Assignment {
            target, value, op, ..
        } => {
            assert_eq!(op, AssignmentOperator::Assign);
            match target.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "x"),
                _ => panic!("Expected identifier as target"),
            }
            match value.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 42),
                _ => panic!("Expected integer literal"),
            }
        }
        _ => panic!("Expected assignment"),
    }
}

#[test]
fn test_parse_compound_assignment() {
    let expr = parse_expression("result += value").unwrap();
    match expr {
        Expression::Assignment {
            target, value, op, ..
        } => {
            assert_eq!(op, AssignmentOperator::AddAssign);
            match target.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "result"),
                _ => panic!("Expected identifier as target"),
            }
            match value.as_ref() {
                Expression::Identifier { name, .. } => assert_eq!(name, "value"),
                _ => panic!("Expected identifier as value"),
            }
        }
        _ => panic!("Expected assignment"),
    }
}

#[test]
fn test_parse_let_binding() {
    let expr = parse_expression("let x = 10").unwrap();
    match expr {
        Expression::Let {
            name,
            value,
            is_mutable,
            type_annotation,
            ..
        } => {
            assert_eq!(name, "x");
            assert!(!is_mutable);
            assert!(type_annotation.is_none());
            match value.as_ref() {
                Expression::IntegerLiteral { value, .. } => assert_eq!(*value, 10),
                _ => panic!("Expected integer literal"),
            }
        }
        _ => panic!("Expected let binding"),
    }
}

#[test]
fn test_parse_var_binding() {
    let expr = parse_expression("var count: Int = 0").unwrap();
    match expr {
        Expression::Let {
            name,
            is_mutable,
            type_annotation,
            ..
        } => {
            assert_eq!(name, "count");
            assert!(is_mutable);
            assert!(type_annotation.is_some());
        }
        _ => panic!("Expected var binding"),
    }
}
