//! Tests for array type checking

use super::test_helpers::*;
use crate::Type;

#[cfg(test)]
mod array_type_tests {
    use super::*;

    #[test]
    fn test_array_literal_inference() {
        let source = "val arr = [1, 2, 3];";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "arr", Type::Array(Box::new(Type::Int)));
    }

    #[test]
    fn test_empty_array_with_type_annotation() {
        let source = "val arr: [Int] = [];";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "arr", Type::Array(Box::new(Type::Int)));
    }

    #[test]
    fn test_nested_array() {
        let source = "val matrix = [[1, 2], [3, 4]];";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "matrix", Type::Array(Box::new(Type::Array(Box::new(Type::Int)))));
    }

    #[test]
    fn test_mixed_array_type_error() {
        let source = r#"val arr = [1, "hello", 3];"#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }

    #[test]
    fn test_array_with_float_elements() {
        let source = "val arr = [1.0, 2.5, 3.14];";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "arr", Type::Array(Box::new(Type::Float)));
    }

    #[test]
    fn test_array_with_string_elements() {
        let source = r#"val arr = ["hello", "world"];"#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "arr", Type::Array(Box::new(Type::String)));
    }

    #[test]
    fn test_array_with_boolean_elements() {
        let source = "val arr = [true, false, true];";
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "arr", Type::Array(Box::new(Type::Bool)));
    }

    #[test]
    #[ignore = "Array indexing not yet implemented in parser/checker"]
    fn test_array_indexing() {
        let source = r#"
            val arr = [1, 2, 3];
            val first = arr[0];
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "first", Type::Int);
    }

    #[test] 
    #[ignore = "Array indexing not yet implemented in parser/checker"]
    fn test_array_index_type_error() {
        let source = r#"
            val arr = [1, 2, 3];
            val item = arr["hello"];
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }
}