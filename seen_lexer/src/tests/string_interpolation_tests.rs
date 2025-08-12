//! Comprehensive tests for string interpolation
//! 
//! Following TDD methodology, these tests define expected behavior
//! for string interpolation BEFORE implementation.

use crate::{
    Lexer, TokenType, KeywordManager, Position,
    token::{Token, InterpolationPart, InterpolationKind},
};
use std::sync::Arc;

/// Test helper to create a lexer with English keywords loaded
fn create_test_lexer(input: &str) -> Lexer {
    let mut keyword_manager = KeywordManager::new();
    // For now, use an empty keyword manager since we're testing interpolation
    Lexer::new(input.to_string(), Arc::new(keyword_manager))
}

#[cfg(test)]
mod basic_interpolation_tests {
    use super::*;

    #[test]
    fn test_simple_interpolation() {
        let mut lexer = create_test_lexer(r#""Hello, {name}!""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 3);
                
                // First part: "Hello, "
                assert_eq!(parts[0].kind, InterpolationKind::Text("Hello, ".to_string()));
                
                // Second part: {name}
                assert_eq!(parts[1].kind, InterpolationKind::Expression("name".to_string()));
                
                // Third part: "!"
                assert_eq!(parts[2].kind, InterpolationKind::Text("!".to_string()));
            }
            _ => panic!("Expected InterpolatedString token"),
        }
    }

    #[test]
    fn test_multiple_interpolations() {
        let mut lexer = create_test_lexer(r#""User {firstName} {lastName} is {age} years old""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 7);
                
                assert_eq!(parts[0].kind, InterpolationKind::Text("User ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression("firstName".to_string()));
                assert_eq!(parts[2].kind, InterpolationKind::Text(" ".to_string()));
                assert_eq!(parts[3].kind, InterpolationKind::Expression("lastName".to_string()));
                assert_eq!(parts[4].kind, InterpolationKind::Text(" is ".to_string()));
                assert_eq!(parts[5].kind, InterpolationKind::Expression("age".to_string()));
                assert_eq!(parts[6].kind, InterpolationKind::Text(" years old".to_string()));
            }
            _ => panic!("Expected InterpolatedString token"),
        }
    }

    #[test]
    fn test_complex_expression_interpolation() {
        let mut lexer = create_test_lexer(r#""Result: {a + b * 2}""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 2);
                
                assert_eq!(parts[0].kind, InterpolationKind::Text("Result: ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression("a + b * 2".to_string()));
            }
            _ => panic!("Expected InterpolatedString token"),
        }
    }

    #[test]
    fn test_method_call_interpolation() {
        let mut lexer = create_test_lexer(r#""Name: {user.getName().toUpperCase()}""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 2);
                
                assert_eq!(parts[0].kind, InterpolationKind::Text("Name: ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression("user.getName().toUpperCase()".to_string()));
            }
            _ => panic!("Expected InterpolatedString token"),
        }
    }

    #[test]
    fn test_no_interpolation_regular_string() {
        let mut lexer = create_test_lexer(r#""Just a regular string""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::StringLiteral(s) => {
                assert_eq!(s, "Just a regular string");
            }
            _ => panic!("Expected StringLiteral token for string without interpolation"),
        }
    }
}

#[cfg(test)]
mod escape_sequence_tests {
    use super::*;

    #[test]
    fn test_escaped_brace_opening() {
        let mut lexer = create_test_lexer(r#""This is {{not interpolated}}""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::StringLiteral(s) => {
                assert_eq!(s, "This is {not interpolated}");
            }
            _ => panic!("Expected StringLiteral with escaped braces"),
        }
    }

    #[test]
    fn test_escaped_brace_with_interpolation() {
        let mut lexer = create_test_lexer(r#""{{literal}} and {interpolated}""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 2);
                
                assert_eq!(parts[0].kind, InterpolationKind::Text("{literal} and ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression("interpolated".to_string()));
            }
            _ => panic!("Expected InterpolatedString with escaped and interpolated parts"),
        }
    }

    #[test]
    fn test_multiple_escaped_braces() {
        let mut lexer = create_test_lexer(r#""{{{{multiple}}}} braces""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::StringLiteral(s) => {
                assert_eq!(s, "{{multiple}} braces");
            }
            _ => panic!("Expected StringLiteral with multiple escaped braces"),
        }
    }
}

#[cfg(test)]
mod nested_brace_tests {
    use super::*;

