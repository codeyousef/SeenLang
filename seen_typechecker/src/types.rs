//! Type definitions for the Seen type system
//! 
//! Implements nullable-by-default types with compile-time safety.
//! All types support generics and smart-casting after null checks.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Represents a type in the Seen language
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    /// Primitive integer type
    Int,
    /// Primitive unsigned integer type
    UInt,
    /// Primitive floating-point type
    Float,
    /// Primitive boolean type
    Bool,
    /// Primitive string type
    String,
    /// Primitive character type
    Char,
    /// Array type with element type
    Array(Box<Type>),
    /// Nullable type (all types can be nullable)
    Nullable(Box<Type>),
    /// Function type with parameter types and return type
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
        is_async: bool,
    },
    /// User-defined struct type
    Struct {
        name: String,
        fields: HashMap<String, Type>,
        generics: Vec<String>,
    },
    /// User-defined enum type
    Enum {
        name: String,
        variants: Vec<String>,
        generics: Vec<String>,
    },
    /// Interface/trait type
    Interface {
        name: String,
        methods: Vec<String>,
        generics: Vec<String>,
    },
    /// Generic type parameter
    Generic(String),
    /// Unit/void type
    Unit,
    /// Unknown type (for inference)
    Unknown,
    /// Error type (for error recovery)
    Error,
}

impl Type {
    /// Check if this type is primitive
    pub fn is_primitive(&self) -> bool {
        matches!(self, Type::Int | Type::UInt | Type::Float | Type::Bool | Type::String | Type::Char)
    }
    
