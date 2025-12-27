//! Error types for the lexer

use crate::position::Position;
use seen_support::{ErrorLocation, SeenError, SeenErrorKind};
use thiserror::Error;

pub type LexerResult<T> = Result<T, LexerError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum LexerError {
    #[error("Unexpected character '{character}' at {position}")]
    UnexpectedCharacter { character: char, position: Position },

    #[error("Unterminated string literal at {position}")]
    UnterminatedString { position: Position },

    #[error("Invalid number format at {position}: {message}")]
    InvalidNumber { position: Position, message: String },

    #[error("Invalid Unicode escape sequence at {position}")]
    InvalidUnicodeEscape { position: Position },

    #[error("Invalid string interpolation at {position}: {message}")]
    InvalidInterpolation { position: Position, message: String },

    #[error("Unterminated comment at {position}")]
    UnterminatedComment { position: Position },

    #[error("Keyword file not found: {language}")]
    KeywordFileNotFound { language: String },

    #[error("Invalid keyword file format for {language}: {message}")]
    InvalidKeywordFile { language: String, message: String },

    #[error("Missing keyword in {language}: {keyword}")]
    MissingKeyword { language: String, keyword: String },

    #[error("IO error: {message}")]
    IoError { message: String },

    #[error("Expected space after return type label '{label}' at {position}")]
    MissingSpaceAfterReturnLabel { position: Position, label: String },
}

impl From<std::io::Error> for LexerError {
    fn from(error: std::io::Error) -> Self {
        LexerError::IoError {
            message: error.to_string(),
        }
    }
}

impl From<LexerError> for SeenError {
    fn from(error: LexerError) -> Self {
        let location = match &error {
            LexerError::UnexpectedCharacter { position, .. }
            | LexerError::UnterminatedString { position }
            | LexerError::InvalidNumber { position, .. }
            | LexerError::InvalidUnicodeEscape { position }
            | LexerError::InvalidInterpolation { position, .. }
            | LexerError::UnterminatedComment { position }
            | LexerError::MissingSpaceAfterReturnLabel { position, .. } => Some(ErrorLocation::new(
                position.line as u32,
                position.column as u32,
                position.offset as u32,
            )),
            _ => None,
        };

        SeenError::with_optional_location(
            SeenError::new(SeenErrorKind::Lexer, error.to_string()),
            location,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let pos = Position::new(1, 5, 4);
        let error = LexerError::UnexpectedCharacter {
            character: '@',
            position: pos,
        };

        assert!(error.to_string().contains("Unexpected character '@'"));
        assert!(error.to_string().contains("1:5"));
    }
}
