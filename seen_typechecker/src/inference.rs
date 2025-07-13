//! Type inference engine for the Seen programming language

use crate::errors::TypeError;
use crate::types::Type;
use std::collections::HashMap;

/// Type inference context
#[derive(Debug, Clone)]
pub struct InferenceContext {
    /// Type variables and their constraints
    type_variables: HashMap<String, TypeVariable>,
    /// Current type variable counter
    next_var_id: usize,
}

/// A type variable used during inference
#[derive(Debug, Clone)]
pub struct TypeVariable {
    /// Unique identifier for this type variable
    pub id: usize,
    /// Constraints on this type variable
    pub constraints: Vec<TypeConstraint>,
    /// Resolved type if inference is complete
    pub resolved: Option<Type>,
}

/// Constraints on type variables
#[derive(Debug, Clone)]
pub enum TypeConstraint {
    /// Must be equal to another type
    EqualTo(Type),
    /// Must be a numeric type
    Numeric,
    /// Must be a comparable type
    Comparable,
    /// Must be indexable (array-like)
    Indexable,
}

impl InferenceContext {
    /// Create a new inference context
    pub fn new() -> Self {
        Self {
            type_variables: HashMap::new(),
            next_var_id: 0,
        }
    }

    /// Create a fresh type variable
    pub fn fresh_type_var(&mut self) -> Type {
        let id = self.next_var_id;
        self.next_var_id += 1;

        let var_name = format!("t{}", id);
        let type_var = TypeVariable {
            id,
            constraints: Vec::new(),
            resolved: None,
        };

        self.type_variables.insert(var_name.clone(), type_var);
        Type::Generic(var_name)
    }

    /// Add a constraint to a type variable
    pub fn add_constraint(&mut self, type_var: &str, constraint: TypeConstraint) -> Result<(), TypeError> {
        if let Some(var) = self.type_variables.get_mut(type_var) {
            var.constraints.push(constraint);
            self.try_resolve_variable(type_var)?;
        }
        Ok(())
    }

    /// Unify two types
    pub fn unify(&mut self, t1: &Type, t2: &Type) -> Result<Type, TypeError> {
        match (t1, t2) {
            // Same types unify to themselves
            (a, b) if a == b => Ok(a.clone()),

            // Type variables
            (Type::Generic(var), other) | (other, Type::Generic(var)) => {
                self.unify_with_variable(var, other)
            }

            // Arrays
            (Type::Array(elem1), Type::Array(elem2)) => {
                let unified_elem = self.unify(elem1, elem2)?;
                Ok(Type::Array(Box::new(unified_elem)))
            }

            // Functions
            (Type::Function { params: p1, return_type: r1 },
                Type::Function { params: p2, return_type: r2 }) => {
                if p1.len() != p2.len() {
                    return Err(TypeError::InferenceFailed {
                        position: seen_lexer::token::Position::new(0, 0)
                    });
                }

                let unified_params = p1.iter()
                    .zip(p2.iter())
                    .map(|(a, b)| self.unify(a, b))
                    .collect::<Result<Vec<_>, _>>()?;

                let unified_return = self.unify(r1, r2)?;

                Ok(Type::Function {
                    params: unified_params,
                    return_type: Box::new(unified_return),
                })
            }

            // Optionals
            (Type::Optional(inner1), Type::Optional(inner2)) => {
                let unified_inner = self.unify(inner1, inner2)?;
                Ok(Type::Optional(Box::new(unified_inner)))
            }

            // Non-optional can unify with optional of same type
            (non_opt, Type::Optional(inner)) | (Type::Optional(inner), non_opt) => {
                let unified = self.unify(non_opt, inner)?;
                Ok(Type::Optional(Box::new(unified)))
            }

            // Numeric promotion
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => Ok(Type::Float),

            // Unknown type unifies with anything
            (Type::Unknown, other) | (other, Type::Unknown) => Ok(other.clone()),

            // Otherwise, types don't unify
            _ => Err(TypeError::InferenceFailed {
                position: seen_lexer::token::Position::new(0, 0)
            }),
        }
    }

    /// Unify a type with a type variable
    fn unify_with_variable(&mut self, var_name: &str, other_type: &Type) -> Result<Type, TypeError> {
        let var = self.type_variables.get(var_name).cloned();
        if let Some(var) = var {
            // Check if already resolved
            if let Some(resolved) = &var.resolved {
                return self.unify(&resolved, other_type);
            }

            // Check constraints
            if !self.satisfies_constraints(other_type, &var.constraints)? {
                return Err(TypeError::GenericConstraintViolation {
                    actual_type: other_type.clone(),
                    position: seen_lexer::token::Position::new(0, 0),
                });
            }

            // Resolve the variable
            if let Some(var_mut) = self.type_variables.get_mut(var_name) {
                var_mut.resolved = Some(other_type.clone());
            }
            Ok(other_type.clone())
        } else {
            // Variable doesn't exist, treat as regular type
            Ok(other_type.clone())
        }
    }

