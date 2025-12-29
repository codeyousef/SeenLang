//! Interpreter values with rich type information
//!
//! This module provides a value representation that maintains full type
//! information at runtime, enabling type checking and rich error messages.

use crate::value::{IRType, IRValue};
use super::memory::Address;
use std::fmt;
use std::collections::HashMap;

/// Runtime value representation for the interpreter
#[derive(Debug, Clone)]
pub enum InterpreterValue {
    /// Void/unit value
    Void,
    
    /// 64-bit signed integer
    Integer(i64),
    
    /// 64-bit floating point
    Float(f64),
    
    /// Boolean value
    Boolean(bool),
    
    /// Single character
    Char(char),
    
    /// String value (stored as owned String)
    String(String),
    
    /// Pointer to heap memory
    Pointer {
        address: Address,
        pointee_type: Box<ValueType>,
    },
    
    /// Array with element type and data
    Array {
        element_type: Box<ValueType>,
        elements: Vec<InterpreterValue>,
    },
    
    /// Struct value with named fields
    Struct {
        type_name: String,
        fields: HashMap<String, InterpreterValue>,
    },
    
    /// Enum variant
    Enum {
        type_name: String,
        variant_name: String,
        tag: u64,
        payload: Option<Vec<InterpreterValue>>,
    },
    
    /// Function reference
    Function {
        name: String,
        parameter_types: Vec<ValueType>,
        return_type: Box<ValueType>,
    },
    
    /// Optional value (Some or None)
    Optional {
        inner_type: Box<ValueType>,
        value: Option<Box<InterpreterValue>>,
    },
    
    /// Null pointer
    Null,
    
    /// Undefined (uninitialized)
    Undefined,
}

/// Type information for interpreter values
#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    Void,
    Integer,
    Float,
    Boolean,
    Char,
    String,
    Pointer(Box<ValueType>),
    Array(Box<ValueType>),
    Struct { name: String, fields: Vec<(String, ValueType)> },
    Enum { name: String, variants: Vec<(String, Option<Vec<ValueType>>)> },
    Function { params: Vec<ValueType>, return_type: Box<ValueType> },
    Optional(Box<ValueType>),
    Generic(String),
}

impl InterpreterValue {
    /// Get the type of this value
    pub fn get_type(&self) -> ValueType {
        match self {
            InterpreterValue::Void => ValueType::Void,
            InterpreterValue::Integer(_) => ValueType::Integer,
            InterpreterValue::Float(_) => ValueType::Float,
            InterpreterValue::Boolean(_) => ValueType::Boolean,
            InterpreterValue::Char(_) => ValueType::Char,
            InterpreterValue::String(_) => ValueType::String,
            InterpreterValue::Pointer { pointee_type, .. } => {
                ValueType::Pointer(pointee_type.clone())
            }
            InterpreterValue::Array { element_type, .. } => {
                ValueType::Array(element_type.clone())
            }
            InterpreterValue::Struct { type_name, fields } => {
                let field_types: Vec<(String, ValueType)> = fields
                    .iter()
                    .map(|(name, val)| (name.clone(), val.get_type()))
                    .collect();
                ValueType::Struct { name: type_name.clone(), fields: field_types }
            }
            InterpreterValue::Enum { type_name, .. } => {
                // For simplicity, return a basic enum type
                ValueType::Enum { name: type_name.clone(), variants: vec![] }
            }
            InterpreterValue::Function { parameter_types, return_type, .. } => {
                ValueType::Function {
                    params: parameter_types.clone(),
                    return_type: return_type.clone(),
                }
            }
            InterpreterValue::Optional { inner_type, .. } => {
                ValueType::Optional(inner_type.clone())
            }
            InterpreterValue::Null => ValueType::Pointer(Box::new(ValueType::Void)),
            InterpreterValue::Undefined => ValueType::Void,
        }
    }

    /// Check if this value is truthy for boolean contexts
    pub fn is_truthy(&self) -> bool {
        match self {
            InterpreterValue::Boolean(b) => *b,
            InterpreterValue::Integer(i) => *i != 0,
            InterpreterValue::Float(f) => *f != 0.0,
            InterpreterValue::Null => false,
            InterpreterValue::Void => false,
            InterpreterValue::Optional { value, .. } => value.is_some(),
            InterpreterValue::String(s) => !s.is_empty(),
            InterpreterValue::Array { elements, .. } => !elements.is_empty(),
            _ => true,
        }
    }

