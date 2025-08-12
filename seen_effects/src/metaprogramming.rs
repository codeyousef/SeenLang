//! Compile-time execution and metaprogramming for Seen Language
//!
//! This module implements metaprogramming according to Seen's syntax design:
//! - comptime blocks for compile-time execution
//! - Macro system with hygiene and proper scoping
//! - Template metaprogramming with type-safe generation
//! - Reflection capabilities for runtime type inspection
//! - Code generation with AST manipulation

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::any::{Any, TypeId};
use std::time::{Duration, Instant};
use seen_lexer::position::Position;
use seen_parser::ast::{Expression, Type};
use crate::types::{AsyncValue, AsyncError, AsyncResult};

/// Unique identifier for compile-time contexts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComptimeContextId(u64);

impl ComptimeContextId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Unique identifier for macros
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroId(u64);

impl MacroId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Unique identifier for templates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TemplateId(u64);

impl TemplateId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Compile-time execution context
#[derive(Debug, Clone)]
pub struct ComptimeContext {
    /// Context identifier
    pub id: ComptimeContextId,
    /// Context name
    pub name: String,
    /// Variables available at compile time
    pub variables: HashMap<String, ComptimeValue>,
    /// Functions available at compile time
    pub functions: HashMap<String, ComptimeFunction>,
    /// Imported modules
    pub imports: Vec<String>,
    /// Context metadata
    pub metadata: ComptimeMetadata,
}

/// Metadata for compile-time contexts
#[derive(Debug, Clone)]
pub struct ComptimeMetadata {
    /// Position where context is defined
    pub position: Position,
    /// Context creation time
    pub created_at: Instant,
    /// Whether context is isolated
    pub is_isolated: bool,
    /// Maximum execution time
    pub max_execution_time: Duration,
    /// Memory limit
    pub memory_limit_bytes: usize,
}

/// Value that can exist at compile time
#[derive(Debug, Clone)]
pub enum ComptimeValue {
    /// Primitive values that can be computed at compile time
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    /// Type information
    Type(ComptimeType),
    /// AST nodes that can be manipulated
    AstNode(Expression),
    /// Arrays of compile-time values
    Array(Vec<ComptimeValue>),
    /// Functions that can be called at compile time
    Function(ComptimeFunction),
    /// Custom compile-time objects
    Object(HashMap<String, ComptimeValue>),
}

/// Type information available at compile time
#[derive(Debug, Clone)]
pub struct ComptimeType {
    /// Type name
    pub name: String,
    /// Type parameters
    pub parameters: Vec<ComptimeType>,
    /// Type size in bytes (if known)
    pub size_bytes: Option<usize>,
    /// Type alignment (if known)
    pub alignment: Option<usize>,
    /// Type metadata
    pub metadata: ComptimeTypeMetadata,
}

/// Metadata for compile-time types
#[derive(Debug, Clone)]
pub struct ComptimeTypeMetadata {
    /// Whether type is primitive
    pub is_primitive: bool,
    /// Whether type is reference
    pub is_reference: bool,
    /// Whether type is nullable
    pub is_nullable: bool,
    /// Type methods
    pub methods: Vec<String>,
    /// Type fields
    pub fields: Vec<String>,
}

/// Function that can be called at compile time
#[derive(Debug, Clone)]
pub struct ComptimeFunction {
    /// Function name
    pub name: String,
    /// Function parameters
    pub parameters: Vec<ComptimeParameter>,
    /// Function return type
    pub return_type: ComptimeType,
    /// Function body (AST or builtin)
    pub body: ComptimeFunctionBody,
    /// Function metadata
    pub metadata: ComptimeFunctionMetadata,
}

/// Parameter for compile-time functions
#[derive(Debug, Clone)]
pub struct ComptimeParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: ComptimeType,
    /// Default value
    pub default_value: Option<ComptimeValue>,
}

/// Body of a compile-time function
#[derive(Debug, Clone)]
pub enum ComptimeFunctionBody {
    /// AST expression to evaluate
    Expression(Expression),
    /// Built-in function
    Builtin(BuiltinFunction),
    /// External function that can be called at compile time
    External(String),
}

