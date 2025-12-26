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

// Re-export public types
pub use types::{
    Avx10Width, CpuFeature, LinkOutput, LlvmOptLevel, MemoryTopologyHint,
    SveVectorLength, TargetOptions, LinkerFlavor, LinkerInvocation,
};

// Re-export the main backend from the original file
pub use super::llvm_backend::LlvmBackend;