    /// Try to convert to i64
    pub fn as_integer(&self) -> Result<i64, String> {
        match self {
            InterpreterValue::Integer(i) => Ok(*i),
            InterpreterValue::Float(f) => Ok(*f as i64),
            InterpreterValue::Boolean(b) => Ok(if *b { 1 } else { 0 }),
            InterpreterValue::Char(c) => Ok(*c as i64),
            _ => Err(format!("Cannot convert {:?} to integer", self.get_type())),
        }
    }

    /// Try to convert to f64
    pub fn as_float(&self) -> Result<f64, String> {
        match self {
            InterpreterValue::Float(f) => Ok(*f),
            InterpreterValue::Integer(i) => Ok(*i as f64),
            _ => Err(format!("Cannot convert {:?} to float", self.get_type())),
        }
    }

    /// Try to convert to bool
    pub fn as_boolean(&self) -> Result<bool, String> {
        match self {
            InterpreterValue::Boolean(b) => Ok(*b),
            _ => Ok(self.is_truthy()),
        }
    }

    /// Try to get as string
    pub fn as_string(&self) -> Result<&str, String> {
        match self {
            InterpreterValue::String(s) => Ok(s),
            _ => Err(format!("Cannot get {:?} as string", self.get_type())),
        }
    }

    /// Try to get pointer address
    pub fn as_pointer(&self) -> Result<Address, String> {
        match self {
            InterpreterValue::Pointer { address, .. } => Ok(*address),
            InterpreterValue::Null => Ok(Address::NULL),
            _ => Err(format!("Cannot get {:?} as pointer", self.get_type())),
        }
    }

    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, InterpreterValue::Null)
    }

    /// Create a new integer value
    pub fn integer(value: i64) -> Self {
        InterpreterValue::Integer(value)
    }

    /// Create a new float value  
    pub fn float(value: f64) -> Self {
        InterpreterValue::Float(value)
    }

    /// Create a new boolean value
    pub fn boolean(value: bool) -> Self {
        InterpreterValue::Boolean(value)
    }

    /// Create a new string value
    pub fn string(value: impl Into<String>) -> Self {
        InterpreterValue::String(value.into())
    }

    /// Create a new pointer value
    pub fn pointer(address: Address, pointee_type: ValueType) -> Self {
        InterpreterValue::Pointer {
            address,
            pointee_type: Box::new(pointee_type),
        }
    }

    /// Create a null pointer
    pub fn null() -> Self {
        InterpreterValue::Null
    }

    /// Create an array value
    pub fn array(element_type: ValueType, elements: Vec<InterpreterValue>) -> Self {
        InterpreterValue::Array {
            element_type: Box::new(element_type),
            elements,
        }
    }

    /// Create a struct value
    pub fn struct_value(type_name: impl Into<String>, fields: HashMap<String, InterpreterValue>) -> Self {
        InterpreterValue::Struct {
            type_name: type_name.into(),
            fields,
        }
    }

    /// Convert from IR value (compile-time) to interpreter value (runtime)
    pub fn from_ir_value(ir_value: &IRValue) -> Self {
        match ir_value {
            IRValue::Void => InterpreterValue::Void,
            IRValue::Integer(i) => InterpreterValue::Integer(*i),
            IRValue::Float(f) => InterpreterValue::Float(*f),
            IRValue::Boolean(b) => InterpreterValue::Boolean(*b),
            IRValue::Char(c) => InterpreterValue::Char(*c),
            IRValue::String(s) => InterpreterValue::String(s.clone()),
            IRValue::Null => InterpreterValue::Null,
            IRValue::Undefined => InterpreterValue::Undefined,
            IRValue::Array(elements) => {
                let converted: Vec<InterpreterValue> = elements
                    .iter()
                    .map(|e| InterpreterValue::from_ir_value(e))
                    .collect();
                let elem_type = if converted.is_empty() {
                    ValueType::Void
                } else {
                    converted[0].get_type()
                };
                InterpreterValue::Array {
                    element_type: Box::new(elem_type),
                    elements: converted,
                }
            }
            IRValue::Struct { type_name, fields } => {
                let converted: HashMap<String, InterpreterValue> = fields
                    .iter()
                    .map(|(k, v)| (k.clone(), InterpreterValue::from_ir_value(v)))
                    .collect();
                InterpreterValue::Struct {
                    type_name: type_name.clone(),
                    fields: converted,
                }
            }
            IRValue::Function { name, parameters } => {
                InterpreterValue::Function {
                    name: name.clone(),
                    parameter_types: vec![ValueType::Generic("T".to_string()); parameters.len()],
                    return_type: Box::new(ValueType::Void),
                }
            }
            // Variables and registers need to be resolved by the executor
            IRValue::Variable(_) | IRValue::Register(_) | IRValue::GlobalVariable(_) => {
                InterpreterValue::Undefined
            }
            IRValue::Label(_) => InterpreterValue::Void,
            IRValue::AddressOf(_) => InterpreterValue::Null, // Needs runtime resolution
            IRValue::StringConstant(_) => InterpreterValue::Undefined, // Needs string table
            IRValue::ByteArray(bytes) => {
                let elements: Vec<InterpreterValue> = bytes
                    .iter()
                    .map(|b| InterpreterValue::Integer(*b as i64))
                    .collect();
                InterpreterValue::Array {
                    element_type: Box::new(ValueType::Integer),
                    elements,
                }
            }
        }
    }
}

