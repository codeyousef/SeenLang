//! Error types for the IR interpreter
//!
//! Rich error types with source location information and suggestions.

use super::memory::Address;
use super::value::ValueType;
use std::fmt;

/// Main interpreter error type
#[derive(Debug, Clone)]
pub struct InterpreterError {
    pub kind: InterpreterErrorKind,
    pub message: String,
    pub location: Option<ErrorLocation>,
    pub suggestion: Option<String>,
    pub backtrace: Vec<StackFrame>,
}

/// Location information for errors
#[derive(Debug, Clone)]
pub struct ErrorLocation {
    pub function_name: String,
    pub instruction_index: usize,
    pub source_line: Option<u32>,
    pub source_column: Option<u32>,
    pub source_file: Option<String>,
}

/// Stack frame for backtraces
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function_name: String,
    pub instruction_index: usize,
    pub source_location: Option<String>,
}

/// Categories of interpreter errors
#[derive(Debug, Clone, PartialEq)]
pub enum InterpreterErrorKind {
    // Memory errors
    NullPointerDereference,
    UseAfterFree,
    DoubleFree,
    BufferOverflow,
    BufferUnderflow,
    OutOfBoundsAccess,
    UninitializedRead,
    InvalidPointer,
    MemoryLeak,

    // Type errors
    TypeMismatch {
        expected: ValueType,
        found: ValueType,
    },
    InvalidCast,
    InvalidOperation,

    // Control flow errors
    StackOverflow,
    StackUnderflow,
    InvalidJump,
    MissingLabel,
    InfiniteLoop,

    // Value errors
    DivisionByZero,
    IntegerOverflow,
    UndefinedVariable,
    UndefinedFunction,
    InvalidArrayIndex,
    InvalidFieldAccess,

    // Execution errors
    InvalidInstruction,
    MissingReturn,
    ArgumentCountMismatch,
    InstructionLimitExceeded,

    // Internal errors (bugs in interpreter)
    InternalError,
}

impl InterpreterError {
    /// Create a new error with just a kind and message
    pub fn new(kind: InterpreterErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            location: None,
            suggestion: None,
            backtrace: Vec::new(),
        }
    }

    /// Add location information
    pub fn with_location(mut self, location: ErrorLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// Add a suggestion for fixing the error
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add backtrace
    pub fn with_backtrace(mut self, backtrace: Vec<StackFrame>) -> Self {
        self.backtrace = backtrace;
        self
    }

    // Convenience constructors for common errors

    pub fn null_pointer(operation: &str) -> Self {
        Self::new(
            InterpreterErrorKind::NullPointerDereference,
            format!("Null pointer dereference during {}", operation),
        ).with_suggestion("Check if the pointer is null before dereferencing")
    }

    pub fn use_after_free(address: Address, freed_at: Option<&str>) -> Self {
        let msg = match freed_at {
            Some(loc) => format!("Use-after-free: memory at {} was freed at {}", address, loc),
            None => format!("Use-after-free: memory at {} has been freed", address),
        };
        Self::new(InterpreterErrorKind::UseAfterFree, msg)
            .with_suggestion("Ensure memory is not accessed after being freed")
    }

    pub fn double_free(address: Address) -> Self {
        Self::new(
            InterpreterErrorKind::DoubleFree,
            format!("Double free: memory at {} has already been freed", address),
        ).with_suggestion("Track allocation ownership to prevent double frees")
    }

    pub fn buffer_overflow(address: Address, size: usize, allocation_size: usize) -> Self {
        Self::new(
            InterpreterErrorKind::BufferOverflow,
            format!(
                "Buffer overflow: attempted to access {} bytes at {}, but allocation is only {} bytes",
                size, address, allocation_size
            ),
        ).with_suggestion("Check array bounds before accessing")
    }

    pub fn out_of_bounds(index: i64, length: usize) -> Self {
        Self::new(
            InterpreterErrorKind::OutOfBoundsAccess,
            format!("Index {} out of bounds for array of length {}", index, length),
        ).with_suggestion("Ensure index is within valid range [0, length)")
    }

    pub fn type_mismatch(expected: ValueType, found: ValueType, context: &str) -> Self {
        Self::new(
            InterpreterErrorKind::TypeMismatch { expected: expected.clone(), found: found.clone() },
            format!(
                "Type mismatch in {}: expected {}, found {}",
                context, expected, found
            ),
        )
    }

    pub fn division_by_zero() -> Self {
        Self::new(
            InterpreterErrorKind::DivisionByZero,
            "Division by zero",
        ).with_suggestion("Add a check for zero before dividing")
    }

    pub fn stack_overflow(depth: usize, max_depth: usize) -> Self {
        Self::new(
            InterpreterErrorKind::StackOverflow,
            format!("Stack overflow: depth {} exceeds maximum {}", depth, max_depth),
        ).with_suggestion("Check for infinite recursion or increase stack limit")
    }

    pub fn undefined_variable(name: &str) -> Self {
        Self::new(
            InterpreterErrorKind::UndefinedVariable,
            format!("Undefined variable: {}", name),
        ).with_suggestion("Ensure the variable is declared before use")
    }

    pub fn undefined_function(name: &str) -> Self {
        Self::new(
            InterpreterErrorKind::UndefinedFunction,
            format!("Undefined function: {}", name),
        ).with_suggestion("Ensure the function is defined before calling")
    }

    pub fn argument_count_mismatch(function: &str, expected: usize, found: usize) -> Self {
        Self::new(
            InterpreterErrorKind::ArgumentCountMismatch,
            format!(
                "Function {} expects {} arguments, but {} were provided",
                function, expected, found
            ),
        )
    }

    pub fn instruction_limit(limit: u64) -> Self {
        Self::new(
            InterpreterErrorKind::InstructionLimitExceeded,
            format!("Instruction limit of {} exceeded (possible infinite loop)", limit),
        ).with_suggestion("Check for infinite loops or increase the instruction limit")
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            InterpreterErrorKind::InternalError,
            format!("Internal interpreter error: {}", message.into()),
        ).with_suggestion("This is a bug in the interpreter, please report it")
    }
}

