//! Reactive properties system for Seen Language
//!
//! This module implements reactive properties according to Seen's syntax design:
//! - @Reactive var Username = ""
//! - @Computed let IsValid: Bool { return Username.isNotEmpty() and Email.contains("@") }
//! - Automatic dependency tracking and change propagation
//! - Integration with UI bindings and reactive updates

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;
use std::any::Any;
use seen_lexer::position::Position;
use seen_parser::ast::{Expression, Type};
use seen_concurrency::types::{AsyncValue, AsyncError, AsyncResult};
use crate::observable::{Observable, ObservableId};

/// Unique identifier for reactive properties
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyId(u64);

impl PropertyId {
    /// Create a new property ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Reactive property that can notify observers of changes
#[derive(Debug)]
pub struct ReactiveProperty {
    /// Unique property identifier
    pub id: PropertyId,
    /// Property name
    pub name: String,
    /// Current value
    value: AsyncValue,
    /// Property type
    property_type: Type,
    /// Whether property is mutable
    pub is_mutable: bool,
    /// Observers listening to this property
    observers: Arc<Mutex<HashMap<ObserverId, Observer>>>,
    /// Property metadata
    metadata: PropertyMetadata,
    /// Change history for debugging
    change_history: Vec<PropertyChange>,
}

/// Computed property that depends on other reactive properties
#[derive(Debug)]
pub struct ComputedProperty {
    /// Unique property identifier
    pub id: PropertyId,
    /// Property name
    pub name: String,
    /// Computation expression
    computation: Expression,
    /// Dependencies (other properties this depends on)
    dependencies: HashSet<PropertyId>,
    /// Cached computed value
    cached_value: Option<AsyncValue>,
    /// Whether cache is valid
    cache_valid: bool,
    /// Property type
    property_type: Type,
    /// Property metadata
    metadata: PropertyMetadata,
    /// Computation history for debugging
    computation_history: Vec<ComputationRecord>,
}

/// Property metadata for debugging and monitoring
#[derive(Debug, Clone)]
pub struct PropertyMetadata {
    /// Property visibility (public/private based on name)
    pub is_public: bool,
    /// Position where property is defined
    pub position: Position,
    /// Property creation time
    pub created_at: Instant,
    /// Total number of changes
    pub change_count: u64,
    /// Last change time
    pub last_changed: Option<Instant>,
    /// Whether property supports undo/redo
    pub supports_history: bool,
}

/// Observer of property changes
pub struct Observer {
    /// Unique observer identifier
    pub id: ObserverId,
    /// Callback function for property changes
    pub callback: Arc<dyn Fn(&PropertyChange) -> AsyncResult + Send + Sync>,
    /// Observer metadata
    pub metadata: ObserverMetadata,
    /// Whether observer is active
    pub is_active: bool,
}

impl std::fmt::Debug for Observer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observer")
            .field("id", &self.id)
            .field("metadata", &self.metadata)
            .field("is_active", &self.is_active)
            .finish()
    }
}

/// Unique identifier for observers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObserverId(u64);