/// Built-in compile-time functions
#[derive(Debug, Clone)]
pub enum BuiltinFunction {
    /// Get type information
    TypeOf,
    /// Get size of type
    SizeOf,
    /// Get alignment of type
    AlignOf,
    /// Check if type has method
    HasMethod,
    /// Get all methods of type
    GetMethods,
    /// Generate code from template
    GenerateCode,
    /// Parse string as AST
    ParseAst,
    /// Convert AST to string
    AstToString,
}

/// Metadata for compile-time functions
#[derive(Debug, Clone)]
pub struct ComptimeFunctionMetadata {
    /// Function position
    pub position: Position,
    /// Whether function is pure (no side effects)
    pub is_pure: bool,
    /// Whether function can be memoized
    pub is_memoizable: bool,
    /// Execution complexity
    pub complexity: ExecutionComplexity,
}

/// Execution complexity levels
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionComplexity {
    /// Constant time
    Constant,
    /// Linear time
    Linear,
    /// Quadratic time
    Quadratic,
    /// Exponential time (dangerous)
    Exponential,
    /// Unknown complexity
    Unknown,
}

/// Macro definition for code generation
#[derive(Debug, Clone)]
pub struct MacroDefinition {
    /// Macro identifier
    pub id: MacroId,
    /// Macro name
    pub name: String,
    /// Macro parameters
    pub parameters: Vec<MacroParameter>,
    /// Macro body
    pub body: Expression,
    /// Macro hygiene rules
    pub hygiene: MacroHygiene,
    /// Macro metadata
    pub metadata: MacroMetadata,
}

/// Parameter for macros
#[derive(Debug, Clone)]
pub struct MacroParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type constraint
    pub type_constraint: Option<ComptimeType>,
    /// Whether parameter is variadic
    pub is_variadic: bool,
}

/// Hygiene rules for macros
#[derive(Debug, Clone)]
pub struct MacroHygiene {
    /// Variables that should be captured from call site
    pub captured_variables: Vec<String>,
    /// Variables that should be introduced into call site
    pub introduced_variables: Vec<String>,
    /// Whether macro respects lexical scoping
    pub respects_scoping: bool,
}

/// Metadata for macros
#[derive(Debug, Clone)]
pub struct MacroMetadata {
    /// Macro position
    pub position: Position,
    /// Macro documentation
    pub documentation: Option<String>,
    /// Whether macro is experimental
    pub is_experimental: bool,
    /// Macro expansion limit
    pub expansion_limit: usize,
}

/// Template for generic code generation
#[derive(Debug, Clone)]
pub struct Template {
    /// Template identifier
    pub id: TemplateId,
    /// Template name
    pub name: String,
    /// Template parameters
    pub parameters: Vec<TemplateParameter>,
    /// Template body
    pub body: Vec<Expression>,
    /// Template constraints
    pub constraints: Vec<TemplateConstraint>,
    /// Template metadata
    pub metadata: TemplateMetadata,
}

/// Parameter for templates
#[derive(Debug, Clone)]
pub struct TemplateParameter {
    /// Parameter name
    pub name: String,
    /// Parameter kind (type, value, etc.)
    pub kind: TemplateParameterKind,
    /// Default value
    pub default: Option<ComptimeValue>,
}

/// Kinds of template parameters
#[derive(Debug, Clone)]
pub enum TemplateParameterKind {
    /// Type parameter
    Type,
    /// Value parameter
    Value(ComptimeType),
    /// Template parameter
    Template,
}

/// Constraint on template parameters
#[derive(Debug, Clone)]
pub struct TemplateConstraint {
    /// Constraint expression
    pub expression: Expression,
    /// Error message if constraint fails
    pub error_message: String,
}

/// Metadata for templates
#[derive(Debug, Clone)]
pub struct TemplateMetadata {
    /// Template position
    pub position: Position,
    /// Template documentation
    pub documentation: Option<String>,
    /// Whether template is recursive
    pub is_recursive: bool,
    /// Maximum instantiation depth
    pub max_depth: usize,
}

/// Result of compile-time execution
#[derive(Debug, Clone)]
pub enum ComptimeResult {
    /// Successful execution with result
    Value(ComptimeValue),
    /// Execution produced AST
    Ast(Expression),
    /// Execution produced multiple expressions
    Code(Vec<Expression>),
    /// Execution failed
    Error(ComptimeError),
}

