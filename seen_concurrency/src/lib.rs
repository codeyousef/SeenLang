//! Concurrency and Async features for Seen Language

pub mod actors;
pub mod async_functions;
pub mod async_runtime;
pub mod channels;
pub mod jobs;
pub mod select;
pub mod types;

#[cfg(test)]
mod tests {
    #[test]
    fn test_concurrency_module() {
        // Test that the concurrency module exports are available
        use crate::types::AsyncValue;
        assert_eq!(AsyncValue::Unit, AsyncValue::Unit);
    }
}