impl ObserverId {
    /// Create a new observer ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Observer metadata
#[derive(Debug, Clone)]
pub struct ObserverMetadata {
    /// Observer name
    pub name: String,
    /// Observer creation time
    pub created_at: Instant,
    /// Number of notifications received
    pub notifications_received: u64,
    /// Last notification time
    pub last_notified: Option<Instant>,
}

/// Record of a property change
#[derive(Debug, Clone)]
pub struct PropertyChange {
    /// Property that changed
    pub property_id: PropertyId,
    /// Previous value
    pub old_value: AsyncValue,
    /// New value
    pub new_value: AsyncValue,
    /// Change timestamp
    pub timestamp: Instant,
    /// Change source (what caused the change)
    pub source: ChangeSource,
}

/// Source of a property change
#[derive(Debug, Clone)]
pub enum ChangeSource {
    /// Direct assignment
    DirectAssignment,
    /// Computed property recalculation
    ComputedRecalculation,
    /// External system update
    ExternalUpdate(String),
    /// Reactive propagation from dependency
    DependencyUpdate(PropertyId),
}

/// Record of a computation execution
#[derive(Debug, Clone)]
pub struct ComputationRecord {
    /// Computation timestamp
    pub timestamp: Instant,
    /// Computed result
    pub result: AsyncValue,
    /// Dependencies at computation time
    pub dependencies_snapshot: HashSet<PropertyId>,
    /// Computation duration
    pub duration_ms: u64,
}

/// Manager for all reactive properties in the system
#[derive(Debug)]
pub struct ReactivePropertyManager {
    /// All reactive properties
    properties: HashMap<PropertyId, ReactiveProperty>,
    /// All computed properties
    computed_properties: HashMap<PropertyId, ComputedProperty>,
    /// Dependency graph for efficient updates
    dependency_graph: DependencyGraph,
    /// Next available property ID
    next_property_id: u64,
    /// Next available observer ID
    next_observer_id: u64,
    /// Update queue for batch processing
    update_queue: Vec<PropertyId>,
    /// Configuration for reactive system
    config: ReactiveConfig,
}

/// Dependency graph for tracking property relationships
#[derive(Debug)]
pub struct DependencyGraph {
    /// Dependencies: property_id -> set of properties it depends on
    dependencies: HashMap<PropertyId, HashSet<PropertyId>>,
    /// Dependents: property_id -> set of properties that depend on it
    dependents: HashMap<PropertyId, HashSet<PropertyId>>,
}

/// Configuration for reactive property system
#[derive(Debug, Clone)]
pub struct ReactiveConfig {
    /// Maximum dependency depth to prevent cycles
    pub max_dependency_depth: usize,
    /// Whether to batch property updates
    pub batch_updates: bool,
    /// Maximum history size for properties
    pub max_history_size: usize,
    /// Whether to enable debug tracing
    pub enable_debug_tracing: bool,
}

impl Default for ReactiveConfig {
    fn default() -> Self {
        Self {
            max_dependency_depth: 50,
            batch_updates: true,
            max_history_size: 100,
            enable_debug_tracing: false,
        }
    }
}

impl ReactiveProperty {
    /// Create a new reactive property
    pub fn new(
        name: String,
        initial_value: AsyncValue,
        property_type: Type,
        is_mutable: bool,
        position: Position,
    ) -> Self {
        let id = PropertyId::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );
        let is_public = name.chars().next().map_or(false, |c| c.is_uppercase());
        
        Self {
            id,
            name,
            value: initial_value,
            property_type,
            is_mutable,
            observers: Arc::new(Mutex::new(HashMap::new())),
            metadata: PropertyMetadata {
                is_public,
                position,
                created_at: Instant::now(),
                change_count: 0,
                last_changed: None,
                supports_history: true,
            },
            change_history: Vec::new(),
        }
    }
    
    /// Get the current value
    pub fn get(&self) -> &AsyncValue {
        &self.value
    }
    
    /// Set a new value (if mutable)
    pub fn set(&mut self, new_value: AsyncValue) -> Result<(), AsyncError> {
        if !self.is_mutable {
            return Err(AsyncError::RuntimeError {
                message: format!("Property '{}' is not mutable", self.name),
                position: self.metadata.position,
            });
        }
        
        let old_value = self.value.clone();
        
        // Validate type compatibility
        if !self.is_value_compatible(&new_value) {
            return Err(AsyncError::RuntimeError {
                message: format!(
                    "Value type mismatch for property '{}': expected {}, got {:?}",
                    self.name, self.property_type.name, new_value
                ),
                position: self.metadata.position,
            });
        }
        
        // Update value
        self.value = new_value.clone();
        
        // Record change
        let change = PropertyChange {
            property_id: self.id,
            old_value,
            new_value,
            timestamp: Instant::now(),
            source: ChangeSource::DirectAssignment,
        };
        
        // Add to history
        self.change_history.push(change.clone());
        if self.change_history.len() > 100 { // Max history size
            self.change_history.remove(0);
        }
        
        // Update metadata
        self.metadata.change_count += 1;
        self.metadata.last_changed = Some(change.timestamp);
        
        // Notify observers
        self.notify_observers(&change);
        
        Ok(())
    }
    
    /// Add an observer to this property
    pub fn add_observer<F>(&mut self, name: String, callback: F) -> ObserverId
    where
        F: Fn(&PropertyChange) -> AsyncResult + Send + Sync + 'static,
    {
        let observer_id = ObserverId::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );
        
