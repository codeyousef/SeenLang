//! Type constraint solving

use crate::types::*;
use seen_common::{SeenResult, SeenError};

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
        // TODO: Implement actual constraint solving
        // For now, return empty substitution
        Ok(Substitution::new())
    }
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self::new()
    }
}