//! Type constraint solving

use crate::types::*;
use seen_common::SeenResult;

/// Type constraint
#[derive(Debug, Clone)]
pub enum Constraint {
    Equal(Type, Type),
    Subtype(Type, Type),
}

/// Constraint solver
pub struct ConstraintSolver {
    constraints: Vec<Constraint>,
}

impl ConstraintSolver {
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }
    
    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }
    
    pub fn solve(&self) -> SeenResult<Substitution> {
        let mut substitution = Substitution::new();
        
        for constraint in &self.constraints {
            match constraint {
                Constraint::Equal(t1, t2) => {
                    substitution = self.unify(t1, t2, substitution)?;
                }
                Constraint::Subtype(t1, t2) => {
                    substitution = self.check_subtype(t1, t2, substitution)?;
                }
            }
        }
        
        Ok(substitution)
    }
    
    fn unify(&self, t1: &Type, t2: &Type, mut substitution: Substitution) -> SeenResult<Substitution> {
        let t1 = substitution.apply(t1);
        let t2 = substitution.apply(t2);
        
        match (&t1, &t2) {
            (Type::Primitive(p1), Type::Primitive(p2)) if p1 == p2 => Ok(substitution),
            (Type::Variable(var), other) | (other, Type::Variable(var)) => {
                if let Type::Variable(other_var) = other {
                    if var == other_var {
                        return Ok(substitution);
                    }
                }
                substitution.bind(*var, other.clone());
                Ok(substitution)
            }
            (Type::Function { params: p1, return_type: r1 }, Type::Function { params: p2, return_type: r2 }) => {
                if p1.len() != p2.len() {
                    return Err(seen_common::SeenError::type_error("Function arity mismatch"));
                }
                
                for (param1, param2) in p1.iter().zip(p2.iter()) {
                    substitution = self.unify(param1, param2, substitution)?;
                }
                
                self.unify(r1, r2, substitution)
            }
            _ if t1 == t2 => Ok(substitution),
            _ => Err(seen_common::SeenError::type_error(format!("Cannot unify {:?} with {:?}", t1, t2))),
        }
    }
    
    fn check_subtype(&self, _t1: &Type, _t2: &Type, substitution: Substitution) -> SeenResult<Substitution> {
        Ok(substitution)
    }
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self::new()
    }
}