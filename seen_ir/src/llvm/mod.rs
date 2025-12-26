//! LLVM backend that lowers Seen IR (IRProgram) to native code via inkwell.
//!
//! This module is organized by single responsibility principle:
//!
//! - [`types`] - Public types and configuration structures
//! - [`target`] - Target machine and linker infrastructure
//! - [`types_ir`] - IR-to-LLVM type conversion and type builders
//! - [`c_library`] - C library function declarations and wrappers
//! - [`memory`] - Memory operations and slot management
//! - [`string_ops`] - Runtime string operations (concat, substring, etc.)
//! - [`instructions`] - Instruction handlers and categorization
//! - [`type_inference`] - Register type inference and slot allocation
//! - [`runtime_fns`] - Runtime function declarations (boxing, string conversion)
//! - [`concurrency`] - Channel operations, spawn, scope, and await
//! - [`type_cast`] - Type casting operations (as_bool, as_i64, etc.)
//!
//! For now, this module re-exports the existing llvm_backend.rs implementation.
//! Future refactoring will extract more functionality into sub-modules.

pub mod types;
pub mod target;
pub mod types_ir;
pub mod c_library;
pub mod memory;
pub mod string_ops;
pub mod instructions;
pub mod type_inference;
pub mod runtime_fns;
pub mod concurrency;
pub mod type_cast;

// Re-export public types
pub use types::{
    Avx10Width, CpuFeature, LinkOutput, LlvmOptLevel, MemoryTopologyHint,
    SveVectorLength, TargetOptions, LinkerFlavor, LinkerInvocation,
};

// Re-export traits for external use
pub use runtime_fns::RuntimeFunctions;
pub use concurrency::ConcurrencyOps;
pub use type_cast::TypeCastOps;

// Re-export the main backend from the original file
pub use super::llvm_backend::LlvmBackend;

