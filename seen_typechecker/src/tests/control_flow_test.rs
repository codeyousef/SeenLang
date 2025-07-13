//! Tests for control flow type checking

use super::test_helpers::*;
use crate::Type;

#[cfg(test)]
mod control_flow_tests {
    use super::*;

    #[test]
    #[ignore = "If statements not yet implemented in parser/checker"]
    fn test_if_condition_type_check() {
        let source = r#"
            if 42 {
                val x = 1;
            }
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }

    #[test]
    #[ignore = "If statements not yet implemented in parser/checker"]
    fn test_if_condition_bool_ok() {
        let source = r#"
            if true {
                val x = 1;
            }
        "#;
        let _result = assert_type_check_ok(source);
    }

    #[test]
    #[ignore = "If expressions not yet implemented in parser/checker"]
    fn test_if_branches_type_consistency() {
        let source = r#"val x = if true { 1 } else { "hello" };"#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }

    #[test]
    #[ignore = "If expressions not yet implemented in parser/checker"]
    fn test_if_branches_consistent_types() {
        let source = r#"val x = if true { 1 } else { 2 };"#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Int);
    }

    #[test]
    #[ignore = "While loops not yet implemented in parser/checker"]
    fn test_while_condition_type_check() {
        let source = r#"
            while "hello" {
                val x = 1;
            }
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }

    #[test]
    #[ignore = "While loops not yet implemented in parser/checker"]
    fn test_while_condition_bool_ok() {
        let source = r#"
            while true {
                val x = 1;
            }
        "#;
        let _result = assert_type_check_ok(source);
    }

    #[test]
    #[ignore = "For loops not yet implemented in parser/checker"]
    fn test_for_in_loop_type_check() {
        let source = r#"
            for x in [1, 2, 3] {
                val y = x + 1;
            }
        "#;
        let _result = assert_type_check_ok(source);
        // x should have type Int in loop body
    }

    #[test]
    #[ignore = "For loops not yet implemented in parser/checker"]
    fn test_for_in_loop_invalid_iterable() {
        let source = r#"
            for x in 42 {
                val y = x;
            }
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }
}