//! Error definitions for the Seen interpreter

use thiserror::Error;
use seen_lexer::token::Location;

/// Errors that can occur during interpretation
#[derive(Debug, Clone, Error)]
pub enum InterpreterError {
    #[error("Runtime error: {message} at {location}")]
    Runtime {
        message: String,
        location: Location,
    },

    #[error("Type error: {message} at {location}")]
    Type {
        message: String,
        location: Location,
    },

    #[error("Undefined variable '{name}' at {location}")]
    UndefinedVariable {
        name: String,
        location: Location,
    },

    #[error("Undefined function '{name}' at {location}")]
    UndefinedFunction {
        name: String,
        location: Location,
    },

    #[error("Division by zero at {location}")]
    DivisionByZero {
        location: Location,
    },

    #[error("Array index out of bounds: index {index} at {location}")]
    IndexOutOfBounds {
        index: usize,
        location: Location,
    },

    #[error("Invalid operation: {operation} on {value_type} at {location}")]
    InvalidOperation {
        operation: String,
        value_type: String,
        location: Location,
    },

    #[error("Function '{name}' expects {expected} arguments, got {actual} at {location}")]
    ArgumentCountMismatch {
        name: String,
        expected: usize,
        actual: usize,
        location: Location,
    },

    #[error("Stack overflow at {location}")]
    StackOverflow {
        location: Location,
    },

    #[error("Return statement outside function at {location}")]
    ReturnOutsideFunction {
        location: Location,
    },

    #[error("IO error: {message} at {location}")]
    IO {
        message: String,
        location: Location,
    },
}

/// Runtime errors that can occur during interpretation
#[derive(Debug, Clone, Error)]
pub enum RuntimeError {
    #[error("Division by zero")]
    DivisionByZero,

    #[error("Array index out of bounds: {0}, array length: {1}")]
    IndexOutOfBounds(i64, usize),

    #[error("Invalid operation: {operation} on {value_type}")]
    InvalidOperation {
        operation: String,
        value_type: String,
    },

    #[error("Undefined variable: {name}")]
    UndefinedVariable { name: String },

    #[error("Undefined function: {name}")]
    UndefinedFunction { name: String },

    #[error("Type error: {message}")]
    TypeError { message: String },

    #[error("Stack overflow")]
    StackOverflow,

    #[error("Return statement outside function")]
    ReturnOutsideFunction,

    #[error("IO error: {message}")]
    IO { message: String },

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

impl InterpreterError {
    /// Create a runtime error
    pub fn runtime(message: String, location: Location) -> Self {
        Self::Runtime { message, location }
    }

    /// Create a type error
    pub fn type_error(message: String, location: Location) -> Self {
        Self::Type { message, location }
    }

    /// Create an undefined variable error
    pub fn undefined_variable(name: String, location: Location) -> Self {
        Self::UndefinedVariable { name, location }
    }

    /// Create an undefined function error
    pub fn undefined_function(name: String, location: Location) -> Self {
        Self::UndefinedFunction { name, location }
    }

    /// Create a division by zero error
    pub fn division_by_zero(location: Location) -> Self {
        Self::DivisionByZero { location }
    }

    /// Create an index out of bounds error
    pub fn index_out_of_bounds(index: usize, location: Location) -> Self {
        Self::IndexOutOfBounds { index, location }
    }

    /// Create an invalid operation error
    pub fn invalid_operation(operation: String, value_type: String, location: Location) -> Self {
        Self::InvalidOperation { operation, value_type, location }
    }

    /// Create an argument count mismatch error
    pub fn argument_count_mismatch(name: String, expected: usize, actual: usize, location: Location) -> Self {
        Self::ArgumentCountMismatch { name, expected, actual, location }
    }

    /// Create a stack overflow error
    pub fn stack_overflow(location: Location) -> Self {
        Self::StackOverflow { location }
    }

    /// Create a return outside function error
    pub fn return_outside_function(location: Location) -> Self {
        Self::ReturnOutsideFunction { location }
    }

    /// Create an IO error
    pub fn io_error(message: String, location: Location) -> Self {
        Self::IO { message, location }
    }

    /// Get the location where this error occurred
    pub fn location(&self) -> &Location {
        match self {
            InterpreterError::Runtime { location, .. } |
            InterpreterError::Type { location, .. } |
            InterpreterError::UndefinedVariable { location, .. } |
            InterpreterError::UndefinedFunction { location, .. } |
            InterpreterError::DivisionByZero { location } |
            InterpreterError::IndexOutOfBounds { location, .. } |
            InterpreterError::InvalidOperation { location, .. } |
            InterpreterError::ArgumentCountMismatch { location, .. } |
            InterpreterError::StackOverflow { location } |
            InterpreterError::ReturnOutsideFunction { location } |
            InterpreterError::IO { location, .. } => location,
        }
    }
}

impl From<RuntimeError> for InterpreterError {
    fn from(error: RuntimeError) -> Self {
        let location = Location::from_positions(0, 0, 0, 0);
        match error {
            RuntimeError::DivisionByZero => InterpreterError::division_by_zero(location),
            RuntimeError::IndexOutOfBounds(index, _length) => InterpreterError::index_out_of_bounds(index as usize, location),
            RuntimeError::InvalidOperation { operation, value_type } => {
                InterpreterError::invalid_operation(operation, value_type, location)
            }
            RuntimeError::UndefinedVariable { name } => InterpreterError::undefined_variable(name, location),
            RuntimeError::UndefinedFunction { name } => InterpreterError::undefined_function(name, location),
            RuntimeError::TypeError { message } => InterpreterError::type_error(message, location),
            RuntimeError::StackOverflow => InterpreterError::stack_overflow(location),
            RuntimeError::ReturnOutsideFunction => InterpreterError::return_outside_function(location),
            RuntimeError::IO { message } => InterpreterError::io_error(message, location),
            RuntimeError::NotImplemented(feature) => InterpreterError::runtime(
                format!("Not implemented: {}", feature), 
                location
            ),
        }
    }
}