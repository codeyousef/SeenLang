//! Error types for the IR generation system

use thiserror::Error;

pub type Result<T> = std::result::Result<T, CodeGenError>;

#[derive(Error, Debug)]
pub enum CodeGenError {
    #[error("Code generation error: {0}")]
    CodeGeneration(String),
    
    #[error("Unknown type: {0}")]
    UnknownType(String),
    
    #[error("Type mismatch: expected {expected}, found {actual}")]
    TypeMismatch { expected: String, actual: String },
    
    #[error("Undefined symbol: {0}")]
    UndefinedSymbol(String),
    
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
    
    #[error("Invalid AST node at {location}: {message}")]
    InvalidASTNode { location: String, message: String },
}