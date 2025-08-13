//! Tests for interface/trait parsing

use crate::{Parser, Expression, Program};
use seen_lexer::{Lexer, KeywordManager};
use std::sync::Arc;

fn parse_program(input: &str) -> Result<Program, crate::ParseError> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_program()
}

fn parse_top_level_item(input: &str) -> Result<Expression, crate::ParseError> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_top_level_item()
}

// Interface Definition Tests (following Syntax Design spec)

#[test]
fn test_parse_simple_interface() {
    let expr = parse_top_level_item("interface Drawable {}").unwrap();
    
    match expr {
        Expression::Interface { name, methods, .. } => {
            assert_eq!(name, "Drawable");
            assert_eq!(methods.len(), 0);
        }
        other => panic!("Expected interface definition, got: {:?}", other),
    }
}

#[test]
fn test_parse_interface_with_method() {
    let expr = parse_top_level_item(r#"
        interface Drawable {
            fun Draw(canvas: Canvas)
        }
    "#).unwrap();
    
    match expr {
        Expression::Interface { name, methods, .. } => {
            assert_eq!(name, "Drawable");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "Draw");
            assert_eq!(methods[0].params.len(), 1);
            assert_eq!(methods[0].params[0].name, "canvas");
            assert!(!methods[0].is_default); // Interface methods without body are not default
            assert!(methods[0].default_impl.is_none()); // Interface methods have no default implementation
        }
        _ => panic!("Expected interface definition"),
    }
}

#[test]
fn test_parse_interface_multiple_methods() {
    let expr = parse_top_level_item(r#"
        interface Logger {
            fun Log(message: String)
            fun Debug(level: Int, message: String)
            fun Error(error: String)
        }
    "#).unwrap();
    
    match expr {
        Expression::Interface { name, methods, .. } => {
            assert_eq!(name, "Logger");
            assert_eq!(methods.len(), 3);
            
            // Check Log method
            assert_eq!(methods[0].name, "Log");
            assert_eq!(methods[0].params.len(), 1);
            assert_eq!(methods[0].params[0].name, "message");
            
            // Check Debug method  
            assert_eq!(methods[1].name, "Debug");
            assert_eq!(methods[1].params.len(), 2);
            assert_eq!(methods[1].params[0].name, "level");
            assert_eq!(methods[1].params[1].name, "message");
            
            // Check Error method
            assert_eq!(methods[2].name, "Error");
            assert_eq!(methods[2].params.len(), 1);
            assert_eq!(methods[2].params[0].name, "error");
        }
        _ => panic!("Expected interface definition"),
    }
}

#[test]
fn test_parse_interface_with_return_types() {
    let expr = parse_top_level_item(r#"
        interface Repository {
            fun GetUser(id: Int): User?
            fun SaveUser(user: User): Bool
            fun DeleteUser(id: Int): Result<Bool, Error>
        }
    "#).unwrap();
    
    match expr {
        Expression::Interface { name, methods, .. } => {
            assert_eq!(name, "Repository");
            assert_eq!(methods.len(), 3);
            
            // Check GetUser method has nullable return type
            assert_eq!(methods[0].name, "GetUser");
            assert!(methods[0].return_type.is_some());
            let return_type = methods[0].return_type.as_ref().unwrap();
            assert_eq!(return_type.name, "User");
            assert!(return_type.is_nullable);
            
            // Check SaveUser method has Bool return type
            assert_eq!(methods[1].name, "SaveUser");
            assert!(methods[1].return_type.is_some());
            let return_type = methods[1].return_type.as_ref().unwrap();
            assert_eq!(return_type.name, "Bool");
            assert!(!return_type.is_nullable);
            
            // Check DeleteUser method has generic Result return type
            assert_eq!(methods[2].name, "DeleteUser");
            assert!(methods[2].return_type.is_some());
            let return_type = methods[2].return_type.as_ref().unwrap();
            assert_eq!(return_type.name, "Result");
            assert_eq!(return_type.generics.len(), 2);
            assert_eq!(return_type.generics[0].name, "Bool");
            assert_eq!(return_type.generics[1].name, "Error");
        }
        _ => panic!("Expected interface definition"),
    }
}

#[test]
fn test_parse_interface_visibility() {
    // Test public interface (capitalized name)
    let expr = parse_top_level_item(r#"
        interface Drawable {
            fun Draw()
        }
    "#).unwrap();
    
    match expr {
        Expression::Interface { name, .. } => {
            assert_eq!(name, "Drawable");
            // Visibility is determined by capitalization of the name
            assert!(name.chars().next().unwrap().is_uppercase()); // Capital D = public
        }
        _ => panic!("Expected interface definition"),
    }
    
    // Test private interface (lowercase name)  
    let expr2 = parse_top_level_item(r#"
        interface drawable {
            fun draw()
        }
    "#).unwrap();
    
    match expr2 {
        Expression::Interface { name, .. } => {
            assert_eq!(name, "drawable");
            // Visibility is determined by capitalization of the name
            assert!(name.chars().next().unwrap().is_lowercase()); // lowercase d = private
        }
        _ => panic!("Expected interface definition"),
    }
}

// Note: Generic interfaces are not implemented in current AST
// This test would be added once generic interface support is implemented
// #[test]
// fn test_parse_interface_with_generics() { ... }

#[test]
fn test_parse_interface_with_default_implementations() {
    // Note: This tests whether we can parse default method implementations in interfaces
    // This is a more advanced feature that might not be implemented initially
    let expr = parse_top_level_item(r#"
        interface Loggable {
            fun Log(message: String)
            
            fun LogWithLevel(level: String, message: String): Void = {
                Log("[{level}] {message}")
            }
        }
    "#).unwrap();
    
    match expr {
        Expression::Interface { methods, .. } => {
            assert_eq!(methods.len(), 2);
            
            // First method has no body (abstract)
            assert_eq!(methods[0].name, "Log");
            assert!(!methods[0].is_default);
            assert!(methods[0].default_impl.is_none());
            
            // Second method has default implementation
            assert_eq!(methods[1].name, "LogWithLevel");
            assert!(methods[1].is_default); // Marked as having default implementation
            assert!(methods[1].default_impl.is_some()); // Default implementation provided
        }
        _ => panic!("Expected interface definition"),
    }
}

#[test]
fn test_parse_empty_interface() {
    let expr = parse_top_level_item(r#"
        interface Marker {
        }
    "#).unwrap();
    
    match expr {
        Expression::Interface { name, methods, .. } => {
            assert_eq!(name, "Marker");
            assert_eq!(methods.len(), 0); // Empty interface (marker interface)
        }
        _ => panic!("Expected empty interface definition"),
    }
}