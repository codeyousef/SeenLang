//! Tests for operator type checking

use super::test_helpers::*;
use crate::Type;

#[cfg(test)]
mod operator_type_tests {
    use super::*;

    #[test]
    fn test_arithmetic_operators_int() {
        let source = r#"
            val add = 1 + 2;
            val sub = 5 - 3;
            val mul = 4 * 2;
            val div = 8 / 4;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "add", Type::Int);
        assert_variable_type(&result, "sub", Type::Int);
        assert_variable_type(&result, "mul", Type::Int);
        assert_variable_type(&result, "div", Type::Int);
    }

    #[test]
    fn test_arithmetic_operators_float() {
        let source = r#"
            val add = 1.5 + 2.5;
            val sub = 5.0 - 3.2;
            val mul = 4.1 * 2.0;
            val div = 8.4 / 2.1;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "add", Type::Float);
        assert_variable_type(&result, "sub", Type::Float);
        assert_variable_type(&result, "mul", Type::Float);
        assert_variable_type(&result, "div", Type::Float);
    }

    #[test]
    fn test_comparison_operators() {
        let source = r#"
            val eq = 5 == 5;
            val neq = 3 != 7;
            val lt = 2 < 4;
            val lte = 3 <= 5;
            val gt = 7 > 2;
            val gte = 6 >= 4;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "eq", Type::Bool);
        assert_variable_type(&result, "neq", Type::Bool);
        assert_variable_type(&result, "lt", Type::Bool);
        assert_variable_type(&result, "lte", Type::Bool);
        assert_variable_type(&result, "gt", Type::Bool);
        assert_variable_type(&result, "gte", Type::Bool);
    }

    #[test]
    fn test_logical_operators() {
        let source = r#"
            val and_op = true && false;
            val or_op = true || false;
            val not_op = !true;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "and_op", Type::Bool);
        assert_variable_type(&result, "or_op", Type::Bool);
        assert_variable_type(&result, "not_op", Type::Bool);
    }

    #[test]
    fn test_mixed_type_arithmetic_error() {
        let source = r#"val result = 5 + "hello";"#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::InvalidOperation)
        });
    }

    #[test]
    fn test_string_comparison() {
        let source = r#"
            val eq = "hello" == "world";
            val neq = "test" != "other";
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "eq", Type::Bool);
        assert_variable_type(&result, "neq", Type::Bool);
    }

    #[test]
    fn test_invalid_logical_operator_operands() {
        let source = r#"val result = 5 && true;"#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::InvalidOperation)
        });
    }

    #[test]
    fn test_unary_minus() {
        let source = r#"
            val neg_int = -42;
            val neg_float = -3.14;
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "neg_int", Type::Int);
        assert_variable_type(&result, "neg_float", Type::Float);
    }

    #[test]
    fn test_invalid_unary_minus() {
        let source = r#"val result = -"hello";"#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::InvalidOperation)
        });
    }

    #[test]
    fn test_mixed_numeric_promotion() {
        let source = r#"val result = 5 + 3.14;"#;
        let result = assert_type_check_ok(source);
        // Integer should be promoted to float when mixed with float
        assert_variable_type(&result, "result", Type::Float);
    }
}