//! Reactive Programming features for Seen Language
//!
//! This crate implements reactive programming features according to Seen's syntax design:
//! - Observable streams with operators (Map, Filter, Throttle, etc.)
//! - Reactive properties with @Reactive and @Computed annotations
//! - Flow coroutines with Emit() and Delay() functions
//! - Automatic dependency tracking and change propagation
//! - Integration with async/await and concurrency systems

pub mod observable;
pub mod properties;
pub mod flow;

// Re-export main types for convenience
pub use observable::{Observable, ObservableFactory};
pub use properties::{ReactiveProperty, ComputedProperty, ReactivePropertyManager};
pub use flow::{Flow, FlowFactory, FlowCollector};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use seen_lexer::position::Position;
use seen_concurrency::types::{AsyncValue, AsyncError, AsyncResult};

/// Main reactive runtime for managing all reactive features
#[derive(Debug)]
pub struct ReactiveRuntime {
    /// Observable factory for creating observables
    pub observable_factory: ObservableFactory,
    /// Reactive property manager
    pub property_manager: ReactivePropertyManager,
    /// Active flows
    flows: HashMap<flow::FlowId, Box<dyn std::any::Any + Send>>,
    /// Runtime configuration
    config: ReactiveRuntimeConfig,
    /// Runtime statistics
    stats: ReactiveRuntimeStats,
}

/// Configuration for reactive runtime
#[derive(Debug, Clone)]
pub struct ReactiveRuntimeConfig {
    /// Maximum number of observables
    pub max_observables: usize,
    /// Maximum number of reactive properties
    pub max_properties: usize,
    /// Enable performance monitoring
    pub enable_monitoring: bool,
    /// Garbage collection interval for completed flows
    pub gc_interval_ms: u64,
}

impl Default for ReactiveRuntimeConfig {
    fn default() -> Self {
        Self {
            max_observables: 10000,
            max_properties: 10000,
            enable_monitoring: true,
            gc_interval_ms: 60000, // 1 minute
        }
    }
}

/// Runtime statistics for monitoring
#[derive(Debug, Clone)]
pub struct ReactiveRuntimeStats {
    /// Total observables created
    pub total_observables_created: u64,
    /// Total properties created
    pub total_properties_created: u64,
    /// Total flows created
    pub total_flows_created: u64,
    /// Active subscriptions
    pub active_subscriptions: usize,
    /// Memory usage (approximate)
    pub memory_usage_bytes: usize,
}

impl Default for ReactiveRuntimeStats {
    fn default() -> Self {
        Self {
            total_observables_created: 0,
            total_properties_created: 0,
            total_flows_created: 0,
            active_subscriptions: 0,
            memory_usage_bytes: 0,
        }
    }
}

impl ReactiveRuntime {
    /// Create a new reactive runtime
    pub fn new() -> Self {
        Self {
            observable_factory: ObservableFactory::new(),
            property_manager: ReactivePropertyManager::new(),
            flows: HashMap::new(),
            config: ReactiveRuntimeConfig::default(),
            stats: ReactiveRuntimeStats::default(),
        }
    }
    
    /// Create reactive runtime with custom configuration
    pub fn with_config(config: ReactiveRuntimeConfig) -> Self {
        Self {
            observable_factory: ObservableFactory::new(),
            property_manager: ReactivePropertyManager::new(),
            flows: HashMap::new(),
            config,
            stats: ReactiveRuntimeStats::default(),
        }
    }
    
    /// Create an observable from range (Seen syntax: Observable.Range(1, 10))
    pub fn create_observable_range(&mut self, start: i32, end: i32, step: i32) -> Observable<i32> {
        let observable = self.observable_factory.range(start, end, step);
        self.stats.total_observables_created += 1;
        observable
    }
    
    /// Create an observable from vector
    pub fn create_observable_from_vec<T>(&mut self, values: Vec<T>) -> Observable<T>
    where
        T: Clone + Send + 'static,
    {
        let observable = self.observable_factory.from_vec(values);
        self.stats.total_observables_created += 1;
        observable
    }
    
