//! Test Flow DSL builder syntax parsing

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
mod flow_dsl_tests {
    use super::*;

    #[test]
    fn test_simple_flow_dsl() {
        let code = r#"
            fun createFlow() {
                val myFlow = flow {
                    emit("hello")
                    emit("world")
                }
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse simple flow DSL");

        assert_eq!(program.items.len(), 1);

        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "createFlow");
                // The flow expression should be in the function body
                assert_eq!(func.body.statements.len(), 1);
            }
            _ => panic!("Expected function declaration"),
        }
    }

    #[test]
    fn test_suspend_function_with_flow() {
        let code = r#"
            suspend fun fetchData(): Flow<String> {
                return flow {
                    emit("data1")
                    emit("data2")
                }
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse suspend function with flow");

        assert_eq!(program.items.len(), 1);

        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "fetchData");
                assert_eq!(func.attributes.len(), 1);
                assert_eq!(func.attributes[0].name.value, "suspend");
            }
            _ => panic!("Expected function declaration"),
        }
    }

    #[test] 
    fn test_flow_with_await() {
        let code = r#"
            fun createCombinedFlow() {
                val combined = flow {
                    val result = await someAsyncCall()
                    emit(result)
                }
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse flow with await");

        assert_eq!(program.items.len(), 1);
    }
}