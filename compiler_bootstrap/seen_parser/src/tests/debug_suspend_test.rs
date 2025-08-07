//! Debug test for suspend function parsing

use crate::parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};
use crate::ast::*;

fn setup_parser(code: &str) -> Parser {
    let lang_config = LanguageConfig::new_english();
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().expect("Failed to tokenize test input");
    Parser::new(tokens)
}

#[cfg(test)]
mod debug_suspend_tests {
    use super::*;

    #[test]
    fn test_simple_suspend_function() {
        let code = r#"
            suspend fun simple(): String {
                return "test"
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse simple suspend function");

        assert_eq!(program.items.len(), 1);
        
        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "simple");
                // Should have suspend attribute
                assert_eq!(func.attributes.len(), 1);
                assert_eq!(func.attributes[0].name.value, "suspend");
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_suspend_function_simple_generic() {
        let code = r#"
            suspend fun simple(): Observable<String> {
                return Observable.empty()
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse suspend with simple generic");

        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn test_nested_generic_types() {
        let code = r#"
            fun nested(): Observable<List<String>> {
                return Observable.empty()
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse nested generics");

        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn test_suspend_with_nested_generics() {
        let code = r#"
            suspend fun fetchUsers(): Observable<List<User>> {
                return Observable.empty()
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse suspend with nested generics");

        assert_eq!(program.items.len(), 1);
    }
}