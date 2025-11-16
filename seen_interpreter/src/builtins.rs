//! Built-in functions for the Seen interpreter

use crate::errors::{InterpreterError, InterpreterResult};
use crate::value::Value;
use seen_concurrency::types::{Channel, ChannelId};
use seen_parser::Position;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Type signature for built-in functions
pub type BuiltinFunction = fn(&[Value], Position) -> InterpreterResult<Value>;

#[derive(Copy, Clone)]
enum BuiltinArity {
    Exact(usize),
    Range { min: usize, max: Option<usize> },
}

/// Registry of built-in functions
pub struct BuiltinRegistry {
    functions: HashMap<String, (BuiltinFunction, BuiltinArity)>,
}

impl BuiltinRegistry {
    /// Create a new builtin registry with all standard functions
    pub fn new() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
        };

        // Register all built-in functions
        registry.register_exact("print", builtin_print, 1);
        registry.register_exact("println", builtin_println, 1);
        registry.register_exact("len", builtin_len, 1);
        registry.register_exact("type_of", builtin_type_of, 1);
        registry.register_exact("to_string", builtin_to_string, 1);
        registry.register_exact("parse_int", builtin_parse_int, 1);
        registry.register_exact("parse_float", builtin_parse_float, 1);
        registry.register_exact("abs", builtin_abs, 1);
        registry.register_exact("min", builtin_min, 2);
        registry.register_exact("max", builtin_max, 2);
        registry.register_exact("floor", builtin_floor, 1);
        registry.register_exact("ceil", builtin_ceil, 1);
        registry.register_exact("round", builtin_round, 1);
        registry.register_exact("sqrt", builtin_sqrt, 1);
        registry.register_exact("pow", builtin_pow, 2);

        // System/IO builtins (double-underscore to avoid name clashes with user code)
        registry.register_exact("__GetCommandLineArgs", builtin_get_command_line_args, 0);
        registry.register_exact("__GetTimestamp", builtin_get_timestamp, 0);
        registry.register_exact("__ReadFile", builtin_read_file, 1);
        registry.register_exact("__WriteFile", builtin_write_file, 2);
        registry.register_exact("__CreateDirectory", builtin_create_directory, 1);
        registry.register_exact("__DeleteFile", builtin_delete_file, 1);
        registry.register_exact("__ExecuteProgram", builtin_execute_program, 1);
        registry.register_exact("__ExecuteCommand", builtin_execute_command, 1);
        registry.register_exact("__CommandOutput", builtin_command_output, 1);
        registry.register_exact("__GetEnv", builtin_get_env, 1);
        registry.register_exact("__HasEnv", builtin_has_env, 1);
        registry.register_exact("__SetEnv", builtin_set_env, 2);
        registry.register_exact("__RemoveEnv", builtin_remove_env, 1);
        registry.register_exact("__FormatSeenCode", builtin_format_seen_code, 1);
        registry.register_exact("__Abort", builtin_abort, 1);
        registry.register_range("Channel", builtin_channel, 0, Some(1));

        // Additional I/O and conversion functions for benchmarks
        registry.register_exact("__Print", builtin_print, 1);
        registry.register_exact("__Println", builtin_println, 1);
        registry.register_exact("__IntToString", builtin_int_to_string, 1);
        registry.register_exact("__FloatToString", builtin_float_to_string, 1);
        registry.register_exact("__BoolToString", builtin_bool_to_string, 1);

        registry
    }

    /// Register a built-in function
    fn register_exact(&mut self, name: &str, function: BuiltinFunction, arity: usize) {
        self.functions
            .insert(name.to_string(), (function, BuiltinArity::Exact(arity)));
    }

    fn register_range(
        &mut self,
        name: &str,
        function: BuiltinFunction,
        min: usize,
        max: Option<usize>,
    ) {
        self.functions.insert(
            name.to_string(),
            (function, BuiltinArity::Range { min, max }),
        );
    }

    /// Check if a function is a built-in
    pub fn is_builtin(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Call a built-in function
    pub fn call(&self, name: &str, args: &[Value], position: Position) -> InterpreterResult<Value> {
        if let Some((function, expected_arity)) = self.functions.get(name) {
            match expected_arity {
                BuiltinArity::Exact(n) => {
                    if args.len() != *n {
                        return Err(InterpreterError::argument_count_mismatch(
                            name.to_string(),
                            *n,
                            args.len(),
                            position,
                        ));
                    }
                }
                BuiltinArity::Range { min, max } => {
                    if args.len() < *min || max.map_or(false, |max| args.len() > max) {
                        return Err(InterpreterError::argument_count_mismatch(
                            name.to_string(),
                            max.unwrap_or(*min),
                            args.len(),
                            position,
                        ));
                    }
                }
            }
            function(args, position)
        } else {
            Err(InterpreterError::undefined_variable(
                name.to_string(),
                position,
            ))
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
        Value::Array(arr) => {
            let len = arr
                .lock()
                .map_err(|_| InterpreterError::runtime("Array access failed", position))?
                .len();
            Ok(Value::Integer(len as i64))
        }
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
        Value::String(s) => s.parse::<i64>().map(Value::Integer).map_err(|_| {
            InterpreterError::runtime(format!("Cannot parse '{}' as integer", s), position)
        }),
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
        Value::String(s) => s.parse::<f64>().map(Value::Float).map_err(|_| {
            InterpreterError::runtime(format!("Cannot parse '{}' as float", s), position)
        }),
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
            format!(
                "Cannot compare {} and {}",
                args[0].type_name(),
                args[1].type_name()
            ),
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
            format!(
                "Cannot compare {} and {}",
                args[0].type_name(),
                args[1].type_name()
            ),
            position,
        )),
    }
}

