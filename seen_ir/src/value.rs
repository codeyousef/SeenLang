//! IR value and type system for the Seen programming language

use std::fmt;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Types in the IR system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IRType {
    Void,
    Integer,
    Float,
    Boolean,
    Char,
    String,
    Array(Box<IRType>),
    Function {
        parameters: Vec<IRType>,
        return_type: Box<IRType>,
    },
    Struct {
        name: String,
        fields: Vec<(String, IRType)>, // Changed from HashMap to Vec for Hash compatibility
    },
    Enum {
        name: String,
        variants: Vec<(String, Option<Vec<IRType>>)>, // (variant_name, optional tuple fields)
    },
    Pointer(Box<IRType>),
    Reference(Box<IRType>),
    Optional(Box<IRType>),
    Generic(String), // For generic types like T
}

impl IRType {
    /// Check if this type is compatible with another type for assignment
    pub fn is_assignable_from(&self, other: &IRType) -> bool {
        match (self, other) {
            (IRType::Void, IRType::Void) => true,
            (IRType::Integer, IRType::Integer) => true,
            (IRType::Float, IRType::Float) => true,
            (IRType::Float, IRType::Integer) => true, // Allow integer to float conversion
            (IRType::Boolean, IRType::Boolean) => true,
            (IRType::Char, IRType::Char) => true,
            (IRType::String, IRType::String) => true,
            
            (IRType::Array(a), IRType::Array(b)) => a.is_assignable_from(b),
            (IRType::Pointer(a), IRType::Pointer(b)) => a.is_assignable_from(b),
            (IRType::Reference(a), IRType::Reference(b)) => a.is_assignable_from(b),
            (IRType::Optional(a), IRType::Optional(b)) => a.is_assignable_from(b),
            (IRType::Optional(a), b) => a.is_assignable_from(b), // Allow T to T?
            
            (IRType::Function { parameters: p1, return_type: r1 }, 
             IRType::Function { parameters: p2, return_type: r2 }) => {
                p1.len() == p2.len() && 
                r1.is_assignable_from(r2) &&
                p1.iter().zip(p2.iter()).all(|(a, b)| a.is_assignable_from(b))
            },
            
            (IRType::Struct { name: n1, .. }, IRType::Struct { name: n2, .. }) => n1 == n2,
            (IRType::Enum { name: n1, .. }, IRType::Enum { name: n2, .. }) => n1 == n2,
            
            _ => false,
        }
    }
    
    /// Get the size in bytes of this type (for code generation)
    pub fn size_bytes(&self) -> usize {
        match self {
            IRType::Void => 0,
            IRType::Integer => 8, // 64-bit integers
            IRType::Float => 8,   // 64-bit floats
            IRType::Boolean => 1,
            IRType::Char => 1,    // Single byte character
            IRType::String => 8,  // Pointer to string data
            IRType::Array(_) => 8, // Pointer to array data
            IRType::Function { .. } => 8, // Function pointer
            IRType::Struct { fields, .. } => {
                // Calculate actual struct size from field types
                fields.iter().map(|(_, field_type)| field_type.size_bytes()).sum()
            }
            IRType::Enum { variants, .. } => {
                // Calculate enum size as tag + largest variant
                let tag_size = 8; // Discriminant tag
                let largest_variant = variants.iter()
                    .map(|(_, fields)| {
                        fields.as_ref()
                            .map(|f| f.iter().map(|t| t.size_bytes()).sum())
                            .unwrap_or(0)
                    })
                    .max()
                    .unwrap_or(0);
                tag_size + largest_variant
            }
            IRType::Pointer(_) => 8,
            IRType::Reference(_) => 8,
            IRType::Optional(_) => 9, // 8 bytes for value + 1 byte for null flag
            IRType::Generic(_) => 8, // Default size for generic types
        }
    }
    
    /// Check if this type is a numeric type
    pub fn is_numeric(&self) -> bool {
        matches!(self, IRType::Integer | IRType::Float)
    }
    
    /// Check if this type is a pointer-like type
    pub fn is_pointer(&self) -> bool {
        matches!(self, IRType::Pointer(_) | IRType::Reference(_) | IRType::String | IRType::Array(_))
    }
}

