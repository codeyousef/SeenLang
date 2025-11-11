//! Value representation for the Seen interpreter

use seen_concurrency::types::{ActorRef, Channel, Promise, TaskId};
use seen_effects::{EffectDefinition, EffectId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Values that can be computed by the interpreter
#[derive(Debug, Clone)]
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
    /// Array value (shared, mutable)
    Array(Arc<Mutex<Vec<Value>>>),
    /// Raw byte buffer
    Bytes(Vec<u8>),
    /// Null value
    Null,
    /// Unit value (empty/void)
    Unit,
    /// Struct value
    Struct {
        name: String,
        fields: Arc<Mutex<HashMap<String, Value>>>,
    },
    /// Class/type reference
    Class { name: String },
    /// Function value (closure)
    Function {
        name: String,
        parameters: Vec<String>,
        body: Box<seen_parser::Expression>, // Store function body as expression
        closure: HashMap<String, Value>,
    },
    /// Promise value for async operations
    Promise(Arc<Promise>),
    /// Task ID for spawned tasks
    Task(TaskId),
    /// Channel for message passing
    Channel(Channel),
    /// Actor reference for actor model concurrency
    Actor(ActorRef),
    /// Effect definition
    Effect(Arc<EffectDefinition>),
    /// Effect handle context
    EffectHandle {
        effect_id: EffectId,
        handlers: HashMap<String, Value>,
    },
    /// Observable for reactive streams
    Observable(Arc<dyn std::any::Any + Send + Sync>),
    /// Flow for reactive coroutines
    Flow(Arc<dyn std::any::Any + Send + Sync>),
    /// Reactive property
    ReactiveProperty {
        property_id: seen_reactive::properties::PropertyId,
        name: String,
    },
}

impl Value {
    /// Construct an array value backed by shared mutable storage
    pub fn array_from_vec(values: Vec<Value>) -> Self {
        Value::Array(Arc::new(Mutex::new(values)))
    }

    /// Construct a struct value backed by shared mutable storage
    pub fn struct_from_fields(name: String, fields: HashMap<String, Value>) -> Self {
        Value::Struct {
            name,
            fields: Arc::new(Mutex::new(fields)),
        }
    }