        let observer = Observer {
            id: observer_id,
            callback: Arc::new(callback),
            metadata: ObserverMetadata {
                name,
                created_at: Instant::now(),
                notifications_received: 0,
                last_notified: None,
            },
            is_active: true,
        };
        
        {
            let mut observers = self.observers.lock().unwrap();
            observers.insert(observer_id, observer);
        }
        
        observer_id
    }
    
    /// Remove an observer
    pub fn remove_observer(&mut self, observer_id: ObserverId) -> Result<(), AsyncError> {
        let mut observers = self.observers.lock().unwrap();
        
        if observers.remove(&observer_id).is_some() {
            Ok(())
        } else {
            Err(AsyncError::RuntimeError {
                message: format!("Observer {:?} not found", observer_id),
                position: self.metadata.position,
            })
        }
    }
    
    /// Notify all observers of a change
    fn notify_observers(&self, change: &PropertyChange) {
        let observers = self.observers.lock().unwrap();
        
        for observer in observers.values() {
            if observer.is_active {
                match (observer.callback)(change) {
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!("Observer notification error: {:?}", error);
                    }
                }
            }
        }
    }
    
    /// Check if a value is compatible with the property type
    fn is_value_compatible(&self, value: &AsyncValue) -> bool {
        // Type checking using runtime type information
        match (self.property_type.name.as_str(), value) {
            ("Int", AsyncValue::Integer(_)) => true,
            ("Float", AsyncValue::Float(_)) => true,
            ("String", AsyncValue::String(_)) => true,
            ("Bool", AsyncValue::Boolean(_)) => true,
            ("Unit", AsyncValue::Unit) => true,
            _ => false,
        }
    }
    
    /// Get property metadata
    pub fn metadata(&self) -> &PropertyMetadata {
        &self.metadata
    }
    
    /// Get change history
    pub fn get_change_history(&self) -> &[PropertyChange] {
        &self.change_history
    }
    
    /// Get number of active observers
    pub fn observer_count(&self) -> usize {
        self.observers.lock().unwrap().len()
    }
}

impl ComputedProperty {
    /// Create a new computed property
    pub fn new(
        name: String,
        computation: Expression,
        property_type: Type,
        position: Position,
    ) -> Self {
        let id = PropertyId::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );
        let is_public = name.chars().next().map_or(false, |c| c.is_uppercase());
        
