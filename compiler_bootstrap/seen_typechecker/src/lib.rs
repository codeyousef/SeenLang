//! Type system and inference engine for the Seen language
//! 
//! Target performance: <100μs per function
//! 
//! Features:
//! - Hindley-Milner type inference
//! - Generic type system with constraints
//! - C interop type mapping
//! - Incremental type checking for IDE responsiveness

pub mod types;
pub mod inference;
pub mod checker;
pub mod constraints;
#[cfg(test)]
mod tests;

pub use types::*;
pub use inference::*;
pub use checker::*;
pub use constraints::*;