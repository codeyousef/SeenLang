//! Runtime environment for the Seen interpreter

use std::collections::HashMap;
use crate::value::Value;
use crate::errors::InterpreterError;
use seen_lexer::token::Location;

/// The runtime environment for executing Seen programs
#[derive(Debug, Clone)]
pub struct Runtime {
    /// Global variables
    globals: HashMap<String, Value>,
    /// Environment stack for local scopes
    environments: Vec<Environment>,
    /// Call stack for function calls
    call_stack: Vec<CallFrame>,
    /// Maximum call stack depth
    max_stack_depth: usize,
}

/// A single environment/scope in the interpreter
#[derive(Debug, Clone)]
struct Environment {
    /// Variables in this scope
    variables: HashMap<String, Value>,
    /// Whether this is a function scope
    is_function_scope: bool,
}

/// A call frame on the call stack
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Name of the function being called
    function_name: String,
    /// Location of the call
    location: Location,
    /// Return value (if any)
    return_value: Option<Value>,
}

impl Runtime {
    /// Create a new runtime environment
    pub fn new() -> Self {
        let mut runtime = Self {
            globals: HashMap::new(),
            environments: vec![Environment::new(false)],
            call_stack: Vec::new(),
            max_stack_depth: 1000,
        };

        // Add built-in functions and variables
        runtime.setup_builtins();
        runtime
    }

    /// Setup built-in functions and variables
    fn setup_builtins(&mut self) {
        // Built-in variables (if any)
        // For now, we don't have any global constants
    }

    /// Push a new environment onto the stack
    pub fn push_environment(&mut self, is_function_scope: bool) {
        self.environments.push(Environment::new(is_function_scope));
    }

    /// Pop the current environment from the stack
    pub fn pop_environment(&mut self) -> Result<(), String> {
        if self.environments.len() <= 1 {
            return Err("Cannot pop global environment".to_string());
        }
        self.environments.pop();
        Ok(())
    }

    /// Define a variable in the current environment
    pub fn define_variable(&mut self, name: String, value: Value) {
        if let Some(env) = self.environments.last_mut() {
            env.variables.insert(name, value);
        }
    }

    /// Get a variable value from the environment stack or globals
    pub fn get_variable(&self, name: &str) -> Result<Value, String> {
        // Search from the most recent environment backwards
        for env in self.environments.iter().rev() {
            if let Some(value) = env.variables.get(name) {
                return Ok(value.clone());
            }
        }

        // Check globals
        if let Some(value) = self.globals.get(name) {
            return Ok(value.clone());
        }

        Err(format!("Undefined variable: {}", name))
    }

    /// Set a variable value (must exist in some scope)
    pub fn set_variable(&mut self, name: &str, value: Value) -> Result<(), String> {
        // Search from the most recent environment backwards
        for env in self.environments.iter_mut().rev() {
            if env.variables.contains_key(name) {
                env.variables.insert(name.to_string(), value);
                return Ok(());
            }
        }

        // Check globals
        if self.globals.contains_key(name) {
            self.globals.insert(name.to_string(), value);
            return Ok(());
        }

        Err(format!("Undefined variable: {}", name))
    }

    /// Push a call frame onto the call stack
    pub fn push_call(&mut self, function_name: String, location: Location) -> Result<(), String> {
        if self.call_stack.len() >= self.max_stack_depth {
            return Err("Stack overflow".to_string());
        }

        self.call_stack.push(CallFrame {
            function_name,
            location,
            return_value: None,
        });

        Ok(())
    }

    /// Pop a call frame from the call stack
    pub fn pop_call(&mut self) -> Option<CallFrame> {
        self.call_stack.pop()
    }

    /// Set the return value for the current function call
    pub fn set_return_value(&mut self, value: Value) -> Result<(), String> {
        if let Some(frame) = self.call_stack.last_mut() {
            frame.return_value = Some(value);
            Ok(())
        } else {
            Err("Return statement outside function".to_string())
        }
    }

    /// Get the return value from the most recent call frame
    pub fn get_return_value(&self) -> Option<&Value> {
        self.call_stack.last()?.return_value.as_ref()
    }

    /// Check if we're currently in a function
    pub fn in_function(&self) -> bool {
        !self.call_stack.is_empty()
    }

    /// Get the current call stack depth
    pub fn call_depth(&self) -> usize {
        self.call_stack.len()
    }

    /// Print a value to stdout (built-in print function)
    pub fn print(&self, value: &Value) -> Result<(), String> {
        print!("{}", value.to_string());
        Ok(())
    }

    /// Print a value to stdout with newline (built-in println function)
    pub fn println(&self, value: &Value) -> Result<(), String> {
        println!("{}", value.to_string());
        Ok(())
    }

    /// Get debug information about the current state
    pub fn debug_info(&self) -> String {
        format!(
            "Runtime State:\n\
             - Environments: {}\n\
             - Call stack depth: {}\n\
             - Global variables: {}\n\
             - Current function: {}",
            self.environments.len(),
            self.call_stack.len(),
            self.globals.len(),
            self.call_stack.last()
                .map(|f| f.function_name.as_str())
                .unwrap_or("none")
        )
    }

    /// Clear all state (for testing)
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.globals.clear();
        self.environments = vec![Environment::new(false)];
        self.call_stack.clear();
    }
}

impl Environment {
    /// Create a new environment
    fn new(is_function_scope: bool) -> Self {
        Self {
            variables: HashMap::new(),
            is_function_scope,
        }
    }

    /// Check if this environment contains a variable
    fn contains(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_scoping() {
        let mut runtime = Runtime::new();

        // Define variable in global scope
        runtime.define_variable("x".to_string(), Value::Integer(42));
        assert_eq!(runtime.get_variable("x").unwrap(), Value::Integer(42));

        // Create new scope
        runtime.push_environment(false);
        runtime.define_variable("y".to_string(), Value::String("hello".to_string()));

        // Can access both variables
        assert_eq!(runtime.get_variable("x").unwrap(), Value::Integer(42));
        assert_eq!(runtime.get_variable("y").unwrap(), Value::String("hello".to_string()));

        // Pop scope
        runtime.pop_environment().unwrap();

        // Can still access global variable
        assert_eq!(runtime.get_variable("x").unwrap(), Value::Integer(42));

        // Cannot access local variable
        assert!(runtime.get_variable("y").is_err());
    }

    #[test]
    fn test_call_stack() {
        let mut runtime = Runtime::new();
        let location = Location::from_positions(1, 1, 1, 10);

        assert!(!runtime.in_function());

        runtime.push_call("test_func".to_string(), location).unwrap();
        assert!(runtime.in_function());
        assert_eq!(runtime.call_depth(), 1);

        runtime.set_return_value(Value::Integer(123)).unwrap();
        let frame = runtime.pop_call().unwrap();
        assert_eq!(frame.function_name, "test_func");
        assert_eq!(frame.return_value, Some(Value::Integer(123)));
    }
}