    /// Create a reactive property
    pub fn create_reactive_property(
        &mut self,
        name: String,
        initial_value: AsyncValue,
        property_type: seen_parser::ast::Type,
        is_mutable: bool,
        position: Position,
    ) -> properties::PropertyId {
        let property_id = self.property_manager.create_reactive_property(
            name, initial_value, property_type, is_mutable, position
        );
        self.stats.total_properties_created += 1;
        property_id
    }
    
    /// Create a computed property
    pub fn create_computed_property(
        &mut self,
        name: String,
        computation: seen_parser::ast::Expression,
        property_type: seen_parser::ast::Type,
        position: Position,
    ) -> properties::PropertyId {
        let property_id = self.property_manager.create_computed_property(
            name, computation, property_type, position
        );
        self.stats.total_properties_created += 1;
        property_id
    }
    
    /// Create a flow from vector
    pub fn create_flow_from_vec<T>(&mut self, values: Vec<T>) -> Flow<T>
    where
        T: Clone + Send + 'static,
    {
        let flow = FlowFactory::from_vec(values);
        self.stats.total_flows_created += 1;
        flow
    }
    
    /// Create a range flow
    pub fn create_flow_range(&mut self, start: i64, end: i64, step: i64) -> Flow<i64> {
        let flow = FlowFactory::range(start, end, step);
        self.stats.total_flows_created += 1;
        flow
    }
    
    /// Process all pending reactive updates
    pub fn process_updates(&mut self) -> Result<(), AsyncError> {
        self.property_manager.process_update_queue()?;
        Ok(())
    }
    
    /// Get runtime statistics
    pub fn get_stats(&self) -> &ReactiveRuntimeStats {
        &self.stats
    }
    
    /// Get observable statistics
    pub fn get_observable_stats(&self) -> observable::ObservableStats {
        self.observable_factory.get_stats()
    }
    
    /// Get property statistics
    pub fn get_property_stats(&self) -> properties::ReactiveStats {
        self.property_manager.get_stats()
    }
    
    /// Update runtime statistics
    pub fn update_stats(&mut self) {
        let observable_stats = self.get_observable_stats();
        let property_stats = self.get_property_stats();
        
        self.stats.active_subscriptions = observable_stats.total_subscriptions;
        
        // Approximate memory usage calculation
        self.stats.memory_usage_bytes = 
            (observable_stats.total_observables * 1024) + // ~1KB per observable
            (property_stats.total_reactive_properties * 512) + // ~512B per property
            (property_stats.total_computed_properties * 768); // ~768B per computed property
    }
}

impl Default for ReactiveRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating reactive patterns following Seen syntax
pub mod syntax {
    use super::*;
    use std::time::Duration;
    
    /// Create an observable that emits mouse clicks
    /// Seen syntax: let clicks: Observable<MouseEvent> = button.Clicks()
    pub fn mouse_clicks() -> Observable<MouseEvent> {
        // Simplified implementation - in real system would connect to UI events
        let mut factory = ObservableFactory::new();
        factory.from_vec(vec![
            MouseEvent { x: 100, y: 200, button: MouseButton::Left },
            MouseEvent { x: 150, y: 250, button: MouseButton::Left },
        ])
    }
    
