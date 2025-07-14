//! Tests for scope and variable resolution

use super::test_helpers::*;
use crate::Type;

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn test_variable_in_function_scope() {
        let source = r#"
            func test() {
                val x = 42;
                val y = x + 1;
            }
        "#;
        let result = assert_type_check_ok(source);
        // Variables x and y should be accessible within the function
        let func_sig = result.get_function_signature("test").expect("Function not found");
        assert_eq!(func_sig.name, "test");
    }

    #[test]
    fn test_duplicate_variable_in_same_scope() {
        let source = r#"
            val x = 42;
            val x = "hello";
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::DuplicateDefinition)
        });
    }

    #[test]
    fn test_undefined_variable_reference() {
        let source = r#"val y = x + 1;"#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::UndefinedReference)
        });
    }

    #[test]
    fn test_variable_reference_in_correct_order() {
        let source = r#"
            val x = 42;
            val y = x + 1;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Int);
        assert_variable_type(&result, "y", Type::Int);
    }

    #[test]
    fn test_function_parameter_scope() {
        let source = r#"
            func add(a: Int, b: Int) -> Int {
                val sum = a + b;
                return sum;
            }
        "#;
        let _result = assert_type_check_ok(source);
        // Parameters a and b should be accessible within function body
    }

    #[test]
    fn test_duplicate_function_definition() {
        let source = r#"
            func test() -> Int {
                return 1;
            }
            
            func test() -> String {
                return "hello";
            }
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::DuplicateDefinition)
        });
    }

    #[test]
    #[ignore = "Nested block scopes not yet implemented in parser/checker"]
    fn test_nested_block_scope() {
        let source = r#"
            val x = 1;
            {
                val x = 2;
                val y = x;
            }
            val z = x;
        "#;
        let result = assert_type_check_ok(source);
        // Inner x should shadow outer x in the block
        // Outer x should be accessible again after the block
        assert_variable_type(&result, "z", Type::Int);
    }

    #[test]
    #[ignore = "Variable shadowing in nested blocks not yet implemented"]
    fn test_variable_shadowing() {
        let source = r#"
            val x = 42;
            {
                val x = "hello";
                val y = x;
            }
        "#;
        let _result = assert_type_check_ok(source);
        // Should allow variable shadowing in nested scopes
    }

    #[test]
    fn test_global_and_function_variable_separation() {
        let source = r#"
            val global = 42;
            
            func test() {
                val local = global + 1;
                val other = 100;
            }
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "global", Type::Int);
        // Function should be able to access global variables
    }
}