        Self {
            id,
            name,
            computation,
            dependencies: HashSet::new(),
            cached_value: None,
            cache_valid: false,
            property_type,
            metadata: PropertyMetadata {
                is_public,
                position,
                created_at: Instant::now(),
                change_count: 0,
                last_changed: None,
                supports_history: false, // Computed properties don't need history
            },
            computation_history: Vec::new(),
        }
    }
    
    /// Get the computed value (recompute if cache invalid)
    pub fn get(&mut self, property_manager: &ReactivePropertyManager) -> Result<&AsyncValue, AsyncError> {
        if !self.cache_valid {
            self.recompute(property_manager)?;
        }
        
        Ok(self.cached_value.as_ref().unwrap())
    }
    
    /// Recompute the value based on dependencies
    pub fn recompute(&mut self, property_manager: &ReactivePropertyManager) -> Result<(), AsyncError> {
        let start_time = Instant::now();
        
        // Extract computation reference before borrowing self mutably
        let computation = self.computation.clone();
        
        // Execute computation expression
        let result = self.execute_computation(&computation, property_manager)?;
        
        // Update cache
        self.cached_value = Some(result.clone());
        self.cache_valid = true;
        
        // Record computation
        let record = ComputationRecord {
            timestamp: start_time,
            result,
            dependencies_snapshot: self.dependencies.clone(),
            duration_ms: start_time.elapsed().as_millis() as u64,
        };
        
        self.computation_history.push(record);
        if self.computation_history.len() > 50 { // Max computation history
            self.computation_history.remove(0);
        }
        
        // Update metadata
        self.metadata.change_count += 1;
        self.metadata.last_changed = Some(start_time);
        
        Ok(())
    }
    
    /// Execute computation expression
    fn execute_computation(
        &mut self,
        expr: &Expression,
        property_manager: &ReactivePropertyManager,
    ) -> Result<AsyncValue, AsyncError> {
        // Computation execution using expression evaluator
        match expr {
            Expression::Identifier { name, pos, .. } => {
                // Look up property value
                if let Some(property) = property_manager.find_property_by_name(name) {
                    // Add to dependencies
                    self.dependencies.insert(property.id);
                    Ok(property.get().clone())
                } else {
                    Err(AsyncError::RuntimeError {
                        message: format!("Property '{}' not found in computation", name),
                        position: *pos,
                    })
                }
            }
            Expression::BinaryOp { left, right, op, .. } => {
                let left_val = self.execute_computation(left, property_manager)?;
                let right_val = self.execute_computation(right, property_manager)?;
                
                // Simplified binary operations
                use seen_parser::ast::BinaryOperator;
                match (left_val, right_val, op) {
                    (AsyncValue::Boolean(a), AsyncValue::Boolean(b), BinaryOperator::And) => {
                        Ok(AsyncValue::Boolean(a && b))
                    }
                    (AsyncValue::Boolean(a), AsyncValue::Boolean(b), BinaryOperator::Or) => {
                        Ok(AsyncValue::Boolean(a || b))
                    }
                    (AsyncValue::String(a), AsyncValue::String(b), BinaryOperator::Add) => {
                        Ok(AsyncValue::String(format!("{}{}", a, b)))
                    }
                    _ => Ok(AsyncValue::Unit),
                }
            }
            Expression::BooleanLiteral { value, .. } => Ok(AsyncValue::Boolean(*value)),
            Expression::StringLiteral { value, .. } => Ok(AsyncValue::String(value.clone())),
            Expression::IntegerLiteral { value, .. } => Ok(AsyncValue::Integer(*value)),
            _ => Ok(AsyncValue::Unit),
        }
    }
    
    /// Invalidate cache when dependencies change
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }
    
    /// Add a dependency
    pub fn add_dependency(&mut self, property_id: PropertyId) {
        self.dependencies.insert(property_id);
    }
    
    /// Get dependencies
    pub fn get_dependencies(&self) -> &HashSet<PropertyId> {
        &self.dependencies
    }
    
    /// Get computation history
    pub fn get_computation_history(&self) -> &[ComputationRecord] {
        &self.computation_history
    }
}

