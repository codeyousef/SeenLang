//! Error types for the lexer

use crate::position::Position;
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
    
    #[error("Keyword file not found: {language}")]
    KeywordFileNotFound { language: String },
    
    #[error("Invalid keyword file format for {language}: {message}")]
    InvalidKeywordFile { language: String, message: String },
    
    #[error("Missing keyword in {language}: {keyword}")]
    MissingKeyword { language: String, keyword: String },
    
    #[error("IO error: {message}")]
    IoError { message: String },
}

impl From<std::io::Error> for LexerError {
    fn from(error: std::io::Error) -> Self {
        LexerError::IoError {
            message: error.to_string(),
        }
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