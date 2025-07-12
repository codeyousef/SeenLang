//! Tree-walking interpreter for the Seen programming language

pub mod runtime;
pub mod interpreter;
pub mod value;
pub mod errors;

pub use errors::{InterpreterError, RuntimeError};
pub use interpreter::Interpreter;
pub use runtime::Runtime;
pub use value::Value;

/// Result of interpreting a program
#[derive(Debug, Clone)]
pub struct InterpreterResult {
    /// Final value (if any)
    pub value: Option<Value>,
    /// Runtime errors encountered
    pub errors: Vec<InterpreterError>,
}

impl InterpreterResult {
    /// Create a new empty result
    pub fn new() -> Self {
        Self {
            value: None,
            errors: Vec::new(),
        }
    }
    
    /// Check if interpretation was successful
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
    
    /// Check if interpretation failed
    pub fn is_err(&self) -> bool {
        !self.errors.is_empty()
    }
    
    /// Add an error
    pub fn add_error(&mut self, error: InterpreterError) {
        self.errors.push(error);
    }
    
    /// Set the result value
    pub fn set_value(&mut self, value: Value) {
        self.value = Some(value);
    }
}

impl Default for InterpreterResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Main entry point for interpreting a program
pub fn interpret_program(program: &seen_parser::ast::Program) -> InterpreterResult {
    let mut interpreter = Interpreter::new();
    interpreter.interpret_program(program)
}