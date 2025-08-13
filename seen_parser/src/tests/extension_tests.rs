//! Tests for extension method parsing

use crate::{Parser, Expression};
use seen_lexer::{Lexer, KeywordManager};
use std::sync::Arc;

fn parse_top_level_item(input: &str) -> Result<Expression, crate::ParseError> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_top_level_item()
}

// Extension Method Tests (following Syntax Design spec)

#[test]
fn test_parse_simple_extension() {
    let expr = parse_top_level_item(r#"
        extension String {
            fun Reversed(): String {
                return this.chars().reverse().join()
            }
        }
    "#).unwrap();
    
    match expr {
        Expression::Extension { target_type, methods, .. } => {
            assert_eq!(target_type.name, "String");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "Reversed");
            assert!(methods[0].return_type.is_some());
            let return_type = methods[0].return_type.as_ref().unwrap();
            assert_eq!(return_type.name, "String");
        }
        _ => panic!("Expected extension definition"),
    }
}

#[test]
fn test_parse_extension_multiple_methods() {
    let expr = parse_top_level_item(r#"
        extension String {
            fun Reversed(): String {
                return this.chars().reverse().join()
            }
            
            fun cleaned(): String {
                return this.trim().toLowerCase()
            }
        }
    "#).unwrap();
    
    match expr {
        Expression::Extension { target_type, methods, .. } => {
            assert_eq!(target_type.name, "String");
            assert_eq!(methods.len(), 2);
            
            // Check first method (public)
            assert_eq!(methods[0].name, "Reversed");
            assert!(methods[0].name.chars().next().unwrap().is_uppercase()); // Public
            
            // Check second method (private)
            assert_eq!(methods[1].name, "cleaned");
            assert!(methods[1].name.chars().next().unwrap().is_lowercase()); // Private
        }
        _ => panic!("Expected extension definition"),
    }
}

#[test]
fn test_parse_extension_with_parameters() {
    let expr = parse_top_level_item(r#"
        extension Array {
            fun Contains(item: T): Bool {
                return this.any { it == item }
            }
        }
    "#).unwrap();
    
    match expr {
        Expression::Extension { target_type, methods, .. } => {
            assert_eq!(target_type.name, "Array");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "Contains");
            assert_eq!(methods[0].parameters.len(), 1);
            assert_eq!(methods[0].parameters[0].name, "item");
            assert!(methods[0].parameters[0].type_annotation.is_some());
            let param_type = methods[0].parameters[0].type_annotation.as_ref().unwrap();
            assert_eq!(param_type.name, "T");
        }
        _ => panic!("Expected extension definition"),
    }
}

#[test]
fn test_parse_extension_generic_target() {
    let expr = parse_top_level_item(r#"
        extension List<T> {
            fun First(): T? {
                return this.firstOrNull()
            }
        }
    "#).unwrap();
    
    match expr {
        Expression::Extension { target_type, methods, .. } => {
            assert_eq!(target_type.name, "List");
            assert_eq!(target_type.generics.len(), 1);
            assert_eq!(target_type.generics[0].name, "T");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "First");
            assert!(methods[0].return_type.is_some());
            let return_type = methods[0].return_type.as_ref().unwrap();
            assert_eq!(return_type.name, "T");
            assert!(return_type.is_nullable);
        }
        _ => panic!("Expected extension definition"),
    }
}

#[test]
fn test_parse_extension_visibility() {
    let expr = parse_top_level_item(r#"
        extension String {
            fun ToUpper(): String {     // Public method
                return this.toUpperCase()
            }
            
            fun toLower(): String {     // Private method
                return this.toLowerCase()
            }
        }
    "#).unwrap();
    
    match expr {
        Expression::Extension { target_type, methods, .. } => {
            assert_eq!(target_type.name, "String");
            assert_eq!(methods.len(), 2);
            
            // Check method visibility through name capitalization
            assert_eq!(methods[0].name, "ToUpper");
            assert!(methods[0].name.chars().next().unwrap().is_uppercase()); // Public
            
            assert_eq!(methods[1].name, "toLower");
            assert!(methods[1].name.chars().next().unwrap().is_lowercase()); // Private
        }
        _ => panic!("Expected extension definition"),
    }
}

#[test]
fn test_parse_extension_with_receiver_modifiers() {
    // Note: This tests if extension methods can have receiver modifiers
    // This might be a more advanced feature
    let expr = parse_top_level_item(r#"
        extension StringBuilder {
            fun Append(text: String): StringBuilder {
                // Extension method that modifies the receiver
                return this.append(text)
            }
        }
    "#).unwrap();
    
    match expr {
        Expression::Extension { target_type, methods, .. } => {
            assert_eq!(target_type.name, "StringBuilder");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "Append");
            assert_eq!(methods[0].parameters.len(), 1);
            assert_eq!(methods[0].parameters[0].name, "text");
        }
        _ => panic!("Expected extension definition"),
    }
}

#[test]
fn test_parse_empty_extension() {
    let expr = parse_top_level_item(r#"
        extension String {
        }
    "#).unwrap();
    
    match expr {
        Expression::Extension { target_type, methods, .. } => {
            assert_eq!(target_type.name, "String");
            assert_eq!(methods.len(), 0); // Empty extension (marker extension)
        }
        _ => panic!("Expected empty extension definition"),
    }
}