impl fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Error header
        writeln!(f, "Runtime Error: {}", self.message)?;
        
        // Location
        if let Some(ref loc) = self.location {
            write!(f, "  at function '{}', instruction {}", loc.function_name, loc.instruction_index)?;
            if let Some(line) = loc.source_line {
                write!(f, " (line {})", line)?;
            }
            writeln!(f)?;
        }
        
        // Backtrace
        if !self.backtrace.is_empty() {
            writeln!(f, "\nBacktrace:")?;
            for (i, frame) in self.backtrace.iter().enumerate() {
                write!(f, "  {}: {}[{}]", i, frame.function_name, frame.instruction_index)?;
                if let Some(ref loc) = frame.source_location {
                    write!(f, " at {}", loc)?;
                }
                writeln!(f)?;
            }
        }
        
        // Suggestion
        if let Some(ref suggestion) = self.suggestion {
            writeln!(f, "\nSuggestion: {}", suggestion)?;
        }
        
        Ok(())
    }
}

impl std::error::Error for InterpreterError {}

/// Allow conversion from String errors
impl From<String> for InterpreterError {
    fn from(message: String) -> Self {
        InterpreterError::new(InterpreterErrorKind::InternalError, message)
    }
}

/// Allow conversion from &str errors
impl From<&str> for InterpreterError {
    fn from(message: &str) -> Self {
        InterpreterError::new(InterpreterErrorKind::InternalError, message)
    }
}

/// Result type for interpreter operations
pub type InterpreterResult<T> = Result<T, InterpreterError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = InterpreterError::null_pointer("read");
        let msg = format!("{}", err);
        assert!(msg.contains("Null pointer"));
        assert!(msg.contains("Suggestion"));
    }

    #[test]
    fn test_error_with_location() {
        let err = InterpreterError::division_by_zero()
            .with_location(ErrorLocation {
                function_name: "calculate".to_string(),
                instruction_index: 42,
                source_line: Some(10),
                source_column: Some(5),
                source_file: Some("math.seen".to_string()),
            });
        
        let msg = format!("{}", err);
        assert!(msg.contains("calculate"));
        assert!(msg.contains("42"));
        assert!(msg.contains("line 10"));
    }

    #[test]
    fn test_error_with_backtrace() {
        let err = InterpreterError::stack_overflow(1025, 1024)
            .with_backtrace(vec![
                StackFrame {
                    function_name: "recursive".to_string(),
                    instruction_index: 5,
                    source_location: None,
                },
                StackFrame {
                    function_name: "main".to_string(),
                    instruction_index: 10,
                    source_location: Some("main.seen:15".to_string()),
                },
            ]);
        
        let msg = format!("{}", err);
        assert!(msg.contains("Backtrace"));
        assert!(msg.contains("recursive"));
        assert!(msg.contains("main"));
    }
}
