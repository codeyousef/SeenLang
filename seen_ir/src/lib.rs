//! Intermediate representation for the Seen programming language
//! 
//! This module provides the IR (Intermediate Representation) for the Seen language,
//! which serves as an intermediary between the AST and the target code generation.
//! The IR is designed to be platform-agnostic and suitable for optimization passes.

use std::collections::HashMap;
use std::fmt;
use serde::{Deserialize, Serialize};

// IR system modules
pub mod instruction;
pub mod value;
pub mod function;
pub mod module;
pub mod generator;
pub mod optimizer;

// Simple codegen without LLVM dependency
pub mod simple_codegen;

// Re-export main types
pub use instruction::{Instruction, BasicBlock, Label};
pub use value::{IRValue, IRType};
pub use function::{IRFunction, Parameter};
pub use module::IRModule;
pub use generator::IRGenerator;
pub use optimizer::{IROptimizer, OptimizationLevel};
pub use simple_codegen::CCodeGenerator;

/// A complete IR program consisting of multiple modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRProgram {
    pub modules: Vec<IRModule>,
    pub entry_point: Option<String>, // Function name to start execution
    pub global_variables: HashMap<String, IRValue>,
    pub string_table: Vec<String>, // For string constants
}

impl IRProgram {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
            entry_point: None,
            global_variables: HashMap::new(),
            string_table: Vec::new(),
        }
    }
    
    pub fn add_module(&mut self, module: IRModule) {
        self.modules.push(module);
    }
    
    pub fn set_entry_point(&mut self, function_name: String) {
        self.entry_point = Some(function_name);
    }
    
    pub fn add_global(&mut self, name: String, value: IRValue) {
        self.global_variables.insert(name, value);
    }
    
    pub fn add_string_constant(&mut self, s: String) -> usize {
        let index = self.string_table.len();
        self.string_table.push(s);
        index
    }
    
    pub fn get_string(&self, index: usize) -> Option<&String> {
        self.string_table.get(index)
    }
}

impl Default for IRProgram {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for IRProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "; Seen IR Program")?;
        
        if let Some(entry) = &self.entry_point {
            writeln!(f, "; Entry point: {}", entry)?;
        }
        
        if !self.string_table.is_empty() {
            writeln!(f, "\n; String Constants")?;
            for (i, s) in self.string_table.iter().enumerate() {
                writeln!(f, "@str.{} = \"{}\"", i, s.escape_default())?;
            }
        }
        
        if !self.global_variables.is_empty() {
            writeln!(f, "\n; Global Variables")?;
            for (name, value) in &self.global_variables {
                writeln!(f, "@{} = {}", name, value)?;
            }
        }
        
        for module in &self.modules {
            writeln!(f, "\n{}", module)?;
        }
        
        Ok(())
    }
}

/// Errors that can occur during IR generation
#[derive(Debug, Clone, PartialEq)]
pub enum IRError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    TypeMismatch { expected: IRType, found: IRType },
    InvalidOperation { operation: String, operand_types: Vec<IRType> },
    InvalidJump(String),
    InvalidLabel(String),
    TooManyArguments { expected: usize, found: usize },
    TooFewArguments { expected: usize, found: usize },
    RecursionDepthExceeded,
    InvalidMemoryAccess,
    DivisionByZero,
    Other(String),
}

impl fmt::Display for IRError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IRError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            IRError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            IRError::TypeMismatch { expected, found } => {
                write!(f, "Type mismatch: expected {:?}, found {:?}", expected, found)
            }
            IRError::InvalidOperation { operation, operand_types } => {
                write!(f, "Invalid operation '{}' for types: {:?}", operation, operand_types)
            }
            IRError::InvalidJump(label) => write!(f, "Invalid jump to label: {}", label),
            IRError::InvalidLabel(label) => write!(f, "Invalid label: {}", label),
            IRError::TooManyArguments { expected, found } => {
                write!(f, "Too many arguments: expected {}, found {}", expected, found)
            }
            IRError::TooFewArguments { expected, found } => {
                write!(f, "Too few arguments: expected {}, found {}", expected, found)
            }
            IRError::RecursionDepthExceeded => write!(f, "Recursion depth exceeded"),
            IRError::InvalidMemoryAccess => write!(f, "Invalid memory access"),
            IRError::DivisionByZero => write!(f, "Division by zero"),
            IRError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for IRError {}

pub type IRResult<T> = Result<T, IRError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ir_program_creation() {
        let program = IRProgram::new();
        assert!(program.modules.is_empty());
        assert!(program.entry_point.is_none());
        assert!(program.global_variables.is_empty());
        assert!(program.string_table.is_empty());
    }
    
    #[test]
    fn test_string_constant_management() {
        let mut program = IRProgram::new();
        let index1 = program.add_string_constant("Hello".to_string());
        let index2 = program.add_string_constant("World".to_string());
        
        assert_eq!(index1, 0);
        assert_eq!(index2, 1);
        assert_eq!(program.get_string(0), Some(&"Hello".to_string()));
        assert_eq!(program.get_string(1), Some(&"World".to_string()));
        assert_eq!(program.get_string(2), None);
    }
    
    #[test]
    fn test_ir_error_display() {
        let error = IRError::UndefinedVariable("x".to_string());
        assert_eq!(error.to_string(), "Undefined variable: x");
        
        let error = IRError::TypeMismatch { 
            expected: IRType::Integer, 
            found: IRType::String 
        };
        assert!(error.to_string().contains("Type mismatch"));
    }
}