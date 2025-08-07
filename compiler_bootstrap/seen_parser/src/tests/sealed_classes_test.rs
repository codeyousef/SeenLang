//! Tests for sealed class parsing (Kotlin feature)

use crate::parser::Parser;
use crate::ast::*;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_sealed_class_basic() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        sealed class Result {
            data class Success(val value: String) : Result()
            data class Error(val message: String) : Result()
            object Loading : Result()
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse sealed class");
    let ast = ast.unwrap();
    
    // Verify we have a sealed class
    assert_eq!(ast.items.len(), 1);
    match &ast.items[0].kind {
        ItemKind::SealedClass(class) => {
            assert_eq!(class.name.value, "Result");
            assert_eq!(class.variants.len(), 3); // 3 subclasses
        }
        _ => panic!("Expected sealed class"),
    }
}

#[test]
fn test_sealed_class_when_expression() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        fun processResult(result: Result): String {
            return when (result) {
                is Result.Success -> result.value
                is Result.Error -> "Error: ${result.message}"
                Result.Loading -> "Loading..."
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse when expression with sealed class");
}

#[test]
fn test_sealed_interface() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        sealed interface State {
            object Idle : State
            data class Running(val progress: Int) : State
            data class Finished(val result: String) : State
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse sealed interface");
}

#[test]
fn test_sealed_class_hierarchy() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        sealed class Operation {
            sealed class Binary : Operation() {
                data class Add(val left: Int, val right: Int) : Binary()
                data class Multiply(val left: Int, val right: Int) : Binary()
            }
            
            sealed class Unary : Operation() {
                data class Negate(val value: Int) : Unary()
                data class Abs(val value: Int) : Unary()
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse nested sealed class hierarchy");
}