impl fmt::Display for IRType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IRType::Void => write!(f, "void"),
            IRType::Integer => write!(f, "i64"),
            IRType::Float => write!(f, "f64"),
            IRType::Boolean => write!(f, "bool"),
            IRType::Char => write!(f, "char"),
            IRType::String => write!(f, "string"),
            IRType::Array(inner) => write!(f, "[{}]", inner),
            IRType::Function { parameters, return_type } => {
                write!(f, "(")?;
                for (i, param) in parameters.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", return_type)
            },
            IRType::Struct { name, .. } => write!(f, "struct {}", name),
            IRType::Enum { name, .. } => write!(f, "enum {}", name),
            IRType::Pointer(inner) => write!(f, "*{}", inner),
            IRType::Reference(inner) => write!(f, "&{}", inner),
            IRType::Optional(inner) => write!(f, "{}?", inner),
            IRType::Generic(name) => write!(f, "{}", name),
        }
    }
}

/// Values in the IR system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IRValue {
    Void,
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Char(char),
    String(String),
    StringConstant(usize), // Index into string table
    Array(Vec<IRValue>),
    Struct {
        type_name: String,
        fields: HashMap<String, IRValue>,
    },
    Function {
        name: String,
        parameters: Vec<String>,
    },
    Variable(String),
    Register(u32), // Virtual register for SSA form
    GlobalVariable(String),
    Label(String),
    AddressOf(Box<IRValue>), // Address-of operator for references
    Null,
    Undefined, // For uninitialized values
}

impl IRValue {
    /// Get the type of this value
    pub fn get_type(&self) -> IRType {
        match self {
            IRValue::Void => IRType::Void,
            IRValue::Integer(_) => IRType::Integer,
            IRValue::Float(_) => IRType::Float,
            IRValue::Boolean(_) => IRType::Boolean,
            IRValue::Char(_) => IRType::Char,
            IRValue::String(_) | IRValue::StringConstant(_) => IRType::String,
            IRValue::Array(values) => {
                if let Some(first) = values.first() {
                    IRType::Array(Box::new(first.get_type()))
                } else {
                    IRType::Array(Box::new(IRType::Void))
                }
            },
            IRValue::Struct { type_name, fields } => {
                // Reconstruct struct type from the actual field values
                let field_types: Vec<(String, IRType)> = fields.iter()
                    .map(|(name, value)| (name.clone(), value.get_type()))
                    .collect();
                
                IRType::Struct {
                    name: type_name.clone(),
                    fields: field_types,
                }
            },
            IRValue::Function { name, parameters } => {
                // For function values, we need to look up the actual function signature
                // This requires access to the function registry/symbol table
                // Generate a concrete function type based on parameter count
                let param_types: Vec<IRType> = (0..parameters.len())
                    .map(|i| IRType::Generic(format!("T{}", i)))
                    .collect();
                
                IRType::Function {
                    parameters: param_types,
                    return_type: Box::new(IRType::Generic("R".to_string())),
                }
            },
            IRValue::Variable(_) | IRValue::Register(_) | IRValue::GlobalVariable(_) => {
                // Variables and registers need type context from the IR generation context
                // Return unresolved generic type that will be resolved during type analysis
                IRType::Generic("Unresolved".to_string())
            },
            IRValue::Label(_) => IRType::Void,
            IRValue::AddressOf(value) => {
                IRType::Pointer(Box::new(value.get_type()))
            },
            IRValue::Null => IRType::Optional(Box::new(IRType::Void)),
            IRValue::Undefined => IRType::Void,
        }
    }
    
    /// Check if this value is a constant
    pub fn is_constant(&self) -> bool {
        matches!(self, 
            IRValue::Integer(_) | 
            IRValue::Float(_) | 
            IRValue::Boolean(_) | 
            IRValue::Char(_) |
            IRValue::String(_) | 
            IRValue::StringConstant(_) |
            IRValue::Null
        )
    }
    
    /// Check if this value is a variable reference
    pub fn is_variable(&self) -> bool {
        matches!(self, 
            IRValue::Variable(_) | 
            IRValue::Register(_) | 
            IRValue::GlobalVariable(_)
        )
    }
    
