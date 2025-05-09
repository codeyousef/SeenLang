//! Common utilities and shared functionality for the Seen programming language.
//!
//! This module provides shared functionality that is used across multiple
//! components of the Seen compiler, including error handling, diagnostics,
//! and common data structures.

pub mod error;

// Re-export common types for convenience
pub use error::{
    Diagnostic, Diagnostics, DiagnosticFormatter,
    LexicalError, SyntaxError, SemanticError, CodeGenError,
    Location, Position, LocalizedMessage, LanguageContext, LanguagePreference, 
    Severity, FixSuggestion, RelatedInformation,
};
