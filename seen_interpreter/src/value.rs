//! Value representation for the Seen interpreter

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Values that can be computed by the interpreter
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// Integer value
    Integer(i64),
    /// Floating-point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// String value
    String(String),
    /// Character value
    Character(char),
    /// Array value
    Array(Vec<Value>),
    /// Null value
    Null,
    /// Unit value (empty/void)
    Unit,
    /// Function value (closure)
    Function {
        name: String,
        parameters: Vec<String>,
        body: Box<seen_parser::ast::Block>,
        closure: HashMap<String, Value>,
    },
}

impl Value {
    /// Check if this value is truthy
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Boolean(b) => *b,
            Value::Null => false,
            Value::Unit => false,
            Value::Integer(0) => false,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(arr) => !arr.is_empty(),
            _ => true,
        }
    }

    /// Get the type name of this value
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Integer(_) => "Int",
            Value::Float(_) => "Float",
            Value::Boolean(_) => "Bool",
            Value::String(_) => "String",
            Value::Character(_) => "Char",
            Value::Array(_) => "Array",
            Value::Null => "Null",
            Value::Unit => "Unit",
            Value::Function { .. } => "Function",
        }
    }

    /// Convert this value to a string representation
    pub fn to_string(&self) -> String {
        match self {
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::String(s) => s.clone(),
            Value::Character(c) => c.to_string(),
            Value::Array(arr) => {
                let elements: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                format!("[{}]", elements.join(", "))
            }
            Value::Null => "null".to_string(),
            Value::Unit => "()".to_string(),
            Value::Function { name, .. } => format!("<function {}>", name),
        }
    }

    /// Try to convert this value to an integer
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            Value::Boolean(true) => Some(1),
            Value::Boolean(false) => Some(0),
            _ => None,
        }
    }

    /// Try to convert this value to a float
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Integer(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Boolean(true) => Some(1.0),
            Value::Boolean(false) => Some(0.0),
            _ => None,
        }
    }

    /// Try to convert this value to a boolean
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Check if two values are equal
    pub fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Character(a), Value::Character(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Unit, Value::Unit) => true,
            // Type coercion for numeric comparisons
            (Value::Integer(a), Value::Float(b)) => (*a as f64 - b).abs() < f64::EPSILON,
            (Value::Float(a), Value::Integer(b)) => (a - *b as f64).abs() < f64::EPSILON,
            _ => false,
        }
    }

    /// Perform arithmetic addition
    pub fn add(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a + *b as f64)),
            (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{}{}", a, b))),
            _ => Err(format!(
                "Cannot add {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform arithmetic subtraction
    pub fn subtract(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a - *b as f64)),
            _ => Err(format!(
                "Cannot subtract {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform arithmetic multiplication
    pub fn multiply(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a * *b as f64)),
            _ => Err(format!(
                "Cannot multiply {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform arithmetic division
    pub fn divide(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => {
                if *b == 0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(Value::Float(*a as f64 / *b as f64))
                }
            }
            (Value::Float(a), Value::Float(b)) => {
                if *b == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(Value::Float(a / b))
                }
            }
            (Value::Integer(a), Value::Float(b)) => {
                if *b == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(Value::Float(*a as f64 / b))
                }
            }
            (Value::Float(a), Value::Integer(b)) => {
                if *b == 0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(Value::Float(a / *b as f64))
                }
            }
            _ => Err(format!(
                "Cannot divide {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform comparison (less than)
    pub fn less_than(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Boolean(a < b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Boolean(a < b)),
            (Value::Integer(a), Value::Float(b)) => Ok(Value::Boolean((*a as f64) < *b)),
            (Value::Float(a), Value::Integer(b)) => Ok(Value::Boolean(*a < (*b as f64))),
            (Value::String(a), Value::String(b)) => Ok(Value::Boolean(a < b)),
            _ => Err(format!(
                "Cannot compare {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform logical negation
    pub fn negate(&self) -> Result<Value, String> {
        match self {
            Value::Integer(i) => Ok(Value::Integer(-i)),
            Value::Float(f) => Ok(Value::Float(-f)),
            _ => Err(format!("Cannot negate {}", self.type_name())),
        }
    }

    /// Perform logical NOT
    pub fn logical_not(&self) -> Value {
        Value::Boolean(!self.is_truthy())
    }

    /// Perform modulo operation
    pub fn modulo(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => {
                if *b == 0 {
                    Err("Modulo by zero".to_string())
                } else {
                    Ok(Value::Integer(a % b))
                }
            }
            (Value::Float(a), Value::Float(b)) => {
                if *b == 0.0 {
                    Err("Modulo by zero".to_string())
                } else {
                    Ok(Value::Float(a % b))
                }
            }
            (Value::Integer(a), Value::Float(b)) => {
                if *b == 0.0 {
                    Err("Modulo by zero".to_string())
                } else {
                    Ok(Value::Float((*a as f64) % b))
                }
            }
            (Value::Float(a), Value::Integer(b)) => {
                if *b == 0 {
                    Err("Modulo by zero".to_string())
                } else {
                    Ok(Value::Float(a % (*b as f64)))
                }
            }
            _ => Err(format!(
                "Cannot perform modulo on {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_arithmetic() {
        let a = Value::Integer(5);
        let b = Value::Integer(3);

        assert_eq!(a.add(&b).unwrap(), Value::Integer(8));
        assert_eq!(a.subtract(&b).unwrap(), Value::Integer(2));
        assert_eq!(a.multiply(&b).unwrap(), Value::Integer(15));
    }

    #[test]
    fn test_value_truthiness() {
        assert!(Value::Boolean(true).is_truthy());
        assert!(!Value::Boolean(false).is_truthy());
        assert!(!Value::Null.is_truthy());
        assert!(Value::Integer(1).is_truthy());
        assert!(!Value::Integer(0).is_truthy());
    }

    #[test]
    fn test_value_equality() {
        assert!(Value::Integer(5).equals(&Value::Integer(5)));
        assert!(Value::Integer(5).equals(&Value::Float(5.0)));
        assert!(!Value::Integer(5).equals(&Value::Integer(3)));
    }
}