/// Errors that can occur during compile-time execution
#[derive(Debug, Clone)]
pub enum ComptimeError {
    /// Execution timeout
    Timeout {
        limit: Duration,
        elapsed: Duration,
    },
    /// Memory limit exceeded
    OutOfMemory {
        limit: usize,
        used: usize,
    },
    /// Infinite recursion detected
    InfiniteRecursion {
        depth: usize,
        function: String,
    },
    /// Type error during execution
    TypeError {
        expected: ComptimeType,
        actual: ComptimeType,
        position: Position,
    },
    /// Undefined variable or function
    Undefined {
        name: String,
        position: Position,
    },
    /// Macro expansion error
    MacroExpansionError {
        macro_name: String,
        error: String,
        position: Position,
    },
    /// Template instantiation error
    TemplateError {
        template_name: String,
        error: String,
        position: Position,
    },
}

/// Main metaprogramming system
#[derive(Debug)]
pub struct MetaprogrammingSystem {
    /// All compile-time contexts
    contexts: HashMap<ComptimeContextId, ComptimeContext>,
    /// Global compile-time variables
    global_variables: HashMap<String, ComptimeValue>,
    /// Registered macros
    macros: HashMap<MacroId, MacroDefinition>,
    /// Registered templates
    templates: HashMap<TemplateId, Template>,
    /// Built-in compile-time functions
    builtins: HashMap<String, BuiltinFunction>,
    /// Next available IDs
    next_context_id: u64,
    next_macro_id: u64,
    next_template_id: u64,
    /// System configuration
    config: MetaprogrammingConfig,
    /// System statistics
    stats: MetaprogrammingStats,
}

/// Configuration for metaprogramming system
#[derive(Debug, Clone)]
pub struct MetaprogrammingConfig {
    /// Enable compile-time execution
    pub enable_comptime: bool,
    /// Enable macro system
    pub enable_macros: bool,
    /// Enable templates
    pub enable_templates: bool,
    /// Default execution timeout
    pub default_timeout: Duration,
    /// Default memory limit
    pub default_memory_limit: usize,
    /// Maximum recursion depth
    pub max_recursion_depth: usize,
}

impl Default for MetaprogrammingConfig {
    fn default() -> Self {
        Self {
            enable_comptime: true,
            enable_macros: true,
            enable_templates: true,
            default_timeout: Duration::from_secs(10),
            default_memory_limit: 64 * 1024 * 1024, // 64MB
            max_recursion_depth: 1000,
        }
    }
}

/// Statistics for metaprogramming system
#[derive(Debug, Clone)]
pub struct MetaprogrammingStats {
    /// Total compile-time executions
    pub total_comptime_executions: u64,
    /// Total macro expansions
    pub total_macro_expansions: u64,
    /// Total template instantiations
    pub total_template_instantiations: u64,
    /// Total execution time
    pub total_execution_time: Duration,
    /// Memory usage
    pub memory_usage_bytes: usize,
    /// Execution errors
    pub execution_errors: u64,
}

impl Default for MetaprogrammingStats {
    fn default() -> Self {
        Self {
            total_comptime_executions: 0,
            total_macro_expansions: 0,
            total_template_instantiations: 0,
            total_execution_time: Duration::ZERO,
            memory_usage_bytes: 0,
            execution_errors: 0,
        }
    }
}

impl MetaprogrammingSystem {
    /// Create a new metaprogramming system
    pub fn new() -> Self {
        let mut system = Self {
            contexts: HashMap::new(),
            global_variables: HashMap::new(),
            macros: HashMap::new(),
            templates: HashMap::new(),
            builtins: HashMap::new(),
            next_context_id: 1,
            next_macro_id: 1,
            next_template_id: 1,
            config: MetaprogrammingConfig::default(),
            stats: MetaprogrammingStats::default(),
        };
        
        // Register built-in functions
        system.register_builtins();
        system
    }
    
    /// Create system with custom configuration
    pub fn with_config(config: MetaprogrammingConfig) -> Self {
        let mut system = Self {
            contexts: HashMap::new(),
            global_variables: HashMap::new(),
            macros: HashMap::new(),
            templates: HashMap::new(),
            builtins: HashMap::new(),
            next_context_id: 1,
            next_macro_id: 1,
            next_template_id: 1,
            config,
            stats: MetaprogrammingStats::default(),
        };
        
        system.register_builtins();
        system
    }
    
