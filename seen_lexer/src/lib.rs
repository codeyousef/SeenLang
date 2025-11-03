//! Seen Language Lexer
//!
//! This crate provides lexical analysis for the Seen programming language,
//! implementing dynamic keyword loading from TOML files and complete Unicode support.

pub mod error;
pub mod keyword_manager;
pub mod lexer;
pub mod position;
pub mod token;

pub use error::{LexerError, LexerResult};
pub use keyword_manager::KeywordManager;
pub use lexer::{Lexer, LexerConfig, VisibilityPolicy};
pub use position::Position;
pub use token::{InterpolationKind, InterpolationPart, Token, TokenType};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_creation() {
        // Test lexer functionality
        // Real tests will be implemented following TDD methodology
        assert!(true);
    }

    mod core_lexer_tests;
    mod hardcoded_keyword_scanner;
    mod keyword_validation_integration;
    mod nullable_operators_tests;
    mod string_interpolation_tests;
}
