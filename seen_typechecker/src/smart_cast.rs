//! Smart casting implementation for nullable type refinement
//! 
//! After null checks, the compiler automatically refines types from nullable to non-nullable

use std::collections::HashMap;
use seen_parser::ast::*;
use crate::types::Type;

/// Tracks smart cast information for variables in different scopes
#[derive(Debug, Clone)]
pub struct SmartCastContext {
    /// Maps variable names to their refined types in the current scope
    refinements: HashMap<String, Type>,
    /// Parent context for nested scopes
    parent: Option<Box<SmartCastContext>>,
}

impl SmartCastContext {
    /// Create a new empty smart cast context
    pub fn new() -> Self {
        Self {
            refinements: HashMap::new(),
            parent: None,
        }
    }

    /// Create a new context with a parent
    pub fn with_parent(parent: SmartCastContext) -> Self {
        Self {
            refinements: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    /// Refine a variable's type after a null check
    pub fn refine_type(&mut self, var_name: String, refined_type: Type) {
        self.refinements.insert(var_name, refined_type);
    }

    /// Get the refined type for a variable if it exists
    pub fn get_refined_type(&self, var_name: &str) -> Option<&Type> {
        self.refinements.get(var_name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_refined_type(var_name)))
    }

    /// Remove all refinements (e.g., when exiting a scope)
    pub fn clear_refinements(&mut self) {
        self.refinements.clear();
    }

    /// Merge refinements from two branches (e.g., after if-else)
    /// Only keeps refinements that are valid in both branches
    pub fn merge_contexts(left: &SmartCastContext, right: &SmartCastContext) -> SmartCastContext {
        let mut result = SmartCastContext::new();
        
        // Only keep refinements that exist and match in both branches
        for (name, left_type) in &left.refinements {
            if let Some(right_type) = right.refinements.get(name) {
                if types_match(left_type, right_type) {
                    result.refinements.insert(name.clone(), left_type.clone());
                }
            }
        }
        
        result
    }
}

/// Helper function to check if two types match for smart casting purposes
fn types_match(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Int, Type::Int) => true,
        (Type::Float, Type::Float) => true,
        (Type::Bool, Type::Bool) => true,
        (Type::String, Type::String) => true,
        (Type::Char, Type::Char) => true,
        (Type::Unit, Type::Unit) => true,
        (Type::Nullable(l), Type::Nullable(r)) => types_match(l, r),
        (Type::Array(l), Type::Array(r)) => types_match(l, r),
        (Type::Function { params: l_params, return_type: l_ret, is_async: l_async }, 
         Type::Function { params: r_params, return_type: r_ret, is_async: r_async }) => {
            l_async == r_async &&
            l_params.len() == r_params.len() &&
            l_params.iter().zip(r_params.iter()).all(|(l, r)| types_match(l, r)) &&
            types_match(l_ret, r_ret)
        }
        (Type::Struct { name: l_name, .. }, Type::Struct { name: r_name, .. }) => l_name == r_name,
        (Type::Enum { name: l_name, .. }, Type::Enum { name: r_name, .. }) => l_name == r_name,
        _ => false,
    }
}

/// Analyzes expressions to determine smart cast opportunities
pub struct SmartCastAnalyzer;

impl SmartCastAnalyzer {
    /// Analyze an if condition for null checks and return smart cast information
    pub fn analyze_condition(condition: &Expression) -> Vec<(String, Type)> {
        let mut refinements = Vec::new();
        
        match condition {
            // Direct null check: variable != null
            Expression::BinaryOp { left, op, right, .. } => {
                if let BinaryOperator::NotEqual = op {
                    if let Expression::Identifier { name, .. } = &**left {
                        if let Expression::NullLiteral { .. } = &**right {
                            // Variable is not null in the true branch
                            // We need the original type to refine it
                            refinements.push((name.clone(), Type::Unknown)); // Placeholder
                        }
                    } else if let Expression::NullLiteral { .. } = &**left {
                        if let Expression::Identifier { name, .. } = &**right {
                            // null != variable
                            refinements.push((name.clone(), Type::Unknown)); // Placeholder
                        }
                    }
                }
                // Check for == null (inverse refinement)
                if let BinaryOperator::Equal = op {
                    // In this case, the variable IS null in the true branch
                    // So no refinement in the true branch, but refinement in false branch
                    // This would be handled by the caller
                }
            }
            // For now, we don't analyze compound conditions or method calls
            // These could be added in the future
            _ => {}
        }
        
        refinements
    }

