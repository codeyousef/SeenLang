//! Algebraic effects system for Seen Language
//!
//! This module implements algebraic effects according to Seen's syntax design:
//! - effect IO { fun Read(): String; fun Write(s: String) }
//! - pure fun Add(a: Int, b: Int): Int = a + b
//! - fun ReadConfig(): String uses IO { return Read("/etc/config") }
//! - handle { ... } with IO { override fun Read() = "mocked" }
//! - Effect composition and type-safe effect tracking

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::any::{Any, TypeId};
use seen_lexer::position::Position;
use seen_parser::ast::{Expression, Type};
use crate::types::{AsyncValue, AsyncError, AsyncResult};

/// Unique identifier for effects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectId(u64);

impl EffectId {
    /// Create a new effect ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Unique identifier for effect operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectOperationId(u64);

impl EffectOperationId {
    /// Create a new effect operation ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Definition of an algebraic effect
#[derive(Debug, Clone)]
pub struct EffectDefinition {
    /// Unique effect identifier
    pub id: EffectId,
    /// Effect name
    pub name: String,
    /// Effect operations
    pub operations: HashMap<String, EffectOperation>,
    /// Effect metadata
    pub metadata: EffectMetadata,
    /// Type parameters for generic effects
    pub type_parameters: Vec<String>,
}

/// Individual operation within an effect
#[derive(Debug, Clone)]
pub struct EffectOperation {
    /// Unique operation identifier
    pub id: EffectOperationId,
    /// Operation name
    pub name: String,
    /// Operation parameters
    pub parameters: Vec<EffectParameter>,
    /// Return type
    pub return_type: Type,
    /// Whether operation is pure (no side effects)
    pub is_pure: bool,
    /// Operation metadata
    pub metadata: EffectOperationMetadata,
}

/// Parameter for effect operations
#[derive(Debug, Clone)]
pub struct EffectParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: Type,
    /// Whether parameter is mutable
    pub is_mutable: bool,
    /// Default value if any
    pub default_value: Option<AsyncValue>,
}

/// Metadata for effects
#[derive(Debug, Clone)]
pub struct EffectMetadata {
    /// Effect visibility (public/private based on name)
    pub is_public: bool,
    /// Position where effect is defined
    pub position: Position,
    /// Effect documentation
    pub documentation: Option<String>,
    /// Whether effect can be composed
    pub is_composable: bool,
    /// Effect safety level
    pub safety_level: EffectSafetyLevel,
}

/// Metadata for effect operations
#[derive(Debug, Clone)]
pub struct EffectOperationMetadata {
    /// Operation position
    pub position: Position,
    /// Operation documentation
    pub documentation: Option<String>,
    /// Estimated performance cost
    pub performance_cost: EffectCost,
    /// Whether operation can fail
    pub can_fail: bool,
}

/// Safety levels for effects
#[derive(Debug, Clone, PartialEq)]
pub enum EffectSafetyLevel {
    /// Pure effects with no side effects
    Pure,
    /// Safe effects with controlled side effects
    Safe,
    /// Unsafe effects that can cause system changes
    Unsafe,
    /// IO effects that interact with external world
    IO,
}

/// Performance cost estimation for effects
#[derive(Debug, Clone, PartialEq)]
pub enum EffectCost {
    /// Constant time operation
    Constant,
    /// Linear time operation
    Linear,
    /// Logarithmic time operation
    Logarithmic,
    /// Expensive operation (network, disk, etc.)
    Expensive,
    /// Unknown cost
    Unknown,
}

/// Handler for an effect
#[derive(Debug)]
pub struct EffectHandler {
    /// Effect being handled
    pub effect_id: EffectId,
    /// Handler name
    pub name: String,
    /// Operation implementations
    pub implementations: HashMap<String, Box<dyn EffectImplementation>>,
    /// Handler metadata
    pub metadata: EffectHandlerMetadata,
    /// Handler scope
    pub scope: EffectScope,
}

/// Metadata for effect handlers
#[derive(Debug, Clone)]
pub struct EffectHandlerMetadata {
    /// Handler position
    pub position: Position,
    /// Handler documentation
    pub documentation: Option<String>,
    /// Whether handler is fallible
    pub is_fallible: bool,
    /// Handler priority (for composition)
    pub priority: i32,
}

/// Scope for effect handlers
#[derive(Debug, Clone)]
pub enum EffectScope {
    /// Handler applies globally
    Global,
    /// Handler applies to specific function
    Function(String),
    /// Handler applies to specific block
    Block(Position),
    /// Handler applies to specific thread
    Thread,
}

/// Trait for effect operation implementations
pub trait EffectImplementation: Send + Sync + std::fmt::Debug {
    /// Execute the effect operation
    fn execute(&self, parameters: Vec<AsyncValue>) -> AsyncResult;
    
