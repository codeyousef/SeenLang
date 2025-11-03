//! Interpreter for the Seen programming language

pub mod builtins;
pub mod errors;
pub mod interpreter;
pub mod runtime;
pub mod value;

pub use builtins::BuiltinRegistry;
pub use errors::{InterpreterError, InterpreterResult};
pub use interpreter::Interpreter;
pub use runtime::{Environment, Runtime};
pub use value::Value;