    /// Execute code at compile time
    pub fn execute_comptime(
        &mut self,
        expression: Expression,
        context_id: Option<ComptimeContextId>,
        position: Position,
    ) -> ComptimeResult {
        if !self.config.enable_comptime {
            return ComptimeResult::Error(ComptimeError::Undefined {
                name: "comptime execution disabled".to_string(),
                position,
            });
        }
        
        let start_time = Instant::now();
        self.stats.total_comptime_executions += 1;
        
        // Get or create context
        let context_id = context_id.unwrap_or_else(|| {
            self.create_context("default".to_string(), position)
        });
        
        let result = self.evaluate_expression(&expression, context_id, position);
        
        let elapsed = start_time.elapsed();
        self.stats.total_execution_time += elapsed;
        
        // Check timeout
        if elapsed > self.config.default_timeout {
            self.stats.execution_errors += 1;
            return ComptimeResult::Error(ComptimeError::Timeout {
                limit: self.config.default_timeout,
                elapsed,
            });
        }
        
        result
    }
    
    /// Expand a macro
    pub fn expand_macro(
        &mut self,
        macro_id: MacroId,
        arguments: Vec<ComptimeValue>,
        position: Position,
    ) -> ComptimeResult {
        if !self.config.enable_macros {
            return ComptimeResult::Error(ComptimeError::MacroExpansionError {
                macro_name: "unknown".to_string(),
                error: "Macro system disabled".to_string(),
                position,
            });
        }
        
        self.stats.total_macro_expansions += 1;
        
        if let Some(macro_def) = self.macros.get(&macro_id).cloned() {
            // Validate arguments
            if arguments.len() != macro_def.parameters.len() {
                self.stats.execution_errors += 1;
                return ComptimeResult::Error(ComptimeError::MacroExpansionError {
                    macro_name: macro_def.name.clone(),
                    error: format!(
                        "Expected {} arguments, got {}",
                        macro_def.parameters.len(),
                        arguments.len()
                    ),
                    position,
                });
            }
            
            // Create expansion context
            let context_id = self.create_context(
                format!("macro_{}", macro_def.name),
                position,
            );
            
            // Bind arguments to parameters
            if let Some(context) = self.contexts.get_mut(&context_id) {
                for (param, arg) in macro_def.parameters.iter().zip(arguments.iter()) {
                    context.variables.insert(param.name.clone(), arg.clone());
                }
            }
            
            // Expand macro body
            self.evaluate_expression(&macro_def.body, context_id, position)
        } else {
            self.stats.execution_errors += 1;
            ComptimeResult::Error(ComptimeError::Undefined {
                name: format!("macro {:?}", macro_id),
                position,
            })
        }
    }
    
    /// Instantiate a template
    pub fn instantiate_template(
        &mut self,
        template_id: TemplateId,
        arguments: HashMap<String, ComptimeValue>,
        position: Position,
    ) -> ComptimeResult {
        if !self.config.enable_templates {
            return ComptimeResult::Error(ComptimeError::TemplateError {
                template_name: "unknown".to_string(),
                error: "Template system disabled".to_string(),
                position,
            });
        }
        
        self.stats.total_template_instantiations += 1;
        
        if let Some(template) = self.templates.get(&template_id).cloned() {
            // Validate constraints
            for constraint in &template.constraints {
                // Evaluate constraint in context with arguments
                let context_id = self.create_context(
                    format!("template_{}_constraint", template.name),
                    position,
                );
                
                if let Some(context) = self.contexts.get_mut(&context_id) {
                    for (name, value) in &arguments {
                        context.variables.insert(name.clone(), value.clone());
                    }
                }
                
                match self.evaluate_expression(&constraint.expression, context_id, position) {
                    ComptimeResult::Value(ComptimeValue::Boolean(true)) => {
                        // Constraint satisfied
                        continue;
                    }
                    ComptimeResult::Value(ComptimeValue::Boolean(false)) => {
                        self.stats.execution_errors += 1;
                        return ComptimeResult::Error(ComptimeError::TemplateError {
                            template_name: template.name.clone(),
                            error: constraint.error_message.clone(),
                            position,
                        });
                    }
                    _ => {
                        self.stats.execution_errors += 1;
                        return ComptimeResult::Error(ComptimeError::TemplateError {
                            template_name: template.name.clone(),
                            error: "Failed to evaluate constraint".to_string(),
                            position,
                        });
                    }
                }
            }
            
            // Generate code from template
            ComptimeResult::Code(template.body.clone())
        } else {
            self.stats.execution_errors += 1;
            ComptimeResult::Error(ComptimeError::Undefined {
                name: format!("template {:?}", template_id),
                position,
            })
        }
    }
    
