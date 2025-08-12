//! Seen Language Parser
//! 
//! This crate provides parsing for the Seen programming language,
//! implementing everything-as-expression design and complete AST generation.

pub mod parser;
pub mod ast;
pub mod error;
pub mod position;

pub use parser::Parser;
pub use ast::*;
pub use error::{ParseError, ParseResult};
pub use position::Position;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        // This is a placeholder test to ensure the crate compiles
        // Real tests will be implemented following TDD methodology
        assert!(true);
    }
}