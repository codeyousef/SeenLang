//! Parser error types

use seen_lexer::{Position, TokenType};
use seen_support::{ErrorLocation, SeenError, SeenErrorKind};
use thiserror::Error;

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("Unexpected token: found {found:?}, expected {expected} at {pos:?}")]
    UnexpectedToken {
        found: TokenType,
        expected: String,
        pos: Position,
    },

    #[error("Unexpected end of file at {pos:?}")]
    UnexpectedEof { pos: Position },

    #[error("Invalid expression at {pos:?}")]
    InvalidExpression { pos: Position },

    #[error("Invalid pattern: {message}")]
    InvalidPattern { message: String, pos: Position },

    #[error("Missing closing delimiter: expected {expected}")]
    MissingClosingDelimiter { expected: String, pos: Position },

    #[error("Lexer error: {message}")]
    LexerError { message: String },

    #[error("Invalid number literal: {message}")]
    InvalidNumber { message: String, pos: Position },

    #[error("Invalid string literal: {message}")]
    InvalidString { message: String, pos: Position },
}

impl From<ParseError> for SeenError {
    fn from(error: ParseError) -> Self {
        let (message, location): (String, Option<ErrorLocation>) = match &error {
            ParseError::UnexpectedToken { pos, .. }
            | ParseError::UnexpectedEof { pos }
            | ParseError::InvalidExpression { pos }
            | ParseError::InvalidPattern { pos, .. }
            | ParseError::MissingClosingDelimiter { pos, .. }
            | ParseError::InvalidNumber { pos, .. }
            | ParseError::InvalidString { pos, .. } => (
                error.to_string(),
                Some(ErrorLocation::new(
                    pos.line as u32,
                    pos.column as u32,
                    pos.offset as u32,
                )),
            ),
            ParseError::LexerError { .. } => (error.to_string(), None),
        };

        SeenError::with_optional_location(SeenError::new(SeenErrorKind::Parser, message), location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        let error = ParseError::UnexpectedEof {
            pos: Position::new(1, 1, 0),
        };
        assert!(error.to_string().contains("Unexpected end of file"));
    }
}
