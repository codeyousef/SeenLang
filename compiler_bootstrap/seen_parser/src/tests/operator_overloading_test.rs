//! Tests for operator overloading and infix functions (Kotlin features)

use crate::parser::Parser;
use crate::ast::*;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_operator_overloading_plus() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        class Vector(val x: Double, val y: Double) {
            operator fun plus(other: Vector): Vector {
                return Vector(x + other.x, y + other.y)
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse operator overloading");
}

#[test]
fn test_operator_overloading_get_set() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        class Matrix(val rows: Int, val cols: Int) {
            operator fun get(row: Int, col: Int): Double {
                return data[row * cols + col]
            }
            
            operator fun set(row: Int, col: Int, value: Double) {
                data[row * cols + col] = value
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse get/set operator overloading");
}

#[test]
fn test_infix_function() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        class Point(val x: Int, val y: Int) {
            infix fun distanceTo(other: Point): Double {
                val dx = x - other.x
                val dy = y - other.y
                return sqrt(dx * dx + dy * dy)
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse infix function");
}

#[test]
fn test_infix_function_usage() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        fun main() {
            val p1 = Point(0, 0)
            val p2 = Point(3, 4)
            val dist = p1 distanceTo p2
            println(dist)
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse infix function usage");
}

#[test]
fn test_operator_overloading_comparison() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        class Version(val major: Int, val minor: Int) {
            operator fun compareTo(other: Version): Int {
                return when {
                    major != other.major -> major - other.major
                    else -> minor - other.minor
                }
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse comparison operator overloading");
}

#[test]
fn test_invoke_operator() {
    let lang_config = LanguageConfig::new_english();
    let code = r#"
        class Greeter(val greeting: String) {
            operator fun invoke(name: String) {
                println("$greeting, $name!")
            }
        }
    "#;
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program();
    
    assert!(ast.is_ok(), "Failed to parse invoke operator");
}