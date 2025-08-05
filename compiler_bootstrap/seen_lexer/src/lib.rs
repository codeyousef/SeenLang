//! High-performance lexical analyzer for the Seen language
//! 
//! Target performance: >10M tokens/second
//! 
//! Features:
//! - SIMD-optimized tokenization
//! - Multilingual keyword support (English/Arabic via TOML)
//! - Zero-copy string handling where possible
//! - Comprehensive error recovery
//! - Memory-efficient token representation

pub mod token;
pub mod lexer;
pub mod language_config;
pub mod error_recovery;

pub use token::*;
pub use lexer::*;
pub use language_config::*;
pub use error_recovery::*;