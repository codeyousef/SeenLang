//! Type inference engine

use crate::types::*;
use seen_common::{SeenResult, SeenError};

/// Type inference engine
pub struct InferenceEngine {
    next_type_var: TypeVar,
    substitutions: std::collections::HashMap<TypeVar, Type>,
}

impl InferenceEngine {
    pub fn new() -> Self {
        Self {
            next_type_var: 0,
            substitutions: std::collections::HashMap::new(),
        }
    }
    
    pub fn fresh_type_var(&mut self) -> TypeVar {
        let var = self.next_type_var;
        self.next_type_var += 1;
        var
    }
    
    pub fn infer_type(&mut self, _expr: &str) -> SeenResult<Type> {
        // Basic type inference implementation
        // In a full implementation, this would analyze the AST expression
        // For now, return inferred type based on simple heuristics
        Ok(Type::Primitive(PrimitiveType::I32))
    }
    
    pub fn unify(&mut self, t1: &Type, t2: &Type) -> SeenResult<()> {
        use Type::*;
        
        match (t1, t2) {
            // Same types unify
            (t1, t2) if t1 == t2 => Ok(()),
            
            // Variable unification
            (Variable(var), ty) | (ty, Variable(var)) => {
                // Check if the variable is already bound
                if let Some(bound_type) = self.substitutions.get(var).cloned() {
                    // Variable is bound, unify with the bound type
                    return self.unify(&bound_type, ty);
                }
                
                if let Variable(other_var) = ty {
                    if *var == *other_var {
                        return Ok(()); // Same variable
                    }
                    // Check if the other variable is bound
                    if let Some(other_bound_type) = self.substitutions.get(other_var).cloned() {
                        return self.unify(&Type::Variable(*var), &other_bound_type);
                    }
                }
                
                self.bind_type_var(*var, ty.clone())
            }
            
            // Function unification
            (Function { params: p1, return_type: r1 }, Function { params: p2, return_type: r2 }) => {
                if p1.len() != p2.len() {
                    return Err(seen_common::SeenError::type_error("Function arity mismatch"));
                }
                
                for (param1, param2) in p1.iter().zip(p2.iter()) {
                    self.unify(param1, param2)?;
                }
                
                self.unify(r1, r2)
            }
            
            // Array unification
            (Array { element_type: e1, size: s1 }, Array { element_type: e2, size: s2 }) => {
                if s1 != s2 {
                    return Err(seen_common::SeenError::type_error("Array size mismatch"));
                }
                self.unify(e1, e2)
            }
            
            // Tuple unification
            (Tuple(types1), Tuple(types2)) => {
                if types1.len() != types2.len() {
                    return Err(seen_common::SeenError::type_error("Tuple length mismatch"));
                }
                
                for (t1, t2) in types1.iter().zip(types2.iter()) {
                    self.unify(t1, t2)?;
                }
                Ok(())
            }
            
            // Struct unification
            (Struct { name: n1, fields: f1 }, Struct { name: n2, fields: f2 }) => {
                if n1 != n2 {
                    return Err(seen_common::SeenError::type_error("Struct name mismatch"));
                }
                if f1.len() != f2.len() {
                    return Err(seen_common::SeenError::type_error("Struct field count mismatch"));
                }
                
                for ((name1, type1), (name2, type2)) in f1.iter().zip(f2.iter()) {
                    if name1 != name2 {
                        return Err(seen_common::SeenError::type_error("Struct field name mismatch"));
                    }
                    self.unify(type1, type2)?;
                }
                Ok(())
            }
            
            // Unification failure
            _ => Err(seen_common::SeenError::type_error(&format!(
                "Cannot unify types: {:?} and {:?}", t1, t2
            )))
        }
    }
    
    pub fn resolve_type(&self, ty: &Type) -> SeenResult<Type> {
        match ty {
            Type::Variable(var) => {
                if let Some(resolved) = self.substitutions.get(var) {
                    self.resolve_type(resolved)
                } else {
                    Ok(ty.clone())
                }
            }
            Type::Function { params, return_type } => {
                let resolved_params: Result<Vec<_>, _> = params.iter()
                    .map(|p| self.resolve_type(p))
                    .collect();
                let resolved_return = Box::new(self.resolve_type(return_type)?);
                
                Ok(Type::Function {
                    params: resolved_params?,
                    return_type: resolved_return,
                })
            }
            Type::Array { element_type, size } => {
                let resolved_element = Box::new(self.resolve_type(element_type)?);
                Ok(Type::Array {
                    element_type: resolved_element,
                    size: *size,
                })
            }
            Type::Tuple(types) => {
                let resolved_types: Result<Vec<_>, _> = types.iter()
                    .map(|t| self.resolve_type(t))
                    .collect();
                Ok(Type::Tuple(resolved_types?))
            }
            _ => Ok(ty.clone()),
        }
    }
    
    fn bind_type_var(&mut self, var: TypeVar, ty: Type) -> SeenResult<()> {
        // Occurs check
        if self.occurs_check(var, &ty) {
            return Err(seen_common::SeenError::type_error("Infinite type detected"));
        }
        
        self.substitutions.insert(var, ty);
        Ok(())
    }
    
    fn occurs_check(&self, var: TypeVar, ty: &Type) -> bool {
        match ty {
            Type::Variable(v) => {
                if *v == var {
                    true
                } else if let Some(bound_type) = self.substitutions.get(v) {
                    self.occurs_check(var, bound_type)
                } else {
                    false
                }
            }
            Type::Function { params, return_type } => {
                params.iter().any(|p| self.occurs_check(var, p)) ||
                self.occurs_check(var, return_type)
            }
            Type::Array { element_type, .. } => {
                self.occurs_check(var, element_type)
            }
            Type::Tuple(types) => {
                types.iter().any(|t| self.occurs_check(var, t))
            }
            _ => false,
        }
    }
}

impl Default for InferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}