    /// Get the variable name if this is a variable reference
    pub fn as_variable_name(&self) -> Option<&str> {
        match self {
            IRValue::Variable(name) | IRValue::GlobalVariable(name) => Some(name),
            _ => None,
        }
    }
    
    /// Convert to a string representation for code generation
    pub fn to_code_string(&self) -> String {
        match self {
            IRValue::Void => "void".to_string(),
            IRValue::Integer(i) => i.to_string(),
            IRValue::Float(f) => f.to_string(),
            IRValue::Boolean(b) => if *b { "true".to_string() } else { "false".to_string() },
            IRValue::Char(c) => format!("'{}'", c.escape_default()),
            IRValue::String(s) => format!("\"{}\"", s.escape_default()),
            IRValue::StringConstant(index) => format!("@str.{}", index),
            IRValue::Array(elements) => {
                if elements.is_empty() {
                    "[]".to_string()
                } else {
                    format!("[{}]", elements.iter()
                        .map(|e| e.to_code_string())
                        .collect::<Vec<_>>()
                        .join(", "))
                }
            }
            IRValue::Struct { type_name, .. } => format!("struct {}", type_name),
            IRValue::Function { name, .. } => format!("@{}", name),
            IRValue::Variable(name) => format!("%{}", name),
            IRValue::Register(reg) => format!("%r{}", reg),
            IRValue::GlobalVariable(name) => format!("@{}", name),
            IRValue::Label(name) => format!(".{}", name),
            IRValue::AddressOf(value) => format!("&{}", value.to_code_string()),
            IRValue::Null => "null".to_string(),
            IRValue::Undefined => "undef".to_string(),
        }
    }
}

impl fmt::Display for IRValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_code_string())
    }
}

/// A typed IR value that combines a value with its type information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedIRValue {
    pub value: IRValue,
    pub ir_type: IRType,
}

impl TypedIRValue {
    pub fn new(value: IRValue, ir_type: IRType) -> Self {
        Self { value, ir_type }
    }
    
    /// Create a typed value with automatic type inference
    pub fn infer_type(value: IRValue) -> Self {
        let ir_type = value.get_type();
        Self { value, ir_type }
    }
    
    /// Check if this value is compatible with the given type
    pub fn is_compatible_with(&self, other_type: &IRType) -> bool {
        other_type.is_assignable_from(&self.ir_type)
    }
}

impl fmt::Display for TypedIRValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} : {}", self.value, self.ir_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ir_type_assignability() {
        assert!(IRType::Integer.is_assignable_from(&IRType::Integer));
        assert!(IRType::Float.is_assignable_from(&IRType::Integer));
        assert!(!IRType::Integer.is_assignable_from(&IRType::Float));
        assert!(!IRType::Boolean.is_assignable_from(&IRType::Integer));
        
        let optional_int = IRType::Optional(Box::new(IRType::Integer));
        assert!(optional_int.is_assignable_from(&IRType::Integer));
        assert!(optional_int.is_assignable_from(&optional_int));
    }
    
    #[test]
    fn test_ir_value_types() {
        assert_eq!(IRValue::Integer(42).get_type(), IRType::Integer);
        assert_eq!(IRValue::Float(3.14).get_type(), IRType::Float);
        assert_eq!(IRValue::Boolean(true).get_type(), IRType::Boolean);
        assert_eq!(IRValue::String("hello".to_string()).get_type(), IRType::String);
    }
    
    #[test]
    fn test_value_properties() {
        assert!(IRValue::Integer(42).is_constant());
        assert!(!IRValue::Variable("x".to_string()).is_constant());
        assert!(IRValue::Variable("x".to_string()).is_variable());
        assert!(!IRValue::Integer(42).is_variable());
    }
    
    #[test]
    fn test_typed_value() {
        let value = TypedIRValue::infer_type(IRValue::Integer(42));
        assert_eq!(value.ir_type, IRType::Integer);
        assert!(value.is_compatible_with(&IRType::Integer));
        assert!(value.is_compatible_with(&IRType::Float));
        assert!(!value.is_compatible_with(&IRType::String));
    }
}