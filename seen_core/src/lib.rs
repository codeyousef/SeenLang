//! Core facade crate that re-exports the primary Seen compiler components.
//!
//! This crate exists to provide a stable surface for downstream tools (such as
//! the CLI) so they can depend on a single crate instead of wiring each
//! component individually. Features like the LLVM backend are forwarded to the
//! underlying crates.

// Support types and shared error handling helpers.
pub use seen_support::{SeenError, SeenErrorKind, SeenResult};

// Lexing facade.
pub use seen_lexer::{KeywordManager, Lexer, LexerConfig, Token, TokenType, VisibilityPolicy};

// Parsing facade.
pub use seen_parser::{
    precedence, BinaryOperator, Expression, Parser as SeenParser, Position, Program, Type,
    UnaryOperator,
};

// Type checker facade.
pub use seen_typechecker::TypeChecker;

// Intermediate representation facade.
pub use seen_ir::{
    HardwareProfile, IRGenerator, IROptimizer, IRProgram, OptimizationLevel, SimdPolicy,
};

// Interpreter facade.
pub use seen_interpreter::{
    Interpreter, RuntimeTraceEvent, RuntimeTraceFile, RuntimeTraceHandle, RuntimeTraceMetadata,
    RuntimeTraceValue, Value,
};

// Memory manager facade.
pub use seen_memory_manager::{
    MemoryAnalysisResult, MemoryManager, MemoryManagerConfig, MemoryTier, MemoryTopologyPreference,
};

// LSP facade (re-export entire crate for downstream tools).
pub use seen_lsp::*;

// Optional LLVM backend re-export.
#[cfg(feature = "llvm")]
pub use seen_ir::llvm_backend::{
    Avx10Width, CpuFeature, LinkOutput, LlvmBackend, MemoryTopologyHint, SveVectorLength,
    TargetOptions,
};

// Convenience module re-exports for consumers that still prefer module-level
// paths (e.g., seen_core::parser::Parser).
pub mod lexer {
    pub use seen_lexer::*;
}

pub mod parser {
    pub use seen_parser::*;
}

pub mod typechecker {
    pub use seen_typechecker::*;
}

pub mod ir {
    pub use seen_ir::*;
}

pub mod interpreter {
    pub use seen_interpreter::*;
}

pub mod memory {
    pub use seen_memory_manager::*;
}

pub mod support {
    pub use seen_support::*;
}

pub mod lsp {
    pub use seen_lsp::*;
}
