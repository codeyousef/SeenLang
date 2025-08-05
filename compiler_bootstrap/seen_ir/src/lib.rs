//! Intermediate representation and code generation for the Seen language
//! 
//! Features:
//! - LLVM backend with efficient IR generation
//! - Debug information generation (DWARF)
//! - C ABI compatibility layer
//! - Cross-compilation support

pub mod ir;
pub mod codegen;
pub mod llvm_backend;

pub use ir::*;
pub use codegen::*;
pub use llvm_backend::*;