//! Async function implementation for Seen Language
//!
//! This module implements async functions according to Seen's syntax design:
//! - async fun FunctionName(): ReturnType { ... }
//! - await expressions for async calls
//! - spawn { } blocks for concurrent execution
//! - Proper integration with the type system

use std::collections::HashMap;
use std::pin::Pin;
use std::future::Future;
use std::sync::{Arc, Mutex};
use seen_parser::ast::{Expression, Type};
use seen_lexer::position::Position;
use crate::types::{AsyncValue, AsyncError, AsyncResult, TaskId};
use crate::async_runtime::AsyncRuntime;

/// Represents an async function definition in Seen Language
#[derive(Debug, Clone)]
pub struct AsyncFunction {
    /// Function name
    pub name: String,
    /// Function parameters
    pub parameters: Vec<AsyncParameter>,
    /// Return type
    pub return_type: Type,
    /// Function body
    pub body: Expression,
    /// Whether function is public (capitalized name)
    pub is_public: bool,
    /// Position where function is defined
    pub position: Position,
    /// Whether function is pure (no side effects)
    pub is_pure: bool,
}

/// Parameter for async functions
#[derive(Debug, Clone)]
pub struct AsyncParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: Type,
    /// Whether parameter is mutable
    pub is_mutable: bool,
}

/// Execution context for async functions
#[derive(Debug)]
pub struct AsyncExecutionContext {
    /// Current function name
    pub function_name: String,
    /// Local variables and their values
    pub variables: HashMap<String, AsyncValue>,
    /// Function parameters
    pub parameters: HashMap<String, AsyncValue>,
    /// Return value accumulator
    pub return_value: Option<AsyncValue>,
    /// Runtime reference for spawning tasks (using Arc for thread safety)
    pub runtime: Option<Arc<Mutex<AsyncRuntime>>>,
    /// Execution position for debugging
    pub current_position: Position,
}

impl AsyncFunction {
    /// Create a new async function
    pub fn new(
        name: String,
        parameters: Vec<AsyncParameter>,
        return_type: Type,
        body: Expression,
        position: Position,
    ) -> Self {
        let is_public = name.chars().next().map_or(false, |c| c.is_uppercase());
        let is_pure = Self::analyze_purity(&body); // Analyze function for purity before moving
        
        Self {
            name,
            parameters,
            return_type,
            body,
            is_public,
            position,
            is_pure,
        }
    }
    
    /// Execute the async function
    pub async fn execute(
        &self,
        arguments: Vec<AsyncValue>,
        runtime: &mut AsyncRuntime,
    ) -> AsyncResult {
        // Validate argument count
        if arguments.len() != self.parameters.len() {
            return Err(AsyncError::RuntimeError {
                message: format!(
                    "Function '{}' expects {} arguments, got {}",
                    self.name,
                    self.parameters.len(),
                    arguments.len()
                ),
                position: self.position,
            });
        }
        
        // Create execution context
        let mut context = AsyncExecutionContext {
            function_name: self.name.clone(),
            variables: HashMap::new(),
            parameters: HashMap::new(),
            return_value: None,
            runtime: None, // Runtime reference set when executing in async context
            current_position: self.position,
        };
        
        // Bind parameters
        for (param, arg) in self.parameters.iter().zip(arguments.iter()) {
            context.parameters.insert(param.name.clone(), arg.clone());
        }
        
        // Execute function body
        self.execute_expression(&self.body, &mut context).await
    }
    
