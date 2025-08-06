//! High-performance parser for the Seen language
//! 
//! Target performance: >1M lines/second
//! 
//! Features:
//! - Recursive descent parser with operator precedence
//! - Memory-efficient AST representation using bump allocation
//! - Comprehensive error recovery with helpful diagnostics
//! - Source-to-AST mapping for IDE support
//! - Parse tree validation and consistency checks

pub mod ast;
pub mod parser;
pub mod precedence;
pub mod error_recovery;
pub mod visitor;
pub mod serialization;

pub use ast::*;
pub use parser::*;
pub use precedence::*;
pub use error_recovery::*;
pub use visitor::*;
pub use serialization::*;