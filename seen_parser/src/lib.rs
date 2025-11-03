//! Seen Language Parser
//!
//! This crate provides parsing for the Seen programming language,
//! implementing everything-as-expression design and complete AST generation.

pub mod ast;
pub mod error;
pub mod parser;

pub use ast::*;
pub use error::{ParseError, ParseResult};
pub use parser::Parser;
pub use seen_lexer::Position;

#[cfg(test)]
mod tests;