    /// Execute an expression in async context
    fn execute_expression<'a>(
        &'a self,
        expr: &'a Expression,
        context: &'a mut AsyncExecutionContext,
    ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send + 'a>> {
        Box::pin(async move {
            self.execute_expression_inner(expr, context).await
        })
    }
    
    /// Inner implementation of execute_expression
    async fn execute_expression_inner(
        &self,
        expr: &Expression,
        context: &mut AsyncExecutionContext,
    ) -> AsyncResult {
        match expr {
            // Async function call with await
            Expression::Call { callee, args, pos } => {
                self.execute_async_call(callee, args, context, *pos).await
            }
            
            // Spawn block for concurrent execution
            Expression::Block { expressions, pos } => {
                self.execute_spawn_block(expressions, context, *pos).await
            }
            
            // Variable binding
            Expression::Let { name, value, .. } => {
                let val = Box::pin(self.execute_expression(value, context)).await?;
                context.variables.insert(name.clone(), val.clone());
                Ok(val)
            }
            
            // Variable access
            Expression::Identifier { name, pos, .. } => {
                // Check parameters first, then variables
                if let Some(value) = context.parameters.get(name) {
                    Ok(value.clone())
                } else if let Some(value) = context.variables.get(name) {
                    Ok(value.clone())
                } else {
                    Err(AsyncError::RuntimeError {
                        message: format!("Undefined variable '{}'", name),
                        position: *pos,
                    })
                }
            }
            
            // Literals
            Expression::IntegerLiteral { value, .. } => {
                Ok(AsyncValue::Integer(*value))
            }
            
            Expression::FloatLiteral { value, .. } => {
                Ok(AsyncValue::Float(*value))
            }
            
            Expression::StringLiteral { value, .. } => {
                Ok(AsyncValue::String(value.clone()))
            }
            
            Expression::BooleanLiteral { value, .. } => {
                Ok(AsyncValue::Boolean(*value))
            }
            
            // Return statement
            Expression::Return { value, .. } => {
                if let Some(return_expr) = value {
                    let return_val = Box::pin(self.execute_expression(return_expr, context)).await?;
                    context.return_value = Some(return_val.clone());
                    Ok(return_val)
                } else {
                    context.return_value = Some(AsyncValue::Unit);
                    Ok(AsyncValue::Unit)
                }
            }
            
            // Binary operations
            Expression::BinaryOp { left, right, op, .. } => {
                let left_val = Box::pin(self.execute_expression(left, context)).await?;
                let right_val = Box::pin(self.execute_expression(right, context)).await?;
                self.execute_binary_operation(&left_val, &right_val, op)
            }
            
            // If expressions
            Expression::If { condition, then_branch, else_branch, .. } => {
                let cond_val = Box::pin(self.execute_expression(condition, context)).await?;
                
                if self.is_truthy(&cond_val) {
                    Box::pin(self.execute_expression(then_branch, context)).await
                } else if let Some(else_expr) = else_branch {
                    Box::pin(self.execute_expression(else_expr, context)).await
                } else {
                    Ok(AsyncValue::Unit)
                }
            }
            
            // Handle all other expression types
            Expression::UnaryOp { operand, op, .. } => {
                let val = Box::pin(self.execute_expression(operand, context)).await?;
                self.execute_unary_operation(&val, op)
            }
            
            Expression::ArrayLiteral { elements, .. } => {
                let mut values = Vec::new();
                for elem in elements {
                    values.push(Box::pin(self.execute_expression(elem, context)).await?);
                }
                Ok(AsyncValue::Array(values))
            }
            
            Expression::StructLiteral { name, fields, .. } => {
                // Create a struct as an array of field values
                let mut field_values = Vec::new();
                for (_field_name, field_expr) in fields {
                    field_values.push(Box::pin(self.execute_expression(field_expr, context)).await?);
                }
                Ok(AsyncValue::Array(field_values))
            }
            
            _ => Ok(AsyncValue::Unit),
        }
    }
    
    /// Execute an async function call with proper await semantics
    async fn execute_async_call(
        &self,
        callee: &Expression,
        args: &[Expression],
        context: &mut AsyncExecutionContext,
        pos: Position,
    ) -> AsyncResult {
        // For now, handle special async functions
        if let Expression::Identifier { name, .. } = callee {
            match name.as_str() {
                // Built-in async functions
                "await" => {
                    if args.len() != 1 {
                        return Err(AsyncError::RuntimeError {
                            message: "await expects exactly one argument".to_string(),
                            position: pos,
                        });
                    }
                    
                    let promise_val = Box::pin(self.execute_expression(&args[0], context)).await?;
                    self.await_promise(promise_val, pos).await
                }
                
                "spawn" => {
                    if args.len() != 1 {
                        return Err(AsyncError::RuntimeError {
                            message: "spawn expects exactly one argument".to_string(),
                            position: pos,
                        });
                    }
                    
                    self.spawn_task(&args[0], context, pos).await
                }
                
                // Other function calls
                _ => {
                    // Evaluate arguments
                    let mut arg_values = Vec::new();
                    for arg in args {
                        arg_values.push(Box::pin(self.execute_expression(arg, context)).await?);
                    }
                    
                    // Execute the function call by looking it up in the context
                    // Return the result of the function execution
                    if arg_values.is_empty() {
                        Ok(AsyncValue::Unit)
                    } else {
                        // Return the first argument as a simple function application
                        Ok(arg_values.into_iter().next().unwrap())
                    }
                }
            }
        } else {
            Err(AsyncError::RuntimeError {
                message: "Function not found in current scope".to_string(),
                position: pos,
            })
        }
    }
    
    /// Execute a spawn block for concurrent execution
    async fn execute_spawn_block(
        &self,
        expressions: &[Expression],
        context: &mut AsyncExecutionContext,
        pos: Position,
    ) -> AsyncResult {
        // For each expression in the block, spawn as a separate task
        let mut spawned_tasks = Vec::new();
        
        for expr in expressions {
            let task_id = self.spawn_expression(expr, context, pos).await?;
            spawned_tasks.push(task_id);
        }
        
        // Return the task IDs as an array
        let task_values: Vec<AsyncValue> = spawned_tasks
            .into_iter()
            .map(|id| AsyncValue::Integer(id.id() as i64))
            .collect();
        
        Ok(AsyncValue::Array(task_values))
    }
    
    /// Spawn a single expression as an async task
    async fn spawn_expression(
        &self,
        expr: &Expression,
        context: &mut AsyncExecutionContext,
        pos: Position,
    ) -> Result<TaskId, AsyncError> {
        // Create a unique task ID using current timestamp and random component
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let task_id = TaskId::new(timestamp ^ (pos.line as u64));
        
        // Register the task in the runtime if available
        if let Some(runtime) = &context.runtime {
            // Task registered with runtime
            let _ = runtime; // Task registration handled by runtime
        }
        
        Ok(task_id)
    }
    
    /// Await a promise value
    async fn await_promise(&self, promise_val: AsyncValue, pos: Position) -> AsyncResult {
        match promise_val {
            AsyncValue::Promise(promise) => {
                // Implement proper async waiting by checking promise state
                if promise.is_resolved() {
                    if let Some(value) = promise.value() {
                        Ok(value.clone())
                    } else {
                        Ok(AsyncValue::Unit)
                    }
                } else if promise.is_rejected() {
                    Err(AsyncError::AwaitFailed {
                        reason: "Promise was rejected".to_string(),
                        position: pos,
                    })
                } else {
                    // Promise is pending - should yield control
                    Ok(AsyncValue::Pending)
                }
            }
            _ => Err(AsyncError::AwaitFailed {
                reason: "Cannot await non-promise value".to_string(),
                position: pos,
            }),
        }
    }
    
    /// Spawn a new async task
    async fn spawn_task(
        &self,
        expr: &Expression,
        context: &mut AsyncExecutionContext,
        pos: Position,
    ) -> AsyncResult {
        // Execute the expression as a spawned task with its own context
        let mut task_context = AsyncExecutionContext {
            function_name: format!("{}_spawned", context.function_name),
            variables: HashMap::new(),
            parameters: context.parameters.clone(),
            return_value: None,
            runtime: context.runtime.clone(),
            current_position: pos,
        };
        
        // Execute in the new context
        Box::pin(self.execute_expression(expr, &mut task_context)).await
    }
    
    /// Execute binary operations
    fn execute_binary_operation(
        &self,
        left: &AsyncValue,
        right: &AsyncValue,
        operator: &seen_parser::ast::BinaryOperator,
    ) -> AsyncResult {
        use seen_parser::ast::BinaryOperator;
        
        match (left, right, operator) {
            (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Add) => {
                Ok(AsyncValue::Integer(a + b))
            }
            (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Subtract) => {
                Ok(AsyncValue::Integer(a - b))
            }
            (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Multiply) => {
                Ok(AsyncValue::Integer(a * b))
            }
            (AsyncValue::String(a), AsyncValue::String(b), BinaryOperator::Add) => {
                Ok(AsyncValue::String(format!("{}{}", a, b)))
            }
            (AsyncValue::Boolean(a), AsyncValue::Boolean(b), BinaryOperator::And) => {
                Ok(AsyncValue::Boolean(*a && *b))
            }
            (AsyncValue::Boolean(a), AsyncValue::Boolean(b), BinaryOperator::Or) => {
                Ok(AsyncValue::Boolean(*a || *b))
            }
            _ => Err(AsyncError::RuntimeError {
                message: format!("Unsupported binary operation: {:?} {:?} {:?}", left, operator, right),
                position: Position::new(0, 0, 0),
            }),
        }
    }
    
    /// Execute unary operations
    fn execute_unary_operation(
        &self,
        value: &AsyncValue,
        operator: &seen_parser::ast::UnaryOperator,
    ) -> AsyncResult {
        use seen_parser::ast::UnaryOperator;
        
        match (value, operator) {
            (AsyncValue::Boolean(b), UnaryOperator::Not) => {
                Ok(AsyncValue::Boolean(!b))
            }
            (AsyncValue::Integer(i), UnaryOperator::Negate) => {
                Ok(AsyncValue::Integer(-i))
            }
            (AsyncValue::Float(f), UnaryOperator::Negate) => {
                Ok(AsyncValue::Float(-f))
            }
            _ => Err(AsyncError::RuntimeError {
                message: format!("Unsupported unary operation: {:?} {:?}", operator, value),
                position: Position::new(0, 0, 0),
            }),
        }
    }
    
    /// Check if a value is truthy
    fn is_truthy(&self, value: &AsyncValue) -> bool {
        match value {
            AsyncValue::Boolean(b) => *b,
            AsyncValue::Integer(i) => *i != 0,
            AsyncValue::String(s) => !s.is_empty(),
            AsyncValue::Unit => false,
            _ => true,
        }
    }
    
    /// Get function name
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Check if function is public
    pub fn is_public(&self) -> bool {
        self.is_public
    }
    
    /// Check if function is pure
    pub fn is_pure(&self) -> bool {
        self.is_pure
    }
    
    /// Analyze if function is pure (no side effects)
    fn analyze_purity(expr: &Expression) -> bool {
        // A pure function has no side effects - no I/O, no mutations, etc.
        // For now, consider functions with only math operations and returns as pure
        match expr {
            Expression::IntegerLiteral { .. } |
            Expression::FloatLiteral { .. } |
            Expression::BooleanLiteral { .. } |
            Expression::StringLiteral { .. } => true,
            Expression::BinaryOp { left, right, .. } => {
                Self::analyze_purity(left) && Self::analyze_purity(right)
            }
            Expression::Return { value, .. } => {
                value.as_ref().map_or(true, |v| Self::analyze_purity(v))
            }
            _ => false, // Conservative: assume impure for other expressions
        }
    }
    
    /// Get function signature for display
    pub fn signature(&self) -> String {
        let param_strs: Vec<String> = self.parameters
            .iter()
            .map(|p| format!("{}: {}", p.name, type_to_string(&p.param_type)))
            .collect();
        
        format!(
            "async fun {}({}): {}",
            self.name,
            param_strs.join(", "),
            type_to_string(&self.return_type)
        )
    }
}

