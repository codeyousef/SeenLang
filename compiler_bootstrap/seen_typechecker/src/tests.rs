//! Type checker tests

#[cfg(test)]
mod tests {
    use super::super::*;
    use seen_parser::Parser;
    use seen_lexer::Lexer;
    use seen_common::SeenResult;

    fn check_program(source: &str) -> SeenResult<()> {
        let lang_config = seen_lexer::LanguageConfig::new_english();
        let mut lexer = Lexer::new(source, 0, &lang_config);
        let tokens = lexer.tokenize()?;
        
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_program()?;
        
        let mut type_checker = TypeChecker::new();
        type_checker.check_program(&ast)
    }

    #[test]
    fn test_basic_arithmetic() {
        let source = r#"
            fun add(x: i32, y: i32): i32 {
                return x + y
            }
            
            fun main() {
                val result = add(10, 20)
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }

    #[test]
    fn test_type_mismatch_error() {
        let source = r#"
            fun main() {
                val x: i32 = 42
                val y: str = "hello"
                val z = x + y  // Should fail: can't add i32 and str
            }
        "#;
        
        assert!(check_program(source).is_err());
    }

    #[test]
    fn test_string_operations() {
        let source = r#"
            fun main() {
                val greeting: str = "Hello"
                val name: str = "World"
                val message = greeting + name  // String concatenation should work
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }

    #[test]
    fn test_boolean_operations() {
        let source = r#"
            fun main() {
                val a: bool = true
                val b: bool = false
                val c = a && b
                val d = a || b
                val e = !a
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }

    #[test]
    fn test_comparison_operations() {
        let source = r#"
            fun main() {
                val x: i32 = 10
                val y: i32 = 20
                val less = x < y
                val greater = x > y
                val equal = x == y
                val not_equal = x != y
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }

    #[test]
    fn test_if_else_type_checking() {
        let source = r#"
            fun max(x: i32, y: i32): i32 {
                if (x > y) {
                    return x
                } else {
                    return y
                }
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }

    #[test]
    fn test_function_return_type_mismatch() {
        let source = r#"
            fun get_number(): i32 {
                return "not a number"  // Should fail: returning str instead of i32
            }
        "#;
        
        match check_program(source) {
            Ok(_) => panic!("Expected type error but got success"),
            Err(_) => {} // test passes - we expected this error
        }
    }

    #[test]
    fn test_undefined_variable() {
        let source = r#"
            fun main() {
                val x = unknown_variable  // Should fail: undefined variable
            }
        "#;
        
        assert!(check_program(source).is_err());
    }

    #[test]
    fn test_undefined_function() {
        let source = r#"
            fun main() {
                val result = unknown_function(42)  // Should fail: undefined function
            }
        "#;
        
        assert!(check_program(source).is_err());
    }

    #[test]
    fn test_wrong_number_of_arguments() {
        let source = r#"
            fun add(x: i32, y: i32): i32 {
                return x + y
            }
            
            fun main() {
                val result = add(1, 2, 3)  // Should fail: too many arguments
            }
        "#;
        
        assert!(check_program(source).is_err());
    }

    #[test]
    fn test_primitive_types() {
        let source = r#"
            fun main() {
                val a: i8 = 127
                val b: i16 = 32767
                val c: i32 = 2147483647
                val d: i64 = 9223372036854775807
                val e: u8 = 255
                val f: u16 = 65535
                val g: u32 = 4294967295
                val h: u64 = 18446744073709551615
                val i: f32 = 3.14
                val j: f64 = 3.141592653589793
                val k: bool = true
                val l: str = "hello"
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }

    #[test]
    fn test_array_type_checking() {
        let source = r#"
            fun main() {
                val numbers: [i32] = [1, 2, 3, 4, 5]
                val first = numbers[0]
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }

    #[test]
    fn test_array_type_mismatch() {
        // Test that arrays require consistent element types
        let source = r#"
            fun main() {
                val mixed_array = [1, "two", 3]
            }
        "#;
        
        // This should fail because array elements have different types
        let result = check_program(source);
        assert!(result.is_err(), "Array with mixed types should fail type checking");
        
        // Test that homogeneous arrays work
        let valid_source = r#"
            fun main() {
                val int_array = [1, 2, 3]
                val string_array = ["one", "two", "three"]
            }
        "#;
        
        assert!(check_program(valid_source).is_ok(), "Homogeneous arrays should pass type checking");
    }

    #[test]
    fn test_for_loop_type_checking() {
        let source = r#"
            fun main() {
                for i in 0..10 {
                    val square = i * i
                }
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }

    #[test]
    fn test_while_loop_type_checking() {
        let source = r#"
            fun main() {
                var count = 0
                while (count < 10) {
                    count = count + 1
                }
            }
        "#;
        
        assert!(check_program(source).is_ok());
    }
}