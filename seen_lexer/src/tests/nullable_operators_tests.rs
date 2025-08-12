//! Comprehensive tests for nullable operators
//! 
//! Tests for safe navigation (?.),  elvis (?:), and force unwrap (!!) operators

use crate::{
    Lexer, TokenType, KeywordManager, Position,
};
use std::sync::Arc;

/// Test helper to create a lexer with English keywords loaded
fn create_test_lexer(input: &str) -> Lexer {
    let keyword_manager = KeywordManager::new();
    Lexer::new(input.to_string(), Arc::new(keyword_manager))
}

#[cfg(test)]
mod safe_navigation_tests {
    use super::*;

    #[test]
    fn test_safe_navigation_operator() {
        let mut lexer = create_test_lexer("user?.name");
        
        // First token: identifier "user"
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("user".to_string()));
        
        // Second token: safe navigation operator
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::SafeNavigation);
        assert_eq!(token2.lexeme, "?.");
        
        // Third token: identifier "name"  
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PrivateIdentifier("name".to_string()));
    }

    #[test]
    fn test_chained_safe_navigation() {
        let mut lexer = create_test_lexer("user?.profile?.address?.city");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("user".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::SafeNavigation);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PrivateIdentifier("profile".to_string()));
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::SafeNavigation);
        
        let token5 = lexer.next_token().unwrap();
        assert_eq!(token5.token_type, TokenType::PrivateIdentifier("address".to_string()));
        
        let token6 = lexer.next_token().unwrap();
        assert_eq!(token6.token_type, TokenType::SafeNavigation);
        
        let token7 = lexer.next_token().unwrap();
        assert_eq!(token7.token_type, TokenType::PrivateIdentifier("city".to_string()));
    }

    #[test]
    fn test_safe_navigation_with_method_call() {
        // Note: Using capitals for public methods per Seen syntax
        let mut lexer = create_test_lexer("user?.GetName()");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("user".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::SafeNavigation);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PublicIdentifier("GetName".to_string()));
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::LeftParen);
        
        let token5 = lexer.next_token().unwrap();
        assert_eq!(token5.token_type, TokenType::RightParen);
    }
}

#[cfg(test)]
mod elvis_operator_tests {
    use super::*;

    #[test]
    fn test_elvis_operator() {
        let mut lexer = create_test_lexer(r#"name ?: "Unknown""#);
        
        // First token: identifier "name"
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("name".to_string()));
        
        // Second token: elvis operator
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::Elvis);
        assert_eq!(token2.lexeme, "?:");
        
        // Third token: string literal
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::StringLiteral("Unknown".to_string()));
    }

    #[test]
    fn test_elvis_with_complex_expression() {
        let mut lexer = create_test_lexer("getUserName() ?: defaultName");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("getUserName".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::LeftParen);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::RightParen);
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::Elvis);
        
        let token5 = lexer.next_token().unwrap();
        assert_eq!(token5.token_type, TokenType::PrivateIdentifier("defaultName".to_string()));
    }

    #[test]
    fn test_elvis_in_assignment() {
        let mut lexer = create_test_lexer(r#"let result = value ?: 42"#);
        
        // Skip "let" - will be handled by keyword manager
        let _ = lexer.next_token(); // let
        let _ = lexer.next_token(); // result
        let _ = lexer.next_token(); // =
        let _ = lexer.next_token(); // value
        
        let elvis_token = lexer.next_token().unwrap();
        assert_eq!(elvis_token.token_type, TokenType::Elvis);
        
        let default_token = lexer.next_token().unwrap();
        assert_eq!(default_token.token_type, TokenType::IntegerLiteral(42));
    }
}

#[cfg(test)]
mod force_unwrap_tests {
    use super::*;

    #[test]
    fn test_force_unwrap_operator() {
        let mut lexer = create_test_lexer("maybeValue!!");
        
        // First token: identifier
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("maybeValue".to_string()));
        
        // Second token: force unwrap
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::ForceUnwrap);
        assert_eq!(token2.lexeme, "!!");
    }

    #[test]
    fn test_force_unwrap_with_property_access() {
        // Using capital for public field per Seen syntax
        let mut lexer = create_test_lexer("user!!.Name");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("user".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::ForceUnwrap);
        
        // Note: We don't have a dot operator token yet, this will need parser work
        // For now, the dot is consumed as part of the identifier scanning
    }

    #[test]
    fn test_multiple_force_unwraps() {
        let mut lexer = create_test_lexer("a!! + b!!");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("a".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::ForceUnwrap);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::Plus);
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::PrivateIdentifier("b".to_string()));
        
        let token5 = lexer.next_token().unwrap();
        assert_eq!(token5.token_type, TokenType::ForceUnwrap);
    }
}

#[cfg(test)]
mod combined_nullable_tests {
    use super::*;

    #[test]
    fn test_safe_navigation_with_elvis() {
        let mut lexer = create_test_lexer(r#"user?.Name ?: "Anonymous""#);
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("user".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::SafeNavigation);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PublicIdentifier("Name".to_string()));
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::Elvis);
        
        let token5 = lexer.next_token().unwrap();
        assert_eq!(token5.token_type, TokenType::StringLiteral("Anonymous".to_string()));
    }

    #[test]
    fn test_all_nullable_operators_together() {
        // A complex expression using all nullable operators
        let mut lexer = create_test_lexer(r#"(user?.Profile ?: defaultProfile)!!.Settings"#);
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::LeftParen);
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::PrivateIdentifier("user".to_string()));
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::SafeNavigation);
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::PublicIdentifier("Profile".to_string()));
        
        let token5 = lexer.next_token().unwrap();
        assert_eq!(token5.token_type, TokenType::Elvis);
        
        let token6 = lexer.next_token().unwrap();
        assert_eq!(token6.token_type, TokenType::PrivateIdentifier("defaultProfile".to_string()));
        
        let token7 = lexer.next_token().unwrap();
        assert_eq!(token7.token_type, TokenType::RightParen);
        
        let token8 = lexer.next_token().unwrap();
        assert_eq!(token8.token_type, TokenType::ForceUnwrap);
        
        // The .Settings part would need proper dot operator handling
    }

    #[test]
    fn test_nullable_with_question_mark() {
        // Test that single ? is handled differently from ?. and ?:
        let mut lexer = create_test_lexer("isReady? value");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("isReady".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::Question);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PrivateIdentifier("value".to_string()));
    }
}

#[cfg(test)]
mod position_tracking_tests {
    use super::*;

    #[test]
    fn test_nullable_operator_positions() {
        let mut lexer = create_test_lexer("a?.b ?: c!!");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.position.column, 1);
        
        let token2 = lexer.next_token().unwrap(); // ?.
        assert_eq!(token2.position.column, 2);
        assert_eq!(token2.lexeme.len(), 2);
        
        let token3 = lexer.next_token().unwrap(); // b
        assert_eq!(token3.position.column, 4);
        
        let token4 = lexer.next_token().unwrap(); // ?:
        assert_eq!(token4.position.column, 6);
        
        let token5 = lexer.next_token().unwrap(); // c
        assert_eq!(token5.position.column, 9);
        
        let token6 = lexer.next_token().unwrap(); // !!
        assert_eq!(token6.position.column, 10);
    }
}