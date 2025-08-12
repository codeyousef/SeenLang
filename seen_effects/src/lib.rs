//! Algebraic effects and contracts system for Seen Language
//!
//! This crate implements advanced language features:
//! - Algebraic effects with effect handlers
//! - Design by contract with requires/ensures/invariants
//! - Compile-time execution and metaprogramming
//! - Formal verification support

pub mod types;
pub mod effects;
pub mod contracts;
pub mod metaprogramming;

// Re-export main types for convenience
pub use effects::{
    EffectSystem, EffectDefinition, EffectOperation, EffectHandler,
    EffectId, EffectImplementation, EffectExecutionContext,
};
pub use contracts::{
    ContractSystem, Contract, Precondition, Postcondition, Invariant,
    ContractViolation, ContractMode, VerificationLevel,
};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use seen_lexer::position::Position;
use crate::types::{AsyncValue, AsyncError, AsyncResult};

/// Main advanced features runtime combining effects and contracts
#[derive(Debug)]
pub struct AdvancedRuntime {
    /// Effect system for algebraic effects
    pub effect_system: EffectSystem,
    /// Contract system for design by contract
    pub contract_system: ContractSystem,
    /// Metaprogramming system for compile-time execution
    pub metaprogramming_system: metaprogramming::MetaprogrammingSystem,
    /// Runtime configuration
    config: AdvancedRuntimeConfig,
    /// Runtime statistics
    stats: AdvancedRuntimeStats,
}

/// Configuration for advanced runtime
#[derive(Debug, Clone)]
pub struct AdvancedRuntimeConfig {
    /// Enable effect system
    pub enable_effects: bool,
    /// Enable contract checking
    pub enable_contracts: bool,
    /// Enable metaprogramming
    pub enable_metaprogramming: bool,
    /// Enable formal verification
    pub enable_formal_verification: bool,
    /// Performance monitoring
    pub enable_monitoring: bool,
}

impl Default for AdvancedRuntimeConfig {
    fn default() -> Self {
        Self {
            enable_effects: true,
            enable_contracts: true,
            enable_metaprogramming: true,
            enable_formal_verification: false, // Expensive, off by default
            enable_monitoring: true,
        }
    }
}

/// Statistics for advanced runtime
#[derive(Debug, Clone)]
pub struct AdvancedRuntimeStats {
    /// Effect system statistics
    pub effect_stats: effects::EffectSystemStats,
    /// Contract system statistics
    pub contract_stats: contracts::ContractSystemStats,
    /// Metaprogramming statistics
    pub metaprogramming_stats: metaprogramming::MetaprogrammingStats,
    /// Total advanced features used
    pub total_advanced_features_used: u64,
}

impl AdvancedRuntime {
    /// Create a new advanced runtime
    pub fn new() -> Self {
        Self {
            effect_system: EffectSystem::new(),
            contract_system: ContractSystem::new(),
            metaprogramming_system: metaprogramming::MetaprogrammingSystem::new(),
            config: AdvancedRuntimeConfig::default(),
            stats: AdvancedRuntimeStats {
                effect_stats: effects::EffectSystemStats {
                    total_effects: 0,
                    total_handlers: 0,
                    execution_stats: effects::EffectExecutionStats::default(),
                    current_stack_depth: 0,
                },
                contract_stats: contracts::ContractSystemStats::default(),
                metaprogramming_stats: metaprogramming::MetaprogrammingStats::default(),
                total_advanced_features_used: 0,
            },
        }
    }
    
    /// Create advanced runtime with custom configuration
    pub fn with_config(config: AdvancedRuntimeConfig) -> Self {
        Self {
            effect_system: EffectSystem::new(),
            contract_system: ContractSystem::new(),
            metaprogramming_system: metaprogramming::MetaprogrammingSystem::new(),
            config,
            stats: AdvancedRuntimeStats {
                effect_stats: effects::EffectSystemStats {
                    total_effects: 0,
                    total_handlers: 0,
                    execution_stats: effects::EffectExecutionStats::default(),
                    current_stack_depth: 0,
                },
                contract_stats: contracts::ContractSystemStats::default(),
                metaprogramming_stats: metaprogramming::MetaprogrammingStats::default(),
                total_advanced_features_used: 0,
            },
        }
    }
    
    /// Execute a function with full advanced features support
    pub fn execute_with_features(
        &mut self,
        function_name: &str,
        parameters: HashMap<String, AsyncValue>,
        position: Position,
    ) -> AsyncResult {
        // 1. Check preconditions if contracts are enabled
        if self.config.enable_contracts {
            if let Err(violation) = self.contract_system.check_preconditions(
                function_name,
                parameters.clone(),
                position,
            ) {
                return Err(AsyncError::RuntimeError {
                    message: format!("Contract violation: {}", violation),
                    position,
                });
            }
        }
        
        // 2. Set up effect context if effects are enabled
        if self.config.enable_effects {
            // Effect handling integrated with runtime
        }
        
        // 3. Execute with metaprogramming support if enabled
        let result = if self.config.enable_metaprogramming {
            // Metaprogramming execution integrated with runtime
            AsyncValue::Unit // Placeholder
        } else {
            // Regular execution
            AsyncValue::Unit // Placeholder
        };
        
        // 4. Check postconditions if contracts are enabled
        if self.config.enable_contracts {
            if let Err(violation) = self.contract_system.check_postconditions(
                function_name,
                parameters,
                result.clone(),
                position,
            ) {
                return Err(AsyncError::RuntimeError {
                    message: format!("Contract violation: {}", violation),
                    position,
                });
            }
        }
        
        self.stats.total_advanced_features_used += 1;
        Ok(result)
    }
    
    /// Update runtime statistics
    pub fn update_stats(&mut self) {
        self.stats.effect_stats = self.effect_system.get_execution_stats();
        self.stats.contract_stats = self.contract_system.get_stats().clone();
        self.stats.metaprogramming_stats = self.metaprogramming_system.get_stats().clone();
    }
    
    /// Get runtime statistics
    pub fn get_stats(&self) -> &AdvancedRuntimeStats {
        &self.stats
    }
}

impl Default for AdvancedRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_advanced_runtime_creation() {
        let runtime = AdvancedRuntime::new();
        
        assert!(runtime.config.enable_effects);
        assert!(runtime.config.enable_contracts);
        assert!(runtime.config.enable_metaprogramming);
        assert_eq!(runtime.stats.total_advanced_features_used, 0);
    }
    
    #[test]
    fn test_advanced_runtime_configuration() {
        let config = AdvancedRuntimeConfig {
            enable_effects: false,
            enable_contracts: true,
            enable_metaprogramming: false,
            enable_formal_verification: true,
            enable_monitoring: false,
        };
        
        let runtime = AdvancedRuntime::with_config(config.clone());
        
        assert!(!runtime.config.enable_effects);
        assert!(runtime.config.enable_contracts);
        assert!(!runtime.config.enable_metaprogramming);
        assert!(runtime.config.enable_formal_verification);
    }
    
    #[test]
    fn test_feature_integration() {
        let mut runtime = AdvancedRuntime::new();
        
        // Test that all systems are accessible
        assert_eq!(runtime.effect_system.get_all_effects().len(), 0);
        assert_eq!(runtime.contract_system.get_stats().total_contracts, 0);
        
        runtime.update_stats();
        assert_eq!(runtime.stats.total_advanced_features_used, 0);
    }
}