    /// Get operation name
    fn operation_name(&self) -> &str;
    
    /// Check if implementation is pure
    fn is_pure(&self) -> bool { false }
    
    /// Get estimated cost
    fn cost(&self) -> EffectCost { EffectCost::Unknown }
}

/// Context for effect execution
#[derive(Debug)]
pub struct EffectExecutionContext {
    /// Current effect stack
    pub effect_stack: Vec<EffectCall>,
    /// Active handlers
    pub active_handlers: HashMap<EffectId, Arc<EffectHandler>>,
    /// Execution statistics
    pub stats: EffectExecutionStats,
    /// Error handling mode
    pub error_mode: EffectErrorMode,
}

/// Individual effect call
#[derive(Debug, Clone)]
pub struct EffectCall {
    /// Effect being called
    pub effect_id: EffectId,
    /// Operation being called
    pub operation_name: String,
    /// Call parameters
    pub parameters: Vec<AsyncValue>,
    /// Call position
    pub position: Position,
    /// Call timestamp
    pub timestamp: std::time::Instant,
}

/// Statistics for effect execution
#[derive(Debug, Clone)]
pub struct EffectExecutionStats {
    /// Total effect calls
    pub total_calls: u64,
    /// Effect calls by type
    pub calls_by_effect: HashMap<EffectId, u64>,
    /// Total execution time
    pub total_execution_time_ms: u64,
    /// Failed effect calls
    pub failed_calls: u64,
    /// Pure effect calls (no side effects)
    pub pure_calls: u64,
}

/// Error handling modes for effects
pub enum EffectErrorMode {
    /// Propagate errors immediately
    Propagate,
    /// Collect errors and continue
    Collect,
    /// Ignore errors (dangerous)
    Ignore,
    /// Custom error handling
    Custom(Box<dyn Fn(AsyncError) -> EffectErrorAction + Send + Sync>),
}

impl std::fmt::Debug for EffectErrorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectErrorMode::Propagate => write!(f, "Propagate"),
            EffectErrorMode::Collect => write!(f, "Collect"),
            EffectErrorMode::Ignore => write!(f, "Ignore"),
            EffectErrorMode::Custom(_) => write!(f, "Custom"),
        }
    }
}

impl Clone for EffectErrorMode {
    fn clone(&self) -> Self {
        match self {
            EffectErrorMode::Propagate => EffectErrorMode::Propagate,
            EffectErrorMode::Collect => EffectErrorMode::Collect,
            EffectErrorMode::Ignore => EffectErrorMode::Ignore,
            EffectErrorMode::Custom(_) => EffectErrorMode::Propagate, // Fallback to propagate
        }
    }
}

/// Actions to take on effect errors
#[derive(Debug, Clone)]
pub enum EffectErrorAction {
    /// Propagate the error
    Propagate,
    /// Retry the operation
    Retry,
    /// Use default value
    UseDefault(AsyncValue),
    /// Skip the operation
    Skip,
}

