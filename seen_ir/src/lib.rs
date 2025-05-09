//! LLVM IR generation for the Seen programming language
//!
//! This module is responsible for translating the AST from the parser
//! into LLVM IR for further optimization and code generation.

pub mod codegen;
pub mod error;
pub mod mapping;
pub mod types;

pub use codegen::CodeGenerator;
pub use error::CodeGenError;