    /// Check if this type is numeric
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::UInt | Type::Float)
    }
    
    /// Check if this type is nullable
    pub fn is_nullable(&self) -> bool {
        matches!(self, Type::Nullable(_))
    }
    
    /// Get the non-nullable version of this type
    pub fn non_nullable(&self) -> &Type {
        match self {
            Type::Nullable(inner) => inner.non_nullable(),
            other => other,
        }
    }
    
    /// Get the underlying type if this is nullable
    pub fn unwrap_nullable(&self) -> Option<&Type> {
        match self {
            Type::Nullable(inner) => Some(inner),
            _ => None,
        }
    }
    
    /// Make this type nullable
    pub fn nullable(self) -> Type {
        if self.is_nullable() {
            self
        } else {
            Type::Nullable(Box::new(self))
        }
    }
    
    /// Check if this type supports a specific operation
    pub fn supports_operation(&self, op: &str) -> bool {
        match self.non_nullable() {
            Type::Int | Type::UInt | Type::Float => {
                matches!(op, "+" | "-" | "*" | "/" | "%" | "<" | ">" | "<=" | ">=" | "==" | "!=")
            }
            Type::String => {
                matches!(op, "+" | "==" | "!=" | "<" | ">" | "<=" | ">=")
            }
            Type::Bool => {
                matches!(op, "and" | "or" | "not" | "==" | "!=")
            }
            Type::Array(_) => {
                matches!(op, "[]" | "==" | "!=")
            }
            _ => false,
        }
    }
    
    /// Get the result type of a binary operation
    pub fn binary_operation_result(&self, op: &str, other: &Type) -> Option<Type> {
        // Handle nullable operands
        let (left, right) = (self.non_nullable(), other.non_nullable());
        let result_nullable = self.is_nullable() || other.is_nullable();
        
        let result = match (left, op, right) {
            // Arithmetic operations
            (Type::Int, "+"|"-"|"*"|"/"|"%", Type::Int) => Some(Type::Int),
            (Type::UInt, "+"|"-"|"*"|"/"|"%", Type::UInt) => Some(Type::UInt),
            (Type::Float, "+"|"-"|"*"|"/"|"%", Type::Float) => Some(Type::Float),
            (Type::Int, "+"|"-"|"*"|"/"|"%", Type::Float) => Some(Type::Float),
            (Type::Float, "+"|"-"|"*"|"/"|"%", Type::Int) => Some(Type::Float),
            (Type::UInt, "+"|"-"|"*"|"/"|"%", Type::Int) => Some(Type::Int),
            (Type::UInt, "+"|"-"|"*"|"/"|"%", Type::Float) => Some(Type::Float),
            
            // String concatenation
            (Type::String, "+", Type::String) => Some(Type::String),
            
            // Comparison operations
            (Type::Int, "<"|">"|"<="|">="|"=="|"!=", Type::Int) => Some(Type::Bool),
            (Type::UInt, "<"|">"|"<="|">="|"=="|"!=", Type::UInt) => Some(Type::Bool),
            (Type::Float, "<"|">"|"<="|">="|"=="|"!=", Type::Float) => Some(Type::Bool),
            (Type::String, "<"|">"|"<="|">="|"=="|"!=", Type::String) => Some(Type::Bool),
            
            // Logical operations
            (Type::Bool, "and"|"or", Type::Bool) => Some(Type::Bool),
            
            // Equality for any type
            (a, "=="|"!=", b) if a == b => Some(Type::Bool),
            
            _ => None,
        }?;
        
        Some(if result_nullable && result != Type::Bool {
            result.nullable()
        } else {
            result
        })
    }
    
    /// Helper constructors for common types
    pub fn int() -> Type { Type::Int }
    pub fn uint() -> Type { Type::UInt }
    pub fn float() -> Type { Type::Float }
    pub fn bool() -> Type { Type::Bool }
    pub fn string() -> Type { Type::String }
    pub fn char() -> Type { Type::Char }
    pub fn unit() -> Type { Type::Unit }
    pub fn error() -> Type { Type::Error }
    
    /// Check if two types are compatible for assignment
    pub fn is_assignable_to(&self, other: &Type) -> bool {
        match (self, other) {
            // Exact match
            (a, b) if a == b => true,
            
            // Int can be assigned to Float
            (Type::Int, Type::Float) => true,
            
            // Any type can be assigned to its nullable version
            (inner, Type::Nullable(opt_inner)) => inner.is_assignable_to(opt_inner),
            
            // Array types must have compatible element types
            (Type::Array(a), Type::Array(b)) => a.is_assignable_to(b),
            
            // Function types must have compatible signatures
            (Type::Function { params: p1, return_type: r1, .. }, 
             Type::Function { params: p2, return_type: r2, .. }) => {
                p1.len() == p2.len() && 
                p1.iter().zip(p2).all(|(a, b)| a.is_assignable_to(b)) &&
                r1.is_assignable_to(r2)
            },
            
            // Unknown type is compatible with anything (for inference)
            (Type::Unknown, _) | (_, Type::Unknown) => true,
            
            _ => false,
        }
    }
    
    /// Get a human-readable name for this type
    pub fn name(&self) -> String {
        match self {
            Type::Int => "Int".to_string(),
            Type::UInt => "UInt".to_string(),
            Type::Float => "Float".to_string(),
            Type::Bool => "Bool".to_string(),
            Type::String => "String".to_string(),
            Type::Char => "Char".to_string(),
            Type::Array(inner) => format!("Array<{}>", inner.name()),
            Type::Nullable(inner) => format!("{}?", inner.name()),
            Type::Function { params, return_type, .. } => {
                let param_names: Vec<String> = params.iter().map(|p| p.name()).collect();
                format!("({}) -> {}", param_names.join(", "), return_type.name())
            },
            Type::Struct { name, .. } => name.clone(),
            Type::Enum { name, .. } => name.clone(),
            Type::Interface { name, .. } => name.clone(),
            Type::Generic(name) => name.clone(),
            Type::Unit => "()".to_string(),
            Type::Unknown => "?".to_string(),
            Type::Error => "Error".to_string(),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Type information attached to AST nodes
#[derive(Debug, Clone, PartialEq)]
pub struct TypeInfo {
    /// The resolved type
    pub resolved_type: Type,
    /// Whether this type was inferred or explicitly specified
    pub inferred: bool,
}

impl TypeInfo {
    /// Create type info for an explicitly specified type
    pub fn explicit(t: Type) -> Self {
        Self {
            resolved_type: t,
            inferred: false,
        }
    }
    
    /// Create type info for an inferred type
    pub fn inferred(t: Type) -> Self {
        Self {
            resolved_type: t,
            inferred: true,
        }
    }
}

/// Convert from parser AST types to type checker types
impl From<&seen_parser::ast::Type> for Type {
    fn from(ast_type: &seen_parser::ast::Type) -> Self {
        let base_type = match ast_type.name.as_str() {
            "Int" => Type::Int,
            "UInt" => Type::UInt,
            "Float" => Type::Float,
            "Bool" => Type::Bool,
            "String" => Type::String,
            "Char" => Type::Char,
            "()" => Type::Unit,
            _ => {
                // Handle array types
                if ast_type.name.starts_with("Array<") && ast_type.name.ends_with('>') {
                    // Extract element type from Array<T>
                    let element_name = &ast_type.name[6..ast_type.name.len()-1];
                    let element_type = Type::from(&seen_parser::ast::Type::new(element_name));
                    Type::Array(Box::new(element_type))
                } else {
                    // User-defined type
                    Type::Struct { 
                        name: ast_type.name.clone(), 
                        fields: std::collections::HashMap::new(),
                        generics: Vec::new(),
                    }
                }
            },
        };
        
        // Apply nullable wrapper if needed
        if ast_type.is_nullable {
            Type::Nullable(Box::new(base_type))
        } else {
            base_type
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_compatibility() {
        assert!(Type::Int.is_assignable_to(&Type::Int));
        assert!(Type::Int.is_assignable_to(&Type::Float));
        assert!(!Type::Float.is_assignable_to(&Type::Int));
        assert!(Type::String.is_assignable_to(&Type::Nullable(Box::new(Type::String))));
    }

    #[test]
    fn test_type_names() {
        assert_eq!(Type::Int.name(), "Int");
        assert_eq!(Type::Array(Box::new(Type::String)).name(), "Array<String>");
        assert_eq!(Type::Nullable(Box::new(Type::Int)).name(), "Int?");
    }
}