fn builtin_channel(args: &[Value], position: Position) -> InterpreterResult<Value> {
    if args.len() > 1 {
        return Err(InterpreterError::argument_count_mismatch(
            "Channel".to_string(),
            1,
            args.len(),
            position,
        ));
    }

    let capacity = if let Some(arg) = args.get(0) {
        let capacity_value = match arg {
            Value::Integer(v) => *v,
            other => {
                return Err(InterpreterError::type_error(
                    format!("Channel capacity must be Int, got {}", other.type_name()),
                    position,
                ))
            }
        };

        if capacity_value < 0 {
            return Err(InterpreterError::runtime(
                "Channel capacity must be non-negative",
                position,
            ));
        }

        Some(capacity_value as usize)
    } else {
        None
    };

    let channel = Channel::new(ChannelId::allocate(), capacity);
    let mut fields = HashMap::new();
    fields.insert("Sender".to_string(), Value::Channel(channel.clone()));
    fields.insert("Receiver".to_string(), Value::Channel(channel));

    Ok(Value::struct_from_fields(
        "ChannelEndpoints".to_string(),
        fields,
    ))
}

fn builtin_abort(args: &[Value], position: Position) -> InterpreterResult<Value> {
    let message = match args.first() {
        Some(Value::String(s)) => s.clone(),
        Some(other) => {
            return Err(InterpreterError::type_error(
                format!("__Abort expects a String, got {}", other.type_name()),
                position,
            ))
        }
        None => String::from("Execution aborted"),
    };

    Err(InterpreterError::abort(message, position))
}