/// Manager for all effects in the system
#[derive(Debug)]
pub struct EffectSystem {
    /// All effect definitions
    effects: HashMap<EffectId, EffectDefinition>,
    /// All effect handlers
    handlers: HashMap<EffectId, Vec<Arc<EffectHandler>>>,
    /// Effect execution contexts by thread
    execution_contexts: HashMap<std::thread::ThreadId, EffectExecutionContext>,
    /// Next available effect ID
    next_effect_id: u64,
    /// Next available operation ID
    next_operation_id: u64,
    /// System configuration
    config: EffectSystemConfig,
}

/// Configuration for effect system
#[derive(Debug, Clone)]
pub struct EffectSystemConfig {
    /// Maximum effect stack depth
    pub max_stack_depth: usize,
    /// Enable effect tracing
    pub enable_tracing: bool,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Default error handling mode
    pub default_error_mode: EffectErrorMode,
    /// Maximum handler priority
    pub max_handler_priority: i32,
}

impl Default for EffectSystemConfig {
    fn default() -> Self {
        Self {
            max_stack_depth: 100,
            enable_tracing: true,
            enable_performance_monitoring: true,
            default_error_mode: EffectErrorMode::Propagate,
            max_handler_priority: 1000,
        }
    }
}

impl EffectDefinition {
    /// Create a new effect definition
    pub fn new(name: String, position: Position) -> Self {
        let id = EffectId::new(rand::random());
        let is_public = name.chars().next().map_or(false, |c| c.is_uppercase());
        
        Self {
            id,
            name,
            operations: HashMap::new(),
            metadata: EffectMetadata {
                is_public,
                position,
                documentation: None,
                is_composable: true,
                safety_level: EffectSafetyLevel::Safe,
            },
            type_parameters: Vec::new(),
        }
    }
    
    /// Add an operation to the effect
    pub fn add_operation(&mut self, operation: EffectOperation) {
        self.operations.insert(operation.name.clone(), operation);
    }
    
    /// Get operation by name
    pub fn get_operation(&self, name: &str) -> Option<&EffectOperation> {
        self.operations.get(name)
    }
    
    /// Check if effect has operation
    pub fn has_operation(&self, name: &str) -> bool {
        self.operations.contains_key(name)
    }
    
    /// Set effect safety level
    pub fn with_safety_level(mut self, level: EffectSafetyLevel) -> Self {
        self.metadata.safety_level = level;
        self
    }
    
    /// Add type parameter
    pub fn add_type_parameter(&mut self, param: String) {
        self.type_parameters.push(param);
    }
    
    /// Get effect signature for display
    pub fn signature(&self) -> String {
        let type_params = if self.type_parameters.is_empty() {
            String::new()
        } else {
            format!("<{}>", self.type_parameters.join(", "))
        };
        
        let operations: Vec<String> = self.operations.values()
            .map(|op| op.signature())
            .collect();
        
        format!(
            "effect {}{} {{\n{}\n}}",
            self.name,
            type_params,
            operations.join("\n")
        )
    }
}

impl EffectOperation {
    /// Create a new effect operation
    pub fn new(
        name: String,
        parameters: Vec<EffectParameter>,
        return_type: Type,
        position: Position,
    ) -> Self {
        let id = EffectOperationId::new(rand::random());
        
        Self {
            id,
            name,
            parameters,
            return_type,
            is_pure: false,
            metadata: EffectOperationMetadata {
                position,
                documentation: None,
                performance_cost: EffectCost::Unknown,
                can_fail: true,
            },
        }
    }
    
    /// Mark operation as pure
    pub fn as_pure(mut self) -> Self {
        self.is_pure = true;
        self.metadata.can_fail = false;
        self
    }
    
    /// Set performance cost
    pub fn with_cost(mut self, cost: EffectCost) -> Self {
        self.metadata.performance_cost = cost;
        self
    }
    
    /// Get operation signature
    pub fn signature(&self) -> String {
        let params: Vec<String> = self.parameters.iter()
            .map(|p| format!("{}: {}", p.name, p.param_type.name))
            .collect();
        
        format!(
            "    fun {}({}): {}",
            self.name,
            params.join(", "),
            self.return_type.name
        )
    }
}

