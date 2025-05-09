use seen_parser::ast;
use thiserror::Error;

/// Errors that can occur during code generation
#[derive(Debug, Error)]
pub enum CodeGenError {
    #[error("Unknown type: {0}")]
    UnknownType(String),

    #[error("Undefined symbol: {0}")]
    UndefinedSymbol(String),

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        expected: String,
        actual: String,
    },

    #[error("Operation not supported: {0}")]
    UnsupportedOperation(String),

    #[error("Failed to initialize LLVM: {0}")]
    LLVMInitialization(String),

    #[error("Failed to generate code: {0}")]
    CodeGeneration(String),

    #[error("Invalid function call: {0}")]
    InvalidFunctionCall(String),

    #[error("Invalid variable access: {0}")]
    InvalidVariableAccess(String),

    #[error("Invalid AST node at {location}: {message}")]
    InvalidASTNode {
        location: String,
        message: String,
    },
}

/// Result type for code generation operations
pub type Result<T> = std::result::Result<T, CodeGenError>;