fn builtin_int_to_string(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::Integer(i) => Ok(Value::String(i.to_string())),
        _ => Err(InterpreterError::type_error(
            format!("__IntToString expects Int, got {}", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_float_to_string(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::Float(f) => Ok(Value::String(f.to_string())),
        _ => Err(InterpreterError::type_error(
            format!("__FloatToString expects Float, got {}", args[0].type_name()),
            position,
        )),
    }
}

fn builtin_bool_to_string(args: &[Value], position: Position) -> InterpreterResult<Value> {
    match &args[0] {
        Value::Boolean(b) => Ok(Value::String(b.to_string())),
        _ => Err(InterpreterError::type_error(
            format!("__BoolToString expects Bool, got {}", args[0].type_name()),
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
        (Value::Float(base), Value::Integer(exp)) => Ok(Value::Float(base.powi(*exp as i32))),
        (Value::Integer(base), Value::Float(exp)) => Ok(Value::Float((*base as f64).powf(*exp))),
        (Value::Float(base), Value::Float(exp)) => Ok(Value::Float(base.powf(*exp))),
        _ => Err(InterpreterError::type_error(
            format!(
                "Cannot raise {} to power of {}",
                args[0].type_name(),
                args[1].type_name()
            ),
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
        return Ok(Value::array_from_vec(
            parts.into_iter().map(Value::String).collect(),
        ));
    }
    let vals: Vec<Value> = std::env::args().map(Value::String).collect();
    Ok(Value::array_from_vec(vals))
}

fn builtin_get_timestamp(_args: &[Value], _position: Position) -> InterpreterResult<Value> {
    if std::env::var("SEEN_DETERMINISTIC")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        let epoch = std::env::var("SOURCE_DATE_EPOCH").unwrap_or_else(|_| "0".to_string());
        return Ok(Value::String(epoch));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
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
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).to_string(),
        ),
        Err(_) => (false, String::new()),
    };
    let mut fields = HashMap::new();
    fields.insert("success".to_string(), Value::Boolean(success));
    fields.insert("output".to_string(), Value::String(stdout));
    Ok(Value::struct_from_fields(
        "CommandResult".to_string(),
        fields,
    ))
}

fn builtin_command_output(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let cmd = args[0].to_string();
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd").arg("/C").arg(cmd).output();
    #[cfg(not(target_os = "windows"))]
    let output = Command::new("sh").arg("-c").arg(cmd).output();

    match output {
        Ok(o) => Ok(Value::String(
            String::from_utf8_lossy(&o.stdout).to_string(),
        )),
        Err(_) => Ok(Value::String(String::new())),
    }
}

fn builtin_get_env(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let key = args[0].to_string();
    let value = std::env::var(&key).unwrap_or_else(|_| String::new());
    Ok(Value::String(value))
}

fn builtin_has_env(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let key = args[0].to_string();
    Ok(Value::Boolean(std::env::var(&key).is_ok()))
}

fn builtin_set_env(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let key = args[0].to_string();
    let value = args[1].to_string();
    std::env::set_var(&key, &value);
    Ok(Value::Boolean(true))
}

fn builtin_remove_env(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    let key = args[0].to_string();
    std::env::remove_var(&key);
    Ok(Value::Boolean(true))
}

fn builtin_format_seen_code(args: &[Value], _position: Position) -> InterpreterResult<Value> {
    // Placeholder: return input as-is
    Ok(Value::String(args[0].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_concurrency::types::{AsyncValue, ChannelReceiveStatus, ChannelSendStatus};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn deterministic_timestamp_uses_source_date_epoch() {
        let _guard = env_lock().lock().unwrap();
        let previous_epoch = std::env::var("SOURCE_DATE_EPOCH").ok();
        let previous_flag = std::env::var("SEEN_DETERMINISTIC").ok();

        std::env::set_var("SEEN_DETERMINISTIC", "1");
        std::env::set_var("SOURCE_DATE_EPOCH", "12345");
        let ts = builtin_get_timestamp(&[], Position::start())
            .expect("timestamp builtin should succeed");
        assert_eq!(ts, Value::String("12345".to_string()));

        match previous_epoch {
            Some(value) => std::env::set_var("SOURCE_DATE_EPOCH", value),
            None => std::env::remove_var("SOURCE_DATE_EPOCH"),
        }
        match previous_flag {
            Some(value) => std::env::set_var("SEEN_DETERMINISTIC", value),
            None => std::env::remove_var("SEEN_DETERMINISTIC"),
        }
    }

    #[test]
    fn deterministic_timestamp_defaults_to_zero() {
        let _guard = env_lock().lock().unwrap();
        let previous_epoch = std::env::var("SOURCE_DATE_EPOCH").ok();
        let previous_flag = std::env::var("SEEN_DETERMINISTIC").ok();

        std::env::set_var("SEEN_DETERMINISTIC", "1");
        std::env::remove_var("SOURCE_DATE_EPOCH");
        let ts = builtin_get_timestamp(&[], Position::start())
            .expect("timestamp builtin should succeed");
        assert_eq!(ts, Value::String("0".to_string()));

        match previous_epoch {
            Some(value) => std::env::set_var("SOURCE_DATE_EPOCH", value),
            None => std::env::remove_var("SOURCE_DATE_EPOCH"),
        }
        match previous_flag {
            Some(value) => std::env::set_var("SEEN_DETERMINISTIC", value),
            None => std::env::remove_var("SEEN_DETERMINISTIC"),
        }
    }

    #[test]
    fn channel_builtin_returns_endpoints_struct() {
        let endpoints = builtin_channel(&[], Position::start()).expect("Channel() should succeed");

        let Value::Struct { name, fields } = endpoints else {
            panic!("expected Channel to return struct endpoints");
        };
        assert_eq!(name, "ChannelEndpoints");
        let field_map = fields.lock().expect("fields lock poisoned");
        assert!(matches!(field_map.get("Sender"), Some(Value::Channel(_))));
        assert!(matches!(field_map.get("Receiver"), Some(Value::Channel(_))));
    }

    #[test]
    fn channel_builtin_honours_capacity_argument() {
        let endpoints = builtin_channel(&[Value::Integer(1)], Position::start())
            .expect("Channel(1) should succeed");

        let Value::Struct { fields, .. } = endpoints else {
            panic!("expected Channel to return struct endpoints");
        };

        let field_map = fields.lock().expect("fields lock poisoned");

        let sender_channel = match field_map.get("Sender") {
            Some(Value::Channel(ch)) => ch.clone(),
            other => panic!("unexpected sender field: {:?}", other),
        };

        assert_eq!(
            sender_channel.send_with_status(AsyncValue::Integer(1)),
            ChannelSendStatus::Sent
        );
        assert_eq!(
            sender_channel.send_with_status(AsyncValue::Integer(2)),
            ChannelSendStatus::WouldBlock
        );

        let receiver_channel = match field_map.get("Receiver") {
            Some(Value::Channel(ch)) => ch.clone(),
            other => panic!("unexpected receiver field: {:?}", other),
        };

        match receiver_channel.try_recv_with_status() {
            ChannelReceiveStatus::Received(value) => {
                assert_eq!(value, AsyncValue::Integer(1));
            }
            other => panic!("expected to receive value, got {:?}", other),
        }
    }

    #[test]
    fn channel_builtin_rejects_negative_capacity() {
        let err = builtin_channel(&[Value::Integer(-1)], Position::start())
            .expect_err("Channel(-1) should fail");
        assert!(
            err.to_string()
                .contains("Channel capacity must be non-negative"),
            "unexpected error: {}",
            err
        );
    }
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}
