//! Interpreter for the Seen programming language

pub mod actor_executor;
pub mod builtins;
pub mod errors;
pub mod interpreter;
pub mod runtime;
pub mod value;
mod value_bridge;

pub use builtins::BuiltinRegistry;
pub use errors::{InterpreterError, InterpreterResult};
pub use interpreter::Interpreter;
pub use runtime::{Environment, Runtime};
pub use value::Value;
