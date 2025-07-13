//! Tests for struct type checking

use super::test_helpers::*;
use crate::Type;

#[cfg(test)]
mod struct_type_tests {
    use super::*;

    #[test]
    fn test_struct_declaration() {
        let source = r#"
            struct Point {
                x: Int,
                y: Int
            }
        "#;
        let result = assert_type_check_ok(source);
        // Struct type should be registered
        // Note: We may need to add a method to check struct types in TypeCheckResult
    }

    #[test]
    fn test_struct_instantiation() {
        let source = r#"
            struct Point {
                x: Int,
                y: Int
            }
            
            val p = Point { x: 10, y: 20 };
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "p", Type::Struct("Point".to_string()));
    }

    #[test]
    fn test_struct_missing_field() {
        let source = r#"
            struct Point {
                x: Int,
                y: Int
            }
            
            val p = Point { x: 10 };
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::MissingField)
        });
    }

    #[test]
    fn test_struct_extra_field() {
        let source = r#"
            struct Point {
                x: Int,
                y: Int
            }
            
            val p = Point { x: 10, y: 20, z: 30 };
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::UnknownField)
        });
    }

    #[test]
    fn test_struct_wrong_field_type() {
        let source = r#"
            struct Point {
                x: Int,
                y: Int
            }
            
            val p = Point { x: "hello", y: 20 };
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }

    #[test]
    fn test_struct_field_access() {
        let source = r#"
            struct Point {
                x: Int,
                y: Int
            }
            
            val p = Point { x: 10, y: 20 };
            val x_coord = p.x;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x_coord", Type::Int);
    }

    #[test]
    fn test_struct_unknown_field_access() {
        let source = r#"
            struct Point {
                x: Int,
                y: Int
            }
            
            val p = Point { x: 10, y: 20 };
            val z_coord = p.z;
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::UnknownField)
        });
    }

    #[test]
    fn test_nested_struct() {
        let source = r#"
            struct Point {
                x: Int,
                y: Int
            }
            
            struct Line {
                start: Point,
                end: Point
            }
            
            val line = Line {
                start: Point { x: 0, y: 0 },
                end: Point { x: 10, y: 10 }
            };
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "line", Type::Struct("Line".to_string()));
    }

    #[test]
    fn test_struct_with_array_field() {
        let source = r#"
            struct Data {
                values: [Int]
            }
            
            val data = Data { values: [1, 2, 3] };
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "data", Type::Struct("Data".to_string()));
    }
}