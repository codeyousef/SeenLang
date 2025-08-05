//! Performance tests and benchmarks for the Seen compiler
//! 
//! These tests verify that the compiler meets the performance targets:
//! - Lexer: >10M tokens/second
//! - Parser: >1M lines/second  
//! - Type checking: <100μs per function
//! - JIT startup: <50ms cold start

pub mod lexer_benchmarks;
pub mod parser_benchmarks;
pub mod end_to_end_benchmarks;