impl EffectHandler {
    /// Create a new effect handler
    pub fn new(effect_id: EffectId, name: String, position: Position) -> Self {
        Self {
            effect_id,
            name,
            implementations: HashMap::new(),
            metadata: EffectHandlerMetadata {
                position,
                documentation: None,
                is_fallible: false,
                priority: 0,
            },
            scope: EffectScope::Block(position),
        }
    }
    
    /// Add implementation for an operation
    pub fn add_implementation<I>(&mut self, operation_name: String, implementation: I)
    where
        I: EffectImplementation + 'static,
    {
        self.implementations.insert(operation_name, Box::new(implementation));
    }
    
    /// Execute an operation
    pub fn execute_operation(
        &self,
        operation_name: &str,
        parameters: Vec<AsyncValue>,
    ) -> AsyncResult {
        if let Some(implementation) = self.implementations.get(operation_name) {
            implementation.execute(parameters)
        } else {
            Err(AsyncError::RuntimeError {
                message: format!(
                    "No implementation for operation '{}' in handler '{}'",
                    operation_name, self.name
                ),
                position: self.metadata.position,
            })
        }
    }
    
    /// Check if handler can handle operation
    pub fn can_handle(&self, operation_name: &str) -> bool {
        self.implementations.contains_key(operation_name)
    }
    
    /// Set handler priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.metadata.priority = priority;
        self
    }
    
    /// Set handler scope
    pub fn with_scope(mut self, scope: EffectScope) -> Self {
        self.scope = scope;
        self
    }
}