    /// Create a new compile-time context
    pub fn create_context(&mut self, name: String, position: Position) -> ComptimeContextId {
        let id = ComptimeContextId::new(self.next_context_id);
        self.next_context_id += 1;
        
        let context = ComptimeContext {
            id,
            name,
            variables: HashMap::new(),
            functions: HashMap::new(),
            imports: Vec::new(),
            metadata: ComptimeMetadata {
                position,
                created_at: Instant::now(),
                is_isolated: false,
                max_execution_time: self.config.default_timeout,
                memory_limit_bytes: self.config.default_memory_limit,
            },
        };
        
        self.contexts.insert(id, context);
        id
    }
    
    /// Register a macro
    pub fn register_macro(&mut self, macro_def: MacroDefinition) -> MacroId {
        let macro_id = macro_def.id;
        self.macros.insert(macro_id, macro_def);
        macro_id
    }
    
    /// Register a template
    pub fn register_template(&mut self, template: Template) -> TemplateId {
        let template_id = template.id;
        self.templates.insert(template_id, template);
        template_id
    }
    
    /// Get system statistics
    pub fn get_stats(&self) -> &MetaprogrammingStats {
        &self.stats
    }
    
    /// Register built-in functions
    fn register_builtins(&mut self) {
        self.builtins.insert("typeof".to_string(), BuiltinFunction::TypeOf);
        self.builtins.insert("sizeof".to_string(), BuiltinFunction::SizeOf);
        self.builtins.insert("alignof".to_string(), BuiltinFunction::AlignOf);
        self.builtins.insert("hasmethod".to_string(), BuiltinFunction::HasMethod);
        self.builtins.insert("getmethods".to_string(), BuiltinFunction::GetMethods);
        self.builtins.insert("generatecode".to_string(), BuiltinFunction::GenerateCode);
        self.builtins.insert("parseast".to_string(), BuiltinFunction::ParseAst);
        self.builtins.insert("asttostring".to_string(), BuiltinFunction::AstToString);
    }
    
    /// Evaluate an expression in compile-time context
    fn evaluate_expression(
        &self,
        expression: &Expression,
        context_id: ComptimeContextId,
        position: Position,
    ) -> ComptimeResult {
        match expression {
            Expression::IntegerLiteral { value, .. } => {
                ComptimeResult::Value(ComptimeValue::Integer(*value))
            }
            Expression::FloatLiteral { value, .. } => {
                ComptimeResult::Value(ComptimeValue::Float(*value))
            }
            Expression::BooleanLiteral { value, .. } => {
                ComptimeResult::Value(ComptimeValue::Boolean(*value))
            }
            Expression::StringLiteral { value, .. } => {
                ComptimeResult::Value(ComptimeValue::String(value.clone()))
            }
            Expression::Identifier { name, .. } => {
                if let Some(context) = self.contexts.get(&context_id) {
                    if let Some(value) = context.variables.get(name) {
                        ComptimeResult::Value(value.clone())
                    } else if let Some(value) = self.global_variables.get(name) {
                        ComptimeResult::Value(value.clone())
                    } else {
                        ComptimeResult::Error(ComptimeError::Undefined {
                            name: name.clone(),
                            position,
                        })
                    }
                } else {
                    ComptimeResult::Error(ComptimeError::Undefined {
                        name: format!("context {:?}", context_id),
                        position,
                    })
                }
            }
            _ => {
                // For now, return the AST itself for complex expressions
                ComptimeResult::Ast(expression.clone())
            }
        }
    }
}

impl Default for MetaprogrammingSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metaprogramming_system_creation() {
        let system = MetaprogrammingSystem::new();
        
