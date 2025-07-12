//! Type system and type checking for the Seen programming language
//! 
//! This crate provides static type checking, type inference, and type validation
//! for Seen programs. It operates on the AST produced by the parser.

pub mod types;
pub mod checker;
pub mod inference;
pub mod errors;

pub use checker::TypeChecker;
pub use types::{Type, TypeInfo};
pub use errors::{TypeError, TypeErrorKind};

/// The result of type checking a program
#[derive(Debug, Clone)]
pub struct TypeCheckResult {
    /// Variables and their types
    pub variables: std::collections::HashMap<String, Type>,
    /// Functions and their signatures  
    pub functions: std::collections::HashMap<String, FunctionSignature>,
    /// Any type errors encountered
    pub errors: Vec<TypeError>,
}

/// Function signature information
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
}

/// Function parameter information
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Type,
}

impl TypeCheckResult {
    /// Create a new empty type check result
    pub fn new() -> Self {
        Self {
            variables: std::collections::HashMap::new(),
            functions: std::collections::HashMap::new(),
            errors: Vec::new(),
        }
    }
    
    /// Check if type checking was successful (no errors)
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
    
    /// Check if type checking failed (has errors)
    pub fn is_err(&self) -> bool {
        !self.errors.is_empty()
    }
    
    /// Get the type of a variable by name
    pub fn get_variable_type(&self, name: &str) -> Option<&Type> {
        self.variables.get(name)
    }
    
    /// Get the signature of a function by name
    pub fn get_function_signature(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions.get(name)
    }
    
    /// Add a type error
    pub fn add_error(&mut self, error: TypeError) {
        self.errors.push(error);
    }
    
    /// Add a variable with its type
    pub fn add_variable(&mut self, name: String, var_type: Type) {
        self.variables.insert(name, var_type);
    }
    
    /// Add a function signature
    pub fn add_function(&mut self, name: String, signature: FunctionSignature) {
        self.functions.insert(name, signature);
    }
}

impl Default for TypeCheckResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Main entry point for type checking a program
pub fn type_check_program(program: &seen_parser::ast::Program) -> TypeCheckResult {
    let mut checker = TypeChecker::new();
    checker.check_program(program)
}