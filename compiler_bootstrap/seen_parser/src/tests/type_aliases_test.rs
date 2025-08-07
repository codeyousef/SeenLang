//! Tests for type aliases (Kotlin feature)

use crate::parser::Parser;
use crate::ast::*;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_simple_type_alias() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        typealias StringMap = Map<String, String>
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse type alias");
    let program = ast.unwrap();
    assert_eq!(program.items.len(), 1);
    
    match &program.items[0].kind {
        ItemKind::TypeAlias(alias) => {
            assert_eq!(alias.name.value, "StringMap");
        }
        _ => panic!("Expected type alias"),
    }
}

#[test]
fn test_generic_type_alias() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        typealias Predicate<T> = (T) -> Boolean
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse generic type alias");
}

#[test]
fn test_nested_type_alias() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        typealias NodeSet = Set<Node>
        typealias Graph = Map<Node, NodeSet>
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse nested type aliases");
    let program = ast.unwrap();
    assert_eq!(program.items.len(), 2);
}

#[test]
fn test_function_type_alias() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        typealias ClickHandler = (MouseEvent) -> Unit
        typealias Callback<T> = (T) -> Unit
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse function type aliases");
}

#[test]
fn test_suspend_function_type_alias() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        typealias SuspendCallback = suspend () -> Unit
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse suspend function type alias");
}