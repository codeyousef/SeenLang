//! Concurrency and Async features for Seen Language

pub mod async_runtime;
pub mod async_functions;
pub mod channels;
pub mod actors;
pub mod select;
pub mod types;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_concurrency_module() {
        // Test that the concurrency module exports are available
        use crate::types::AsyncValue;
        assert_eq!(AsyncValue::Unit, AsyncValue::Unit);
    }
}