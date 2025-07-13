//! Tests for primitive type checking

use super::test_helpers::*;
use crate::Type;

#[cfg(test)]
mod primitive_type_tests {
    use super::*;

    #[test]
    fn test_integer_literal() {
        let source = "val x = 42;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Int);
    }

    #[test]
    fn test_float_literal() {
        let source = "val x = 3.14;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Float);
    }

    #[test]
    fn test_boolean_literal_true() {
        let source = "val x = true;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Bool);
    }

    #[test]
    fn test_boolean_literal_false() {
        let source = "val x = false;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Bool);
    }

    #[test]
    fn test_string_literal() {
        let source = r#"val x = "hello";"#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::String);
    }

    #[test]
    fn test_explicit_type_annotation_match() {
        let source = "val x: Int = 42;";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Int);
    }

    #[test]
    fn test_explicit_type_annotation_mismatch() {
        let source = r#"val x: Int = "hello";"#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }

    #[test]
    fn test_multiple_variables() {
        let source = r#"
            val a = 42;
            val b = 3.14;
            val c = true;
            val d = "hello";
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "a", Type::Int);
        assert_variable_type(&result, "b", Type::Float);
        assert_variable_type(&result, "c", Type::Bool);
        assert_variable_type(&result, "d", Type::String);
    }
}