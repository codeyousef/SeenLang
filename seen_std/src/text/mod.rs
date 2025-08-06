//! Text processing utilities for compiler development
//!
//! High-performance text manipulation data structures optimized for:
//! - Large file editing (incremental compilation)
//! - Efficient insertions/deletions 
//! - Fast substring operations
//! - Memory-efficient storage

pub mod rope;

pub use rope::Rope;