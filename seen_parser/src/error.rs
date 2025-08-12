//! Parser error types

use thiserror::Error;

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("Parse error placeholder")]
    Placeholder,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_error() {
        let error = ParseError::Placeholder;
        assert!(error.to_string().contains("placeholder"));
    }
}