//! Error types for the interpreter

use seen_parser::Position;
use seen_support::{ErrorLocation, SeenError, SeenErrorKind};

/// Interpreter error types
#[derive(Debug, Clone)]
pub enum InterpreterError {
    /// Runtime error with message and location
    RuntimeError { message: String, position: Position },
    /// Type error
    TypeError { message: String, position: Position },
    /// Argument count mismatch
    ArgumentCountMismatch {
        function: String,
        expected: usize,
        actual: usize,
        position: Position,
    },
    /// Undefined variable
    UndefinedVariable { name: String, position: Position },
    /// Division by zero
    DivisionByZero { position: Position },
    /// Explicit abort request from the program
    Abort { message: String, position: Position },
}

impl InterpreterError {
    /// Create a runtime error
    pub fn runtime<S: Into<String>>(message: S, position: Position) -> Self {
        Self::RuntimeError {
            message: message.into(),
            position,
        }
    }

    /// Create a type error
    pub fn type_error<S: Into<String>>(message: S, position: Position) -> Self {
        Self::TypeError {
            message: message.into(),
            position,
        }
    }

    /// Create an argument count mismatch error
    pub fn argument_count_mismatch(
        function: String,
        expected: usize,
        actual: usize,
        position: Position,
    ) -> Self {
        Self::ArgumentCountMismatch {
            function,
            expected,
            actual,
            position,
        }
    }

    /// Create an undefined variable error
    pub fn undefined_variable(name: String, position: Position) -> Self {
        Self::UndefinedVariable { name, position }
    }

    /// Create a division by zero error
    pub fn division_by_zero(position: Position) -> Self {
        Self::DivisionByZero { position }
    }

    /// Create an abort error
    pub fn abort<S: Into<String>>(message: S, position: Position) -> Self {
        Self::Abort {
            message: message.into(),
            position,
        }
    }
}

impl std::fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimeError { message, position } => {
                write!(f, "Runtime error at {}: {}", position, message)
            }
            Self::TypeError { message, position } => {
                write!(f, "Type error at {}: {}", position, message)
            }
            Self::ArgumentCountMismatch {
                function,
                expected,
                actual,
                position,
            } => {
                write!(
                    f,
                    "Function '{}' expects {} arguments, but got {} at {}",
                    function, expected, actual, position
                )
            }
            Self::UndefinedVariable { name, position } => {
                write!(f, "Undefined variable '{}' at {}", name, position)
            }
            Self::DivisionByZero { position } => {
                write!(f, "Division by zero at {}", position)
            }
            Self::Abort { message, position } => {
                write!(f, "Abort at {}: {}", position, message)
            }
        }
    }
}

impl std::error::Error for InterpreterError {}

/// Result type for interpreter operations
pub type InterpreterResult<T> = Result<T, InterpreterError>;

impl From<InterpreterError> for SeenError {
    fn from(error: InterpreterError) -> Self {
        let location = match &error {
            InterpreterError::RuntimeError { position, .. }
            | InterpreterError::TypeError { position, .. }
            | InterpreterError::ArgumentCountMismatch { position, .. }
            | InterpreterError::UndefinedVariable { position, .. }
            | InterpreterError::DivisionByZero { position }
            | InterpreterError::Abort { position, .. } => Some(ErrorLocation::new(
                position.line as u32,
                position.column as u32,
                position.offset as u32,
            )),
        };

        let kind = match &error {
            InterpreterError::Abort { .. } => SeenErrorKind::Abort,
            _ => SeenErrorKind::Interpreter,
        };
        let message = error.to_string();

        SeenError::with_optional_location(SeenError::new(kind, message), location)
    }
}