impl ValueType {
    /// Convert from IR type to value type
    pub fn from_ir_type(ir_type: &IRType) -> Self {
        match ir_type {
            IRType::Void => ValueType::Void,
            IRType::Integer => ValueType::Integer,
            IRType::Float => ValueType::Float,
            IRType::Boolean => ValueType::Boolean,
            IRType::Char => ValueType::Char,
            IRType::String => ValueType::String,
            IRType::Pointer(inner) => ValueType::Pointer(Box::new(ValueType::from_ir_type(inner))),
            IRType::Reference(inner) => ValueType::Pointer(Box::new(ValueType::from_ir_type(inner))),
            IRType::Array(inner) => ValueType::Array(Box::new(ValueType::from_ir_type(inner))),
            IRType::Optional(inner) => ValueType::Optional(Box::new(ValueType::from_ir_type(inner))),
            IRType::Function { parameters, return_type } => {
                let params: Vec<ValueType> = parameters
                    .iter()
                    .map(|p| ValueType::from_ir_type(p))
                    .collect();
                ValueType::Function {
                    params,
                    return_type: Box::new(ValueType::from_ir_type(return_type)),
                }
            }
            IRType::Struct { name, fields } => {
                let field_types: Vec<(String, ValueType)> = fields
                    .iter()
                    .map(|(n, t)| (n.clone(), ValueType::from_ir_type(t)))
                    .collect();
                ValueType::Struct { name: name.clone(), fields: field_types }
            }
            IRType::Enum { name, variants } => {
                let variant_types: Vec<(String, Option<Vec<ValueType>>)> = variants
                    .iter()
                    .map(|(n, fields)| {
                        let field_types = fields.as_ref().map(|f| {
                            f.iter().map(|t| ValueType::from_ir_type(t)).collect()
                        });
                        (n.clone(), field_types)
                    })
                    .collect();
                ValueType::Enum { name: name.clone(), variants: variant_types }
            }
            IRType::Generic(name) => ValueType::Generic(name.clone()),
            IRType::Vector { lanes, lane_type } => {
                // Represent as array for now
                ValueType::Array(Box::new(ValueType::from_ir_type(lane_type)))
            }
        }
    }

