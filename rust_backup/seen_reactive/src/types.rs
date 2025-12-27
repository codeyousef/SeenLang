//! Type definitions for reactive programming
//! Re-exports from concurrency module and additional reactive-specific types

pub use seen_concurrency::types::{AsyncValue, AsyncError, AsyncResult};

/// ID type for flows
pub type FlowId = u64;

/// Result type for flow operations
pub type FlowResult = Result<AsyncValue, AsyncError>;