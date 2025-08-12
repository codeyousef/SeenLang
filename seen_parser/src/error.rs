//! Parser error types

use thiserror::Error;
use seen_lexer::{Position, TokenType};

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("Unexpected token: found {found:?}, expected {expected}")]
    UnexpectedToken {
        found: TokenType,
        expected: String,
        pos: Position,
    },
    
    #[error("Unexpected end of file")]
    UnexpectedEof {
        pos: Position,
    },
    
    #[error("Invalid expression at position {pos:?}")]
    InvalidExpression {
        pos: Position,
    },
    
    #[error("Invalid pattern: {message}")]
    InvalidPattern {
        message: String,
        pos: Position,
    },
    
    #[error("Missing closing delimiter: expected {expected}")]
    MissingClosingDelimiter {
        expected: String,
        pos: Position,
    },
    
    #[error("Lexer error: {message}")]
    LexerError {
        message: String,
    },
    
    #[error("Invalid number literal: {message}")]
    InvalidNumber {
        message: String,
        pos: Position,
    },
    
    #[error("Invalid string literal: {message}")]
    InvalidString {
        message: String,
        pos: Position,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_error() {
        let error = ParseError::UnexpectedEof {
            pos: Position::new(1, 1),
        };
        assert!(error.to_string().contains("Unexpected end of file"));
    }
}