impl ReactivePropertyManager {
    /// Create a new reactive property manager
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
            computed_properties: HashMap::new(),
            dependency_graph: DependencyGraph::new(),
            next_property_id: 1,
            next_observer_id: 1,
            update_queue: Vec::new(),
            config: ReactiveConfig::default(),
        }
    }
    
    /// Create a new reactive property
    pub fn create_reactive_property(
        &mut self,
        name: String,
        initial_value: AsyncValue,
        property_type: Type,
        is_mutable: bool,
        position: Position,
    ) -> PropertyId {
        let property = ReactiveProperty::new(name, initial_value, property_type, is_mutable, position);
        let property_id = property.id;
        
        self.properties.insert(property_id, property);
        property_id
    }
    
    /// Create a new computed property
    pub fn create_computed_property(
        &mut self,
        name: String,
        computation: Expression,
        property_type: Type,
        position: Position,
    ) -> PropertyId {
        let property = ComputedProperty::new(name, computation, property_type, position);
        let property_id = property.id;
        
        self.computed_properties.insert(property_id, property);
        property_id
    }
    
    /// Get a reactive property by ID
    pub fn get_reactive_property(&self, property_id: PropertyId) -> Option<&ReactiveProperty> {
        self.properties.get(&property_id)
    }
    
    /// Get a mutable reactive property by ID
    pub fn get_reactive_property_mut(&mut self, property_id: PropertyId) -> Option<&mut ReactiveProperty> {
        self.properties.get_mut(&property_id)
    }
    
    /// Get a computed property by ID
    pub fn get_computed_property_mut(&mut self, property_id: PropertyId) -> Option<&mut ComputedProperty> {
        self.computed_properties.get_mut(&property_id)
    }
    
    /// Find property by name
    pub fn find_property_by_name(&self, name: &str) -> Option<&ReactiveProperty> {
        self.properties.values().find(|p| p.name == name)
    }
    
    /// Set a reactive property value
    pub fn set_property_value(&mut self, property_id: PropertyId, value: AsyncValue) -> Result<(), AsyncError> {
        if let Some(property) = self.properties.get_mut(&property_id) {
            property.set(value)?;
            
            // Queue dependent properties for update
            if let Some(dependents) = self.dependency_graph.dependents.get(&property_id) {
                for &dependent_id in dependents {
                    self.update_queue.push(dependent_id);
                }
            }
            
            // Process updates if batching is disabled
            if !self.config.batch_updates {
                self.process_update_queue()?;
            }
            
            Ok(())
        } else {
            Err(AsyncError::RuntimeError {
                message: format!("Property {:?} not found", property_id),
                position: Position::new(0, 0, 0),
            })
        }
    }
    
    /// Process the update queue
    pub fn process_update_queue(&mut self) -> Result<(), AsyncError> {
        // Process one item at a time to avoid borrowing conflicts
        while let Some(property_id) = self.update_queue.pop() {
            if self.computed_properties.contains_key(&property_id) {
                // Extract the computed property temporarily
                if let Some(mut computed) = self.computed_properties.remove(&property_id) {
                    computed.invalidate_cache();
                    computed.recompute(self)?;
                    // Put it back
                    self.computed_properties.insert(property_id, computed);
                }
            }
        }
        Ok(())
    }
    
    /// Add dependency relationship
    pub fn add_dependency(&mut self, dependent: PropertyId, dependency: PropertyId) {
        self.dependency_graph.add_dependency(dependent, dependency);
    }
    
    /// Get system statistics
    pub fn get_stats(&self) -> ReactiveStats {
        ReactiveStats {
            total_reactive_properties: self.properties.len(),
            total_computed_properties: self.computed_properties.len(),
            total_dependencies: self.dependency_graph.dependencies.values()
                .map(|deps| deps.len())
                .sum(),
            pending_updates: self.update_queue.len(),
        }
    }
}

impl Default for ReactivePropertyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    /// Create a new dependency graph
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
        }
    }
    
    /// Add a dependency relationship
    pub fn add_dependency(&mut self, dependent: PropertyId, dependency: PropertyId) {
        // Add to dependencies map
        self.dependencies.entry(dependent)
            .or_insert_with(HashSet::new)
            .insert(dependency);
        
        // Add to dependents map
        self.dependents.entry(dependency)
            .or_insert_with(HashSet::new)
            .insert(dependent);
    }
    
    /// Remove a dependency relationship
    pub fn remove_dependency(&mut self, dependent: PropertyId, dependency: PropertyId) {
        if let Some(deps) = self.dependencies.get_mut(&dependent) {
            deps.remove(&dependency);
        }
        
        if let Some(deps) = self.dependents.get_mut(&dependency) {
            deps.remove(&dependent);
        }
    }
    
    /// Check for circular dependencies
    pub fn has_circular_dependency(&self, property: PropertyId) -> bool {
        let mut visited = HashSet::new();
        self.dfs_check_cycle(property, &mut visited)
    }
    
    /// Depth-first search to check for cycles
    fn dfs_check_cycle(&self, current: PropertyId, visited: &mut HashSet<PropertyId>) -> bool {
        if visited.contains(&current) {
            return true; // Cycle detected
        }
        
        visited.insert(current);
        
        if let Some(dependencies) = self.dependencies.get(&current) {
            for &dependency in dependencies {
                if self.dfs_check_cycle(dependency, visited) {
                    return true;
                }
            }
        }
        
        visited.remove(&current);
        false
    }
}

/// Statistics about the reactive property system
#[derive(Debug, Clone)]
pub struct ReactiveStats {
    /// Total number of reactive properties
    pub total_reactive_properties: usize,
    /// Total number of computed properties
    pub total_computed_properties: usize,
    /// Total number of dependency relationships
    pub total_dependencies: usize,
    /// Number of pending updates
    pub pending_updates: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_lexer::position::Position;
    
    #[test]
    fn test_reactive_property_creation() {
        let property = ReactiveProperty::new(
            "Username".to_string(),
            AsyncValue::String("Alice".to_string()),
            Type { name: "String".to_string(), is_nullable: false, generics: Vec::new() },
            true,
            Position::new(1, 1, 0),
        );
        
        assert_eq!(property.name, "Username");
        assert!(property.is_mutable);
        assert!(property.metadata.is_public); // Capital U = public
        assert_eq!(property.observer_count(), 0);
    }
    
