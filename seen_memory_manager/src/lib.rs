//! Vale-style Memory Management for Seen Language
//!
//! This crate provides automatic memory management without garbage collection
//! through ownership analysis and region-based allocation. The system ensures
//! memory safety at compile time with zero runtime overhead.
//!
//! ## Features
//!
//! - **Automatic Ownership Inference**: No manual lifetime annotations required
//! - **Region-based Allocation**: Efficient memory management through regions
//! - **Move/Borrow Semantics**: Prevents use-after-free and double-free errors
//! - **Zero Runtime Overhead**: All analysis happens at compile time
//! - **Integration with Type System**: Works seamlessly with Seen's type checker
//!
//! ## Example Usage
//!
//! ```rust
//! use seen_memory_manager::{MemoryManager, MemoryManagerConfig};
//! use seen_parser::ast::Program;
//!
//! let mut manager = MemoryManager::new();
//! let result = manager.analyze_program(&program);
//!
//! if result.has_errors() {
//!     for error in result.get_errors() {
//!         println!("Memory error: {}", error);
//!     }
//! }
//!
//! for optimization in result.get_optimizations() {
//!     println!("Optimization: {}", optimization);
//! }
//! ```

pub mod memory_manager;
pub mod ownership;
pub mod regions;

// Re-export main types for convenience
pub use memory_manager::{
    MemoryManager, 
    MemoryManagerConfig, 
    MemoryAnalysisResult, 
    MemoryError, 
    MemoryOptimization
};
pub use ownership::{
    OwnershipAnalyzer, 
    OwnershipInfo, 
    OwnershipMode, 
    OwnershipError, 
    VariableOwnership
};
pub use regions::{
    RegionManager, 
    RegionAnalyzer, 
    Region, 
    RegionId, 
    RegionError
};

#[cfg(test)]
mod tests {
    use super::*;
    use seen_parser::ast::{Program, Expression};
    use seen_lexer::Position;
    
    #[test]
    fn test_integration() {
        let mut manager = MemoryManager::new();
        
        // Create a simple program for testing
        let program = Program {
            expressions: vec![
                Expression::Let {
                    name: "test_var".to_string(),
                    type_annotation: None,
                    value: Box::new(Expression::IntegerLiteral {
                        value: 42,
                        pos: Position::new(1, 9, 8),
                    }),
                    is_mutable: false,
                    pos: Position::new(1, 1, 0),
                }
            ],
        };
        
        let result = manager.analyze_program(&program);
        
        // Basic integration test - should have analyzed the variable
        assert!(result.ownership_info.variables.contains_key("test_var"));
        assert!(result.region_manager.is_allocated("test_var"));
    }
    
    #[test]
    fn test_memory_manager_config() {
        let config = MemoryManagerConfig {
            aggressive_optimizations: true,
            enable_region_merging: false,
            max_region_depth: 5,
            validate_move_semantics: true,
            suggest_lifetime_optimizations: false,
        };
        
        let manager = MemoryManager::with_config(config);
        assert!(manager.config().aggressive_optimizations);
        assert!(!manager.config().enable_region_merging);
        assert_eq!(manager.config().max_region_depth, 5);
    }
}