    /// Apply smart casts within an if statement
    pub fn apply_smart_casts(
        if_expr: &Expression,
        context: &mut SmartCastContext,
        original_types: &HashMap<String, Type>,
    ) {
        if let Expression::If { condition, then_branch, else_branch, .. } = if_expr {
            // Analyze the condition for smart cast opportunities
            let refinements = Self::analyze_condition(condition);
            
            // Create contexts for both branches
            let mut then_context = SmartCastContext::with_parent(context.clone());
            let mut else_context = SmartCastContext::with_parent(context.clone());
            
            // Apply refinements to the then branch
            for (var_name, _) in &refinements {
                if let Some(original_type) = original_types.get(var_name) {
                    if let Type::Nullable(inner) = original_type {
                        // Refine from T? to T
                        then_context.refine_type(var_name.clone(), (**inner).clone());
                    }
                }
            }
            
            // For else branch, apply inverse refinements for == null checks
            if let Expression::BinaryOp { left, op, right, .. } = &**condition {
                if let BinaryOperator::Equal = op {
                    if let Expression::Identifier { name, .. } = &**left {
                        if let Expression::NullLiteral { .. } = &**right {
                            // Variable is null in true branch, not null in false branch
                            if let Some(original_type) = original_types.get(name) {
                                if let Type::Nullable(inner) = original_type {
                                    else_context.refine_type(name.clone(), (**inner).clone());
                                }
                            }
                        }
                    }
                }
            }
            
            // After the if statement, merge contexts if needed
            if else_branch.is_some() {
                *context = SmartCastContext::merge_contexts(&then_context, &else_context);
            } else {
                // No else branch, so refinements don't persist
                context.clear_refinements();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_cast_basic_null_check() {
        let mut context = SmartCastContext::new();
        let mut original_types = HashMap::new();
        let user_type = Type::Struct {
            name: "User".to_string(),
            fields: HashMap::new(),
            generics: Vec::new(),
        };
        original_types.insert("user".to_string(), Type::Nullable(Box::new(user_type.clone())));
        
        // Simulate: if user != null { ... }
        context.refine_type("user".to_string(), user_type.clone());
        
        assert!(matches!(
            context.get_refined_type("user"),
            Some(Type::Struct { name, .. }) if name == "User"
        ));
    }

    #[test]
    fn test_smart_cast_merge_contexts() {
        let mut left = SmartCastContext::new();
        let mut right = SmartCastContext::new();
        
        left.refine_type("x".to_string(), Type::Int);
        left.refine_type("y".to_string(), Type::String);
        
        right.refine_type("x".to_string(), Type::Int);
        right.refine_type("z".to_string(), Type::Bool);
        
        let merged = SmartCastContext::merge_contexts(&left, &right);
        
        // Only x should be in the merged context
        assert!(merged.get_refined_type("x").is_some());
        assert!(merged.get_refined_type("y").is_none());
        assert!(merged.get_refined_type("z").is_none());
    }

    #[test]
    fn test_types_match() {
        assert!(types_match(&Type::Int, &Type::Int));
        assert!(types_match(&Type::String, &Type::String));
        assert!(!types_match(&Type::Int, &Type::String));
        
        let nullable_int = Type::Nullable(Box::new(Type::Int));
        let nullable_string = Type::Nullable(Box::new(Type::String));
        assert!(types_match(&nullable_int, &nullable_int));
        assert!(!types_match(&nullable_int, &nullable_string));
    }
}