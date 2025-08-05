//! Seen CLI library
//! 
//! This library provides the core functionality for the Seen programming language
//! command-line interface. It includes project management, configuration handling,
//! and command implementations.

pub mod commands;
pub mod config;
pub mod project;
pub mod utils;

// Re-export commonly used types
pub use config::*;
pub use project::*;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");