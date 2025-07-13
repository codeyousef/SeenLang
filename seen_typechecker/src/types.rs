//! Type definitions for the Seen type system

use serde::{Deserialize, Serialize};

/// Represents a type in the Seen language
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    /// Primitive integer type
    Int,
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
    /// Optional/nullable type
    Optional(Box<Type>),
    /// Function type with parameter types and return type
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    /// User-defined struct type
    Struct(String),
    /// User-defined enum type
    Enum(String),
    /// Generic type parameter
    Generic(String),
    /// Unit/void type
    Unit,
    /// Unknown type (for inference)
    Unknown,
}

impl Type {
    /// Check if this type is primitive
    pub fn is_primitive(&self) -> bool {
        matches!(self, Type::Int | Type::Float | Type::Bool | Type::String | Type::Char)
    }

    /// Check if this type is numeric
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }

    /// Check if this type is optional/nullable
    pub fn is_optional(&self) -> bool {
        matches!(self, Type::Optional(_))
    }

    /// Get the underlying type if this is optional
    pub fn unwrap_optional(&self) -> Option<&Type> {
        match self {
            Type::Optional(inner) => Some(inner),
            _ => None,
        }
    }

    /// Make this type optional
    pub fn make_optional(self) -> Type {
        Type::Optional(Box::new(self))
    }

    /// Check if two types are compatible for assignment
    pub fn is_assignable_to(&self, other: &Type) -> bool {
        match (self, other) {
            // Exact match
            (a, b) if a == b => true,

            // Int can be assigned to Float
            (Type::Int, Type::Float) => true,

            // Any type can be assigned to its optional version
            (inner, Type::Optional(opt_inner)) => inner.is_assignable_to(opt_inner),

            // Array types must have compatible element types
            (Type::Array(a), Type::Array(b)) => a.is_assignable_to(b),

            // Function types must have compatible signatures
            (Type::Function { params: p1, return_type: r1 },
                Type::Function { params: p2, return_type: r2 }) => {
                p1.len() == p2.len() &&
                    p1.iter().zip(p2).all(|(a, b)| a.is_assignable_to(b)) &&
                    r1.is_assignable_to(r2)
            }

            // Unknown type is compatible with anything (for inference)
            (Type::Unknown, _) | (_, Type::Unknown) => true,

            _ => false,
        }
    }

    /// Get a human-readable name for this type
    pub fn name(&self) -> String {
        match self {
            Type::Int => "Int".to_string(),
            Type::Float => "Float".to_string(),
            Type::Bool => "Bool".to_string(),
            Type::String => "String".to_string(),
            Type::Char => "Char".to_string(),
            Type::Array(inner) => format!("Array<{}>", inner.name()),
            Type::Optional(inner) => format!("{}?", inner.name()),
            Type::Function { params, return_type } => {
                let param_names: Vec<String> = params.iter().map(|p| p.name()).collect();
                format!("({}) -> {}", param_names.join(", "), return_type.name())
            }
            Type::Struct(name) => name.clone(),
            Type::Enum(name) => name.clone(),
            Type::Generic(name) => name.clone(),
            Type::Unit => "()".to_string(),
            Type::Unknown => "?".to_string(),
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
        match ast_type {
            seen_parser::ast::Type::Simple(name) => {
                match name.as_str() {
                    "Int" => Type::Int,
                    "Float" => Type::Float,
                    "Bool" => Type::Bool,
                    "String" => Type::String,
                    "Char" => Type::Char,
                    "()" => Type::Unit,
                    _ => Type::Struct(name.clone()),
                }
            }
            seen_parser::ast::Type::Array(inner) => {
                Type::Array(Box::new(Type::from(inner.as_ref())))
            }
            seen_parser::ast::Type::Struct(name) => {
                Type::Struct(name.clone())
            }
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
        assert!(Type::String.is_assignable_to(&Type::Optional(Box::new(Type::String))));
    }

    #[test]
    fn test_type_names() {
        assert_eq!(Type::Int.name(), "Int");
        assert_eq!(Type::Array(Box::new(Type::String)).name(), "Array<String>");
        assert_eq!(Type::Optional(Box::new(Type::Int)).name(), "Int?");
    }
}