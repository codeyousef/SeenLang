//! Lexer for the Seen programming language
//!
//! This crate provides the lexical analysis functionality for Seen, supporting
//! bilingual keywords in both English and Arabic

pub mod keyword_config;
pub mod lexer;
pub mod project_config;
pub mod token;

#[cfg(test)]
mod tests;

pub use keyword_config::{KeywordConfig, KeywordConfigError, KeywordManager, Language};
pub use lexer::{Lexer, LexerError};
pub use project_config::{ProjectConfig, ProjectConfigError};
pub use token::{Location, Position, Token, TokenType};

/// Re-export everything needed by client code
pub mod prelude {
    pub use crate::keyword_config::{KeywordConfig, KeywordConfigError, KeywordManager};
    pub use crate::lexer::{Lexer, LexerError};
    pub use crate::project_config::{ProjectConfig, ProjectConfigError};
    pub use crate::token::{Location, Position, Token, TokenType};
}
