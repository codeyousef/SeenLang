//! Type checking error definitions

use thiserror::Error;
use seen_lexer::token::Position;
use crate::types::Type;

/// Errors that can occur during type checking
#[derive(Debug, Clone, Error)]
pub enum TypeError {
    #[error("Type mismatch: expected {expected}, found {actual} at {position}")]
    TypeMismatch {
        expected: Type,
        actual: Type,
        position: Position,
    },
    
    #[error("Undefined variable '{name}' at {position}")]
    UndefinedVariable {
        name: String,
        position: Position,
    },
    
    #[error("Undefined function '{name}' at {position}")]
    UndefinedFunction {
        name: String,
        position: Position,
    },
    
    #[error("Function '{name}' expects {expected} arguments, but {actual} were provided at {position}")]
    ArgumentCountMismatch {
        name: String,
        expected: usize,
        actual: usize,
        position: Position,
    },
    
    #[error("Invalid operation '{operation}' for types {left_type} and {right_type} at {position}")]
    InvalidOperation {
        operation: String,
        left_type: Type,
        right_type: Type,
        position: Position,
    },
    
    #[error("Cannot assign to immutable variable '{name}' at {position}")]
    ImmutableAssignment {
        name: String,
        position: Position,
    },
    
    #[error("Variable '{name}' is already defined at {position}")]
    DuplicateVariable {
        name: String,
        position: Position,
    },
    
    #[error("Function '{name}' is already defined at {position}")]
    DuplicateFunction {
        name: String,
        position: Position,
    },
    
    #[error("Cannot use null value with non-nullable type {expected_type} at {position}")]
    NullSafety {
        expected_type: Type,
        position: Position,
    },
    
    #[error("Array index out of bounds at {position}")]
    IndexOutOfBounds {
        position: Position,
    },
    
    #[error("Invalid array index type: expected Int, found {actual_type} at {position}")]
    InvalidIndexType {
        actual_type: Type,
        position: Position,
    },
    
    #[error("Return type mismatch: function expects {expected}, found {actual} at {position}")]
    ReturnTypeMismatch {
        expected: Type,
        actual: Type,
        position: Position,
    },
    
    #[error("Missing return statement in function '{function_name}' at {position}")]
    MissingReturn {
        function_name: String,
        position: Position,
    },
    
    #[error("Generic constraint violation: type {actual_type} does not satisfy constraint at {position}")]
    GenericConstraintViolation {
        actual_type: Type,
        position: Position,
    },
    
    #[error("Type inference failed: unable to determine type at {position}")]
    InferenceFailed {
        position: Position,
    },
    
    #[error("Circular type dependency detected at {position}")]
    CircularDependency {
        position: Position,
    },
}

/// Kind of type error for categorization
#[derive(Debug, Clone, PartialEq)]
pub enum TypeErrorKind {
    TypeMismatch,
    UndefinedReference,
    ArgumentMismatch,
    InvalidOperation,
    AssignmentError,
    DuplicateDefinition,
    NullSafety,
    IndexError,
    ReturnError,
    GenericError,
    InferenceError,
    CircularDependency,
}

impl TypeError {
    /// Get the position where this error occurred
    pub fn position(&self) -> Position {
        match self {
            TypeError::TypeMismatch { position, .. } |
            TypeError::UndefinedVariable { position, .. } |
            TypeError::UndefinedFunction { position, .. } |
            TypeError::ArgumentCountMismatch { position, .. } |
            TypeError::InvalidOperation { position, .. } |
            TypeError::ImmutableAssignment { position, .. } |
            TypeError::DuplicateVariable { position, .. } |
            TypeError::DuplicateFunction { position, .. } |
            TypeError::NullSafety { position, .. } |
            TypeError::IndexOutOfBounds { position, .. } |
            TypeError::InvalidIndexType { position, .. } |
            TypeError::ReturnTypeMismatch { position, .. } |
            TypeError::MissingReturn { position, .. } |
            TypeError::GenericConstraintViolation { position, .. } |
            TypeError::InferenceFailed { position, .. } |
            TypeError::CircularDependency { position, .. } => *position,
        }
    }
    
    /// Get the kind of this error
    pub fn kind(&self) -> TypeErrorKind {
        match self {
            TypeError::TypeMismatch { .. } => TypeErrorKind::TypeMismatch,
            TypeError::UndefinedVariable { .. } | TypeError::UndefinedFunction { .. } => TypeErrorKind::UndefinedReference,
            TypeError::ArgumentCountMismatch { .. } => TypeErrorKind::ArgumentMismatch,
            TypeError::InvalidOperation { .. } => TypeErrorKind::InvalidOperation,
            TypeError::ImmutableAssignment { .. } => TypeErrorKind::AssignmentError,
            TypeError::DuplicateVariable { .. } | TypeError::DuplicateFunction { .. } => TypeErrorKind::DuplicateDefinition,
            TypeError::NullSafety { .. } => TypeErrorKind::NullSafety,
            TypeError::IndexOutOfBounds { .. } | TypeError::InvalidIndexType { .. } => TypeErrorKind::IndexError,
            TypeError::ReturnTypeMismatch { .. } | TypeError::MissingReturn { .. } => TypeErrorKind::ReturnError,
            TypeError::GenericConstraintViolation { .. } => TypeErrorKind::GenericError,
            TypeError::InferenceFailed { .. } => TypeErrorKind::InferenceError,
            TypeError::CircularDependency { .. } => TypeErrorKind::CircularDependency,
        }
    }
    
    /// Check if this is a type mismatch error
    pub fn is_type_mismatch(&self) -> bool {
        matches!(self.kind(), TypeErrorKind::TypeMismatch)
    }
    
    /// Get the expected type if this is a type mismatch
    pub fn expected_type(&self) -> Option<&Type> {
        match self {
            TypeError::TypeMismatch { expected, .. } => Some(expected),
            TypeError::ReturnTypeMismatch { expected, .. } => Some(expected),
            _ => None,
        }
    }
    
    /// Get the actual type if this is a type mismatch
    pub fn actual_type(&self) -> Option<&Type> {
        match self {
            TypeError::TypeMismatch { actual, .. } => Some(actual),
            TypeError::ReturnTypeMismatch { actual, .. } => Some(actual),
            TypeError::InvalidIndexType { actual_type, .. } => Some(actual_type),
            _ => None,
        }
    }
}

/// Helper function to create a type mismatch error
pub fn type_mismatch(expected: Type, actual: Type, position: Position) -> TypeError {
    TypeError::TypeMismatch { expected, actual, position }
}

/// Helper function to create an undefined variable error
pub fn undefined_variable(name: String, position: Position) -> TypeError {
    TypeError::UndefinedVariable { name, position }
}

/// Helper function to create an undefined function error
pub fn undefined_function(name: String, position: Position) -> TypeError {
    TypeError::UndefinedFunction { name, position }
}