    /// Check type compatibility
    pub fn is_compatible_with(&self, other: &ValueType) -> bool {
        match (self, other) {
            (ValueType::Void, ValueType::Void) => true,
            (ValueType::Integer, ValueType::Integer) => true,
            (ValueType::Float, ValueType::Float) => true,
            (ValueType::Float, ValueType::Integer) => true, // Allow promotion
            (ValueType::Boolean, ValueType::Boolean) => true,
            (ValueType::Char, ValueType::Char) => true,
            (ValueType::String, ValueType::String) => true,
            (ValueType::Pointer(a), ValueType::Pointer(b)) => a.is_compatible_with(b),
            (ValueType::Array(a), ValueType::Array(b)) => a.is_compatible_with(b),
            (ValueType::Optional(a), ValueType::Optional(b)) => a.is_compatible_with(b),
            (ValueType::Optional(a), b) => a.is_compatible_with(b), // Allow T to T?
            (ValueType::Struct { name: n1, .. }, ValueType::Struct { name: n2, .. }) => n1 == n2,
            (ValueType::Enum { name: n1, .. }, ValueType::Enum { name: n2, .. }) => n1 == n2,
            (ValueType::Generic(_), _) => true, // Generic matches anything
            (_, ValueType::Generic(_)) => true,
            _ => false,
        }
    }
}

impl fmt::Display for InterpreterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterpreterValue::Void => write!(f, "void"),
            InterpreterValue::Integer(i) => write!(f, "{}", i),
            InterpreterValue::Float(fl) => write!(f, "{}", fl),
            InterpreterValue::Boolean(b) => write!(f, "{}", b),
            InterpreterValue::Char(c) => write!(f, "'{}'", c),
            InterpreterValue::String(s) => write!(f, "\"{}\"", s),
            InterpreterValue::Pointer { address, .. } => write!(f, "ptr({})", address),
            InterpreterValue::Array { elements, .. } => {
                write!(f, "[")?;
                for (i, e) in elements.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", e)?;
                }
                write!(f, "]")
            }
            InterpreterValue::Struct { type_name, fields } => {
                write!(f, "{} {{ ", type_name)?;
                for (i, (name, val)) in fields.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", name, val)?;
                }
                write!(f, " }}")
            }
            InterpreterValue::Enum { type_name, variant_name, .. } => {
                write!(f, "{}::{}", type_name, variant_name)
            }
            InterpreterValue::Function { name, .. } => write!(f, "fn {}", name),
            InterpreterValue::Optional { value, .. } => {
                match value {
                    Some(v) => write!(f, "Some({})", v),
                    None => write!(f, "None"),
                }
            }
            InterpreterValue::Null => write!(f, "null"),
            InterpreterValue::Undefined => write!(f, "undefined"),
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::Void => write!(f, "void"),
            ValueType::Integer => write!(f, "i64"),
            ValueType::Float => write!(f, "f64"),
            ValueType::Boolean => write!(f, "bool"),
            ValueType::Char => write!(f, "char"),
            ValueType::String => write!(f, "String"),
            ValueType::Pointer(inner) => write!(f, "*{}", inner),
            ValueType::Array(inner) => write!(f, "[{}]", inner),
            ValueType::Struct { name, .. } => write!(f, "struct {}", name),
            ValueType::Enum { name, .. } => write!(f, "enum {}", name),
            ValueType::Function { params, return_type } => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", return_type)
            }
            ValueType::Optional(inner) => write!(f, "{}?", inner),
            ValueType::Generic(name) => write!(f, "{}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integer_value() {
        let val = InterpreterValue::integer(42);
        assert_eq!(val.get_type(), ValueType::Integer);
        assert_eq!(val.as_integer().unwrap(), 42);
        assert!(val.is_truthy());
    }

    #[test]
    fn test_float_value() {
        let val = InterpreterValue::float(3.14);
        assert_eq!(val.get_type(), ValueType::Float);
        assert_eq!(val.as_float().unwrap(), 3.14);
    }

    #[test]
    fn test_boolean_value() {
        let val = InterpreterValue::boolean(true);
        assert_eq!(val.get_type(), ValueType::Boolean);
        assert!(val.is_truthy());

        let val2 = InterpreterValue::boolean(false);
        assert!(!val2.is_truthy());
    }

    #[test]
    fn test_type_compatibility() {
        assert!(ValueType::Integer.is_compatible_with(&ValueType::Integer));
        assert!(ValueType::Float.is_compatible_with(&ValueType::Integer));
        assert!(!ValueType::Integer.is_compatible_with(&ValueType::String));
    }
}