impl EffectSystem {
    /// Create a new effect system
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
            handlers: HashMap::new(),
            execution_contexts: HashMap::new(),
            next_effect_id: 1,
            next_operation_id: 1,
            config: EffectSystemConfig::default(),
        }
    }
    
    /// Create effect system with custom configuration
    pub fn with_config(config: EffectSystemConfig) -> Self {
        Self {
            effects: HashMap::new(),
            handlers: HashMap::new(),
            execution_contexts: HashMap::new(),
            next_effect_id: 1,
            next_operation_id: 1,
            config,
        }
    }
    
    /// Register an effect definition
    pub fn register_effect(&mut self, effect: EffectDefinition) -> Result<EffectId, AsyncError> {
        let effect_id = effect.id;
        
        // Check for name conflicts
        for existing_effect in self.effects.values() {
            if existing_effect.name == effect.name {
                return Err(AsyncError::RuntimeError {
                    message: format!("Effect '{}' already exists", effect.name),
                    position: effect.metadata.position,
                });
            }
        }
        
        self.effects.insert(effect_id, effect);
        self.handlers.insert(effect_id, Vec::new());
        
        Ok(effect_id)
    }
    
    /// Register an effect handler
    pub fn register_handler(&mut self, handler: EffectHandler) -> Result<(), AsyncError> {
        let effect_id = handler.effect_id;
        
        // Check if effect exists
        if !self.effects.contains_key(&effect_id) {
            return Err(AsyncError::RuntimeError {
                message: format!("Effect {:?} not found for handler", effect_id),
                position: handler.metadata.position,
            });
        }
        
        // Add handler to the list for this effect
        let handlers = self.handlers.entry(effect_id).or_insert_with(Vec::new);
        handlers.push(Arc::new(handler));
        
        // Sort handlers by priority (highest first)
        handlers.sort_by(|a, b| b.metadata.priority.cmp(&a.metadata.priority));
        
        Ok(())
    }
    
    /// Call an effect operation
    pub fn call_effect(
        &mut self,
        effect_id: EffectId,
        operation_name: &str,
        parameters: Vec<AsyncValue>,
        position: Position,
    ) -> AsyncResult {
        let start_time = std::time::Instant::now();
        
        // Get effect name early to avoid borrowing issues
        let effect_name = self.get_effect_name(effect_id).unwrap_or("Unknown").to_string();
        
        // Get current thread context
        let thread_id = std::thread::current().id();
        let context = self.execution_contexts.entry(thread_id)
            .or_insert_with(|| EffectExecutionContext::new());
        
        // Check stack depth
        if context.effect_stack.len() >= self.config.max_stack_depth {
            return Err(AsyncError::RuntimeError {
                message: format!("Effect stack overflow (max depth: {})", self.config.max_stack_depth),
                position,
            });
        }
        
        // Create effect call
        let effect_call = EffectCall {
            effect_id,
            operation_name: operation_name.to_string(),
            parameters: parameters.clone(),
            position,
            timestamp: start_time,
        };
        
        // Push to stack
        context.effect_stack.push(effect_call.clone());
        
        // Find appropriate handler
        let result = if let Some(handlers) = self.handlers.get(&effect_id) {
            let mut result = None;
            
            for handler in handlers {
                if handler.can_handle(operation_name) {
                    result = Some(handler.execute_operation(operation_name, parameters.clone()));
                    break;
                }
            }
            
            result.unwrap_or_else(|| {
                Err(AsyncError::RuntimeError {
                    message: format!("No handler found for effect operation '{}::{}'", 
                        effect_name, operation_name),
                    position,
                })
            })
        } else {
            Err(AsyncError::RuntimeError {
                message: format!("Effect {:?} not registered", effect_id),
                position,
            })
        };
        
        // Pop from stack
        context.effect_stack.pop();
        
        // Update statistics
        context.stats.total_calls += 1;
        *context.stats.calls_by_effect.entry(effect_id).or_insert(0) += 1;
        context.stats.total_execution_time_ms += start_time.elapsed().as_millis() as u64;
        
        if result.is_err() {
            context.stats.failed_calls += 1;
        }
        
        // Check if operation is pure
        if let Some(effect) = self.effects.get(&effect_id) {
            if let Some(operation) = effect.get_operation(operation_name) {
                if operation.is_pure {
                    context.stats.pure_calls += 1;
                }
            }
        }
        
        result
    }
    
    /// Get effect by ID
    pub fn get_effect(&self, effect_id: EffectId) -> Option<&EffectDefinition> {
        self.effects.get(&effect_id)
    }
    
    /// Get effect by name
    pub fn get_effect_by_name(&self, name: &str) -> Option<&EffectDefinition> {
        self.effects.values().find(|effect| effect.name == name)
    }
    
    /// Get effect name by ID
    pub fn get_effect_name(&self, effect_id: EffectId) -> Option<&str> {
        self.effects.get(&effect_id).map(|effect| effect.name.as_str())
    }
    
    /// Get all effects
    pub fn get_all_effects(&self) -> Vec<&EffectDefinition> {
        self.effects.values().collect()
    }
    
    /// Get effect execution statistics
    pub fn get_execution_stats(&self) -> EffectSystemStats {
        let thread_id = std::thread::current().id();
        
        if let Some(context) = self.execution_contexts.get(&thread_id) {
            EffectSystemStats {
                total_effects: self.effects.len(),
                total_handlers: self.handlers.values().map(|h| h.len()).sum(),
                execution_stats: context.stats.clone(),
                current_stack_depth: context.effect_stack.len(),
            }
        } else {
            EffectSystemStats {
                total_effects: self.effects.len(),
                total_handlers: self.handlers.values().map(|h| h.len()).sum(),
                execution_stats: EffectExecutionStats::default(),
                current_stack_depth: 0,
            }
        }
    }
    
    /// Clear execution context for current thread
    pub fn clear_context(&mut self) {
        let thread_id = std::thread::current().id();
        self.execution_contexts.remove(&thread_id);
    }
}

impl Default for EffectSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectExecutionContext {
    /// Create new execution context
    pub fn new() -> Self {
        Self {
            effect_stack: Vec::new(),
            active_handlers: HashMap::new(),
            stats: EffectExecutionStats::default(),
            error_mode: EffectErrorMode::Propagate,
        }
    }
}