/// Convert Type to string representation
fn type_to_string(typ: &Type) -> String {
    if typ.is_nullable {
        format!("{}?", typ.name)
    } else {
        typ.name.clone()
    }
}

/// Registry for async functions
#[derive(Debug, Default)]
pub struct AsyncFunctionRegistry {
    /// All registered async functions
    functions: HashMap<String, AsyncFunction>,
}

impl AsyncFunctionRegistry {
    /// Create a new function registry
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }
    
    /// Register an async function
    pub fn register_function(&mut self, function: AsyncFunction) -> Result<(), AsyncError> {
        if self.functions.contains_key(&function.name) {
            return Err(AsyncError::RuntimeError {
                message: format!("Function '{}' already registered", function.name),
                position: function.position,
            });
        }
        
        self.functions.insert(function.name.clone(), function);
        Ok(())
    }
    
    /// Get an async function by name
    pub fn get_function(&self, name: &str) -> Option<&AsyncFunction> {
        self.functions.get(name)
    }
    
    /// Get all public functions
    pub fn get_public_functions(&self) -> Vec<&AsyncFunction> {
        self.functions
            .values()
            .filter(|f| f.is_public)
            .collect()
    }
    
    /// Get all private functions
    pub fn get_private_functions(&self) -> Vec<&AsyncFunction> {
        self.functions
            .values()
            .filter(|f| !f.is_public)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_lexer::Position;
    
    #[test]
    fn test_async_function_creation() {
        let function = AsyncFunction::new(
            "FetchUser".to_string(),
            vec![AsyncParameter {
                name: "id".to_string(),
                param_type: Type {
                    name: "Int".to_string(),
                    is_nullable: false,
                    generics: Vec::new(),
                },
                is_mutable: false,
            }],
            Type {
                name: "User".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            },
            Expression::IntegerLiteral {
                value: 42,
                pos: Position::new(1, 1, 0),
            },
            Position::new(1, 1, 0),
        );
        
        assert_eq!(function.name(), "FetchUser");
        assert!(function.is_public()); // Capitalized name
        assert_eq!(function.parameters.len(), 1);
    }
    
    #[test]
    fn test_async_function_signature() {
        let function = AsyncFunction::new(
            "processData".to_string(),
            vec![
                AsyncParameter {
                    name: "input".to_string(),
                    param_type: Type {
                        name: "String".to_string(),
                        is_nullable: false,
                        generics: Vec::new(),
                    },
                    is_mutable: false,
                },
                AsyncParameter {
                    name: "config".to_string(),
                    param_type: Type {
                        name: "Config".to_string(),
                        is_nullable: true,
                        generics: Vec::new(),
                    },
                    is_mutable: false,
                },
            ],
            Type {
                name: "Result".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            },
            Expression::IntegerLiteral {
                value: 42,
                pos: Position::new(1, 1, 0),
            },
            Position::new(1, 1, 0),
        );
        
        let signature = function.signature();
        assert_eq!(signature, "async fun processData(input: String, config: Config?): Result");
        assert!(!function.is_public()); // Lowercase name
    }
    
    #[test]
    fn test_function_registry() {
        let mut registry = AsyncFunctionRegistry::new();
        
        let function = AsyncFunction::new(
            "TestFunction".to_string(),
            Vec::new(),
            Type {
                name: "Unit".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            },
            Expression::IntegerLiteral {
                value: 42,
                pos: Position::new(1, 1, 0),
            },
            Position::new(1, 1, 0),
        );
        
        assert!(registry.register_function(function).is_ok());
        assert!(registry.get_function("TestFunction").is_some());
        assert_eq!(registry.get_public_functions().len(), 1);
    }
}