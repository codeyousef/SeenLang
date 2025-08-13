//! Interpreter for the Seen programming language

pub mod value;
pub mod runtime;
pub mod errors;
pub mod builtins;
pub mod interpreter;

pub use interpreter::Interpreter;
pub use value::Value;
pub use runtime::{Runtime, Environment};
pub use errors::{InterpreterError, InterpreterResult};
pub use builtins::BuiltinRegistry;