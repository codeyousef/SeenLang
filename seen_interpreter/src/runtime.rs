//! Runtime environment for the Seen interpreter

use crate::value::Value;
use seen_concurrency::{
    actors::ActorSystem,
    async_runtime::{AsyncRuntime, AsyncRuntimeConfig},
    channels::ChannelManager,
    jobs::JobSystem,
    types::TaskId,
};
use seen_effects::{AdvancedRuntime, AdvancedRuntimeConfig};
use seen_parser::Expression;
use seen_reactive::{ReactiveRuntime, ReactiveRuntimeConfig};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Runtime error types
#[derive(Debug, Clone)]
pub enum RuntimeError {
    UndefinedVariable(String),
    VariableAlreadyDefined(String),
    StackUnderflow,
    RecursionLimit,
    TypeError(String),
    AsyncError(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            RuntimeError::VariableAlreadyDefined(name) => {
                write!(f, "Variable already defined: {}", name)
            }
            RuntimeError::StackUnderflow => write!(f, "Stack underflow"),
            RuntimeError::RecursionLimit => write!(f, "Recursion limit exceeded"),
            RuntimeError::TypeError(msg) => write!(f, "Type error: {}", msg),
            RuntimeError::AsyncError(msg) => write!(f, "Async runtime error: {}", msg),
        }
    }
}

/// Environment for variable bindings
#[derive(Debug, Clone)]
pub struct Environment {
    variables: HashMap<String, Value>,
    parent: Option<Box<Environment>>,
    #[allow(dead_code)]
    is_function_scope: bool,
    deferred: Vec<Expression>,
}

impl Environment {
    /// Create a new root environment
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parent: None,
            is_function_scope: false,
            deferred: Vec::new(),
        }
    }

    /// Create a new child environment
    pub fn with_parent(parent: Environment, is_function_scope: bool) -> Self {
        Self {
            variables: HashMap::new(),
            parent: Some(Box::new(parent)),
            is_function_scope,
            deferred: Vec::new(),
        }
    }

    /// Define a new variable
    pub fn define(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// Get a variable value
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.variables.get(name) {
            Some(value.clone())
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    /// Set a variable value (must exist)
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        if self.variables.contains_key(name) {
            self.variables.insert(name.to_string(), value);
            Ok(())
        } else if let Some(parent) = &mut self.parent {
            parent.set(name, value)
        } else {
            Err(RuntimeError::UndefinedVariable(name.to_string()))
        }
    }

    /// Register a deferred expression to be executed when scope ends
    pub fn register_defer(&mut self, expr: Expression) {
        self.deferred.push(expr);
    }

    /// Drain deferred expressions in the order they were registered
    pub fn take_deferred(&mut self) -> Vec<Expression> {
        self.deferred.drain(..).collect()
    }
}

/// Call frame for function calls
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_name: String,
    pub location: seen_parser::Position,
}

/// Runtime state for the interpreter
pub struct Runtime {
    /// Stack of environments (scopes)
    environment_stack: Vec<Environment>,
    /// Call stack for function calls
    call_stack: Vec<CallFrame>,
    /// Return value from functions
    return_value: Option<Value>,
    /// Maximum recursion depth
    max_recursion_depth: usize,
    /// Async runtime for handling async/await operations
    async_runtime: Arc<Mutex<AsyncRuntime>>,
    /// Channel manager for channel operations
    channel_manager: Arc<Mutex<ChannelManager>>,
    /// Actor system for actor-based concurrency
    actor_system: Arc<Mutex<ActorSystem>>,
    /// Advanced runtime for effects and contracts
    advanced_runtime: Arc<Mutex<AdvancedRuntime>>,
    /// Reactive runtime for observables, flows, and reactive properties
    reactive_runtime: Arc<Mutex<ReactiveRuntime>>,
    /// Structured concurrency task scopes
    task_scope_stack: Vec<Vec<TaskId>>,
    /// Job system for parallel job execution
    job_system: Arc<JobSystem>,
}

impl Runtime {
    /// Create a new runtime
    pub fn new() -> Self {
        Self {
            environment_stack: vec![Environment::new()],
            call_stack: Vec::new(),
            return_value: None,
            max_recursion_depth: 1000,
            async_runtime: Arc::new(Mutex::new(AsyncRuntime::with_config(
                AsyncRuntimeConfig::default(),
            ))),
            channel_manager: Arc::new(Mutex::new(ChannelManager::new())),
            actor_system: Arc::new(Mutex::new(ActorSystem::new())),
            advanced_runtime: Arc::new(Mutex::new(AdvancedRuntime::with_config(
                AdvancedRuntimeConfig::default(),
            ))),
            reactive_runtime: Arc::new(Mutex::new(ReactiveRuntime::with_config(
                ReactiveRuntimeConfig::default(),
            ))),
            task_scope_stack: Vec::new(),
            job_system: Arc::new(JobSystem::new()),
        }
    }

    /// Push a new environment
    pub fn push_environment(&mut self, is_function_scope: bool) {
        let current = self.environment_stack.last().unwrap().clone();
        let new_env = Environment::with_parent(current, is_function_scope);
        self.environment_stack.push(new_env);
    }

    /// Snapshot the current lexical environment.
    pub fn snapshot_environment(&self) -> Environment {
        self.environment_stack
            .last()
            .cloned()
            .unwrap_or_else(Environment::new)
    }