    #[test]
    fn test_reactive_property_set_get() {
        let mut property = ReactiveProperty::new(
            "count".to_string(),
            AsyncValue::Integer(0),
            Type { name: "Int".to_string(), is_nullable: false, generics: Vec::new() },
            true,
            Position::new(1, 1, 0),
        );
        
        assert_eq!(*property.get(), AsyncValue::Integer(0));
        
        property.set(AsyncValue::Integer(42)).unwrap();
        assert_eq!(*property.get(), AsyncValue::Integer(42));
        assert_eq!(property.metadata.change_count, 1);
    }
    
    #[test]
    fn test_reactive_property_observer() {
        let mut property = ReactiveProperty::new(
            "testProp".to_string(),
            AsyncValue::String("initial".to_string()),
            Type { name: "String".to_string(), is_nullable: false, generics: Vec::new() },
            true,
            Position::new(1, 1, 0),
        );
        
        let changed_values = Arc::new(Mutex::new(Vec::new()));
        let changed_clone = changed_values.clone();
        
        let _observer_id = property.add_observer(
            "test_observer".to_string(),
            move |change| {
                changed_clone.lock().unwrap().push(change.new_value.clone());
                Ok(AsyncValue::Unit)
            },
        );
        
        property.set(AsyncValue::String("changed".to_string())).unwrap();
        
        let changes = changed_values.lock().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0], AsyncValue::String("changed".to_string()));
    }
    
    #[test]
    fn test_computed_property_creation() {
        let computation = Expression::BooleanLiteral {
            value: true,
            pos: Position::new(1, 1, 0),
        };
        
        let property = ComputedProperty::new(
            "IsValid".to_string(),
            computation,
            Type { name: "Bool".to_string(), is_nullable: false, generics: Vec::new() },
            Position::new(1, 1, 0),
        );
        
        assert_eq!(property.name, "IsValid");
        assert!(property.metadata.is_public); // Capital I = public
        assert!(!property.cache_valid);
    }
    
    #[test]
    fn test_reactive_property_manager() {
        let mut manager = ReactivePropertyManager::new();
        
        let property_id = manager.create_reactive_property(
            "Username".to_string(),
            AsyncValue::String("Alice".to_string()),
            Type { name: "String".to_string(), is_nullable: false, generics: Vec::new() },
            true,
            Position::new(1, 1, 0),
        );
        
        assert!(manager.get_reactive_property(property_id).is_some());
        
        manager.set_property_value(property_id, AsyncValue::String("Bob".to_string())).unwrap();
        
        let property = manager.get_reactive_property(property_id).unwrap();
        assert_eq!(*property.get(), AsyncValue::String("Bob".to_string()));
    }
    
    #[test]
    fn test_dependency_graph() {
        let mut graph = DependencyGraph::new();
        
        let prop1 = PropertyId::new(1);
        let prop2 = PropertyId::new(2);
        
        graph.add_dependency(prop1, prop2);
        
        assert!(graph.dependencies.get(&prop1).unwrap().contains(&prop2));
        assert!(graph.dependents.get(&prop2).unwrap().contains(&prop1));
    }
    
    #[test]
    fn test_manager_stats() {
        let mut manager = ReactivePropertyManager::new();
        
        let _reactive_id = manager.create_reactive_property(
            "count".to_string(),
            AsyncValue::Integer(0),
            Type { name: "Int".to_string(), is_nullable: false, generics: Vec::new() },
            true,
            Position::new(1, 1, 0),
        );
        
        let computation = Expression::BooleanLiteral {
            value: true,
            pos: Position::new(1, 1, 0),
        };
        
        let _computed_id = manager.create_computed_property(
            "IsEven".to_string(),
            computation,
            Type { name: "Bool".to_string(), is_nullable: false, generics: Vec::new() },
            Position::new(1, 1, 0),
        );
        
        let stats = manager.get_stats();
        assert_eq!(stats.total_reactive_properties, 1);
        assert_eq!(stats.total_computed_properties, 1);
    }
}