//! Runtime environment for the Seen interpreter

use std::collections::HashMap;
use crate::value::Value;

/// Runtime error types
#[derive(Debug, Clone)]
pub enum RuntimeError {
    UndefinedVariable(String),
    VariableAlreadyDefined(String),
    StackUnderflow,
    RecursionLimit,
    TypeError(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            RuntimeError::VariableAlreadyDefined(name) => write!(f, "Variable already defined: {}", name),
            RuntimeError::StackUnderflow => write!(f, "Stack underflow"),
            RuntimeError::RecursionLimit => write!(f, "Recursion limit exceeded"),
            RuntimeError::TypeError(msg) => write!(f, "Type error: {}", msg),
        }
    }
}

/// Environment for variable bindings
#[derive(Debug, Clone)]
pub struct Environment {
    variables: HashMap<String, Value>,
    parent: Option<Box<Environment>>,
    is_function_scope: bool,
}

impl Environment {
    /// Create a new root environment
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parent: None,
            is_function_scope: false,
        }
    }

    /// Create a new child environment
    pub fn with_parent(parent: Environment, is_function_scope: bool) -> Self {
        Self {
            variables: HashMap::new(),
            parent: Some(Box::new(parent)),
            is_function_scope,
        }
    }

    /// Define a new variable
    pub fn define(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// Get a variable value
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.variables.get(name) {
            Some(value.clone())
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    /// Set a variable value (must exist)
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        if self.variables.contains_key(name) {
            self.variables.insert(name.to_string(), value);
            Ok(())
        } else if let Some(parent) = &mut self.parent {
            parent.set(name, value)
        } else {
            Err(RuntimeError::UndefinedVariable(name.to_string()))
        }
    }
}

/// Call frame for function calls
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_name: String,
    pub location: seen_parser::Position,
}

/// Runtime state for the interpreter
pub struct Runtime {
    /// Stack of environments (scopes)
    environment_stack: Vec<Environment>,
    /// Call stack for function calls
    call_stack: Vec<CallFrame>,
    /// Return value from functions
    return_value: Option<Value>,
    /// Maximum recursion depth
    max_recursion_depth: usize,
}

impl Runtime {
    /// Create a new runtime
    pub fn new() -> Self {
        Self {
            environment_stack: vec![Environment::new()],
            call_stack: Vec::new(),
            return_value: None,
            max_recursion_depth: 1000,
        }
    }

    /// Push a new environment
    pub fn push_environment(&mut self, is_function_scope: bool) {
        let current = self.environment_stack.last().unwrap().clone();
        let new_env = Environment::with_parent(current, is_function_scope);
        self.environment_stack.push(new_env);
    }

    /// Pop an environment
    pub fn pop_environment(&mut self) -> Result<(), RuntimeError> {
        if self.environment_stack.len() <= 1 {
            return Err(RuntimeError::StackUnderflow);
        }
        self.environment_stack.pop();
        Ok(())
    }

    /// Define a variable in the current environment
    pub fn define_variable(&mut self, name: String, value: Value) {
        if let Some(env) = self.environment_stack.last_mut() {
            env.define(name, value);
        }
    }

    /// Get a variable value
    pub fn get_variable(&self, name: &str) -> Result<Value, RuntimeError> {
        if let Some(env) = self.environment_stack.last() {
            env.get(name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.to_string()))
        } else {
            Err(RuntimeError::StackUnderflow)
        }
    }

    /// Set a variable value
    pub fn set_variable(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        if let Some(env) = self.environment_stack.last_mut() {
            env.set(name, value)
        } else {
            Err(RuntimeError::StackUnderflow)
        }
    }

    /// Push a function call onto the call stack
    pub fn push_call(&mut self, function_name: String, location: seen_parser::Position) -> Result<(), RuntimeError> {
        if self.call_stack.len() >= self.max_recursion_depth {
            return Err(RuntimeError::RecursionLimit);
        }
        self.call_stack.push(CallFrame {
            function_name,
            location,
        });
        Ok(())
    }

    /// Pop a function call from the call stack
    pub fn pop_call(&mut self) -> Result<(), RuntimeError> {
        self.call_stack.pop()
            .ok_or(RuntimeError::StackUnderflow)?;
        Ok(())
    }

    /// Set the return value
    pub fn set_return_value(&mut self, value: Value) -> Result<(), RuntimeError> {
        self.return_value = Some(value);
        Ok(())
    }

    /// Get the return value
    pub fn get_return_value(&self) -> Option<&Value> {
        self.return_value.as_ref()
    }

    /// Clear the return value
    pub fn clear_return_value(&mut self) {
        self.return_value = None;
    }

    /// Print a value to stdout
    pub fn print(&self, value: &Value) -> Result<(), RuntimeError> {
        print!("{}", value.to_string());
        Ok(())
    }

    /// Print a value with newline to stdout
    pub fn println(&self, value: &Value) -> Result<(), RuntimeError> {
        println!("{}", value.to_string());
        Ok(())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}