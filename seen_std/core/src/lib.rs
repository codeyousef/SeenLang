//! Core standard library for the Seen language
//! 
//! This module provides fundamental types and operations that are always available
//! in Seen programs without explicit imports.

pub mod primitives;
pub mod result;
pub mod option;
pub mod memory;
pub mod traits;

// Re-export core types that should be available in the global namespace
pub use primitives::*;
pub use result::Result;
pub use option::Option;
pub use traits::*;