    /// Check if this value is truthy
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Boolean(b) => *b,
            Value::Null => false,
            Value::Unit => false,
            Value::Integer(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(arr) => arr.lock().map(|values| !values.is_empty()).unwrap_or(false),
            Value::Bytes(bytes) => !bytes.is_empty(),
            Value::Struct { .. } => true,
            Value::Class { .. } => true,
            Value::Function { .. } => true,
            Value::Promise(promise) => !promise.is_rejected(),
            Value::Task(_) => true,
            Value::Channel(_) => true,
            Value::Actor(_) => true,
            Value::Character(_) => true,
            Value::Effect(_) => true,
            Value::EffectHandle { .. } => true,
            Value::Observable(_) => true,
            Value::Flow(_) => true,
            Value::ReactiveProperty { .. } => true,
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
            Value::Bytes(_) => "Bytes",
            Value::Struct { .. } => "Struct",
            Value::Class { .. } => "Class",
            Value::Null => "Null",
            Value::Unit => "Unit",
            Value::Function { .. } => "Function",
            Value::Promise(_) => "Promise",
            Value::Task(_) => "Task",
            Value::Channel(_) => "Channel",
            Value::Actor(_) => "Actor",
            Value::Effect(_) => "Effect",
            Value::EffectHandle { .. } => "EffectHandle",
            Value::Observable(_) => "Observable",
            Value::Flow(_) => "Flow",
            Value::ReactiveProperty { .. } => "ReactiveProperty",
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
            Value::Array(arr) => match arr.lock() {
                Ok(values) => {
                    let elements: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                    format!("[{}]", elements.join(", "))
                }
                Err(_) => "[<locked>]".to_string(),
            },
            Value::Bytes(bytes) => format!("<bytes {}>", bytes.len()),
            Value::Struct { name, fields } => match fields.lock() {
                Ok(map) => {
                    let field_strs: Vec<String> = map
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, v.to_string()))
                        .collect();
                    format!("{}({})", name, field_strs.join(", "))
                }
                Err(_) => format!("{}(<locked>)", name),
            },
            Value::Class { name } => format!("<class {}>", name),
            Value::Null => "null".to_string(),
            Value::Unit => "()".to_string(),
            Value::Function { name, .. } => format!("<function {}>", name),
            Value::Promise(promise) => {
                if promise.is_pending() {
                    "<Promise pending>".to_string()
                } else if promise.is_resolved() {
                    format!("<Promise resolved>")
                } else {
                    "<Promise rejected>".to_string()
                }
            }
            Value::Task(task_id) => format!("<Task {}>", task_id.id()),
            Value::Channel(channel) => format!("<Channel {}>", channel.id().id()),
            Value::Actor(actor_ref) => format!("<Actor {}>", actor_ref.id().id()),
            Value::Effect(effect) => format!("<Effect {}>", effect.name),
            Value::EffectHandle { effect_id, .. } => format!("<EffectHandle {}>", effect_id.id()),
            Value::Observable(_) => "<Observable>".to_string(),
            Value::Flow(_) => "<Flow>".to_string(),
            Value::ReactiveProperty { name, .. } => format!("<ReactiveProperty {}>", name),
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
            (Value::Array(a), Value::Array(b)) => {
                let lhs = a.lock();
                let rhs = b.lock();
                if let (Ok(lhs_vals), Ok(rhs_vals)) = (lhs, rhs) {
                    lhs_vals.len() == rhs_vals.len()
                        && lhs_vals
                            .iter()
                            .zip(rhs_vals.iter())
                            .all(|(v1, v2)| v1.equals(v2))
                } else {
                    false
                }
            }
            (
                Value::Struct {
                    name: n1,
                    fields: f1,
                },
                Value::Struct {
                    name: n2,
                    fields: f2,
                },
            ) => {
                if n1 != n2 {
                    return false;
                }
                let lhs = f1.lock();
                let rhs = f2.lock();
                if let (Ok(lhs_fields), Ok(rhs_fields)) = (lhs, rhs) {
                    lhs_fields.len() == rhs_fields.len()
                        && lhs_fields
                            .iter()
                            .all(|(k, v)| rhs_fields.get(k).map_or(false, |v2| v.equals(v2)))
                } else {
                    false
                }
            }
            (Value::Null, Value::Null) => true,
            (Value::Unit, Value::Unit) => true,
            (Value::Class { name: a }, Value::Class { name: b }) => a == b,
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

    /// Perform bitwise AND
    pub fn bitwise_and(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a & b)),
            _ => Err(format!(
                "Cannot apply & to {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform bitwise OR
    pub fn bitwise_or(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a | b)),
            _ => Err(format!(
                "Cannot apply | to {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform bitwise XOR
    pub fn bitwise_xor(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a ^ b)),
            _ => Err(format!(
                "Cannot apply ^ to {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform left shift
    pub fn left_shift(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(value), Value::Integer(shift)) => {
                if *shift < 0 {
                    Err("Shift amount must be non-negative".to_string())
                } else if *shift >= 64 {
                    Err("Shift amount must be less than 64".to_string())
                } else {
                    Ok(Value::Integer(value << (*shift as u32)))
                }
            }
            _ => Err(format!(
                "Cannot apply << to {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform right shift (arithmetic)
    pub fn right_shift(&self, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Integer(value), Value::Integer(shift)) => {
                if *shift < 0 {
                    Err("Shift amount must be non-negative".to_string())
                } else if *shift >= 64 {
                    Err("Shift amount must be less than 64".to_string())
                } else {
                    Ok(Value::Integer(value >> (*shift as u32)))
                }
            }
            _ => Err(format!(
                "Cannot apply >> to {} and {}",
                self.type_name(),
                other.type_name()
            )),
        }
    }

    /// Perform bitwise NOT
    pub fn bitwise_not(&self) -> Result<Value, String> {
        match self {
            Value::Integer(value) => Ok(Value::Integer(!value)),
            _ => Err(format!("Cannot apply ~ to {}", self.type_name())),
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

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}
