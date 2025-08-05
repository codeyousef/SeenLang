//! Integration tests for the Seen language compiler
//! 
//! These tests verify end-to-end functionality of the compiler pipeline:
//! - Lexical analysis
//! - Parsing  
//! - Type checking
//! - Code generation
//! - Runtime execution

pub mod lexer_integration;
pub mod parser_integration;
pub mod cli_integration;
pub mod examples_integration;