impl Default for EffectExecutionStats {
    fn default() -> Self {
        Self {
            total_calls: 0,
            calls_by_effect: HashMap::new(),
            total_execution_time_ms: 0,
            failed_calls: 0,
            pure_calls: 0,
        }
    }
}

/// System-wide effect statistics
#[derive(Debug, Clone)]
pub struct EffectSystemStats {
    /// Total number of registered effects
    pub total_effects: usize,
    /// Total number of registered handlers
    pub total_handlers: usize,
    /// Execution statistics
    pub execution_stats: EffectExecutionStats,
    /// Current effect stack depth
    pub current_stack_depth: usize,
}

/// Built-in IO effect implementation
#[derive(Debug)]
pub struct IOReadImplementation {
    /// Mock data for testing
    mock_data: Option<String>,
}

impl IOReadImplementation {
    pub fn new() -> Self {
        Self { mock_data: None }
    }
    
    pub fn with_mock_data(mut self, data: String) -> Self {
        self.mock_data = Some(data);
        self
    }
}

impl EffectImplementation for IOReadImplementation {
    fn execute(&self, parameters: Vec<AsyncValue>) -> AsyncResult {
        if let Some(ref mock) = self.mock_data {
            Ok(AsyncValue::String(mock.clone()))
        } else {
            // Read operation implementation
            if let Some(AsyncValue::String(path)) = parameters.first() {
                Ok(AsyncValue::String(format!("Content of {}", path)))
            } else {
                Err(AsyncError::RuntimeError {
                    message: "IO.Read requires string path parameter".to_string(),
                    position: Position::new(0, 0, 0),
                })
            }
        }
    }
    
    fn operation_name(&self) -> &str {
        "Read"
    }
    
    fn cost(&self) -> EffectCost {
        EffectCost::Expensive
    }
}

/// Built-in IO Write implementation
#[derive(Debug)]
pub struct IOWriteImplementation {
    /// Output buffer for testing
    output_buffer: Arc<Mutex<Vec<String>>>,
}

