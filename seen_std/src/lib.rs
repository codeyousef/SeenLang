//! Seen Standard Library
//! 
//! High-performance, zero-cost abstractions for systems programming
//!
//! # Design Principles
//! - Zero-cost abstractions: Pay only for what you use
//! - Memory safety without garbage collection
//! - Optimal memory layout for cache efficiency
//! - SIMD-friendly data structures
//! - C-compatible ABI where needed

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(non_camel_case_types)]  // For primitive type names like I32

pub mod core;
pub mod collections;
pub mod string;
pub mod io;
pub mod error;
pub mod text;
pub mod ffi;
pub mod regex;
pub mod toml;
pub mod json;
pub mod pretty_simple;
pub mod graph;
pub mod testing;
pub mod formatting;
pub mod reactive;

// Re-export commonly used types
pub use core::primitives::*;
pub use collections::{Vec, HashMap, HashSet};
pub use string::{String, StringRef, StringBuilder};
pub use error::{Result, Option};

// Prelude for convenience
pub mod prelude {
    pub use crate::core::primitives::*;
    pub use crate::error::{Result, Option};
    pub use crate::collections::{Vec, HashMap};
    pub use crate::string::String;
    pub use crate::reactive::{Observable, Subject, BehaviorSubject};
}