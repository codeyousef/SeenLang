//! Shared support utilities for the Seen compiler toolchain.
//!
//! This crate provides common result and error types so individual compiler
//! components can report failures in a consistent way without depending on
//! each other directly.

use std::fmt;
use thiserror::Error;

/// Location information associated with an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ErrorLocation {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
    /// Byte offset from the start of the file.
    pub offset: u32,
}

impl ErrorLocation {
    /// Create a new location.
    pub const fn new(line: u32, column: u32, offset: u32) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }
}

impl fmt::Display for ErrorLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// High-level category for an error emitted by the compiler stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SeenErrorKind {
    /// Lexical analysis failure.
    Lexer,
    /// Parsing failure.
    Parser,
    /// Type checking failure.
    TypeChecker,
    /// Memory/region analysis failure.
    Memory,
    /// Intermediate representation and code generation failure.
    Ir,
    /// Interpreter/runtime failure.
    Interpreter,
    /// Input/Output failure from the host environment.
    Io,
    /// CLI/tooling failure (argument parsing, configuration lookup).
    Tooling,
    /// Explicit abort requested by the program.
    Abort,
    /// Catch-all bucket for anything that does not map cleanly to the
    /// categories above.
    Other,
}

impl fmt::Display for SeenErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            SeenErrorKind::Lexer => "Lexer",
            SeenErrorKind::Parser => "Parser",
            SeenErrorKind::TypeChecker => "TypeChecker",
            SeenErrorKind::Memory => "Memory",
            SeenErrorKind::Ir => "IR",
            SeenErrorKind::Interpreter => "Interpreter",
            SeenErrorKind::Io => "IO",
            SeenErrorKind::Tooling => "Tooling",
            SeenErrorKind::Abort => "Abort",
            SeenErrorKind::Other => "Other",
        };
        write!(f, "{name}")
    }
}

/// Unified error type used across compiler crates.
#[derive(Debug, Error)]
pub enum SeenError {
    /// Error with a specific category and optional position.
    #[error("{kind}: {message}")]
    Categorised {
        kind: SeenErrorKind,
        message: String,
        location: Option<ErrorLocation>,
    },
    /// Error originating from an underlying boxed error – used as a fallback.
    #[error("{0}")]
    Wrapped(String),
}

impl SeenError {
    /// Create a new categorised error.
    pub fn new(kind: SeenErrorKind, message: impl Into<String>) -> Self {
        Self::Categorised {
            kind,
            message: message.into(),
            location: None,
        }
    }

    /// Create a new categorised error with location information.
    pub fn with_location(
        kind: SeenErrorKind,
        message: impl Into<String>,
        location: ErrorLocation,
    ) -> Self {
        Self::Categorised {
            kind,
            message: message.into(),
            location: Some(location),
        }
    }

    /// Create an abort error – used when the running program explicitly aborts.
    pub fn abort(message: impl Into<String>, location: Option<ErrorLocation>) -> Self {
        Self::Categorised {
            kind: SeenErrorKind::Abort,
            message: message.into(),
            location,
        }
    }

    /// Attach location information if it was previously missing.
    pub fn with_optional_location(mut self, location: Option<ErrorLocation>) -> Self {
        if let SeenError::Categorised {
            location: ref mut loc,
            ..
        } = self
        {
            if loc.is_none() {
                if let Some(value) = location {
                    *loc = Some(value);
                }
            }
        }
        self
    }

    /// Access the error kind if categorised.
    pub fn kind(&self) -> Option<SeenErrorKind> {
        match self {
            SeenError::Categorised { kind, .. } => Some(*kind),
            SeenError::Wrapped(_) => None,
        }
    }

    /// Access location information if present.
    pub fn location(&self) -> Option<ErrorLocation> {
        match self {
            SeenError::Categorised { location, .. } => *location,
            SeenError::Wrapped(_) => None,
        }
    }
}

impl From<anyhow::Error> for SeenError {
    fn from(error: anyhow::Error) -> Self {
        SeenError::Wrapped(error.to_string())
    }
}

impl From<std::io::Error> for SeenError {
    fn from(error: std::io::Error) -> Self {
        SeenError::new(SeenErrorKind::Io, error.to_string())
    }
}

/// Convenience result alias used across compiler crates.
pub type SeenResult<T> = Result<T, SeenError>;
