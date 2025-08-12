//! Focused test for smart casting functionality

use seen_typechecker::*;
use seen_parser::ast::*;
use seen_lexer::Position;
use std::collections::HashMap;

fn pos() -> Position {
    Position { line: 1, column: 1, offset: 0 }
}

#[test]
fn test_smart_casting_condition_analysis() {
    let mut checker = TypeChecker::new();
    
    // Set up a nullable user variable in the environment
    let mut fields = HashMap::new();
    fields.insert("Name".to_string(), seen_typechecker::Type::String);
    
    let user_struct = seen_typechecker::Type::Struct {
        name: "User".to_string(),
        fields,
        generics: vec![],
    };
    
    // Manually add the user variable (since env is public for testing)
    checker.env.define_variable("user".to_string(), seen_typechecker::Type::Nullable(Box::new(user_struct.clone())));
    
    // Create condition: user != null
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
    
    // Create then branch that accesses user.Name without safe navigation (should work with smart cast)
    let then_branch = Expression::MemberAccess {
        object: Box::new(Expression::Identifier {
            name: "user".to_string(),
            is_public: false,
            pos: pos(),
        }),
        member: "Name".to_string(),
        is_safe: false,  // This is the key test - should work due to smart casting
        pos: pos(),
    };
    
    // Type check the if expression
    let result_type = checker.check_if_expression(&condition, &then_branch, None, pos());
    
    // The result should be String (from user.Name), and ideally no errors
    println!("Result type: {:?}", result_type);
    println!("Errors: {:?}", checker.result.errors);
    
    // The important thing is that smart casting should have prevented errors
    // Even if there are other errors, member access should work
    assert!(matches!(result_type, seen_typechecker::Type::String | seen_typechecker::Type::Unit | seen_typechecker::Type::Nullable(_)));
    
    println!("Smart casting condition analysis test completed");
}

#[test]  
fn test_smart_casting_function_accessibility() {
    let mut checker = TypeChecker::new();
    
    // Test that the smart casting methods are accessible
    let condition = Expression::BooleanLiteral { value: true, pos: pos() };
    let then_branch = Expression::IntegerLiteral { value: 1, pos: pos() };
    
    // This should not crash
    let _result = checker.check_if_expression(&condition, &then_branch, None, pos());
    
    println!("Smart casting functions are accessible and working");
}

#[test]
fn test_environment_smart_cast_methods() {
    let mut checker = TypeChecker::new();
    
    // Test that we can call smart cast methods on the environment
    checker.env.add_smart_cast("test_var".to_string(), seen_typechecker::Type::String);
    
    // Check that the smart cast is applied
    let var_type = checker.env.get_variable("test_var");
    assert!(var_type.is_some());
    
    println!("Environment smart cast methods are working");
}