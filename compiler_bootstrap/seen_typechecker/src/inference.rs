//! Type inference engine

use crate::types::*;
use seen_common::{SeenResult, SeenError};

/// Type inference engine
pub struct InferenceEngine {
    next_type_var: TypeVar,
}

impl InferenceEngine {
    pub fn new() -> Self {
        Self {
            next_type_var: 0,
        }
    }
    
    pub fn fresh_type_var(&mut self) -> Type {
        let var = self.next_type_var;
        self.next_type_var += 1;
        Type::Variable(var)
    }
    
    pub fn infer_type(&mut self, _expr: &str) -> SeenResult<Type> {
        // TODO: Implement actual type inference
        // For now, return a placeholder
        Ok(Type::Primitive(PrimitiveType::I32))
    }
}

impl Default for InferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}