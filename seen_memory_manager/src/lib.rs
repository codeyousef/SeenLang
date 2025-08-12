//! Vale-style Memory Management for Seen Language

pub mod memory_manager;
pub mod ownership;
pub mod regions;

pub use memory_manager::MemoryManager;

#[cfg(test)]
mod tests {
    #[test]
    fn test_memory_manager_placeholder() {
        assert!(true);
    }
}