impl IOWriteImplementation {
    pub fn new() -> Self {
        Self {
            output_buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn get_output(&self) -> Vec<String> {
        self.output_buffer.lock().unwrap().clone()
    }
}

impl EffectImplementation for IOWriteImplementation {
    fn execute(&self, parameters: Vec<AsyncValue>) -> AsyncResult {
        if let Some(AsyncValue::String(content)) = parameters.first() {
            // Write operation implementation
            self.output_buffer.lock().unwrap().push(content.clone());
            Ok(AsyncValue::Unit)
        } else {
            Err(AsyncError::RuntimeError {
                message: "IO.Write requires string content parameter".to_string(),
                position: Position::new(0, 0, 0),
            })
        }
    }
    
    fn operation_name(&self) -> &str {
        "Write"
    }
    
    fn cost(&self) -> EffectCost {
        EffectCost::Expensive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_effect_definition_creation() {
        let effect = EffectDefinition::new("IO".to_string(), Position::new(1, 1, 0));
        
        assert_eq!(effect.name, "IO");
        assert!(effect.metadata.is_public); // Capital I = public
        assert!(effect.operations.is_empty());
    }
    
    #[test]
    fn test_effect_operation_creation() {
        let operation = EffectOperation::new(
            "Read".to_string(),
            vec![EffectParameter {
                name: "path".to_string(),
                param_type: Type { name: "String".to_string(), is_nullable: false, generics: Vec::new() },
                is_mutable: false,
                default_value: None,
            }],
            Type { name: "String".to_string(), is_nullable: false, generics: Vec::new() },
            Position::new(1, 1, 0),
        );
        
        assert_eq!(operation.name, "Read");
        assert_eq!(operation.parameters.len(), 1);
        assert!(!operation.is_pure);
    }
    
    #[test]
    fn test_effect_system_registration() {
        let mut system = EffectSystem::new();
        
        let mut effect = EffectDefinition::new("TestEffect".to_string(), Position::new(1, 1, 0));
        let operation = EffectOperation::new(
            "TestOp".to_string(),
            Vec::new(),
            Type { name: "Unit".to_string(), is_nullable: false, generics: Vec::new() },
            Position::new(1, 1, 0),
        );
        effect.add_operation(operation);
        
        let effect_id = system.register_effect(effect).unwrap();
        
        assert!(system.get_effect(effect_id).is_some());
        assert_eq!(system.get_effect_name(effect_id), Some("TestEffect"));
    }
    
    #[test]
    fn test_effect_handler_creation() {
        let effect_id = EffectId::new(1);
        let mut handler = EffectHandler::new(effect_id, "TestHandler".to_string(), Position::new(1, 1, 0));
        
        let implementation = IOReadImplementation::new().with_mock_data("test data".to_string());
        handler.add_implementation("Read".to_string(), implementation);
        
        assert!(handler.can_handle("Read"));
        assert!(!handler.can_handle("Write"));
    }
    
    #[test]
    fn test_io_effect_implementation() {
        let read_impl = IOReadImplementation::new().with_mock_data("test content".to_string());
        let result = read_impl.execute(vec![AsyncValue::String("/test/path".to_string())]).unwrap();
        
        assert_eq!(result, AsyncValue::String("test content".to_string()));
        assert_eq!(read_impl.operation_name(), "Read");
        assert_eq!(read_impl.cost(), EffectCost::Expensive);
    }
    
    #[test]
    fn test_io_write_implementation() {
        let write_impl = IOWriteImplementation::new();
        let result = write_impl.execute(vec![AsyncValue::String("test output".to_string())]);
        
        assert!(result.is_ok());
        assert_eq!(write_impl.get_output(), vec!["test output"]);
    }
    
    #[test]
    fn test_effect_system_call() {
        let mut system = EffectSystem::new();
        
        // Create IO effect
        let mut io_effect = EffectDefinition::new("IO".to_string(), Position::new(1, 1, 0))
            .with_safety_level(EffectSafetyLevel::IO);
        
        let read_op = EffectOperation::new(
            "Read".to_string(),
            vec![EffectParameter {
                name: "path".to_string(),
                param_type: Type { name: "String".to_string(), is_nullable: false, generics: Vec::new() },
                is_mutable: false,
                default_value: None,
            }],
            Type { name: "String".to_string(), is_nullable: false, generics: Vec::new() },
            Position::new(1, 1, 0),
        ).with_cost(EffectCost::Expensive);
        
        io_effect.add_operation(read_op);
        let effect_id = system.register_effect(io_effect).unwrap();
        
        // Create handler
        let mut handler = EffectHandler::new(effect_id, "IOHandler".to_string(), Position::new(1, 1, 0));
        let read_impl = IOReadImplementation::new().with_mock_data("file content".to_string());
        handler.add_implementation("Read".to_string(), read_impl);
        
        system.register_handler(handler).unwrap();
        
        // Call effect
        let result = system.call_effect(
            effect_id,
            "Read",
            vec![AsyncValue::String("/test/file".to_string())],
            Position::new(1, 1, 0),
        ).unwrap();
        
        assert_eq!(result, AsyncValue::String("file content".to_string()));
        
        let stats = system.get_execution_stats();
        assert_eq!(stats.execution_stats.total_calls, 1);
    }
    
    #[test]
    fn test_effect_signature() {
        let mut effect = EffectDefinition::new("TestEffect".to_string(), Position::new(1, 1, 0));
        effect.add_type_parameter("T".to_string());
        
        let operation = EffectOperation::new(
            "Process".to_string(),
            vec![EffectParameter {
                name: "input".to_string(),
                param_type: Type { name: "T".to_string(), is_nullable: false, generics: Vec::new() },
                is_mutable: false,
                default_value: None,
            }],
            Type { name: "T".to_string(), is_nullable: false, generics: Vec::new() },
            Position::new(1, 1, 0),
        );
        effect.add_operation(operation);
        
        let signature = effect.signature();
        assert!(signature.contains("TestEffect<T>"));
        assert!(signature.contains("fun Process(input: T): T"));
    }
}