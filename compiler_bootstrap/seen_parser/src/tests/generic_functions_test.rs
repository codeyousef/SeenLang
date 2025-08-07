//! Test generic function parsing

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
mod generic_function_tests {
    use super::*;

    #[test]
    fn test_simple_generic_function() {
        let code = r#"
            fun identity<T>(value: T): T {
                return value
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse generic function");

        assert_eq!(program.items.len(), 1);

        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "identity");
                assert_eq!(func.type_params.len(), 1);
                assert_eq!(func.type_params[0].name.value, "T");
            }
            _ => panic!("Expected function declaration"),
        }
    }

    #[test]
    fn test_generic_function_with_multiple_params() {
        let code = r#"
            fun combine<T, U>(first: T, second: U): String {
                return first.toString() + second.toString()
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse multi-generic function");

        assert_eq!(program.items.len(), 1);

        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "combine");
                assert_eq!(func.type_params.len(), 2);
                assert_eq!(func.type_params[0].name.value, "T");
                assert_eq!(func.type_params[1].name.value, "U");
            }
            _ => panic!("Expected function declaration"),
        }
    }

    #[test]
    fn test_generic_function_with_constraints() {
        let code = r#"
            fun serialize<T: Serializable>(value: T): String {
                return value.serialize()
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse constrained generic function");

        assert_eq!(program.items.len(), 1);

        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "serialize");
                assert_eq!(func.type_params.len(), 1);
                assert_eq!(func.type_params[0].name.value, "T");
                // Check constraint bounds
                assert_eq!(func.type_params[0].bounds.len(), 1);
            }
            _ => panic!("Expected function declaration"),
        }
    }
}