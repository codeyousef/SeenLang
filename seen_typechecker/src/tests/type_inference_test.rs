//! Tests for type inference

use super::test_helpers::*;
use crate::Type;

#[cfg(test)]
mod type_inference_tests {
    use super::*;
    #[test]
    fn test_infer_from_integer_literal() {
        let source = "val x = 42;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Int);
    }

    #[test]
    fn test_infer_from_arithmetic_expression() {
        let source = "val x = 1 + 2;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Int);
    }

    #[test]
    fn test_infer_from_float_arithmetic() {
        let source = "val x = 1.5 + 2.5;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Float);
    }

    #[test]
    fn test_infer_from_boolean_expression() {
        let source = "val x = true && false;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Bool);
    }

    #[test]
    fn test_infer_from_comparison() {
        let source = "val x = 5 > 3;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Bool);
    }

    #[test]
    fn test_infer_array_type() {
        let source = "val arr = [1, 2, 3];";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "arr", Type::Array(Box::new(Type::Int)));
    }

    #[test]
    fn test_infer_empty_array_with_annotation() {
        let source = "val arr: [Int] = [];";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "arr", Type::Array(Box::new(Type::Int)));
    }

    #[test]
    fn test_mixed_array_type_error() {
        let source = r#"val arr = [1, "hello", 3];"#;
        let errors = assert_type_check_error(source);
        assert!(!errors.is_empty(), "Expected type error for mixed array types");
    }

    #[test]
    fn test_infer_from_variable_reference() {
        let source = r#"
            val a = 42;
            val b = a;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "a", Type::Int);
        assert_variable_type(&result, "b", Type::Int);
    }

    #[test]
    fn test_infer_from_complex_expression() {
        let source = r#"
            val a = 10;
            val b = 20;
            val c = (a + b) * 2;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "c", Type::Int);
    }
}