    /// Pop an environment
    pub fn pop_environment(&mut self) -> Result<(), RuntimeError> {
        if self.environment_stack.len() <= 1 {
            return Err(RuntimeError::StackUnderflow);
        }
        self.environment_stack.pop();
        Ok(())
    }

    /// Replace the root environment stack with a captured snapshot.
    pub fn initialize_with_environment(&mut self, env: Environment) {
        self.environment_stack.clear();
        self.environment_stack.push(env);
    }

    /// Push a structured concurrency scope frame
    pub fn push_task_scope(&mut self) {
        self.task_scope_stack.push(Vec::new());
    }

    /// Pop the current structured concurrency scope
    pub fn pop_task_scope(&mut self) -> Result<Vec<TaskId>, RuntimeError> {
        self.task_scope_stack
            .pop()
            .ok_or(RuntimeError::StackUnderflow)
    }

    /// Register a spawned task with the current scope if available
    pub fn register_scope_task(&mut self, task_id: TaskId) {
        if let Some(scope) = self.task_scope_stack.last_mut() {
            scope.push(task_id);
        }
    }

    /// Register a deferred expression in the current environment
    pub fn register_defer(&mut self, expr: Expression) -> Result<(), RuntimeError> {
        if let Some(env) = self.environment_stack.last_mut() {
            env.register_defer(expr);
            Ok(())
        } else {
            Err(RuntimeError::StackUnderflow)
        }
    }

    /// Take deferred expressions from the current environment without popping it
    pub fn take_current_deferred(&mut self) -> Result<Vec<Expression>, RuntimeError> {
        if let Some(env) = self.environment_stack.last_mut() {
            Ok(env.take_deferred())
        } else {
            Err(RuntimeError::StackUnderflow)
        }
    }

    /// Define a variable in the current environment
    pub fn define_variable(&mut self, name: String, value: Value) {
        if let Some(env) = self.environment_stack.last_mut() {
            env.define(name, value);
        }
    }

    /// Get a variable value
    pub fn get_variable(&self, name: &str) -> Result<Value, RuntimeError> {
        if let Some(env) = self.environment_stack.last() {
            env.get(name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.to_string()))
        } else {
            Err(RuntimeError::StackUnderflow)
        }
    }

    /// Set a variable value
    pub fn set_variable(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        if let Some(env) = self.environment_stack.last_mut() {
            env.set(name, value)
        } else {
            Err(RuntimeError::StackUnderflow)
        }
    }

    /// Push a function call onto the call stack
    pub fn push_call(
        &mut self,
        function_name: String,
        location: seen_parser::Position,
    ) -> Result<(), RuntimeError> {
        if self.call_stack.len() >= self.max_recursion_depth {
            return Err(RuntimeError::RecursionLimit);
        }
        self.call_stack.push(CallFrame {
            function_name,
            location,
        });
        Ok(())
    }

    /// Pop a function call from the call stack
    pub fn pop_call(&mut self) -> Result<(), RuntimeError> {
        self.call_stack.pop().ok_or(RuntimeError::StackUnderflow)?;
        Ok(())
    }

    /// Set the return value
    pub fn set_return_value(&mut self, value: Value) -> Result<(), RuntimeError> {
        self.return_value = Some(value);
        Ok(())
    }

    /// Get the return value
    pub fn get_return_value(&self) -> Option<&Value> {
        self.return_value.as_ref()
    }

    /// Clear the return value
    pub fn clear_return_value(&mut self) {
        self.return_value = None;
    }

    /// Print a value to stdout
    pub fn print(&self, value: &Value) -> Result<(), RuntimeError> {
        print!("{}", value.to_string());
        Ok(())
    }

    /// Print a value with newline to stdout
    pub fn println(&self, value: &Value) -> Result<(), RuntimeError> {
        println!("{}", value.to_string());
        Ok(())
    }

    /// Get reference to async runtime
    pub fn async_runtime(&self) -> Arc<Mutex<AsyncRuntime>> {
        Arc::clone(&self.async_runtime)
    }

    /// Cancel a running task via the async runtime
    pub fn cancel_task(&self, task_id: TaskId) -> Result<bool, RuntimeError> {
        let runtime_arc = self.async_runtime();
        let mut runtime = runtime_arc
            .lock()
            .map_err(|_| RuntimeError::AsyncError("Failed to acquire async runtime lock".into()))?;
        runtime
            .cancel_task(task_id)
            .map_err(|err| RuntimeError::AsyncError(format!("{:?}", err)))
    }

    pub fn job_system(&self) -> Arc<JobSystem> {
        Arc::clone(&self.job_system)
    }

    /// Get reference to channel manager
    pub fn channel_manager(&self) -> Arc<Mutex<ChannelManager>> {
        Arc::clone(&self.channel_manager)
    }

    /// Get reference to actor system
    pub fn actor_system(&self) -> Arc<Mutex<ActorSystem>> {
        Arc::clone(&self.actor_system)
    }

    /// Get reference to advanced runtime (effects and contracts)
    pub fn advanced_runtime(&self) -> Arc<Mutex<AdvancedRuntime>> {
        Arc::clone(&self.advanced_runtime)
    }

    /// Get reference to reactive runtime (observables, flows, reactive properties)
    pub fn reactive_runtime(&self) -> Arc<Mutex<ReactiveRuntime>> {
        Arc::clone(&self.reactive_runtime)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
