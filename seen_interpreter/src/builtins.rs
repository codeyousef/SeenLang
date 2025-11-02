//! Built-in functions for the Seen interpreter

use crate::value::Value;
use crate::errors::{InterpreterError, InterpreterResult};
use seen_parser::Position;
use std::collections::HashMap;
use std::fs;
use std::io::Read as _;
use std::time::{SystemTime, UNIX_EPOCH};
use std::process::Command;

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
        
        // System/IO builtins (double-underscore to avoid name clashes with user code)
        registry.register("__GetCommandLineArgs", builtin_get_command_line_args, 0);
        registry.register("__GetTimestamp", builtin_get_timestamp, 0);
        registry.register("__ReadFile", builtin_read_file, 1);
        registry.register("__WriteFile", builtin_write_file, 2);
        registry.register("__CreateDirectory", builtin_create_directory, 1);
        registry.register("__DeleteFile", builtin_delete_file, 1);
        registry.register("__ExecuteProgram", builtin_execute_program, 1);
        registry.register("__ExecuteCommand", builtin_execute_command, 1);
        registry.register("__FormatSeenCode", builtin_format_seen_code, 1);

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

// --- System/IO builtin implementations ---

fn builtin_get_command_line_args(_args: &[Value], _position: Position) -> InterpreterResult<Value> {
    // Prefer SEEN_PROGRAM_ARGS env override for interpreter-run programs
    if let Ok(over) = std::env::var("SEEN_PROGRAM_ARGS") {
        let parts: Vec<String> = over
            .split(' ')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        return Ok(Value::Array(parts.into_iter().map(Value::String).collect()));
    }
    let vals: Vec<Value> = std::env::args().map(Value::String).collect();
    Ok(Value::Array(vals))
}

fn builtin_get_timestamp(_args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    Ok(Value::String(now.as_secs().to_string()))
}

fn builtin_read_file(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let path = args[0].to_string();
    match fs::read_to_string(&path) {
        Ok(s) => Ok(Value::String(s)),
        Err(_) => Ok(Value::String(String::new())), // Align with Seen stubs' empty-string-on-fail pattern
    }
}

fn builtin_write_file(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let path = args[0].to_string();
    let content = args[1].to_string();
    match fs::write(&path, content) {
        Ok(_) => Ok(Value::Boolean(true)),
        Err(_) => Ok(Value::Boolean(false)),
    }
}

fn builtin_create_directory(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let path = args[0].to_string();
    match fs::create_dir_all(&path) {
        Ok(_) => Ok(Value::Boolean(true)),
        Err(_) => Ok(Value::Boolean(false)),
    }
}

fn builtin_delete_file(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let path = args[0].to_string();
    match fs::remove_file(&path) {
        Ok(_) => Ok(Value::Boolean(true)),
        Err(_) => Ok(Value::Boolean(false)),
    }
}

fn builtin_execute_program(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let path = args[0].to_string();
    let status = Command::new(path).status();
    Ok(Value::Integer(match status {
        Ok(s) => s.code().unwrap_or(1) as i64,
        Err(_) => 1,
    }))
}

fn builtin_execute_command(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let cmd = args[0].to_string();
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd").arg("/C").arg(cmd).output();
    #[cfg(not(target_os = "windows"))]
    let output = Command::new("sh").arg("-c").arg(cmd).output();

    let (success, stdout) = match output {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).to_string()),
        Err(_) => (false, String::new()),
    };
    let mut fields = HashMap::new();
    fields.insert("success".to_string(), Value::Boolean(success));
    fields.insert("output".to_string(), Value::String(stdout));
    Ok(Value::Struct { name: "CommandResult".to_string(), fields })
}

fn builtin_format_seen_code(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    // Placeholder: return input as-is
    Ok(Value::String(args[0].to_string()))
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}
