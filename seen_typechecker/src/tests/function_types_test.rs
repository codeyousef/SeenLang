//! Tests for function type checking

use super::test_helpers::*;
use crate::Type;

#[cfg(test)]
mod function_type_tests {
    use super::*;

    #[test]
    fn test_function_declaration_with_return_type() {
        let source = r#"
            func add(a: Int, b: Int) -> Int {
                return a + b;
            }
        "#;
        let result = assert_type_check_ok(source);
        let func_sig = result.get_function_signature("add")
            .expect("Function 'add' not found");
        assert_eq!(func_sig.name, "add");
        assert_eq!(func_sig.parameters.len(), 2);
        assert_eq!(func_sig.return_type, Some(Type::Int));
    }

    #[test]
    fn test_function_return_type_mismatch() {
        let source = r#"
            func getValue() -> Int {
                return "hello";
            }
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::ReturnError)
        });
    }

    #[test]
    fn test_function_call_correct_args() {
        let source = r#"
            func add(a: Int, b: Int) -> Int {
                return a + b;
            }
            
            val result = add(1, 2);
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "result", Type::Int);
    }

    #[test]
    fn test_function_call_wrong_arg_type() {
        let source = r#"
            func add(a: Int, b: Int) -> Int {
                return a + b;
            }
            
            val result = add("hello", 2);
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::TypeMismatch)
        });
    }

    #[test]
    fn test_function_call_wrong_arg_count() {
        let source = r#"
            func add(a: Int, b: Int) -> Int {
                return a + b;
            }
            
            val result = add(1);
        "#;
        let errors = assert_type_check_error(source);
        assert_has_type_error(&errors, |e| {
            matches!(e.kind(), crate::errors::TypeErrorKind::WrongArgumentCount)
        });
    }

    #[test]
    fn test_void_function() {
        let source = r#"
            func printValue(x: Int) {
                println(x);
            }
            
            func main() {
                printValue(42);
            }
        "#;
        let result = assert_type_check_ok(source);
        let func_sig = result.get_function_signature("printValue")
            .expect("Function 'printValue' not found");
        assert_eq!(func_sig.return_type, None);
    }

    #[test]
    fn test_recursive_function() {
        let source = r#"
            func factorial(n: Int) -> Int {
                if n <= 1 {
                    return 1;
                } else {
                    return n * factorial(n - 1);
                }
            }
        "#;
        let result = assert_type_check_ok(source);
        let func_sig = result.get_function_signature("factorial")
            .expect("Function 'factorial' not found");
        assert_eq!(func_sig.return_type, Some(Type::Int));
    }

    #[test]
    fn test_function_as_expression() {
        let source = r#"
            func getValue() -> Int {
                return 42;
            }
            
            val x = getValue();
        "#;
        let result = assert_type_check_ok(source);
        assert_variable_type(&result, "x", Type::Int);
    }
}