    #[test]
    fn test_nested_object_literal() {
        let mut lexer = create_test_lexer(r#""User: {User { name: "Alice", age: 30 }}""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 2);
                
                assert_eq!(parts[0].kind, InterpolationKind::Text("User: ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression(r#"User { name: "Alice", age: 30 }"#.to_string()));
            }
            _ => panic!("Expected InterpolatedString with nested braces"),
        }
    }

    #[test]
    fn test_nested_lambda() {
        let mut lexer = create_test_lexer(r#""Result: {list.map({ x -> x * 2 })}""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 2);
                
                assert_eq!(parts[0].kind, InterpolationKind::Text("Result: ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression("list.map({ x -> x * 2 })".to_string()));
            }
            _ => panic!("Expected InterpolatedString with nested lambda"),
        }
    }

    #[test]
    fn test_deeply_nested_braces() {
        let mut lexer = create_test_lexer(r#""Complex: {fn() { if (true) { return { value: 42 } } }}""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 2);
                
                assert_eq!(parts[0].kind, InterpolationKind::Text("Complex: ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression("fn() { if (true) { return { value: 42 } } }".to_string()));
            }
            _ => panic!("Expected InterpolatedString with deeply nested braces"),
        }
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    use crate::error::LexerError;

    #[test]
    fn test_unclosed_interpolation() {
        let mut lexer = create_test_lexer(r#""Unclosed {interpolation"#);
        
        let result = lexer.next_token();
        assert!(result.is_err());
        
        match result {
            Err(LexerError::UnterminatedString { .. }) => {
                // Expected error
            }
            _ => panic!("Expected UnterminatedString error for unclosed interpolation"),
        }
    }

    #[test]
    fn test_unmatched_closing_brace_in_string() {
        // A closing brace without opening should be treated as regular text
        let mut lexer = create_test_lexer(r#""Just a } brace""#);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::StringLiteral(s) => {
                assert_eq!(s, "Just a } brace");
            }
            _ => panic!("Expected StringLiteral with unmatched closing brace"),
        }
    }

    #[test]
    fn test_empty_interpolation() {
        let mut lexer = create_test_lexer(r#""Empty: {}""#);
        
        let result = lexer.next_token();
        assert!(result.is_err());
        
        match result {
            Err(LexerError::InvalidInterpolation { .. }) => {
                // Expected error for empty interpolation
            }
            _ => panic!("Expected InvalidInterpolation error for empty braces"),
        }
    }
}

#[cfg(test)]
mod position_tracking_tests {
    use super::*;

    #[test]
    fn test_interpolation_position_tracking() {
        let mut lexer = create_test_lexer(r#""Hello {name}!""#);
        
        let token = lexer.next_token().unwrap();
        assert_eq!(token.position.line, 1);
        assert_eq!(token.position.column, 1);
        
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                // "Hello " starts at column 2 (after opening quote)
                assert_eq!(parts[0].position.column, 2);
                
                // {name} starts at column 8
                assert_eq!(parts[1].position.column, 8);
                
                // "!" starts at column 13
                assert_eq!(parts[2].position.column, 13);
            }
            _ => panic!("Expected InterpolatedString"),
        }
    }

    #[test]
    fn test_multiline_interpolation_position() {
        let input = r#""Line 1
{interpolation}
Line 3""#;
        let mut lexer = create_test_lexer(input);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 3);
                
                // First part on line 1
                assert_eq!(parts[0].position.line, 1);
                
                // Interpolation on line 2
                assert_eq!(parts[1].position.line, 2);
                
                // Third part on line 3
                assert_eq!(parts[2].position.line, 3);
            }
            _ => panic!("Expected InterpolatedString"),
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_interpolation_with_all_features() {
        let input = r#""User: {user.name}, Age: {user.age}, Status: {{active}}, Lambda: {list.map({ x -> x * 2 })}""#;
        let mut lexer = create_test_lexer(input);
        
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 6);
                
                assert_eq!(parts[0].kind, InterpolationKind::Text("User: ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression("user.name".to_string()));
                assert_eq!(parts[2].kind, InterpolationKind::Text(", Age: ".to_string()));
                assert_eq!(parts[3].kind, InterpolationKind::Expression("user.age".to_string()));
                assert_eq!(parts[4].kind, InterpolationKind::Text(", Status: {active}, Lambda: ".to_string()));
                assert_eq!(parts[5].kind, InterpolationKind::Expression("list.map({ x -> x * 2 })".to_string()));
                // No empty text part at end when string ends with expression
            }
            _ => panic!("Expected InterpolatedString with all features"),
        }
    }

    #[test]
    fn test_interpolation_at_string_boundaries() {
        // Test interpolation at the very beginning
        let mut lexer = create_test_lexer(r#""{name} at start""#);
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0].kind, InterpolationKind::Expression("name".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Text(" at start".to_string()));
            }
            _ => panic!("Expected InterpolatedString"),
        }

        // Test interpolation at the very end
        let mut lexer = create_test_lexer(r#""at end {name}""#);
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0].kind, InterpolationKind::Text("at end ".to_string()));
                assert_eq!(parts[1].kind, InterpolationKind::Expression("name".to_string()));
            }
            _ => panic!("Expected InterpolatedString"),
        }

        // Test only interpolation
        let mut lexer = create_test_lexer(r#""{onlyInterpolation}""#);
        let token = lexer.next_token().unwrap();
        match token.token_type {
            TokenType::InterpolatedString(parts) => {
                assert_eq!(parts.len(), 1);
                assert_eq!(parts[0].kind, InterpolationKind::Expression("onlyInterpolation".to_string()));
            }
            _ => panic!("Expected InterpolatedString"),
        }
    }
}