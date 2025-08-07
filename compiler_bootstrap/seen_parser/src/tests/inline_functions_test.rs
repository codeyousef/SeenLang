//! Tests for inline function parsing (Kotlin feature)

use crate::parser::Parser;
use crate::ast::*;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_inline_function_basic() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        inline fun repeat(times: Int, action: () -> Unit) {
            action()
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse inline function");
    let ast = ast.unwrap();
    
    assert_eq!(ast.items.len(), 1);
    match &ast.items[0].kind {
        ItemKind::Function(func) => {
            assert!(func.is_inline);
            assert_eq!(func.name.value, "repeat");
            assert_eq!(func.params.len(), 2);
        }
        _ => panic!("Expected inline function"),
    }
}

#[test]
fn test_inline_with_reified() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        inline fun <reified T> isType(value: Any): Boolean {
            return value is T
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse inline function with reified type parameter");
}

#[test]
fn test_crossinline_parameter() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        inline fun runInTransaction(crossinline action: () -> Unit) {
            database.beginTransaction()
            try {
                action()
                database.setTransactionSuccessful()
            } finally {
                database.endTransaction()
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse inline function with crossinline parameter");
}

#[test]
fn test_noinline_parameter() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        inline fun performOperation(
            inlineAction: () -> Unit,
            noinline callback: () -> Unit
        ) {
            inlineAction()
            runAsync(callback)
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse inline function with noinline parameter");
}