    /// Check if a type satisfies the given constraints
    fn satisfies_constraints(&self, t: &Type, constraints: &[TypeConstraint]) -> Result<bool, TypeError> {
        for constraint in constraints {
            match constraint {
                TypeConstraint::EqualTo(expected) => {
                    if t != expected {
                        return Ok(false);
                    }
                }
                TypeConstraint::Numeric => {
                    if !t.is_numeric() {
                        return Ok(false);
                    }
                }
                TypeConstraint::Comparable => {
                    // For now, primitives are comparable
                    if !matches!(t, Type::Int | Type::Float | Type::String | Type::Char | Type::Bool) {
                        return Ok(false);
                    }
                }
                TypeConstraint::Indexable => {
                    if !matches!(t, Type::Array(_) | Type::String) {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    /// Try to resolve a type variable based on its constraints
    fn try_resolve_variable(&mut self, var_name: &str) -> Result<(), TypeError> {
        let var = self.type_variables.get(var_name).cloned();
        if let Some(var) = var {
            if var.resolved.is_some() {
                return Ok(());
            }

            // Try to infer type from constraints
            let mut candidate_type = None;

            for constraint in &var.constraints {
                match constraint {
                    TypeConstraint::EqualTo(t) => {
                        if candidate_type.is_none() {
                            candidate_type = Some(t.clone());
                        } else if candidate_type.as_ref() != Some(t) {
                            return Err(TypeError::InferenceFailed {
                                position: seen_lexer::token::Position::new(0, 0),
                            });
                        }
                    }
                    TypeConstraint::Numeric => {
                        if candidate_type.is_none() {
                            candidate_type = Some(Type::Int); // Default to Int for numeric
                        } else if let Some(ref t) = candidate_type {
                            if !t.is_numeric() {
                                return Err(TypeError::InferenceFailed {
                                    position: seen_lexer::token::Position::new(0, 0),
                                });
                            }
                        }
                    }
                    _ => {
                        // Other constraints don't provide specific types
                    }
                }
            }

            if let Some(resolved_type) = candidate_type {
                if let Some(var_mut) = self.type_variables.get_mut(var_name) {
                    var_mut.resolved = Some(resolved_type);
                }
            }
        }

        Ok(())
    }

    /// Resolve all type variables to concrete types
    pub fn resolve_all(&mut self) -> Result<(), Vec<TypeError>> {
        let mut errors = Vec::new();
        let var_names: Vec<String> = self.type_variables.keys().cloned().collect();

        for var_name in var_names {
            if let Some(var) = self.type_variables.get(&var_name) {
                if var.resolved.is_none() {
                    // Try to resolve based on constraints
                    if let Err(e) = self.try_resolve_variable(&var_name) {
                        errors.push(e);
                    } else if let Some(var) = self.type_variables.get_mut(&var_name) {
                        if var.resolved.is_none() {
                            // Still unresolved, use Unknown as fallback
                            var.resolved = Some(Type::Unknown);
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get the resolved type for a type variable
    pub fn get_resolved_type(&self, var_name: &str) -> Option<&Type> {
        self.type_variables.get(var_name)?.resolved.as_ref()
    }

    /// Substitute resolved types in a type expression
    pub fn substitute(&self, t: &Type) -> Type {
        match t {
            Type::Generic(var_name) => {
                self.get_resolved_type(var_name)
                    .cloned()
                    .unwrap_or_else(|| t.clone())
            }
            Type::Array(elem) => {
                Type::Array(Box::new(self.substitute(elem)))
            }
            Type::Optional(inner) => {
                Type::Optional(Box::new(self.substitute(inner)))
            }
            Type::Function { params, return_type } => {
                let substituted_params = params.iter()
                    .map(|p| self.substitute(p))
                    .collect();
                let substituted_return = self.substitute(return_type);
                Type::Function {
                    params: substituted_params,
                    return_type: Box::new(substituted_return),
                }
            }
            _ => t.clone(),
        }
    }
}

impl Default for InferenceContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fresh_type_var() {
        let mut ctx = InferenceContext::new();
        let var1 = ctx.fresh_type_var();
        let var2 = ctx.fresh_type_var();

        assert!(matches!(var1, Type::Generic(_)));
        assert!(matches!(var2, Type::Generic(_)));
        assert_ne!(var1, var2);
    }

    #[test]
    fn test_unify_same_types() {
        let mut ctx = InferenceContext::new();
        let result = ctx.unify(&Type::Int, &Type::Int).unwrap();
        assert_eq!(result, Type::Int);
    }

    #[test]
    fn test_unify_numeric_promotion() {
        let mut ctx = InferenceContext::new();
        let result = ctx.unify(&Type::Int, &Type::Float).unwrap();
        assert_eq!(result, Type::Float);
    }

    #[test]
    fn test_unify_arrays() {
        let mut ctx = InferenceContext::new();
        let arr1 = Type::Array(Box::new(Type::Int));
        let arr2 = Type::Array(Box::new(Type::Int));
        let result = ctx.unify(&arr1, &arr2).unwrap();
        assert_eq!(result, arr1);
    }
}