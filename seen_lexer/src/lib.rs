//! Seen Language Lexer
//! 
//! This crate provides lexical analysis for the Seen programming language,
//! implementing dynamic keyword loading from TOML files and complete Unicode support.

pub mod keyword_manager;
pub mod lexer;
pub mod token;
pub mod error;
pub mod position;

pub use keyword_manager::KeywordManager;
pub use lexer::Lexer;
pub use token::{Token, TokenType, InterpolationPart, InterpolationKind};
pub use error::{LexerError, LexerResult};
pub use position::Position;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_creation() {
        // Test lexer functionality
        // Real tests will be implemented following TDD methodology
        assert!(true);
    }
    
    mod hardcoded_keyword_scanner;
    mod keyword_validation_integration;
    mod core_lexer_tests;
    mod string_interpolation_tests;
    mod nullable_operators_tests;
}