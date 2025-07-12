//! Lexer for the Seen programming language
//!
//! This crate provides the lexical analysis functionality for Seen, supporting
//! bilingual keywords in both English and Arabic

pub mod token;
pub mod keyword_config;
pub mod lexer;
pub mod project_config;

#[cfg(test)]
mod tests;

pub use token::{Token, TokenType, Position, Location};
pub use keyword_config::{KeywordConfig, KeywordManager, KeywordConfigError, Language};
pub use lexer::{Lexer, LexerError};
pub use project_config::{ProjectConfig, ProjectConfigError};

/// Re-export everything needed by client code
pub mod prelude {
    pub use crate::token::{Token, TokenType, Position, Location};
    pub use crate::keyword_config::{KeywordConfig, KeywordManager, KeywordConfigError};
    pub use crate::lexer::{Lexer, LexerError};
    pub use crate::project_config::{ProjectConfig, ProjectConfigError};
}