        assert!(system.config.enable_comptime);
        assert!(system.config.enable_macros);
        assert!(system.config.enable_templates);
        assert_eq!(system.stats.total_comptime_executions, 0);
        assert!(!system.builtins.is_empty()); // Should have built-ins
    }
    
    #[test]
    fn test_comptime_context_creation() {
        let mut system = MetaprogrammingSystem::new();
        
        let context_id = system.create_context("test".to_string(), Position::new(1, 1, 0));
        
        assert!(system.contexts.contains_key(&context_id));
        let context = system.contexts.get(&context_id).unwrap();
        assert_eq!(context.name, "test");
    }
    
    #[test]
    fn test_comptime_execution() {
        let mut system = MetaprogrammingSystem::new();
        
        let expression = Expression::IntegerLiteral {
            value: 42,
            pos: Position::new(1, 1, 0),
        };
        
        let result = system.execute_comptime(expression, None, Position::new(1, 1, 0));
        
        match result {
            ComptimeResult::Value(ComptimeValue::Integer(42)) => {
                // Success
            }
            _ => panic!("Expected integer 42, got {:?}", result),
        }
        
        assert_eq!(system.stats.total_comptime_executions, 1);
    }
    
    #[test]
    fn test_macro_definition() {
        let macro_def = MacroDefinition {
            id: MacroId::new(1),
            name: "test_macro".to_string(),
            parameters: vec![MacroParameter {
                name: "x".to_string(),
                type_constraint: None,
                is_variadic: false,
            }],
            body: Expression::Identifier {
                name: "x".to_string(),
                is_public: false,
                pos: Position::new(1, 1, 0),
            },
            hygiene: MacroHygiene {
                captured_variables: Vec::new(),
                introduced_variables: Vec::new(),
                respects_scoping: true,
            },
            metadata: MacroMetadata {
                position: Position::new(1, 1, 0),
                documentation: None,
                is_experimental: false,
                expansion_limit: 100,
            },
        };
        
        let mut system = MetaprogrammingSystem::new();
        let macro_id = system.register_macro(macro_def);
        
        assert!(system.macros.contains_key(&macro_id));
    }
    
    #[test]
    fn test_template_definition() {
        let template = Template {
            id: TemplateId::new(1),
            name: "test_template".to_string(),
            parameters: vec![TemplateParameter {
                name: "T".to_string(),
                kind: TemplateParameterKind::Type,
                default: None,
            }],
            body: Vec::new(),
            constraints: Vec::new(),
            metadata: TemplateMetadata {
                position: Position::new(1, 1, 0),
                documentation: None,
                is_recursive: false,
                max_depth: 10,
            },
        };
        
        let mut system = MetaprogrammingSystem::new();
        let template_id = system.register_template(template);
        
        assert!(system.templates.contains_key(&template_id));
    }
    
    #[test]
    fn test_comptime_value_types() {
        let values = vec![
            ComptimeValue::Integer(42),
            ComptimeValue::Float(3.14),
            ComptimeValue::Boolean(true),
            ComptimeValue::String("hello".to_string()),
            ComptimeValue::Array(vec![ComptimeValue::Integer(1), ComptimeValue::Integer(2)]),
        ];
        
        assert_eq!(values.len(), 5);
        
        // Test cloning
        for value in &values {
            let _cloned = value.clone();
        }
    }
    
    #[test]
    fn test_comptime_error_handling() {
        let error = ComptimeError::Timeout {
            limit: Duration::from_secs(1),
            elapsed: Duration::from_secs(2),
        };
        
        match error {
            ComptimeError::Timeout { limit, elapsed } => {
                assert_eq!(limit, Duration::from_secs(1));
                assert_eq!(elapsed, Duration::from_secs(2));
            }
            _ => panic!("Expected timeout error"),
        }
    }
    
    #[test]
    fn test_builtin_functions() {
        let system = MetaprogrammingSystem::new();
        
        assert!(system.builtins.contains_key("typeof"));
        assert!(system.builtins.contains_key("sizeof"));
        assert!(system.builtins.contains_key("alignof"));
        assert!(system.builtins.contains_key("hasmethod"));
        assert!(system.builtins.contains_key("getmethods"));
    }
}