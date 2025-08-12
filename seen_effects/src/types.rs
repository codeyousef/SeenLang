//! Type definitions for the effects system

use std::fmt;
use seen_lexer::position::Position;

/// Values that can be used in effects and contracts
#[derive(Debug, Clone, PartialEq)]
pub enum AsyncValue {
    /// Unit type ()
    Unit,
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<AsyncValue>),
}

/// Errors that can occur during effect execution
#[derive(Debug, Clone)]
pub enum AsyncError {
    /// Runtime error with message and position
    RuntimeError {
        message: String,
        position: Position,
    },
    /// Task was cancelled
    TaskCancelled { task_id: TaskId },
    /// Task timed out
    TaskTimeout { task_id: TaskId, timeout_ms: u64 },
}

/// Result type for async operations
pub type AsyncResult = Result<AsyncValue, AsyncError>;

/// Task identifier for async operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

impl TaskId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl fmt::Display for AsyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsyncError::RuntimeError { message, position } => {
                write!(f, "Runtime error at {}:{}: {}", position.line, position.column, message)
            }
            AsyncError::TaskCancelled { task_id } => {
                write!(f, "Task {:?} was cancelled", task_id)
            }
            AsyncError::TaskTimeout { task_id, timeout_ms } => {
                write!(f, "Task {:?} timed out after {}ms", task_id, timeout_ms)
            }
        }
    }
}

impl std::error::Error for AsyncError {}