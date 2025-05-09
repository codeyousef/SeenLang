//! Standard library for the Seen programming language
//!
//! This crate provides the core standard library functionality for the Seen programming language.
//! It is designed to be used by the Seen compiler as a runtime dependency.

// Export the core MVP functions
pub mod core_mvp;

// Re-export the core functions for convenience
pub use core_mvp::*;
