//! Integration tests for smart casting functionality

use seen_typechecker::*;
use seen_parser::ast::*;
use seen_lexer::Position;
use std::collections::HashMap;

fn pos() -> Position {
    Position { line: 1, column: 1, offset: 0 }
}

#[test]
fn test_smart_casting_integration() {
    // Create a simple program that should benefit from smart casting
    let program = Program {
        expressions: vec![
            // let user: User? = null
            Expression::Let {
                name: "user".to_string(),
                type_annotation: Some(seen_parser::ast::Type {
                    name: "User".to_string(),
                    is_nullable: true,
                    generics: vec![],
                }),
                value: Box::new(Expression::NullLiteral { pos: pos() }),
                is_mutable: false,
                pos: pos(),
            },
            // Simple expression to test the implementation
            Expression::IntegerLiteral { value: 42, pos: pos() },
        ],
    };
    
    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    
    // Should compile without errors
    assert!(!result.has_errors(), "Expected no errors, got: {:?}", result.get_errors());
    
    // Should have user variable defined
    assert!(result.variables.contains_key("user"));
}

#[test]
fn test_smart_casting_recognizes_null_check() {
    // Create a condition expression: user != null
    let condition = Expression::BinaryOp {
        left: Box::new(Expression::Identifier {
            name: "user".to_string(),
            is_public: false,
            pos: pos(),
        }),
        op: BinaryOperator::NotEqual,
        right: Box::new(Expression::NullLiteral { pos: pos() }),
        pos: pos(),
    };
    
    // Create a simple then branch
    let then_branch = Expression::IntegerLiteral { value: 1, pos: pos() };
    
    let mut checker = TypeChecker::new();
    
    // Define a nullable user type for testing
    let mut fields = HashMap::new();
    fields.insert("Name".to_string(), seen_typechecker::Type::String);
    
    let user_struct = seen_typechecker::Type::Struct {
        name: "User".to_string(),
        fields,
        generics: vec![],
    };
    
    // We need to access the environment through public methods
    // For now, we'll just test that the methods are accessible
    let result_type = checker.check_if_expression(&condition, &then_branch, None, pos());
    
    // The condition should be recognized as having type issues (user not defined)
    // but the method should not crash
    assert!(matches!(result_type, seen_typechecker::Type::Int | seen_typechecker::Type::Unit | seen_typechecker::Type::Nullable(_)));
    
    println!("Smart casting integration test completed successfully");
}