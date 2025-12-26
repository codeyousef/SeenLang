//! Instruction handlers for the LLVM backend.
//!
//! This module organizes the instruction lowering code by instruction category.
//! Each submodule handles a specific category of IR instructions.
//!
//! The handlers are implemented as methods on `LlvmBackend` to maintain
//! access to all backend state. The organization here is for code clarity
//! and maintainability, not encapsulation.

pub mod call;

// Re-export commonly used items
pub use call::{categorize_call, normalize_method_name, get_result_method_alias, CallCategory};
