//! Tests for object expressions and companion objects (Kotlin features)

use crate::parser::Parser;
use crate::ast::*;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_object_expression_basic() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        val listener = object : ClickListener {
            fun onClick(view: View) {
                println("Clicked!")
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse object expression");
}

#[test]
fn test_companion_object() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        class MyClass {
            companion object {
                val CONSTANT = 42
                fun factory(): MyClass {
                    return MyClass()
                }
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse companion object");
}

#[test]
fn test_named_companion_object() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        class Person(val name: String) {
            companion object Factory {
                fun create(name: String): Person {
                    return Person(name)
                }
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse named companion object");
}

#[test]
fn test_object_expression_with_multiple_interfaces() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        val handler = object : MouseListener, KeyListener {
            fun onMouseClick(x: Int, y: Int) {
                println("Mouse clicked at $x, $y")
            }
            
            fun onKeyPress(key: Char) {
                println("Key pressed: $key")
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse object expression with multiple interfaces");
}

#[test]
fn test_anonymous_object() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        val point = object {
            val x = 10
            val y = 20
            
            fun distance(): Double {
                return sqrt(x * x + y * y)
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse anonymous object");
}