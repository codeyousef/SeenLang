//! Type checking implementation

use crate::types::*;
use seen_common::{SeenResult, SeenError, Diagnostics};
use seen_parser::Program;

/// Type checker
pub struct TypeChecker {
    env: TypeEnvironment,
    diagnostics: Diagnostics,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: TypeEnvironment::new(),
            diagnostics: Diagnostics::new(),
        }
    }
    
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
    
    pub fn check_program(&mut self, program: &Program) -> SeenResult<()> {
        // Basic type checking - verify program structure
        for item in &program.items {
            match &item.kind {
                seen_parser::ItemKind::Function(func) => {
                    // Basic function validation
                    if func.name.value.is_empty() {
                        self.diagnostics.error("Function name cannot be empty", func.name.span);
                    }
                    // TODO: Add more comprehensive type checking
                }
                _ => {
                    // TODO: Handle other item types
                }
            }
        }
        
        Ok(())
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}