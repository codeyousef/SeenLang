//! Type checker for the Seen programming language

pub mod types;
pub mod errors;
pub mod checker;

use std::collections::HashMap;
pub use types::Type;
pub use errors::TypeError;
pub use checker::TypeChecker;

/// Result of type checking a program
#[derive(Debug, Clone)]
pub struct TypeCheckResult {
    /// All type errors found
    pub errors: Vec<TypeError>,
    /// Variable type information
    pub variables: HashMap<String, Type>,
    /// Function signatures
    pub functions: HashMap<String, FunctionSignature>,
}

impl TypeCheckResult {
    /// Create a new empty result
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            variables: HashMap::new(),
            functions: HashMap::new(),
        }
    }
    
    /// Add a type error
    pub fn add_error(&mut self, error: TypeError) {
        self.errors.push(error);
    }
    
    /// Add a variable type
    pub fn add_variable(&mut self, name: String, var_type: Type) {
        self.variables.insert(name, var_type);
    }
    
    /// Add a function signature
    pub fn add_function(&mut self, name: String, signature: FunctionSignature) {
        self.functions.insert(name, signature);
    }
    
    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
    
    /// Get all errors
    pub fn get_errors(&self) -> &[TypeError] {
        &self.errors
    }
}

impl Default for TypeCheckResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Function signature information
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    /// Function name
    pub name: String,
    /// Parameter information
    pub parameters: Vec<Parameter>,
    /// Return type (None for Unit/void)
    pub return_type: Option<Type>,
}

/// Function parameter information
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: Type,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_type_check_result_creation() {
        let result = TypeCheckResult::new();
        assert_eq!(result.errors.len(), 0);
        assert_eq!(result.variables.len(), 0);
        assert_eq!(result.functions.len(), 0);
    }
    
    #[test]
    fn test_add_error() {
        let mut result = TypeCheckResult::new();
        assert!(!result.has_errors());
        
        let error = TypeError::InferenceFailed { 
            position: seen_lexer::position::Position::new(1, 1, 0) 
        };
        result.add_error(error);
        
        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
    }
}