//! Vale-style memory model implementation for Seen
//! 
//! This module implements a region-based memory management system inspired by Vale,
//! providing memory safety without garbage collection and with minimal overhead.
//!
//! Key features:
//! - Region inference for automatic memory management
//! - Generational references for safe memory access
//! - Escape analysis integration
//! - <5% performance overhead requirement

pub mod regions;
pub mod references;
pub mod analysis;
pub mod runtime;

pub use regions::*;
pub use references::*;
pub use analysis::*;
pub use runtime::*;