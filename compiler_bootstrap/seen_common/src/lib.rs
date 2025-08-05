//! Shared utilities and error types for the Seen language compiler
//! 
//! This crate provides common functionality used across all compiler components:
//! - Error types and handling utilities
//! - Source location tracking
//! - Common data structures
//! - Utility functions

pub mod error;
pub mod span;
pub mod diagnostics;

pub use error::*;
pub use span::*;
pub use diagnostics::*;