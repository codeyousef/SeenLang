//! Error handling utilities for the Seen compiler

// use std::fmt; // Will be used when implementing custom Display impls
use thiserror::Error;

/// The main error type for the Seen compiler
#[derive(Error, Debug, Clone)]
pub enum SeenError {
    #[error("Lexical error: {message}")]
    LexError { message: String },
    
    #[error("Parse error: {message}")]
    ParseError { message: String },
    
    #[error("Type error: {message}")]
    TypeError { message: String },
    
    #[error("Code generation error: {message}")]
    CodegenError { message: String },
    
    #[error("I/O error: {message}")]
    IoError { message: String },
    
    #[error("Configuration error: {message}")]
    ConfigError { message: String },
    
    #[error("Runtime error: {message}")]
    RuntimeError { message: String },
    
    #[error("FFI error: {message}")]
    FfiError { message: String },
}

/// Result type alias for Seen compiler operations
pub type SeenResult<T> = Result<T, SeenError>;

impl SeenError {
    pub fn lex_error(message: impl Into<String>) -> Self {
        Self::LexError { message: message.into() }
    }
    
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError { message: message.into() }
    }
    
    pub fn type_error(message: impl Into<String>) -> Self {
        Self::TypeError { message: message.into() }
    }
    
    pub fn codegen_error(message: impl Into<String>) -> Self {
        Self::CodegenError { message: message.into() }
    }
    
    pub fn io_error(message: impl Into<String>) -> Self {
        Self::IoError { message: message.into() }
    }
    
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError { message: message.into() }
    }
    
    pub fn runtime_error(message: impl Into<String>) -> Self {
        Self::RuntimeError { message: message.into() }
    }
    
    pub fn ffi_error(message: impl Into<String>) -> Self {
        Self::FfiError { message: message.into() }
    }
}