    /// Create a reactive ViewModel following Seen syntax
    /// ```seen
    /// struct ViewModel {
    ///     @Reactive var Username = ""
    ///     @Reactive var Email = ""
    ///     @Computed let IsValid: Bool { return Username.isNotEmpty() and Email.contains("@") }
    /// }
    /// ```
    pub fn create_view_model(runtime: &mut ReactiveRuntime) -> ViewModelExample {
        let username_id = runtime.create_reactive_property(
            "Username".to_string(),
            AsyncValue::String("".to_string()),
            seen_parser::ast::Type {
                name: "String".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            },
            true,
            Position::new(1, 1, 0),
        );
        
        let email_id = runtime.create_reactive_property(
            "Email".to_string(),
            AsyncValue::String("".to_string()),
            seen_parser::ast::Type {
                name: "String".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            },
            true,
            Position::new(2, 1, 0),
        );
        
        // Create computed property for validation
        let validation_expr = seen_parser::ast::Expression::BinaryOp {
            left: Box::new(seen_parser::ast::Expression::Identifier {
                name: "Username".to_string(),
                pos: Position::new(3, 1, 0),
                is_public: false,
            }),
            right: Box::new(seen_parser::ast::Expression::Identifier {
                name: "Email".to_string(),
                pos: Position::new(3, 20, 0),
                is_public: false,
            }),
            op: seen_parser::ast::BinaryOperator::And,
            pos: Position::new(3, 10, 0),
        };
        
        let is_valid_id = runtime.create_computed_property(
            "IsValid".to_string(),
            validation_expr,
            seen_parser::ast::Type {
                name: "Bool".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            },
            Position::new(3, 1, 0),
        );
        
        ViewModelExample {
            username_id,
            email_id,
            is_valid_id,
        }
    }
}

/// Example mouse event for reactive UI
#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    pub button: MouseButton,
}

/// Mouse button types
#[derive(Debug, Clone)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Example ViewModel structure
#[derive(Debug)]
pub struct ViewModelExample {
    pub username_id: properties::PropertyId,
    pub email_id: properties::PropertyId,
    pub is_valid_id: properties::PropertyId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_reactive_runtime_creation() {
        let runtime = ReactiveRuntime::new();
        
        assert_eq!(runtime.stats.total_observables_created, 0);
        assert_eq!(runtime.stats.total_properties_created, 0);
        assert_eq!(runtime.stats.total_flows_created, 0);
    }
    
    #[test]
    fn test_reactive_runtime_observable_creation() {
        let mut runtime = ReactiveRuntime::new();
        
        let _observable = runtime.create_observable_range(1, 5, 1);
        
        assert_eq!(runtime.stats.total_observables_created, 1);
    }
    
    #[test]
    fn test_reactive_runtime_property_creation() {
        let mut runtime = ReactiveRuntime::new();
        
        let _property_id = runtime.create_reactive_property(
            "TestProperty".to_string(),
            AsyncValue::String("test".to_string()),
            seen_parser::ast::Type {
                name: "String".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            },
            true,
            Position::new(1, 1, 0),
        );
        
        assert_eq!(runtime.stats.total_properties_created, 1);
    }
    
    #[test]
    fn test_reactive_runtime_flow_creation() {
        let mut runtime = ReactiveRuntime::new();
        
        let _flow = runtime.create_flow_from_vec(vec![1, 2, 3]);
        
        assert_eq!(runtime.stats.total_flows_created, 1);
    }
    
    #[tokio::test]
    async fn test_syntax_helpers() {
        let _mouse_clicks = syntax::mouse_clicks();
        // Test that syntax helpers work
    }
    
    #[test]
    fn test_view_model_creation() {
        let mut runtime = ReactiveRuntime::new();
        let _view_model = syntax::create_view_model(&mut runtime);
        
        // Should have created 2 reactive properties and 1 computed property
        assert_eq!(runtime.stats.total_properties_created, 3);
    }
    
    #[test]
    fn test_runtime_stats_update() {
        let mut runtime = ReactiveRuntime::new();
        
        // Create some reactive elements
        let _observable = runtime.create_observable_range(1, 5, 1);
        let _property = runtime.create_reactive_property(
            "test".to_string(),
            AsyncValue::Integer(42),
            seen_parser::ast::Type {
                name: "Int".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            },
            true,
            Position::new(1, 1, 0),
        );
        
        runtime.update_stats();
        
        assert!(runtime.stats.memory_usage_bytes > 0);
    }
    
    #[test]
    fn test_runtime_configuration() {
        let config = ReactiveRuntimeConfig {
            max_observables: 5000,
            max_properties: 5000,
            enable_monitoring: false,
            gc_interval_ms: 30000,
        };
        
        let runtime = ReactiveRuntime::with_config(config.clone());
        
        assert_eq!(runtime.config.max_observables, 5000);
        assert!(!runtime.config.enable_monitoring);
    }
}