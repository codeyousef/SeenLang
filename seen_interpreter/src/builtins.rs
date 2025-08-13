//! Built-in functions for the Seen interpreter

use crate::value::Value;
use crate::errors::{InterpreterError, InterpreterResult};
use seen_parser::Position;
use std::collections::HashMap;

/// Type signature for built-in functions
pub type BuiltinFunction = fn(&[Value], Position) -> InterpreterResult<Value>;

/// Registry of built-in functions
pub struct BuiltinRegistry {
    functions: HashMap<String, (BuiltinFunction, usize)>, // (function, arity)
}

impl BuiltinRegistry {
    /// Create a new builtin registry with all standard functions
    pub fn new() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
        };
        
        // Register all built-in functions
        registry.register("print", builtin_print, 1);
        registry.register("println", builtin_println, 1);
        registry.register("len", builtin_len, 1);
        registry.register("type_of", builtin_type_of, 1);
        registry.register("to_string", builtin_to_string, 1);
        registry.register("parse_int", builtin_parse_int, 1);
        registry.register("parse_float", builtin_parse_float, 1);
        registry.register("abs", builtin_abs, 1);
        registry.register("min", builtin_min, 2);
        registry.register("max", builtin_max, 2);
        registry.register("floor", builtin_floor, 1);
        registry.register("ceil", builtin_ceil, 1);
        registry.register("round", builtin_round, 1);
        registry.register("sqrt", builtin_sqrt, 1);
        registry.register("pow", builtin_pow, 2);
        
        registry
    }
    
    /// Register a built-in function
    fn register(&mut self, name: &str, function: BuiltinFunction, arity: usize) {
        self.functions.insert(name.to_string(), (function, arity));
    }
    
    /// Check if a function is a built-in
    pub fn is_builtin(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }
    
    /// Call a built-in function
    pub fn call(&self, name: &str, args: &[Value], position: Position) -> InterpreterResult<Value> {
        if let Some((function, expected_arity)) = self.functions.get(name) {
            if args.len() != *expected_arity {
                return Err(InterpreterError::argument_count_mismatch(
                    name.to_string(),
                    *expected_arity,
                    args.len(),
                    position,
                ));
            }
            function(args, position)
        } else {
            Err(InterpreterError::undefined_variable(name.to_string(), position))
        }
    }
}

// Built-in function implementations

fn builtin_print(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    print!("{}", args[0].to_string());
    Ok(Value::Unit)
}

fn builtin_println(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    println!("{}", args[0].to_string());
    Ok(Value::Unit)
}

fn builtin_len(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::String(s) => Ok(Value::Integer(s.len() as i64)),
        Value::Array(arr) => Ok(Value::Integer(arr.len() as i64)),
        _ => Err(InterpreterError::type_error(
            format!("Cannot get length of {}", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_type_of(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    Ok(Value::String(args[0].type_name().to_string()))
}

fn builtin_to_string(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    Ok(Value::String(args[0].to_string()))
}

fn builtin_parse_int(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::String(s) => {
            s.parse::<i64>()
                .map(Value::Integer)
                .map_err(|_| InterpreterError::runtime(
                    format!("Cannot parse '{}' as integer", s),
                    position,
                ))
        }
        Value::Integer(i) => Ok(Value::Integer(*i)),
        Value::Float(f) => Ok(Value::Integer(*f as i64)),
        _ => Err(InterpreterError::type_error(
            format!("Cannot parse {} as integer", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_parse_float(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::String(s) => {
            s.parse::<f64>()
                .map(Value::Float)
                .map_err(|_| InterpreterError::runtime(
                    format!("Cannot parse '{}' as float", s),
                    position,
                ))
        }
        Value::Integer(i) => Ok(Value::Float(*i as f64)),
        Value::Float(f) => Ok(Value::Float(*f)),
        _ => Err(InterpreterError::type_error(
            format!("Cannot parse {} as float", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_abs(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::Integer(i) => Ok(Value::Integer(i.abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        _ => Err(InterpreterError::type_error(
            format!("Cannot get absolute value of {}", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_min(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match (&args[0], &args[1]) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(*a.min(b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
        (Value::Integer(a), Value::Float(b)) => Ok(Value::Float((*a as f64).min(*b))),
        (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a.min(*b as f64))),
        _ => Err(InterpreterError::type_error(
            format!("Cannot compare {} and {}", args[0].type_name(), args[1].type_name()),
            position,
        )),
    }
}

fn builtin_max(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match (&args[0], &args[1]) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(*a.max(b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
        (Value::Integer(a), Value::Float(b)) => Ok(Value::Float((*a as f64).max(*b))),
        (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a.max(*b as f64))),
        _ => Err(InterpreterError::type_error(
            format!("Cannot compare {} and {}", args[0].type_name(), args[1].type_name()),
            position,
        )),
    }
}

fn builtin_floor(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.floor())),
        Value::Integer(i) => Ok(Value::Integer(*i)),
        _ => Err(InterpreterError::type_error(
            format!("Cannot floor {}", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_ceil(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.ceil())),
        Value::Integer(i) => Ok(Value::Integer(*i)),
        _ => Err(InterpreterError::type_error(
            format!("Cannot ceil {}", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_round(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.round())),
        Value::Integer(i) => Ok(Value::Integer(*i)),
        _ => Err(InterpreterError::type_error(
            format!("Cannot round {}", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_sqrt(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::Integer(i) => {
            if *i < 0 {
                Err(InterpreterError::runtime(
                    "Cannot take square root of negative number",
                    position,
                ))
            } else {
                Ok(Value::Float((*i as f64).sqrt()))
            }
        }
        Value::Float(f) => {
            if *f < 0.0 {
                Err(InterpreterError::runtime(
                    "Cannot take square root of negative number",
                    position,
                ))
            } else {
                Ok(Value::Float(f.sqrt()))
            }
        }
        _ => Err(InterpreterError::type_error(
            format!("Cannot take square root of {}", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_pow(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match (&args[0], &args[1]) {
        (Value::Integer(base), Value::Integer(exp)) => {
            if *exp < 0 {
                Ok(Value::Float((*base as f64).powi(*exp as i32)))
            } else {
                Ok(Value::Integer(base.pow(*exp as u32)))
            }
        }
        (Value::Float(base), Value::Integer(exp)) => {
            Ok(Value::Float(base.powi(*exp as i32)))
        }
        (Value::Integer(base), Value::Float(exp)) => {
            Ok(Value::Float((*base as f64).powf(*exp)))
        }
        (Value::Float(base), Value::Float(exp)) => {
            Ok(Value::Float(base.powf(*exp)))
        }
        _ => Err(InterpreterError::type_error(
            format!("Cannot raise {} to power of {}", args[0].type_name(), args[1].type_name()),
            position,
        )),
    }
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}