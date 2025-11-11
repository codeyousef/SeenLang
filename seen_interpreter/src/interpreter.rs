//! Main interpreter implementation for the Seen programming language

use crate::actor_executor::InterpreterActorExecutor;
use crate::builtins::BuiltinRegistry;
use crate::errors::{InterpreterError, InterpreterResult};
use crate::runtime::{Environment, Runtime};
use crate::trace::{RuntimeTraceEvent, RuntimeTraceHandle, RuntimeTraceValue};
use crate::value::Value;
use crate::value_bridge::{async_to_value, value_to_async};
use seen_concurrency::{
    actors::{ActorDefinition as RuntimeActorDefinition, MessageHandler as RuntimeMessageHandler},
    async_runtime::AsyncExecutionContext,
    async_runtime::AsyncFunction as AsyncFunctionTrait,
    types::{
        channel_select_future, AsyncError, AsyncResult, AsyncValue, Channel, ChannelSelectCase,
        ChannelSelectOutcome, TaskId, TaskPriority,
    },
};
use seen_effects::{
    effects::EffectParameter, types::AsyncValue as EffectAsyncValue, EffectDefinition, EffectId,
    EffectOperation as EffectOp,
};
use seen_parser::{
    ast::{ClassField, MessageHandler as AstMessageHandler, Type as AstType},
    AssignmentOperator, BinaryOperator, Expression, ForBinding, InterpolationKind,
    InterpolationPart, MatchArm, Method, Parameter, Pattern, Position, Program, UnaryOperator,
};
use seen_reactive::properties::PropertyId as ReactivePropertyId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Wrapper for Seen expressions to be executed as async functions
#[derive(Debug, Clone)]
struct SeenAsyncFunction {
    expression: Expression,
    position: Position,
    environment: Environment,
}

impl AsyncFunctionTrait for SeenAsyncFunction {
    fn execute(
        &self,
        _context: &mut AsyncExecutionContext,
    ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        let expr = self.expression.clone();
        let pos = self.position.clone();
        let captured_env = self.environment.clone();

        Box::pin(async move {
            // Create a new interpreter instance for this task
            let mut interpreter = Interpreter::new();
            interpreter
                .runtime
                .initialize_with_environment(captured_env);

            match interpreter.interpret_expression(&expr) {
                Ok(value) => {
                    // Convert Value to AsyncValue
                    let async_value = match value {
                        Value::Unit => AsyncValue::Unit,
                        Value::Integer(i) => AsyncValue::Integer(i),
                        Value::Float(f) => AsyncValue::Float(f),
                        Value::String(s) => AsyncValue::String(s),
                        Value::Boolean(b) => AsyncValue::Boolean(b),
                        _ => AsyncValue::Unit, // For complex types, default to Unit
                    };
                    Ok(async_value)
                }
                Err(e) => {
                    let error = AsyncError::RuntimeError {
                        message: format!("Task execution failed: {:?}", e),
                        position: pos,
                    };
                    Err(error)
                }
            }
        })
    }

    fn name(&self) -> &str {
        "SeenExpression"
    }
}

#[derive(Debug, Clone)]
struct ChannelSendAsyncFunction {
    channel: Channel,
    value: AsyncValue,
    position: Position,
}

impl AsyncFunctionTrait for ChannelSendAsyncFunction {
    fn execute(
        &self,
        _context: &mut AsyncExecutionContext,
    ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        let channel = self.channel.clone();
        let value = self.value.clone();
        let pos = self.position.clone();

        Box::pin(async move {
            channel
                .send_future(value)
                .await
                .map_err(|error| match error {
                    AsyncError::ChannelError { reason, .. } => AsyncError::ChannelError {
                        reason,
                        position: pos.clone(),
                    },
                    AsyncError::RuntimeError { message, .. } => AsyncError::RuntimeError {
                        message,
                        position: pos.clone(),
                    },
                    other => other,
                })
        })
    }

    fn name(&self) -> &str {
        "ChannelSend"
    }
}

const SELECT_STATUS_RECEIVED: &str = "received";
const SELECT_STATUS_SENT: &str = "sent";
const SELECT_STATUS_CLOSED: &str = "closed";
const SELECT_STATUS_ALL_CLOSED: &str = "all_closed";
const SELECT_STATUS_TIMEOUT: &str = "timeout";

#[derive(Debug, Clone)]
struct ChannelSelectAsyncFunction {
    cases: Vec<ChannelSelectCase>,
    timeout: Option<Duration>,
    position: Position,
}

impl AsyncFunctionTrait for ChannelSelectAsyncFunction {
    fn execute(
        &self,
        _context: &mut AsyncExecutionContext,
    ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        let cases = self.cases.clone();
        let timeout = self.timeout;
        let pos = self.position.clone();

        Box::pin(async move {
            match channel_select_future(cases, timeout).await {
                Ok(ChannelSelectOutcome::Received { case_index, value }) => {
                    Ok(AsyncValue::Array(vec![
                        AsyncValue::Integer(case_index as i64),
                        value,
                        AsyncValue::String(SELECT_STATUS_RECEIVED.to_string()),
                    ]))
                }
                Ok(ChannelSelectOutcome::Sent { case_index }) => Ok(AsyncValue::Array(vec![
                    AsyncValue::Integer(case_index as i64),
                    AsyncValue::Unit,
                    AsyncValue::String(SELECT_STATUS_SENT.to_string()),
                ])),
                Ok(ChannelSelectOutcome::Closed { case_index }) => Ok(AsyncValue::Array(vec![
                    AsyncValue::Integer(case_index as i64),
                    AsyncValue::Unit,
                    AsyncValue::String(SELECT_STATUS_CLOSED.to_string()),
                ])),
                Ok(ChannelSelectOutcome::AllClosed) => Ok(AsyncValue::Array(vec![
                    AsyncValue::Integer(-1),
                    AsyncValue::Unit,
                    AsyncValue::String(SELECT_STATUS_ALL_CLOSED.to_string()),
                ])),
                Ok(ChannelSelectOutcome::Timeout) => Ok(AsyncValue::Array(vec![
                    AsyncValue::Integer(-1),
                    AsyncValue::Unit,
                    AsyncValue::String(SELECT_STATUS_TIMEOUT.to_string()),
                ])),
                Err(err) => Err(match err {
                    AsyncError::ChannelError { reason, .. } => AsyncError::ChannelError {
                        reason,
                        position: pos.clone(),
                    },
                    AsyncError::RuntimeError { message, .. } => AsyncError::RuntimeError {
                        message,
                        position: pos.clone(),
                    },
                    other => other,
                }),
            }
        })
    }

    fn name(&self) -> &str {
        "ChannelSelect"
    }
}

#[derive(Clone)]
struct RuntimeClassField {
    name: String,
    default_value: Option<Expression>,
    #[allow(dead_code)]
    is_mutable: bool,
}

#[derive(Clone)]
struct RuntimeMethod {
    #[allow(dead_code)]
    name: String,
    parameters: Vec<Parameter>,
    body: Expression,
    is_static: bool,
    receiver_name: String,
}

#[derive(Clone)]
struct RuntimeClass {
    #[allow(dead_code)]
    name: String,
    fields: Vec<RuntimeClassField>,
    methods: HashMap<String, RuntimeMethod>,
}

struct InstanceContext {
    #[allow(dead_code)]
    class_name: String,
    fields: Arc<Mutex<HashMap<String, Value>>>,
}

struct EffectHandlerFrame {
    effect_id: EffectId,
    handlers: HashMap<String, Value>,
}

impl InstanceContext {
    fn get_field(&self, name: &str) -> Option<Value> {
        self.fields
            .lock()
            .ok()
            .and_then(|fields| fields.get(name).cloned())
    }

    fn set_field(&self, name: &str, value: Value) -> bool {
        if let Ok(mut fields) = self.fields.lock() {
            if fields.contains_key(name) {
                fields.insert(name.to_string(), value);
                return true;
            }
        }
        false
    }
}

/// The main interpreter for Seen programs
pub struct Interpreter {
    /// Runtime environment
    runtime: Runtime,
    /// Built-in functions registry
    builtins: BuiltinRegistry,
    /// Registered class metadata
    class_registry: HashMap<String, RuntimeClass>,
    /// Active instance contexts for field access
    instance_stack: Vec<InstanceContext>,
    /// Break/Continue flags for loop control
    break_flag: bool,
    continue_flag: bool,
    /// Return flag and value
    return_flag: bool,
    return_value: Option<Value>,
    /// Task counter for generating unique task IDs
    #[allow(dead_code)]
    task_counter: std::sync::atomic::AtomicU64,
    /// Active effect handler frames
    effect_handler_stack: Vec<EffectHandlerFrame>,
    /// Registered effect names to IDs
    effect_registry: HashMap<String, EffectId>,
    /// Reactive property bindings by variable name
    reactive_bindings: HashMap<String, ReactivePropertyId>,
    /// Active flow collectors capturing `emit` outputs
    flow_collectors: Vec<Vec<Value>>,
    /// Executor used to evaluate actor handler bodies
    actor_executor: Arc<InterpreterActorExecutor>,
    /// Optional runtime trace sink
    runtime_trace: Option<RuntimeTraceHandle>,
}

impl Interpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        let mut runtime = Runtime::new();
        let actor_executor = Arc::new(InterpreterActorExecutor::new());
        runtime.set_actor_handler_executor(actor_executor.clone());

        Self {
            runtime,
            builtins: BuiltinRegistry::new(),
            class_registry: HashMap::new(),
            instance_stack: Vec::new(),
            break_flag: false,
            continue_flag: false,
            return_flag: false,
            return_value: None,
            task_counter: std::sync::atomic::AtomicU64::new(1),
            effect_handler_stack: Vec::new(),
            effect_registry: HashMap::new(),
            reactive_bindings: HashMap::new(),
            flow_collectors: Vec::new(),
            actor_executor,
            runtime_trace: None,
        }
    }

    /// Attach a trace handle so runtime events are captured.
    pub fn set_trace_handle(&mut self, handle: RuntimeTraceHandle) {
        self.runtime_trace = Some(handle);
    }

    fn trace_event(&self, event: RuntimeTraceEvent) {
        if let Some(handle) = &self.runtime_trace {
            handle.record(event);
        }
    }

    fn trace_effect_outcome(
        &self,
        effect: &str,
        operation: &str,
        result: &InterpreterResult<Value>,
    ) {
        match result {
            Ok(value) => self.trace_event(RuntimeTraceEvent::EffectOperationResult {
                effect: effect.to_string(),
                operation: operation.to_string(),
                result: RuntimeTraceValue::from_value(value),
            }),
            Err(err) => self.trace_event(RuntimeTraceEvent::EffectOperationError {
                effect: effect.to_string(),
                operation: operation.to_string(),
                message: format!("{}", err),
            }),
        }
    }

    fn evaluate_arguments(&mut self, args: &[Expression]) -> InterpreterResult<Vec<Value>> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.interpret_expression(arg)?);
        }
        Ok(values)
    }

    /// Interpret a complete program
    pub fn interpret(&mut self, program: &Program) -> InterpreterResult<Value> {
        self.trace_event(RuntimeTraceEvent::ProgramStart {
            expression_count: program.expressions.len(),
        });
        let mut last_value = Value::Unit;

        for expr in &program.expressions {
            match self.interpret_expression(expr) {
                Ok(value) => {
                    last_value = value;
                }
                Err(err) => {
                    let message = format!("{}", err);
                    self.trace_event(RuntimeTraceEvent::ProgramError { message });
                    return Err(err);
                }
            }

            // Check for early return
            if self.return_flag {
                let value = self.return_value.take().unwrap_or(Value::Unit);
                self.trace_event(RuntimeTraceEvent::ProgramEnd {
                    result: RuntimeTraceValue::from_value(&value),
                });
                return Ok(value);
            }
        }

        // Execute any deferred work registered at the top level. Keep draining until empty
        loop {
            let deferred = match self
                .runtime
                .take_current_deferred()
                .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))
            {
                Ok(value) => value,
                Err(err) => {
                    let message = format!("{}", err);
                    self.trace_event(RuntimeTraceEvent::ProgramError { message });
                    return Err(err);
                }
            };
            if deferred.is_empty() {
                break;
            }
            if let Err(err) = self.run_deferred(deferred) {
                let message = format!("{}", err);
                self.trace_event(RuntimeTraceEvent::ProgramError { message });
                return Err(err);
            }
        }

        self.trace_event(RuntimeTraceEvent::ProgramEnd {
            result: RuntimeTraceValue::from_value(&last_value),
        });
        Ok(last_value)
    }

    /// Execute deferred expressions in reverse order, collecting the first error if any
    fn run_deferred(&mut self, deferred: Vec<Expression>) -> InterpreterResult<()> {
        let mut first_error: Option<InterpreterError> = None;

        let saved_break = self.break_flag;
        let saved_continue = self.continue_flag;
        let saved_return_flag = self.return_flag;
        let saved_return_value = self.return_value.take();

        self.break_flag = false;
        self.continue_flag = false;
        self.return_flag = false;
        self.return_value = None;

        for expr in deferred.into_iter().rev() {
            if let Err(err) = self.interpret_expression(&expr) {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }

        let deferred_return_flag = self.return_flag;
        let deferred_return_value = self.return_value.take();

        self.break_flag = saved_break;
        self.continue_flag = saved_continue;

        if saved_return_flag {
            self.return_flag = true;
            self.return_value = saved_return_value;
        } else if deferred_return_flag {
            self.return_flag = true;
            self.return_value = deferred_return_value;
        } else {
            self.return_flag = false;
            self.return_value = None;
        }

        if let Some(err) = first_error {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Interpret an expression
    pub fn interpret_expression(&mut self, expr: &Expression) -> InterpreterResult<Value> {
        // Check for break/continue/return flags
        if self.break_flag || self.continue_flag || self.return_flag {
            return Ok(Value::Unit);
        }

        match expr {
            // Imports are compile-time only; no runtime semantics
            Expression::Import { .. } => Ok(Value::Unit),
            // Literals
            Expression::IntegerLiteral { value, .. } => Ok(Value::Integer(*value)),
            Expression::FloatLiteral { value, .. } => Ok(Value::Float(*value)),
            Expression::StringLiteral { value, .. } => Ok(Value::String(value.clone())),
            Expression::BooleanLiteral { value, .. } => Ok(Value::Boolean(*value)),
            Expression::NullLiteral { .. } => Ok(Value::Null),

            // Identifier
            Expression::Identifier { name, pos, .. } => {
                // Check if it's a built-in function
                if self.builtins.is_builtin(name) {
                    // Return a placeholder for built-in functions
                    Ok(Value::String(format!("<builtin:{}>", name)))
                } else if let Some(field_value) = self.lookup_instance_field(name) {
                    Ok(field_value)
                } else {
                    self.runtime
                        .get_variable(name)
                        .map_err(|e| InterpreterError::runtime(e.to_string(), *pos))
                }
            }

            // Binary operations
            Expression::BinaryOp {
                left,
                op,
                right,
                pos,
            } => self.interpret_binary_op(left, op.clone(), right, *pos),

            // Unary operations
            Expression::UnaryOp { op, operand, pos } => {
                self.interpret_unary_op(op.clone(), operand, *pos)
            }

            // Control flow
            Expression::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_value = self.interpret_expression(condition)?;
                if cond_value.is_truthy() {
                    self.interpret_expression(then_branch)
                } else if let Some(else_expr) = else_branch {
                    self.interpret_expression(else_expr)
                } else {
                    Ok(Value::Null)
                }
            }

            Expression::While {
                condition, body, ..
            } => {
                let mut last_value = Value::Unit;
                loop {
                    // Check condition
                    if !self.interpret_expression(condition)?.is_truthy() {
                        break;
                    }

                    // Execute body
                    last_value = self.interpret_expression(body)?;

                    // Handle break/continue
                    if self.break_flag {
                        self.break_flag = false;
                        break;
                    }
                    if self.continue_flag {
                        self.continue_flag = false;
                        continue;
                    }
                    if self.return_flag {
                        break;
                    }
                }
                Ok(last_value)
            }

            Expression::For {
                binding,
                iterable,
                body,
                pos,
                ..
            } => self.interpret_for(binding, iterable, body, *pos),

            #[allow(unused_assignments)]
            Expression::Loop { body, .. } => {
                let mut last_value: Option<Value> = None;
                loop {
                    last_value = Some(self.interpret_expression(body)?);
                    let _ = last_value.as_ref();

                    if self.break_flag {
                        self.break_flag = false;
                        break;
                    }
                    if self.continue_flag {
                        self.continue_flag = false;
                        continue;
                    }
                    if self.return_flag {
                        break;
                    }
                }
                Ok(last_value.unwrap_or(Value::Unit))
            }

            Expression::Break { .. } => {
                self.break_flag = true;
                Ok(Value::Unit)
            }

            Expression::Continue { .. } => {
                self.continue_flag = true;
                Ok(Value::Unit)
            }

            Expression::Return { value, .. } => {
                let ret_val = if let Some(v) = value {
                    self.interpret_expression(v)?
                } else {
                    Value::Unit
                };
                self.return_flag = true;
                self.return_value = Some(ret_val.clone());
                Ok(ret_val)
            }

            // Block
            Expression::Block { expressions, .. } => self.interpret_block(expressions),

            // Variable binding
            Expression::Let { name, value, .. } => {
                let val = self.interpret_expression(value)?;
                self.runtime.define_variable(name.clone(), val.clone());
                Ok(val)
            }

            Expression::Const { name, value, .. } => {
                let val = self.interpret_expression(value)?;
                self.runtime.define_variable(name.clone(), val.clone());
                Ok(val)
            }

            // Assignment
            Expression::Assignment {
                target,
                value,
                op,
                pos,
            } => match target.as_ref() {
                Expression::Identifier { name, .. } => {
                    let rhs_val = self.interpret_expression(value)?;
                    let assigned_value = if matches!(op, AssignmentOperator::Assign) {
                        rhs_val
                    } else {
                        let current_val = if let Some(field_val) = self.lookup_instance_field(name)
                        {
                            field_val
                        } else {
                            self.runtime
                                .get_variable(name)
                                .map_err(|e| InterpreterError::runtime(e.to_string(), *pos))?
                        };
                        self.evaluate_compound_assignment(&current_val, &rhs_val, *op, *pos)?
                    };

                    if self.assign_instance_field(name, assigned_value.clone()) {
                        self.sync_reactive_binding(name, &assigned_value, *pos)?;
                        Ok(assigned_value)
                    } else {
                        self.runtime
                            .set_variable(name, assigned_value.clone())
                            .map_err(|e| InterpreterError::runtime(e.to_string(), *pos))?;
                        self.sync_reactive_binding(name, &assigned_value, *pos)?;
                        Ok(assigned_value)
                    }
                }
                _ => Err(InterpreterError::runtime("Invalid assignment target", *pos)),
            },

            Expression::Cast {
                expr,
                target_type,
                pos,
            } => self.evaluate_cast_expression(expr, target_type, *pos),

            Expression::TypeCheck {
                expr, target_type, ..
            } => self.evaluate_type_check_expression(expr, target_type),

            // Function definition
            Expression::Function {
                name, params, body, ..
            } => {
                let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                let function_value = Value::Function {
                    name: name.clone(),
                    parameters: param_names,
                    body: body.clone(),
                    closure: HashMap::new(),
                };
                self.runtime.define_variable(name.clone(), function_value);
                Ok(Value::Unit)
            }

            // Function call
            Expression::Call { callee, args, pos } => self.interpret_call(callee, args, *pos),

            // Lambda
            Expression::Lambda { params, body, .. } => {
                let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                Ok(Value::Function {
                    name: "<lambda>".to_string(),
                    parameters: param_names,
                    body: body.clone(),
                    closure: HashMap::new(),
                })
            }

            // Arrays
            Expression::ArrayLiteral { elements, .. } => {
                let mut values = Vec::new();
                for elem in elements {
                    values.push(self.interpret_expression(elem)?);
                }
                Ok(Value::array_from_vec(values))
            }

            Expression::IndexAccess { object, index, pos } => {
                let arr_val = self.interpret_expression(object)?;
                let idx_val = self.interpret_expression(index)?;

                match arr_val {
                    Value::Array(arr) => {
                        let idx = idx_val.as_integer().ok_or_else(|| {
                            InterpreterError::type_error("Array index must be an integer", *pos)
                        })? as usize;
                        let guard = arr
                            .lock()
                            .map_err(|_| InterpreterError::runtime("Array access failed", *pos))?;
                        guard.get(idx).cloned().ok_or_else(|| {
                            InterpreterError::runtime("Array index out of bounds", *pos)
                        })
                    }
                    Value::String(s) => {
                        if let Some(idx) = idx_val.as_integer() {
                            let idx = idx as usize;
                            let chars: Vec<char> = s.chars().collect();
                            if idx < chars.len() {
                                Ok(Value::Character(chars[idx]))
                            } else {
                                Err(InterpreterError::runtime(
                                    "String index out of bounds",
                                    *pos,
                                ))
                            }
                        } else {
                            Err(InterpreterError::type_error(
                                "String index must be an integer",
                                *pos,
                            ))
                        }
                    }
                    _ => Err(InterpreterError::type_error(
                        format!("Cannot index {}", arr_val.type_name()),
                        *pos,
                    )),
                }
            }

            // Class definitions
            Expression::ClassDefinition {
                name,
                generics,
                fields,
                methods,
                ..
            } => {
                self.register_class(name, generics, fields, methods)?;
                Ok(Value::Class { name: name.clone() })
            }

            // Struct/enum/type/interface definitions are compile-time only
            Expression::StructDefinition { .. }
            | Expression::EnumDefinition { .. }
            | Expression::TypeAlias { .. }
            | Expression::Interface { .. }
            | Expression::Extension { .. }
            | Expression::CompanionObject { .. } => Ok(Value::Unit),

            // Structs
            Expression::StructLiteral { name, fields, .. } => {
                let mut field_map = HashMap::new();
                for (field_name, field_expr) in fields {
                    let value = self.interpret_expression(field_expr)?;
                    field_map.insert(field_name.clone(), value);
                }
                if let Some(class_info) = self.class_registry.get(name).cloned() {
                    for runtime_field in &class_info.fields {
                        if !field_map.contains_key(&runtime_field.name) {
                            let default_value = if let Some(expr) = &runtime_field.default_value {
                                self.interpret_expression(expr)?
                            } else {
                                Value::Null
                            };
                            field_map.insert(runtime_field.name.clone(), default_value);
                        }
                    }
                }
                Ok(Value::struct_from_fields(name.clone(), field_map))
            }

            Expression::MemberAccess {
                object,
                member,
                is_safe,
                pos,
            } => {
                if let Expression::Identifier { name, .. } = object.as_ref() {
                    if let Some(effect_id) = self.effect_registry.get(name) {
                        if let Some(handler) =
                            self.lookup_effect_handler(*effect_id, member.as_str())
                        {
                            return Ok(handler);
                        }
                    }
                }

                let obj_val = self.interpret_expression(object)?;

                // Handle safe navigation
                if *is_safe && matches!(obj_val, Value::Null) {
                    return Ok(Value::Null);
                }

                match obj_val {
                    Value::Struct { fields, .. } => {
                        let guard = fields
                            .lock()
                            .map_err(|_| InterpreterError::runtime("Struct access failed", *pos))?;
                        guard.get(member).cloned().ok_or_else(|| {
                            InterpreterError::runtime(format!("Field '{}' not found", member), *pos)
                        })
                    }
                    _ => Err(InterpreterError::type_error(
                        format!("Cannot access field on {}", obj_val.type_name()),
                        *pos,
                    )),
                }
            }

            // Pattern matching
            Expression::Match { expr, arms, .. } => self.interpret_match(expr, arms),

            // String interpolation
            Expression::InterpolatedString { parts, .. } => {
                self.interpret_interpolated_string(parts)
            }

            Expression::Elvis {
                nullable, default, ..
            } => {
                let val = self.interpret_expression(nullable)?;
                if matches!(val, Value::Null) {
                    self.interpret_expression(default)
                } else {
                    Ok(val)
                }
            }

            Expression::ForceUnwrap { nullable, pos } => {
                let val = self.interpret_expression(nullable)?;
                if matches!(val, Value::Null) {
                    Err(InterpreterError::runtime("Unwrapped null value", *pos))
                } else {
                    Ok(val)
                }
            }

            // Async/Await expressions
            Expression::Await { expr, pos } => self.interpret_await(expr, *pos),

            // Spawn expressions for concurrency
            Expression::Spawn {
                expr,
                detached,
                pos,
            } => self.interpret_spawn(expr, *detached, *pos),

            Expression::Scope { body, pos } => self.interpret_scope(body, *pos),
            Expression::JobsScope { body, pos } => self.interpret_jobs_scope(body, *pos),

            Expression::Cancel { task, pos } => self.interpret_cancel(task, *pos),

            Expression::ParallelFor {
                binding,
                iterable,
                body,
                pos,
            } => self.interpret_parallel_for(binding, iterable, body, *pos),

            // Select expressions for channel operations
            Expression::Select { cases, pos } => self.interpret_select(cases, *pos),

            // Actor definitions
            Expression::Actor {
                name,
                fields,
                handlers,
                pos,
            } => self.interpret_actor_definition(name, fields, handlers, *pos),

            // Send expressions
            Expression::Send {
                target,
                message,
                pos,
            } => self.interpret_send(target, message, *pos),

            Expression::Request {
                message,
                source,
                pos,
            } => self.interpret_request(message, source, *pos),

            // Receive expressions
            Expression::Receive {
                pattern: _,
                handler,
                pos: _,
            } => {
                // Simplified receive implementation
                self.interpret_expression(handler)
            }

            // Effect definition
            Expression::Effect {
                name,
                operations,
                pos,
            } => self.interpret_effect_definition(name, operations, *pos),

            // Handle expression for effects
            Expression::Handle {
                body,
                effect,
                handlers,
                pos,
            } => self.interpret_handle(body, effect, handlers, *pos),

            // Contract-annotated function
            Expression::ContractedFunction {
                function,
                requires,
                ensures,
                invariants,
                pos,
            } => self.interpret_contracted_function(function, requires, ensures, invariants, *pos),

            // Observable creation (Seen syntax: Observable.Range(1, 10))
            Expression::ObservableCreation { source, pos } => {
                self.interpret_observable_creation(source, *pos)
            }

            // Flow creation (Seen syntax: flow { emit(1); delay(100ms) })
            Expression::FlowCreation { body, pos } => self.interpret_flow_creation(body, *pos),

            // Reactive property (Seen syntax: @Reactive var Username = "")
            Expression::ReactiveProperty {
                name,
                value,
                is_computed,
                pos,
            } => self.interpret_reactive_property(name, value, *is_computed, *pos),

            // Stream operations (Map, Filter, etc.)
            Expression::StreamOperation {
                stream,
                operation,
                pos,
            } => self.interpret_stream_operation(stream, operation, *pos),

            Expression::Assert {
                condition,
                message,
                pos,
            } => {
                let cond_val = self.interpret_expression(condition)?;
                if !cond_val.is_truthy() {
                    let msg = message.as_deref().unwrap_or("Assertion failed");
                    return Err(InterpreterError::runtime(msg, *pos));
                }
                Ok(Value::Unit)
            }

            Expression::Defer { body, pos } => {
                let deferred_expr = (**body).clone();
                self.runtime
                    .register_defer(deferred_expr)
                    .map_err(|e| InterpreterError::runtime(e.to_string(), *pos))?;
                Ok(Value::Unit)
            }

            // Unhandled expressions - provide meaningful error messages
            _ => Err(InterpreterError::runtime(
                &format!("Expression type not yet implemented: {:?}", expr),
                Position::new(0, 0, 0),
            )),
        }
    }

    /// Interpret a binary operation
    fn interpret_binary_op(
        &mut self,
        left: &Expression,
        op: BinaryOperator,
        right: &Expression,
        pos: Position,
    ) -> InterpreterResult<Value> {
        // Short-circuit evaluation for logical operators
        if matches!(op, BinaryOperator::And | BinaryOperator::Or) {
            let left_val = self.interpret_expression(left)?;
            let left_bool = left_val.is_truthy();

            match op {
                BinaryOperator::And => {
                    if !left_bool {
                        return Ok(Value::Boolean(false));
                    }
                    let right_val = self.interpret_expression(right)?;
                    Ok(Value::Boolean(right_val.is_truthy()))
                }
                BinaryOperator::Or => {
                    if left_bool {
                        return Ok(Value::Boolean(true));
                    }
                    let right_val = self.interpret_expression(right)?;
                    Ok(Value::Boolean(right_val.is_truthy()))
                }
                _ => unreachable!(),
            }
        } else {
            let left_val = self.interpret_expression(left)?;
            let right_val = self.interpret_expression(right)?;

            match op {
                BinaryOperator::Add => left_val
                    .add(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::Subtract => left_val
                    .subtract(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::Multiply => left_val
                    .multiply(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::Divide => left_val
                    .divide(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::Modulo => match (&left_val, &right_val) {
                    (Value::Integer(a), Value::Integer(b)) if *b != 0 => Ok(Value::Integer(a % b)),
                    (Value::Integer(_), Value::Integer(0)) => {
                        Err(InterpreterError::division_by_zero(pos))
                    }
                    _ => Err(InterpreterError::type_error(
                        "Modulo requires integer operands",
                        pos,
                    )),
                },
                BinaryOperator::Equal => Ok(Value::Boolean(left_val.equals(&right_val))),
                BinaryOperator::NotEqual => Ok(Value::Boolean(!left_val.equals(&right_val))),
                BinaryOperator::Less => left_val
                    .less_than(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::LessEqual => match left_val.less_than(&right_val) {
                    Ok(Value::Boolean(lt)) => Ok(Value::Boolean(lt || left_val.equals(&right_val))),
                    Ok(_) => Err(InterpreterError::runtime("Invalid comparison result", pos)),
                    Err(e) => Err(InterpreterError::runtime(e, pos)),
                },
                BinaryOperator::Greater => right_val
                    .less_than(&left_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::GreaterEqual => match right_val.less_than(&left_val) {
                    Ok(Value::Boolean(gt)) => Ok(Value::Boolean(gt || left_val.equals(&right_val))),
                    Ok(_) => Err(InterpreterError::runtime("Invalid comparison result", pos)),
                    Err(e) => Err(InterpreterError::runtime(e, pos)),
                },
                BinaryOperator::BitwiseAnd => left_val
                    .bitwise_and(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::BitwiseOr => left_val
                    .bitwise_or(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::BitwiseXor => left_val
                    .bitwise_xor(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::LeftShift => left_val
                    .left_shift(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::RightShift => left_val
                    .right_shift(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                _ => Ok(Value::Unit), // Other operators not implemented yet
            }
        }
    }

    fn evaluate_cast_expression(
        &mut self,
        expr: &Expression,
        target_type: &AstType,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let value = self.interpret_expression(expr)?;
        if Self::value_matches_ast_type(&value, target_type) {
            Ok(value)
        } else {
            Err(InterpreterError::runtime(
                format!(
                    "Cannot cast value of type {} to {}",
                    Self::runtime_value_name(&value),
                    Self::format_ast_type(target_type)
                ),
                pos,
            ))
        }
    }

    fn evaluate_type_check_expression(
        &mut self,
        expr: &Expression,
        target_type: &AstType,
    ) -> InterpreterResult<Value> {
        let value = self.interpret_expression(expr)?;
        Ok(Value::Boolean(Self::value_matches_ast_type(
            &value,
            target_type,
        )))
    }

    fn value_matches_ast_type(value: &Value, target_type: &AstType) -> bool {
        if target_type.is_nullable && matches!(value, Value::Null) {
            return true;
        }
        if matches!(value, Value::Null) {
            return target_type.name == "Null";
        }
        Self::value_matches_nonnullable_type(value, target_type)
    }

    fn value_matches_nonnullable_type(value: &Value, target_type: &AstType) -> bool {
        match target_type.name.as_str() {
            "Any" => true,
            "Int" => matches!(value, Value::Integer(_)),
            "UInt" => matches!(value, Value::Integer(_)),
            "Float" => matches!(value, Value::Float(_)),
            "Bool" => matches!(value, Value::Boolean(_)),
            "String" => matches!(value, Value::String(_)),
            "Char" => matches!(value, Value::Character(_)),
            "Array" | "List" | "Vec" => matches!(value, Value::Array(_)),
            "Bytes" => matches!(value, Value::Bytes(_)),
            "Promise" => matches!(value, Value::Promise(_)),
            "Task" => matches!(value, Value::Task(_)),
            "Channel" => matches!(value, Value::Channel(_)),
            "Actor" => matches!(value, Value::Actor(_)),
            "Effect" => matches!(value, Value::Effect(_)),
            "EffectHandle" => matches!(value, Value::EffectHandle { .. }),
            "Observable" => matches!(value, Value::Observable(_)),
            "Flow" => matches!(value, Value::Flow(_)),
            "ReactiveProperty" => matches!(value, Value::ReactiveProperty { .. }),
            "Unit" => matches!(value, Value::Unit),
            "Null" => matches!(value, Value::Null),
            _ => match value {
                Value::Struct { name, .. } => name == &target_type.name,
                Value::Class { name } => name == &target_type.name,
                _ => false,
            },
        }
    }

    fn runtime_value_name(value: &Value) -> String {
        match value {
            Value::Integer(_) => "Int".to_string(),
            Value::Float(_) => "Float".to_string(),
            Value::Boolean(_) => "Bool".to_string(),
            Value::String(_) => "String".to_string(),
            Value::Character(_) => "Char".to_string(),
            Value::Array(_) => "Array".to_string(),
            Value::Bytes(_) => "Bytes".to_string(),
            Value::Struct { name, .. } => name.clone(),
            Value::Class { name } => name.clone(),
            Value::Null => "Null".to_string(),
            Value::Unit => "Unit".to_string(),
            Value::Function { .. } => "Function".to_string(),
            Value::Promise(_) => "Promise".to_string(),
            Value::Task(_) => "Task".to_string(),
            Value::Channel(_) => "Channel".to_string(),
            Value::Actor(_) => "Actor".to_string(),
            Value::Effect(_) => "Effect".to_string(),
            Value::EffectHandle { .. } => "EffectHandle".to_string(),
            Value::Observable(_) => "Observable".to_string(),
            Value::Flow(_) => "Flow".to_string(),
            Value::ReactiveProperty { .. } => "ReactiveProperty".to_string(),
        }
    }

    fn format_ast_type(ast_type: &AstType) -> String {
        let mut base = ast_type.name.clone();
        if !ast_type.generics.is_empty() {
            let generics = ast_type
                .generics
                .iter()
                .map(Self::format_ast_type)
                .collect::<Vec<_>>()
                .join(", ");
            base = format!("{base}<{generics}>");
        }
        if ast_type.is_nullable {
            base.push('?');
        }
        base
    }

    fn evaluate_compound_assignment(
        &self,
        current: &Value,
        rhs: &Value,
        op: AssignmentOperator,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let result = match op {
            AssignmentOperator::Assign => return Ok(rhs.clone()),
            AssignmentOperator::AddAssign => current.add(rhs),
            AssignmentOperator::SubAssign => current.subtract(rhs),
            AssignmentOperator::MulAssign => current.multiply(rhs),
            AssignmentOperator::DivAssign => current.divide(rhs),
            AssignmentOperator::ModAssign => match (current, rhs) {
                (Value::Integer(a), Value::Integer(b)) if *b != 0 => Ok(Value::Integer(*a % *b)),
                (Value::Integer(_), Value::Integer(0)) => {
                    Err("Modulo by zero in compound assignment".to_string())
                }
                _ => Err(format!(
                    "Cannot apply %= on {} and {}",
                    current.type_name(),
                    rhs.type_name()
                )),
            },
        };

        result.map_err(|e| InterpreterError::runtime(e, pos))
    }

    /// Interpret a unary operation
    fn interpret_unary_op(
        &mut self,
        op: UnaryOperator,
        operand: &Expression,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let val = self.interpret_expression(operand)?;

        match op {
            UnaryOperator::Negate => val.negate().map_err(|e| InterpreterError::runtime(e, pos)),
            UnaryOperator::Not => Ok(val.logical_not()),
            UnaryOperator::BitwiseNot => val
                .bitwise_not()
                .map_err(|e| InterpreterError::runtime(e, pos)),
        }
    }

    /// Interpret a for loop
    fn interpret_for(
        &mut self,
        binding: &ForBinding,
        iterable: &Expression,
        body: &Expression,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let iter_val = self.interpret_expression(iterable)?;

        // Push new scope for loop variables
        self.runtime.push_environment(false);

        let result = match iter_val {
            Value::Array(arr) => {
                let items = arr
                    .lock()
                    .map_err(|_| InterpreterError::runtime("Array access failed", pos))?
                    .clone();
                let mut last_value = Value::Unit;
                for item in items {
                    self.bind_for_binding(binding, item, pos)?;
                    last_value = self.interpret_expression(body)?;

                    if self.break_flag {
                        self.break_flag = false;
                        break;
                    }
                    if self.continue_flag {
                        self.continue_flag = false;
                        continue;
                    }
                    if self.return_flag {
                        break;
                    }
                }
                Ok(last_value)
            }
            Value::String(s) => {
                let mut last_value = Value::Unit;
                for ch in s.chars() {
                    self.bind_for_binding(binding, Value::Character(ch), pos)?;
                    last_value = self.interpret_expression(body)?;

                    if self.break_flag {
                        self.break_flag = false;
                        break;
                    }
                    if self.continue_flag {
                        self.continue_flag = false;
                        continue;
                    }
                    if self.return_flag {
                        break;
                    }
                }
                Ok(last_value)
            }
            _ => Err(InterpreterError::type_error(
                format!("Cannot iterate over {}", iter_val.type_name()),
                Position::start(),
            )),
        };

        let deferred = self
            .runtime
            .take_current_deferred()
            .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))?;
        let defer_result = self.run_deferred(deferred);

        // Pop loop scope
        let pop_result = self
            .runtime
            .pop_environment()
            .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()));
        if let Err(err) = pop_result {
            return Err(err);
        }

        match result {
            Ok(value) => {
                if let Err(err) = defer_result {
                    Err(err)
                } else {
                    Ok(value)
                }
            }
            Err(err) => Err(err),
        }
    }

    fn bind_for_binding(
        &mut self,
        binding: &ForBinding,
        value: Value,
        pos: Position,
    ) -> InterpreterResult<()> {
        match binding {
            ForBinding::Identifier(name) => {
                self.runtime.define_variable(name.clone(), value);
                Ok(())
            }
            ForBinding::Tuple(names) => match value {
                Value::Array(arr) => {
                    let items = arr
                        .lock()
                        .map_err(|_| InterpreterError::runtime("Array access failed", pos))?
                        .clone();
                    if items.len() < names.len() {
                        return Err(InterpreterError::runtime(
                            "Not enough elements to destructure tuple in for-loop",
                            pos,
                        ));
                    }
                    for (idx, name) in names.iter().enumerate() {
                        self.runtime
                            .define_variable(name.clone(), items[idx].clone());
                    }
                    Ok(())
                }
                other => Err(InterpreterError::type_error(
                    format!(
                        "Tuple destructuring in for-loops requires array entries, found {}",
                        other.type_name()
                    ),
                    pos,
                )),
            },
        }
    }

    /// Interpret a block expression
    fn interpret_block(&mut self, expressions: &[Expression]) -> InterpreterResult<Value> {
        self.runtime.push_environment(false);

        let mut last_value = Value::Unit;
        let mut block_error: Option<InterpreterError> = None;
        for expr in expressions {
            match self.interpret_expression(expr) {
                Ok(val) => last_value = val,
                Err(err) => {
                    block_error = Some(err);
                    break;
                }
            }

            // Check for early exit
            if self.break_flag || self.continue_flag || self.return_flag {
                break;
            }
        }

        let deferred = self
            .runtime
            .take_current_deferred()
            .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))?;
        if let Err(err) = self.run_deferred(deferred) {
            if block_error.is_none() {
                block_error = Some(err);
            }
        }

        if let Err(err) = self
            .runtime
            .pop_environment()
            .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))
        {
            if block_error.is_none() {
                block_error = Some(err);
            }
        }

        if let Some(err) = block_error {
            Err(err)
        } else {
            Ok(last_value)
        }
    }

    /// Interpret a function/method call
    fn interpret_call(
        &mut self,
        callee: &Expression,
        args: &[Expression],
        pos: Position,
    ) -> InterpreterResult<Value> {
        if let Expression::MemberAccess {
            object,
            member,
            pos: member_pos,
            ..
        } = callee
        {
            if let Expression::Identifier { name, .. } = object.as_ref() {
                if let Some(effect_id) = self.effect_registry.get(name).cloned() {
                    let effect_name = name.clone();
                    let operation_name = member.clone();
                    if let Some(handler) =
                        self.lookup_effect_handler(effect_id, operation_name.as_str())
                    {
                        let evaluated_args = self.evaluate_arguments(args)?;
                        let arg_trace = evaluated_args
                            .iter()
                            .map(RuntimeTraceValue::from_value)
                            .collect();
                        self.trace_event(RuntimeTraceEvent::EffectOperationInvoke {
                            effect: effect_name.clone(),
                            operation: operation_name.clone(),
                            arguments: arg_trace,
                            via_handler: true,
                        });
                        let result =
                            self.call_function_value(handler, args, Some(evaluated_args), pos);
                        self.trace_effect_outcome(&effect_name, &operation_name, &result);
                        return result;
                    }

                    let arg_values = self.evaluate_arguments(args)?;
                    let arg_trace = arg_values
                        .iter()
                        .map(RuntimeTraceValue::from_value)
                        .collect();
                    self.trace_event(RuntimeTraceEvent::EffectOperationInvoke {
                        effect: effect_name.clone(),
                        operation: operation_name.clone(),
                        arguments: arg_trace,
                        via_handler: false,
                    });
                    let params: Vec<EffectAsyncValue> = arg_values
                        .iter()
                        .map(|v| self.value_to_effect_async_value(v))
                        .collect();

                    let advanced = self.runtime.advanced_runtime();
                    let mut advanced = advanced.lock().map_err(|_| {
                        InterpreterError::runtime("Advanced runtime unavailable", pos)
                    })?;
                    match advanced
                        .effect_system
                        .call_effect(effect_id, member, params, pos)
                    {
                        Ok(result) => {
                            let value = self.effect_async_value_to_value(&result);
                            self.trace_event(RuntimeTraceEvent::EffectOperationResult {
                                effect: effect_name,
                                operation: operation_name,
                                result: RuntimeTraceValue::from_value(&value),
                            });
                            return Ok(value);
                        }
                        Err(err) => {
                            let message = err.to_string();
                            self.trace_event(RuntimeTraceEvent::EffectOperationError {
                                effect: effect_name,
                                operation: operation_name,
                                message: message.clone(),
                            });
                            return Err(InterpreterError::runtime(message, pos));
                        }
                    }
                }
            }

            let receiver_value = self.interpret_expression(object)?;
            if let Some(result) =
                self.try_dispatch_builtin_member(&receiver_value, member, args, pos)?
            {
                return Ok(result);
            }

            if let Value::Struct { name, .. } = &receiver_value {
                return self.invoke_class_method(
                    name,
                    member,
                    Some(receiver_value.clone()),
                    args,
                    *member_pos,
                );
            }

            if let Value::Class { name } = &receiver_value {
                return self.invoke_class_method(name, member, None, args, *member_pos);
            }

            if self.builtins.is_builtin(member) {
                let mut arg_values = vec![receiver_value];
                for arg in args {
                    arg_values.push(self.interpret_expression(arg)?);
                }
                return self.builtins.call(member, &arg_values, pos);
            }
        }
        // Check if it's a built-in function call
        if let Expression::Identifier { name, .. } = callee {
            if name == "emit" {
                if !self.flow_collectors.is_empty() {
                    if args.len() != 1 {
                        return Err(InterpreterError::argument_count_mismatch(
                            "emit".to_string(),
                            1,
                            args.len(),
                            pos,
                        ));
                    }
                    let value = self.interpret_expression(&args[0])?;
                    if let Some(collector) = self.flow_collectors.last_mut() {
                        collector.push(value);
                    }
                    return Ok(Value::Unit);
                }
            } else if name == "delay" {
                if self.flow_collectors.last().is_some() {
                    for arg in args {
                        let _ = self.interpret_expression(arg)?;
                    }
                    return Ok(Value::Unit);
                }
            }

            if self.builtins.is_builtin(name) {
                let mut arg_values = Vec::new();
                for arg in args {
                    arg_values.push(self.interpret_expression(arg)?);
                }
                return self.builtins.call(name, &arg_values, pos);
            }
        }

        // Evaluate the callee expression
        let func_val = self.interpret_expression(callee)?;

        self.call_function_value(func_val, args, None, pos)
    }

    fn call_function_value(
        &mut self,
        func_val: Value,
        args: &[Expression],
        pre_evaluated_args: Option<Vec<Value>>,
        pos: Position,
    ) -> InterpreterResult<Value> {
        if let Value::Function {
            name,
            parameters,
            body,
            ..
        } = func_val
        {
            if args.len() != parameters.len() {
                return Err(InterpreterError::argument_count_mismatch(
                    name.clone(),
                    parameters.len(),
                    args.len(),
                    pos,
                ));
            }

            let mut arg_values = if let Some(values) = pre_evaluated_args {
                values
            } else {
                self.evaluate_arguments(args)?
            };
            let arg_summaries: Vec<RuntimeTraceValue> = arg_values
                .iter()
                .map(RuntimeTraceValue::from_value)
                .collect();
            self.trace_event(RuntimeTraceEvent::FunctionEnter {
                name: name.clone(),
                arguments: arg_summaries,
            });

            self.runtime.push_environment(true);

            for (param, value) in parameters.iter().zip(arg_values.drain(..)) {
                self.runtime.define_variable(param.clone(), value);
            }

            let prev_break = self.break_flag;
            let prev_continue = self.continue_flag;
            self.break_flag = false;
            self.continue_flag = false;

            let result = self.interpret_expression(&body);

            let deferred = self
                .runtime
                .take_current_deferred()
                .map_err(|e| InterpreterError::runtime(e.to_string(), pos.clone()))?;
            let defer_result = self.run_deferred(deferred);

            let return_value = if self.return_flag {
                self.return_flag = false;
                self.return_value.take()
            } else {
                None
            };

            self.break_flag = prev_break;
            self.continue_flag = prev_continue;

            self.runtime
                .pop_environment()
                .map_err(|e| InterpreterError::runtime(e.to_string(), pos))?;

            let final_result = match result {
                Ok(val) => {
                    if let Err(err) = defer_result {
                        Err(err)
                    } else {
                        Ok(return_value.unwrap_or(val))
                    }
                }
                Err(e) => Err(e),
            };

            match final_result {
                Ok(value) => {
                    self.trace_event(RuntimeTraceEvent::FunctionExit {
                        name: name.clone(),
                        result: RuntimeTraceValue::from_value(&value),
                    });
                    Ok(value)
                }
                Err(err) => {
                    let message = format!("{}", err);
                    self.trace_event(RuntimeTraceEvent::FunctionError {
                        name: name.clone(),
                        message,
                    });
                    Err(err)
                }
            }
        } else {
            Err(InterpreterError::type_error(
                format!("Cannot call {}", func_val.type_name()),
                pos,
            ))
        }
    }

    /// Interpret pattern matching
    fn interpret_match(
        &mut self,
        value: &Expression,
        arms: &[MatchArm],
    ) -> InterpreterResult<Value> {
        let val = self.interpret_expression(value)?;

        for arm in arms {
            if self.pattern_matches(&val, &arm.pattern) {
                return self.interpret_expression(&arm.body);
            }
        }

        Err(InterpreterError::runtime(
            "No matching pattern",
            Position::start(),
        ))
    }

    fn lookup_effect_handler(&self, effect_id: EffectId, operation: &str) -> Option<Value> {
        self.effect_handler_stack
            .iter()
            .rev()
            .find(|frame| frame.effect_id == effect_id)
            .and_then(|frame| frame.handlers.get(operation).cloned())
    }

    /// Check if a pattern matches a value
    fn pattern_matches(&self, value: &Value, pattern: &Pattern) -> bool {
        match pattern {
            Pattern::Wildcard => true,
            Pattern::Identifier(_) => true, // Binds to any value
            Pattern::Literal(expr) => {
                // Compare literal expression value
                if let Ok(mut temp_interp) = std::panic::catch_unwind(|| Interpreter::new()) {
                    if let Ok(pattern_val) = temp_interp.interpret_expression(expr) {
                        return value.equals(&pattern_val);
                    }
                }
                false
            }
            _ => false, // Other patterns not implemented yet
        }
    }

    /// Interpret an interpolated string
    fn interpret_interpolated_string(
        &mut self,
        parts: &[InterpolationPart],
    ) -> InterpreterResult<Value> {
        let mut result = String::new();

        for part in parts {
            match &part.kind {
                InterpolationKind::Text(text) => result.push_str(text),
                InterpolationKind::Expression(expr) => {
                    // Evaluate the interpolated expression
                    let value = self.interpret_expression(expr)?;
                    result.push_str(&value.to_string());
                }
            }
        }

        Ok(Value::String(result))
    }

    /// Interpret await expression
    fn interpret_await(&mut self, expr: &Expression, pos: Position) -> InterpreterResult<Value> {
        // First evaluate the expression to get a promise/task
        let promise_value = self.interpret_expression(expr)?;

        match promise_value {
            Value::Promise(promise) => {
                if let Some(value) = promise.value() {
                    return Ok(self.async_value_to_value(&value));
                }

                if promise.is_rejected() {
                    let reason = promise
                        .get_error()
                        .unwrap_or_else(|| "Promise rejected".to_string());
                    return Err(InterpreterError::runtime(reason, pos));
                }

                if promise.is_pending() && promise.task_id() == TaskId::placeholder() {
                    let actor_system = self.runtime.actor_system();
                    if let Ok(mut system) = actor_system.lock() {
                        let _ = system.process_messages();
                    }
                    if let Some(value) = promise.value() {
                        return Ok(self.async_value_to_value(&value));
                    }
                    if promise.is_rejected() {
                        let reason = promise
                            .get_error()
                            .unwrap_or_else(|| "Promise rejected".to_string());
                        return Err(InterpreterError::runtime(reason, pos));
                    }
                    return Err(InterpreterError::runtime("Promise is still pending", pos));
                }

                let async_runtime = self.runtime.async_runtime();
                let mut runtime = async_runtime.lock().map_err(|_| {
                    InterpreterError::runtime("Failed to acquire async runtime lock", pos.clone())
                })?;

                let task_id = promise.task_id();
                let async_result = runtime.wait_for_task(task_id).map_err(|e| {
                    InterpreterError::runtime(
                        format!("Async execution failed: {:?}", e),
                        pos.clone(),
                    )
                })?;

                Ok(self.async_value_to_value(&async_result))
            }
            Value::Task(task_id) => {
                let async_runtime = self.runtime.async_runtime();
                let mut runtime = async_runtime.lock().map_err(|_| {
                    InterpreterError::runtime("Failed to acquire async runtime lock", pos.clone())
                })?;

                let async_result = runtime.wait_for_task(task_id).map_err(|e| {
                    InterpreterError::runtime(
                        format!("Task execution failed: {:?}", e),
                        pos.clone(),
                    )
                })?;

                Ok(self.async_value_to_value(&async_result))
            }
            _ => Err(InterpreterError::runtime(
                "Cannot await non-promise value",
                pos,
            )),
        }
    }

    /// Interpret spawn expression
    fn interpret_spawn(
        &mut self,
        expr: &Expression,
        detached: bool,
        pos: Position,
    ) -> InterpreterResult<Value> {
        // Get async runtime
        let async_runtime = self.runtime.async_runtime();
        let mut runtime = async_runtime.lock().unwrap();

        // Capture the current lexical environment for the spawned task
        let captured_env = self.runtime.snapshot_environment();

        // Create async function wrapper for the expression
        let async_function = Box::new(SeenAsyncFunction {
            expression: expr.clone(),
            position: pos.clone(),
            environment: captured_env,
        });

        // Spawn the task with normal priority
        let task_handle = runtime.spawn_task(async_function, TaskPriority::Normal);

        match task_handle.task_id() {
            Some(id) => {
                if !detached {
                    self.runtime.register_scope_task(id);
                }
                Ok(Value::Task(id))
            }
            None => {
                if let Some(error) = task_handle.get_error() {
                    Err(InterpreterError::runtime(
                        &format!("Failed to spawn task: {:?}", error),
                        pos,
                    ))
                } else {
                    Err(InterpreterError::runtime(
                        "Failed to spawn task: unknown error",
                        pos,
                    ))
                }
            }
        }
    }

    fn interpret_scope(&mut self, body: &Expression, pos: Position) -> InterpreterResult<Value> {
        self.runtime.push_task_scope();
        let evaluation = self.interpret_expression(body);
        let tasks = match self.runtime.pop_task_scope() {
            Ok(tasks) => tasks,
            Err(err) => {
                return Err(InterpreterError::runtime(err.to_string(), pos));
            }
        };
        let join_result = self.join_scope_tasks(tasks, pos.clone());

        match (evaluation, join_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(err), _) => Err(err),
            (Ok(_), Err(join_err)) => Err(join_err),
        }
    }

    fn interpret_jobs_scope(
        &mut self,
        body: &Expression,
        pos: Position,
    ) -> InterpreterResult<Value> {
        // Jobs scope currently mirrors task scope semantics. When the job system
        // supports explicit job handles we can extend this path to gather and wait
        // on outstanding jobs before unwinding the scope.
        self.interpret_scope(body, pos)
    }

    fn join_scope_tasks(&mut self, tasks: Vec<TaskId>, pos: Position) -> InterpreterResult<()> {
        if tasks.is_empty() {
            return Ok(());
        }

        let async_runtime = self.runtime.async_runtime();
        let mut runtime = async_runtime.lock().map_err(|_| {
            InterpreterError::runtime("Failed to acquire async runtime lock", pos.clone())
        })?;

        for task_id in tasks {
            match runtime.wait_for_task(task_id) {
                Ok(async_value) => {
                    // Convert result to interpreter value to surface errors if needed
                    let _ = self.async_value_to_value(&async_value);
                }
                Err(err) => {
                    return Err(InterpreterError::runtime(
                        format!("Task {:?} failed: {:?}", task_id.id(), err),
                        pos.clone(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn interpret_cancel(
        &mut self,
        task_expr: &Expression,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let task_value = self.interpret_expression(task_expr)?;
        match task_value {
            Value::Task(task_id) => {
                let cancelled = self
                    .runtime
                    .cancel_task(task_id)
                    .map_err(|err| InterpreterError::runtime(err.to_string(), pos))?;
                Ok(Value::Boolean(cancelled))
            }
            _ => Err(InterpreterError::runtime(
                "cancel expects a Task handle",
                pos,
            )),
        }
    }

    fn try_dispatch_builtin_member(
        &mut self,
        receiver: &Value,
        member: &str,
        args: &[Expression],
        pos: Position,
    ) -> InterpreterResult<Option<Value>> {
        match (member, receiver) {
            ("length", Value::String(s)) | ("size", Value::String(s)) if args.is_empty() => {
                Ok(Some(Value::Integer(s.chars().count() as i64)))
            }
            ("length", Value::Array(arr)) | ("size", Value::Array(arr)) if args.is_empty() => {
                let len = arr
                    .lock()
                    .map_err(|_| InterpreterError::runtime("Array access failed", pos))?
                    .len();
                Ok(Some(Value::Integer(len as i64)))
            }
            ("push", Value::Array(arr)) if args.len() == 1 => {
                let value = self.interpret_expression(&args[0])?;
                let mut guard = arr
                    .lock()
                    .map_err(|_| InterpreterError::runtime("Array access failed", pos))?;
                guard.push(value);
                Ok(Some(Value::Array(Arc::clone(arr))))
            }
            ("pop", Value::Array(arr)) if args.is_empty() => {
                let mut guard = arr
                    .lock()
                    .map_err(|_| InterpreterError::runtime("Array access failed", pos))?;
                if !guard.is_empty() {
                    guard.pop();
                }
                Ok(Some(Value::Array(Arc::clone(arr))))
            }
            ("slice", Value::Array(arr)) if args.len() == 2 => {
                let start = self
                    .interpret_expression(&args[0])?
                    .as_integer()
                    .ok_or_else(|| InterpreterError::type_error("slice start must be int", pos))?
                    as usize;
                let end = self
                    .interpret_expression(&args[1])?
                    .as_integer()
                    .ok_or_else(|| InterpreterError::type_error("slice end must be int", pos))?
                    as usize;
                let guard = arr
                    .lock()
                    .map_err(|_| InterpreterError::runtime("Array access failed", pos))?;
                if start > end || end > guard.len() {
                    return Err(InterpreterError::runtime("slice out of bounds", pos));
                }
                let slice = guard[start..end].to_vec();
                Ok(Some(Value::array_from_vec(slice)))
            }
            ("endsWith", Value::String(s)) if args.len() == 1 => {
                let suffix = self.interpret_expression(&args[0])?.to_string();
                Ok(Some(Value::Boolean(s.ends_with(&suffix))))
            }
            ("substring", Value::String(s)) if args.len() == 2 => {
                let start = self
                    .interpret_expression(&args[0])?
                    .as_integer()
                    .ok_or_else(|| {
                        InterpreterError::type_error("substring start must be int", pos)
                    })? as usize;
                let end = self
                    .interpret_expression(&args[1])?
                    .as_integer()
                    .ok_or_else(|| InterpreterError::type_error("substring end must be int", pos))?
                    as usize;
                let chars: Vec<char> = s.chars().collect();
                if start > end || end > chars.len() {
                    return Err(InterpreterError::runtime("substring out of bounds", pos));
                }
                let sub: String = chars[start..end].iter().collect();
                Ok(Some(Value::String(sub)))
            }
            _ => Ok(None),
        }
    }
    fn interpret_parallel_for(
        &mut self,
        binding: &str,
        iterable: &Expression,
        body: &Expression,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let iter_val = self.interpret_expression(iterable)?;

        self.runtime.push_environment(false);

        let mut result = Ok(Value::Unit);

        match iter_val {
            Value::Array(items) => {
                let items_vec = items
                    .lock()
                    .map_err(|_| InterpreterError::runtime("Array access failed", pos.clone()))?
                    .clone();
                let items = Arc::new(items_vec);
                let job_system = self.runtime.job_system();
                let mut job_error: Option<InterpreterError> = None;
                let mut first_iteration = true;

                let items_for_closure = Arc::clone(&items);
                job_system.parallel_for_sequential(0, items_for_closure.len(), |idx| {
                    if job_error.is_some() || self.return_flag {
                        return;
                    }

                    let item = items_for_closure[idx].clone();

                    if first_iteration {
                        self.runtime.define_variable(binding.to_string(), item);
                        first_iteration = false;
                    } else if let Err(_) = self.runtime.set_variable(binding, item.clone()) {
                        self.runtime.define_variable(binding.to_string(), item);
                    }

                    if let Err(err) = self.interpret_expression(body) {
                        job_error = Some(err);
                        return;
                    }

                    if self.break_flag {
                        job_error = Some(InterpreterError::runtime(
                            "break is not supported inside parallel_for",
                            pos.clone(),
                        ));
                        self.break_flag = false;
                    }

                    if self.continue_flag {
                        job_error = Some(InterpreterError::runtime(
                            "continue is not supported inside parallel_for",
                            pos.clone(),
                        ));
                        self.continue_flag = false;
                    }
                });

                if let Some(err) = job_error {
                    result = Err(err);
                }
            }
            Value::String(s) => {
                let job_system = self.runtime.job_system();
                let chars: Vec<Value> = s.chars().map(Value::Character).collect();
                let mut job_error: Option<InterpreterError> = None;
                let mut first_iteration = true;

                job_system.parallel_for_sequential(0, chars.len(), |idx| {
                    if job_error.is_some() || self.return_flag {
                        return;
                    }

                    let ch = chars[idx].clone();

                    if first_iteration {
                        self.runtime.define_variable(binding.to_string(), ch);
                        first_iteration = false;
                    } else if let Err(_) = self.runtime.set_variable(binding, ch.clone()) {
                        self.runtime.define_variable(binding.to_string(), ch);
                    }

                    if let Err(err) = self.interpret_expression(body) {
                        job_error = Some(err);
                        return;
                    }

                    if self.break_flag {
                        job_error = Some(InterpreterError::runtime(
                            "break is not supported inside parallel_for",
                            pos.clone(),
                        ));
                        self.break_flag = false;
                    }

                    if self.continue_flag {
                        job_error = Some(InterpreterError::runtime(
                            "continue is not supported inside parallel_for",
                            pos.clone(),
                        ));
                        self.continue_flag = false;
                    }
                });

                if let Some(err) = job_error {
                    result = Err(err);
                }
            }
            other => {
                result = Err(InterpreterError::runtime(
                    format!("Cannot parallel_for over {}", other.type_name()),
                    pos,
                ));
            }
        }

        let deferred = self
            .runtime
            .take_current_deferred()
            .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))?;
        let defer_result = self.run_deferred(deferred);

        if let Err(err) = self
            .runtime
            .pop_environment()
            .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))
        {
            return Err(err);
        }

        match (result, defer_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(err), _) => Err(err),
            (_, Err(err)) => Err(err),
        }
    }

    /// Interpret select expression for channel operations
    fn interpret_select(
        &mut self,
        cases: &[seen_parser::ast::SelectCase],
        pos: Position,
    ) -> InterpreterResult<Value> {
        if cases.is_empty() {
            return Err(InterpreterError::runtime(
                "Select statement must have at least one case",
                pos,
            ));
        }

        let mut evaluated_cases = Vec::new();
        for case in cases {
            let channel_value = self.interpret_expression(&case.channel)?;
            match channel_value {
                Value::Channel(channel) => {
                    evaluated_cases.push((channel, &case.pattern, &case.handler));
                }
                _ => {
                    return Err(InterpreterError::runtime(
                        "Select case must be a channel",
                        pos.clone(),
                    ));
                }
            }
        }

        if evaluated_cases.is_empty() {
            return Ok(Value::Unit);
        }

        loop {
            let select_cases: Vec<ChannelSelectCase> = evaluated_cases
                .iter()
                .map(|(channel, _, _)| ChannelSelectCase::Receive {
                    channel: channel.clone(),
                })
                .collect();

            let async_runtime = self.runtime.async_runtime();
            let promise = {
                let mut runtime = async_runtime.lock().map_err(|_| {
                    InterpreterError::runtime("Failed to acquire async runtime lock", pos.clone())
                })?;
                let select_fn = ChannelSelectAsyncFunction {
                    cases: select_cases,
                    timeout: None,
                    position: pos.clone(),
                };
                runtime.execute_async_function(Box::new(select_fn))
            };
            let task_id = promise.task_id();

            let outcome = {
                let mut runtime = async_runtime.lock().map_err(|_| {
                    InterpreterError::runtime("Failed to acquire async runtime lock", pos.clone())
                })?;
                runtime.wait_for_task(task_id).map_err(|err| {
                    InterpreterError::runtime(format!("Select failed: {:?}", err), pos.clone())
                })?
            };

            let parts = match outcome {
                AsyncValue::Array(parts) => parts,
                other => {
                    return Err(InterpreterError::runtime(
                        format!("Unexpected select outcome: {:?}", other),
                        pos.clone(),
                    ))
                }
            };

            if parts.len() != 3 {
                return Err(InterpreterError::runtime(
                    "Malformed select outcome",
                    pos.clone(),
                ));
            }

            let case_index = match &parts[0] {
                AsyncValue::Integer(idx) => *idx,
                _ => {
                    return Err(InterpreterError::runtime(
                        "Invalid select outcome index",
                        pos.clone(),
                    ))
                }
            };

            let payload = parts[1].clone();
            let kind = match &parts[2] {
                AsyncValue::String(kind) => kind.clone(),
                _ => {
                    return Err(InterpreterError::runtime(
                        "Invalid select outcome discriminator",
                        pos.clone(),
                    ))
                }
            };

            match kind.as_str() {
                SELECT_STATUS_RECEIVED => {
                    if case_index < 0 {
                        return Err(InterpreterError::runtime(
                            "Select outcome reported negative index",
                            pos.clone(),
                        ));
                    }

                    let case_idx = case_index as usize;
                    if let Some((_, pattern, handler)) = evaluated_cases.get(case_idx) {
                        let received_value = self.async_value_to_value(&payload);
                        if self.match_pattern(pattern, &received_value) {
                            return self.interpret_expression(handler);
                        }
                        // Pattern mismatch; continue waiting for another case.
                    } else {
                        return Err(InterpreterError::runtime(
                            "Select outcome referenced an invalid case",
                            pos.clone(),
                        ));
                    }
                }
                SELECT_STATUS_SENT => {
                    return Ok(Value::Boolean(true));
                }
                SELECT_STATUS_CLOSED => {
                    // One channel closed; loop again to see if others can make progress.
                    continue;
                }
                SELECT_STATUS_ALL_CLOSED | SELECT_STATUS_TIMEOUT => {
                    return Ok(Value::Unit);
                }
                other => {
                    return Err(InterpreterError::runtime(
                        format!("Unknown select outcome: {}", other),
                        pos.clone(),
                    ));
                }
            }
        }
    }

    /// Helper function to match patterns (simplified)
    fn match_pattern(&mut self, pattern: &Pattern, value: &Value) -> bool {
        match pattern {
            Pattern::Wildcard => true,
            Pattern::Identifier(name) => {
                // Bind the value to the identifier
                self.runtime.set_variable(name, value.clone()).unwrap_or(());
                true
            }
            Pattern::Literal(expr) => {
                // Compare with literal value
                if let Ok(literal_value) = self.interpret_expression(expr) {
                    literal_value == *value
                } else {
                    false
                }
            }
            _ => false, // Other patterns not implemented yet
        }
    }

    /// Interpret actor definition
    fn interpret_actor_definition(
        &mut self,
        name: &str,
        fields: &[(String, seen_parser::ast::Type)],
        handlers: &[AstMessageHandler],
        pos: Position,
    ) -> InterpreterResult<Value> {
        let mut state_variables = HashMap::new();
        for (field_name, field_type) in fields {
            state_variables.insert(field_name.clone(), field_type.clone());
        }

        let mut handler_map = HashMap::new();
        for handler in handlers {
            let mut runtime_handler = RuntimeMessageHandler::send_handler(
                handler.message_type.clone(),
                (*handler.body).clone(),
                pos,
            );
            let context_id = self
                .actor_executor
                .register_context(self.runtime.snapshot_environment());
            runtime_handler.executor_context_id = Some(context_id);
            handler_map.insert(handler.message_type.clone(), runtime_handler);
        }

        let definition =
            RuntimeActorDefinition::new(name.to_string(), state_variables, handler_map, pos);

        let actor_system = self.runtime.actor_system();
        let mut system = actor_system
            .lock()
            .map_err(|_| InterpreterError::runtime("Failed to acquire actor system lock", pos))?;

        if let Err(err) = system.register_actor_definition(definition) {
            match err {
                AsyncError::ActorError { ref reason, .. }
                if reason.contains("already registered") =>
                    {
                        // Ignore duplicate registrations so programs can instantiate the same actor type multiple times.
                    }
                other => return Err(self.map_async_error(other, pos)),
            }
        }

        let actor_ref = system
            .spawn_actor(name, Vec::new())
            .map_err(|err| self.map_async_error(err, pos))?;

        Ok(Value::Actor(actor_ref))
    }

    /// Interpret send expression
    fn interpret_send(
        &mut self,
        target: &Expression,
        message: &Expression,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let target_value = self.interpret_expression(target)?;
        let message_value = self.interpret_expression(message)?;

        match target_value {
            Value::Actor(actor_ref) => {
                let (message_name, payload_value) =
                    self.split_actor_message(message_value, pos.clone())?;
                let payload_async = self.value_to_async_value(&payload_value);
                let actor_system = self.runtime.actor_system();
                let mut system = actor_system.lock().map_err(|_| {
                    InterpreterError::runtime("Failed to acquire actor system lock", pos.clone())
                })?;

                system
                    .send_message(actor_ref.id(), message_name, payload_async)
                    .map_err(|err| self.map_async_error(err, pos.clone()))?;
                let _ = system
                    .process_messages()
                    .map_err(|err| self.map_async_error(err, pos.clone()))?;
                Ok(Value::Boolean(true))
            }
            Value::Channel(channel) => {
                let async_runtime = self.runtime.async_runtime();
                let mut runtime = async_runtime.lock().map_err(|_| {
                    InterpreterError::runtime("Failed to acquire async runtime lock", pos.clone())
                })?;

                let async_value = self.value_to_async_value(&message_value);
                let send_task = ChannelSendAsyncFunction {
                    channel: channel.with_refreshed_generation(),
                    value: async_value,
                    position: pos.clone(),
                };

                let promise = runtime.execute_async_function(Box::new(send_task));
                drop(runtime);

                Ok(Value::Promise(Arc::new(promise)))
            }
            _ => Err(InterpreterError::runtime(
                "Can only send to actors or channels",
                pos,
            )),
        }
    }

    fn interpret_request(
        &mut self,
        message: &Expression,
        source: &Expression,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let target_value = self.interpret_expression(source)?;
        let message_value = self.interpret_expression(message)?;

        match target_value {
            Value::Actor(actor_ref) => {
                let (message_name, payload_value) =
                    self.split_actor_message(message_value, pos.clone())?;
                let payload_async = self.value_to_async_value(&payload_value);
                let actor_system = self.runtime.actor_system();
                let mut system = actor_system.lock().map_err(|_| {
                    InterpreterError::runtime("Failed to acquire actor system lock", pos.clone())
                })?;

                let promise = system
                    .request_from_actor(actor_ref.id(), message_name, payload_async, None)
                    .map_err(|err| self.map_async_error(err, pos.clone()))?;
                let _ = system
                    .process_messages()
                    .map_err(|err| self.map_async_error(err, pos.clone()))?;

                Ok(Value::Promise(promise))
            }
            _ => Err(InterpreterError::runtime(
                "request expects an Actor target",
                pos,
            )),
        }
    }

    fn split_actor_message(
        &self,
        message_value: Value,
        pos: Position,
    ) -> InterpreterResult<(String, Value)> {
        match message_value {
            Value::String(name) => Ok((name, Value::Unit)),
            Value::Array(values) => {
                let guard = values
                    .lock()
                    .map_err(|_| InterpreterError::runtime("Failed to read actor message", pos))?;
                if guard.is_empty() {
                    return Err(InterpreterError::runtime(
                        "Actor message arrays must include at least a name",
                        pos,
                    ));
                }
                if let Value::String(name) = &guard[0] {
                    let payload = guard.get(1).cloned().unwrap_or(Value::Unit);
                    Ok((name.clone(), payload))
                } else {
                    Err(InterpreterError::runtime(
                        "Actor message arrays must start with a string name",
                        pos,
                    ))
                }
            }
            _ => Err(InterpreterError::runtime(
                "Actor messages must be a string or [String, payload]",
                pos,
            )),
        }
    }

    fn map_async_error(&self, error: AsyncError, pos: Position) -> InterpreterError {
        InterpreterError::runtime(format!("Actor runtime error: {:?}", error), pos)
    }

    /// Convert AsyncValue to interpreter Value
    fn async_value_to_value(&self, async_value: &AsyncValue) -> Value {
        async_to_value(async_value)
    }

    /// Interpret effect definition
    fn interpret_effect_definition(
        &mut self,
        name: &str,
        operations: &[seen_parser::ast::EffectOperation],
        pos: Position,
    ) -> InterpreterResult<Value> {
        let mut effect_def = EffectDefinition::new(name.to_string(), pos);

        for op in operations {
            let parameters: Vec<EffectParameter> = op
                .params
                .iter()
                .map(|p| EffectParameter {
                    name: p.name.clone(),
                    param_type: p
                        .type_annotation
                        .clone()
                        .unwrap_or(seen_parser::ast::Type::new("Any")),
                    is_mutable: false,
                    default_value: None,
                })
                .collect();

            let return_ty = op
                .return_type
                .clone()
                .unwrap_or(seen_parser::ast::Type::new("Unit"));

            let effect_op = EffectOp::new(op.name.clone(), parameters, return_ty, pos);
            effect_def.add_operation(effect_op);
        }

        let effect_arc = Arc::new(effect_def.clone());

        let advanced = self.runtime.advanced_runtime();
        let mut advanced = advanced
            .lock()
            .map_err(|_| InterpreterError::runtime("Advanced runtime unavailable", pos))?;

        let effect_id = advanced
            .effect_system
            .register_effect(effect_def)
            .map_err(|err| InterpreterError::runtime(err.to_string(), pos))?;

        self.effect_registry.insert(name.to_string(), effect_id);
        self.trace_event(RuntimeTraceEvent::EffectRegistered {
            effect: name.to_string(),
            effect_id: effect_id.id().to_string(),
            operations: operations.iter().map(|op| op.name.clone()).collect(),
        });

        Ok(Value::Effect(effect_arc))
    }

    /// Interpret handle expression for effects
    fn interpret_handle(
        &mut self,
        body: &Expression,
        effect_name: &str,
        handlers: &[seen_parser::ast::EffectHandler],
        _pos: Position,
    ) -> InterpreterResult<Value> {
        let effect_id = self
            .effect_registry
            .get(effect_name)
            .copied()
            .ok_or_else(|| {
                InterpreterError::runtime(
                    format!("Effect '{}' is not defined", effect_name),
                    body.position().clone(),
                )
            })?;

        // Set up effect handlers
        let mut handler_map = HashMap::new();
        let handler_names: Vec<String> = handlers.iter().map(|h| h.operation.clone()).collect();

        for handler in handlers {
            // Store actual handler implementation
            let handler_value = {
                // Create a closure for the handler
                Value::Function {
                    name: format!("{}Handler", handler.operation),
                    parameters: handler.params.iter().map(|p| p.name.clone()).collect(),
                    body: handler.body.clone(),
                    closure: HashMap::new(),
                }
            };
            handler_map.insert(handler.operation.clone(), handler_value);
        }

        self.effect_handler_stack.push(EffectHandlerFrame {
            effect_id,
            handlers: handler_map,
        });

        let effect_id_str = effect_id.id().to_string();
        let effect_name_owned = effect_name.to_string();
        self.trace_event(RuntimeTraceEvent::EffectHandleEnter {
            effect: effect_name_owned.clone(),
            effect_id: effect_id_str.clone(),
            handlers: handler_names,
        });

        let result = self.interpret_expression(body);

        self.effect_handler_stack.pop();

        match &result {
            Ok(value) => self.trace_event(RuntimeTraceEvent::EffectHandleExit {
                effect: effect_name_owned.clone(),
                effect_id: effect_id_str.clone(),
                result: Some(RuntimeTraceValue::from_value(value)),
                error: None,
            }),
            Err(err) => self.trace_event(RuntimeTraceEvent::EffectHandleExit {
                effect: effect_name_owned,
                effect_id: effect_id_str,
                result: None,
                error: Some(format!("{}", err)),
            }),
        }

        result
    }

    /// Interpret contract-annotated function
    fn interpret_contracted_function(
        &mut self,
        function: &Expression,
        requires: &Option<Box<Expression>>,
        ensures: &Option<Box<Expression>>,
        invariants: &[Expression],
        pos: Position,
    ) -> InterpreterResult<Value> {
        // Check preconditions
        if let Some(req) = requires {
            let req_value = self.interpret_expression(req)?;
            if !req_value.is_truthy() {
                return Err(InterpreterError::runtime("Precondition violation", pos));
            }
        }

        // Execute function
        let result = self.interpret_expression(function)?;

        // Check postconditions
        if let Some(ens) = ensures {
            let ens_value = self.interpret_expression(ens)?;
            if !ens_value.is_truthy() {
                return Err(InterpreterError::runtime("Postcondition violation", pos));
            }
        }

        // Check invariants
        for inv in invariants {
            let inv_value = self.interpret_expression(inv)?;
            if !inv_value.is_truthy() {
                return Err(InterpreterError::runtime("Invariant violation", pos));
            }
        }

        Ok(result)
    }

    /// Interpret observable creation
    fn interpret_observable_creation(
        &mut self,
        source: &seen_parser::ast::ObservableSource,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let reactive_runtime = self.runtime.reactive_runtime();
        let mut runtime = reactive_runtime.lock().unwrap();

        match source {
            seen_parser::ast::ObservableSource::Range { start, end, step } => {
                // Create observable from range
                let start_val = self.interpret_expression(start)?;
                let end_val = self.interpret_expression(end)?;
                let step_val = step
                    .as_ref()
                    .map(|s| self.interpret_expression(s))
                    .transpose()?
                    .unwrap_or(Value::Integer(1));

                if let (Some(s), Some(e), Some(st)) = (
                    start_val.as_integer(),
                    end_val.as_integer(),
                    step_val.as_integer(),
                ) {
                    let observable = runtime.create_observable_range(s as i32, e as i32, st as i32);
                    // Box the observable and wrap in Arc
                    let boxed: Box<dyn std::any::Any + Send + Sync> = Box::new(observable);
                    Ok(Value::Observable(Arc::new(boxed)))
                } else {
                    Err(InterpreterError::runtime(
                        "Observable.Range requires integer arguments",
                        pos,
                    ))
                }
            }
            seen_parser::ast::ObservableSource::FromArray(array_expr) => {
                // Create observable from array
                let array_val = self.interpret_expression(array_expr)?;
                if let Value::Array(values) = array_val {
                    let guard = values
                        .lock()
                        .map_err(|_| InterpreterError::runtime("Array access failed", pos))?;
                    let async_values: Vec<seen_concurrency::types::AsyncValue> =
                        guard.iter().map(|v| self.value_to_async_value(v)).collect();
                    let observable = runtime.create_observable_from_vec(async_values);
                    let boxed: Box<dyn std::any::Any + Send + Sync> = Box::new(observable);
                    Ok(Value::Observable(Arc::new(boxed)))
                } else {
                    Err(InterpreterError::runtime(
                        "Observable.FromArray requires an array",
                        pos,
                    ))
                }
            }
            _ => Ok(Value::Unit), // Other sources not implemented yet
        }
    }

    /// Interpret flow creation
    fn interpret_flow_creation(
        &mut self,
        body: &Expression,
        _pos: Position,
    ) -> InterpreterResult<Value> {
        self.flow_collectors.push(Vec::new());
        let _ = self.interpret_expression(body)?;
        let collected = self.flow_collectors.pop().unwrap_or_default();
        let mut values = collected;
        if values.is_empty() {
            values.push(Value::Unit);
        }

        let reactive_runtime = self.runtime.reactive_runtime();
        let mut runtime = reactive_runtime.lock().unwrap();
        let flow = runtime.create_flow_from_vec(values);
        let boxed: Box<dyn std::any::Any + Send + Sync> = Box::new(flow);
        Ok(Value::Flow(Arc::new(boxed)))
    }

    /// Interpret reactive property creation
    fn interpret_reactive_property(
        &mut self,
        name: &str,
        value: &Expression,
        is_computed: bool,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let reactive_runtime = self.runtime.reactive_runtime();
        let mut runtime = reactive_runtime.lock().unwrap();

        if is_computed {
            // Create computed property
            let property_id = runtime.create_computed_property(
                name.to_string(),
                value.clone(),
                seen_parser::ast::Type::new("Any"), // Type inference would determine real type
                pos,
            );
            Ok(Value::ReactiveProperty {
                property_id,
                name: name.to_string(),
            })
        } else {
            // Create reactive property
            let initial_val = self.interpret_expression(value)?;
            let async_val = self.value_to_async_value(&initial_val);

            let property_id = runtime.create_reactive_property(
                name.to_string(),
                async_val,
                seen_parser::ast::Type::new("Any"),
                true, // is_mutable
                pos,
            );

            // Also store the property value in the runtime for access
            self.runtime.define_variable(name.to_string(), initial_val);
            self.reactive_bindings.insert(name.to_string(), property_id);

            Ok(Value::ReactiveProperty {
                property_id,
                name: name.to_string(),
            })
        }
    }

    /// Interpret stream operations (Map, Filter, etc.)
    fn interpret_stream_operation(
        &mut self,
        stream: &Expression,
        operation: &seen_parser::ast::StreamOp,
        pos: Position,
    ) -> InterpreterResult<Value> {
        let stream_val = self.interpret_expression(stream)?;

        match stream_val {
            Value::Observable(_obs) => {
                // Apply operation to observable using available reactive runtime methods
                let reactive_runtime = self.runtime.reactive_runtime();
                let mut runtime = reactive_runtime.lock().unwrap();

                match operation {
                    seen_parser::ast::StreamOp::Map(_transform_fn) => {
                        // For map operations, create a new observable with transformed values
                        // Since we can't directly transform existing observables, create a range
                        // and simulate the transformation result
                        let new_obs = runtime.create_observable_range(0, 10, 1);
                        Ok(Value::Observable(Arc::new(new_obs)))
                    }
                    seen_parser::ast::StreamOp::Filter(_predicate_fn) => {
                        // For filter operations, create a filtered observable
                        // Simulate by creating a smaller range
                        let new_obs = runtime.create_observable_range(1, 5, 1);
                        Ok(Value::Observable(Arc::new(new_obs)))
                    }
                    seen_parser::ast::StreamOp::Take(count) => {
                        // Take operation - create observable with limited range
                        let take_count = *count as i32;
                        let new_obs = runtime.create_observable_range(0, take_count, 1);
                        Ok(Value::Observable(Arc::new(new_obs)))
                    }
                    seen_parser::ast::StreamOp::Throttle(_)
                    | seen_parser::ast::StreamOp::Debounce(_)
                    | seen_parser::ast::StreamOp::Skip(_)
                    | seen_parser::ast::StreamOp::Distinct => {
                        // For timing and deduplication operations, return transformed observable
                        // In a real implementation, these would apply actual timing logic
                        let new_obs = runtime.create_observable_range(0, 5, 1);
                        Ok(Value::Observable(Arc::new(new_obs)))
                    }
                }
            }
            Value::Flow(flow) => {
                // Apply operation to flow using available flow methods
                let reactive_runtime = self.runtime.reactive_runtime();
                let mut runtime = reactive_runtime.lock().unwrap();

                match operation {
                    seen_parser::ast::StreamOp::Map(_transform_fn) => {
                        // Create new flow with transformation simulation
                        let new_flow = runtime.create_flow_range(0, 10, 1);
                        Ok(Value::Flow(Arc::new(new_flow)))
                    }
                    seen_parser::ast::StreamOp::Filter(_predicate_fn) => {
                        // Create filtered flow
                        let new_flow = runtime.create_flow_range(1, 5, 1);
                        Ok(Value::Flow(Arc::new(new_flow)))
                    }
                    seen_parser::ast::StreamOp::Take(count) => {
                        // Take operation for flows
                        let take_count = *count as i64;
                        let new_flow = runtime.create_flow_range(0, take_count, 1);
                        Ok(Value::Flow(Arc::new(new_flow)))
                    }
                    _ => {
                        // Return original flow for other operations
                        Ok(Value::Flow(flow))
                    }
                }
            }
            _ => Err(InterpreterError::runtime(
                "Stream operations require Observable or Flow",
                pos,
            )),
        }
    }

    /// Convert Value to AsyncValue for reactive runtime
    fn value_to_async_value(&self, value: &Value) -> AsyncValue {
        value_to_async(value)
    }

    fn value_to_effect_async_value(&self, value: &Value) -> EffectAsyncValue {
        match value {
            Value::Unit | Value::Null => EffectAsyncValue::Unit,
            Value::Integer(i) => EffectAsyncValue::Integer(*i),
            Value::Float(f) => EffectAsyncValue::Float(*f),
            Value::Boolean(b) => EffectAsyncValue::Boolean(*b),
            Value::String(s) => EffectAsyncValue::String(s.clone()),
            Value::Array(arr) => {
                let guard = arr.lock();
                if let Ok(values) = guard {
                    let converted = values
                        .iter()
                        .map(|v| self.value_to_effect_async_value(v))
                        .collect();
                    EffectAsyncValue::Array(converted)
                } else {
                    EffectAsyncValue::Unit
                }
            }
            _ => EffectAsyncValue::Unit,
        }
    }

    fn effect_async_value_to_value(&self, value: &EffectAsyncValue) -> Value {
        match value {
            EffectAsyncValue::Unit => Value::Unit,
            EffectAsyncValue::Integer(i) => Value::Integer(*i),
            EffectAsyncValue::Float(f) => Value::Float(*f),
            EffectAsyncValue::Boolean(b) => Value::Boolean(*b),
            EffectAsyncValue::String(s) => Value::String(s.clone()),
            EffectAsyncValue::Array(values) => {
                let converted: Vec<Value> = values
                    .iter()
                    .map(|v| self.effect_async_value_to_value(v))
                    .collect();
                Value::array_from_vec(converted)
            }
        }
    }

    fn register_class(
        &mut self,
        name: &str,
        _generics: &[String],
        fields: &[ClassField],
        methods: &[Method],
    ) -> InterpreterResult<()> {
        let runtime_fields = fields
            .iter()
            .map(|field| RuntimeClassField {
                name: field.name.clone(),
                default_value: field.default_value.clone(),
                is_mutable: field.is_mutable,
            })
            .collect();

        let mut runtime_methods = HashMap::new();
        for method in methods {
            let receiver_name = method
                .receiver
                .as_ref()
                .map(|r| r.name.clone())
                .unwrap_or_else(|| "this".to_string());
            runtime_methods.insert(
                method.name.clone(),
                RuntimeMethod {
                    name: method.name.clone(),
                    parameters: method.parameters.clone(),
                    body: method.body.clone(),
                    is_static: method.is_static,
                    receiver_name,
                },
            );
        }

        self.class_registry.insert(
            name.to_string(),
            RuntimeClass {
                name: name.to_string(),
                fields: runtime_fields,
                methods: runtime_methods,
            },
        );

        self.runtime.define_variable(
            name.to_string(),
            Value::Class {
                name: name.to_string(),
            },
        );
        Ok(())
    }

    fn invoke_class_method(
        &mut self,
        class_name: &str,
        method_name: &str,
        receiver: Option<Value>,
        args: &[Expression],
        pos: Position,
    ) -> InterpreterResult<Value> {
        let class = self
            .class_registry
            .get(class_name)
            .cloned()
            .ok_or_else(|| {
                InterpreterError::runtime(format!("Unknown class {}", class_name), pos)
            })?;

        let method = class.methods.get(method_name).cloned().ok_or_else(|| {
            InterpreterError::runtime(
                format!("Class '{}' has no method '{}'", class_name, method_name),
                pos,
            )
        })?;

        if !method.is_static && receiver.is_none() {
            return Err(InterpreterError::runtime(
                format!(
                    "Method '{}::{}' requires an instance",
                    class_name, method_name
                ),
                pos,
            ));
        }

        let mut evaluated_args = Vec::new();
        for (idx, param) in method.parameters.iter().enumerate() {
            if let Some(arg_expr) = args.get(idx) {
                evaluated_args.push(self.interpret_expression(arg_expr)?);
            } else if let Some(default_expr) = &param.default_value {
                evaluated_args.push(self.interpret_expression(default_expr)?);
            } else {
                return Err(InterpreterError::argument_count_mismatch(
                    format!("{}::{}", class_name, method_name),
                    method.parameters.len(),
                    args.len(),
                    pos,
                ));
            }
        }

        let arg_summaries: Vec<RuntimeTraceValue> = evaluated_args
            .iter()
            .map(RuntimeTraceValue::from_value)
            .collect();
        self.trace_event(RuntimeTraceEvent::MethodEnter {
            class: class_name.to_string(),
            method: method_name.to_string(),
            arguments: arg_summaries,
        });

        self.runtime.push_environment(true);

        let mut context_pushed = false;
        if !method.is_static {
            let recv_value = receiver.expect("receiver checked above");
            self.runtime
                .define_variable(method.receiver_name.clone(), recv_value.clone());
            if let Value::Struct { fields, .. } = recv_value {
                self.instance_stack.push(InstanceContext {
                    class_name: class_name.to_string(),
                    fields,
                });
                context_pushed = true;
            } else {
                return Err(InterpreterError::runtime(
                    format!("{}::{} receiver is not a struct", class_name, method_name),
                    pos,
                ));
            }
        }

        for (param, value) in method.parameters.iter().zip(evaluated_args.into_iter()) {
            self.runtime.define_variable(param.name.clone(), value);
        }

        let prev_break = self.break_flag;
        let prev_continue = self.continue_flag;
        self.break_flag = false;
        self.continue_flag = false;

        let result = self.interpret_expression(&method.body);

        if context_pushed {
            self.instance_stack.pop();
        }

        let deferred = self
            .runtime
            .take_current_deferred()
            .map_err(|e| InterpreterError::runtime(e.to_string(), pos))?;
        let defer_result = self.run_deferred(deferred);

        let pop_result = self
            .runtime
            .pop_environment()
            .map_err(|e| InterpreterError::runtime(e.to_string(), pos));

        let match_result = match (result, defer_result, pop_result) {
            (Err(err), _, _) => Err(err),
            (Ok(_), Err(err), _) => Err(err),
            (Ok(_), Ok(_), Err(err)) => Err(err),
            (Ok(value), Ok(_), Ok(_)) => Ok(value),
        };

        self.break_flag = prev_break;
        self.continue_flag = prev_continue;

        let mut call_result = match_result;

        if call_result.is_ok() && self.return_flag {
            let ret = self.return_value.take().unwrap_or(Value::Unit);
            self.return_flag = false;
            call_result = Ok(ret);
        }

        match call_result {
            Ok(value) => {
                self.trace_event(RuntimeTraceEvent::MethodExit {
                    class: class_name.to_string(),
                    method: method_name.to_string(),
                    result: RuntimeTraceValue::from_value(&value),
                });
                Ok(value)
            }
            Err(err) => {
                let message = format!("{}", err);
                self.trace_event(RuntimeTraceEvent::MethodError {
                    class: class_name.to_string(),
                    method: method_name.to_string(),
                    message,
                });
                Err(err)
            }
        }
    }

    fn lookup_instance_field(&self, name: &str) -> Option<Value> {
        for ctx in self.instance_stack.iter().rev() {
            if let Some(value) = ctx.get_field(name) {
                return Some(value);
            }
        }
        None
    }

    fn assign_instance_field(&mut self, name: &str, value: Value) -> bool {
        for ctx in self.instance_stack.iter().rev() {
            if ctx.set_field(name, value.clone()) {
                return true;
            }
        }
        false
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub(crate) fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    fn sync_reactive_binding(
        &mut self,
        name: &str,
        value: &Value,
        pos: Position,
    ) -> InterpreterResult<()> {
        if let Some(property_id) = self.reactive_bindings.get(name).copied() {
            let async_value = self.value_to_async_value(value);
            let reactive_runtime = self.runtime.reactive_runtime();
            let mut runtime = reactive_runtime.lock().map_err(|_| {
                InterpreterError::runtime("Failed to acquire reactive runtime lock", pos)
            })?;
            runtime
                .property_manager
                .set_property_value(property_id, async_value)
                .map_err(|err| InterpreterError::runtime(err.to_string(), pos))?;
            runtime
                .process_updates()
                .map_err(|err| InterpreterError::runtime(err.to_string(), pos))?;
        }
        Ok(())
    }

    pub(crate) fn push_instance_context(
        &mut self,
        class_name: String,
        fields: Arc<Mutex<HashMap<String, Value>>>,
    ) {
        self.instance_stack
            .push(InstanceContext { class_name, fields });
    }

    pub(crate) fn pop_instance_context(&mut self) {
        self.instance_stack.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeTraceMetadata;
    use seen_concurrency::types::{AsyncValue, Channel, ChannelId, ChannelReceiveStatus};
    use seen_parser::ast::{MessageHandler, Pattern};
    use seen_parser::{ClassField, Method, Type};
    use seen_reactive::flow::Flow;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    fn send_expression(value: i64, pos: Position) -> Expression {
        Expression::Send {
            message: Box::new(Expression::IntegerLiteral {
                value,
                pos: pos.clone(),
            }),
            target: Box::new(Expression::Identifier {
                name: "tx".to_string(),
                is_public: false,
                pos,
            }),
            pos: Position::start(),
        }
    }

    #[test]
    fn test_interpreter_creation() {
        let interpreter = Interpreter::new();
        let _ = interpreter; // Use the value
    }

    #[test]
    fn request_expression_returns_value() {
        let mut interpreter = Interpreter::new();
        let pos = Position::start();

        let actor_expr = Expression::Actor {
            name: "RequestActor".to_string(),
            fields: vec![],
            handlers: vec![MessageHandler {
                message_type: "Ping".to_string(),
                params: vec![],
                body: Box::new(Expression::Identifier {
                    name: "__message_payload".to_string(),
                    is_public: false,
                    pos: pos.clone(),
                }),
            }],
            pos: pos.clone(),
        };

        let actor_value = interpreter
            .interpret_expression(&actor_expr)
            .expect("actor creation should succeed");
        interpreter
            .runtime
            .define_variable("target".to_string(), actor_value);

        let message_expr = Expression::ArrayLiteral {
            elements: vec![
                Expression::StringLiteral {
                    value: "Ping".to_string(),
                    pos: pos.clone(),
                },
                Expression::IntegerLiteral {
                    value: 7,
                    pos: pos.clone(),
                },
            ],
            pos: pos.clone(),
        };

        let request_expr = Expression::Request {
            message: Box::new(message_expr),
            source: Box::new(Expression::Identifier {
                name: "target".to_string(),
                is_public: false,
                pos: pos.clone(),
            }),
            pos: pos.clone(),
        };

        let await_expr = Expression::Await {
            expr: Box::new(request_expr),
            pos,
        };

        let result = interpreter
            .interpret_expression(&await_expr)
            .expect("await should resolve request promise");

        assert_eq!(result, Value::Integer(7));
    }

    #[test]
    fn cast_expression_preserves_struct_value() {
        let mut interpreter = Interpreter::new();
        let pos = Position::start();
        let struct_value =
            Value::struct_from_fields("ExpressionStatement".to_string(), HashMap::new());
        interpreter
            .runtime
            .define_variable("node".to_string(), struct_value.clone());

        let cast_expr = Expression::Cast {
            expr: Box::new(Expression::Identifier {
                name: "node".to_string(),
                is_public: false,
                pos: pos.clone(),
            }),
            target_type: Type::new("ExpressionStatement"),
            pos: pos.clone(),
        };

        let result = interpreter
            .interpret_expression(&cast_expr)
            .expect("cast should succeed for matching struct type");

        match result {
            Value::Struct { name, .. } => assert_eq!(name, "ExpressionStatement"),
            _ => panic!("expected struct value from cast"),
        }
    }

    #[test]
    fn type_check_expression_returns_boolean() {
        let mut interpreter = Interpreter::new();
        let pos = Position::start();
        let struct_value =
            Value::struct_from_fields("ExpressionStatement".to_string(), HashMap::new());
        interpreter
            .runtime
            .define_variable("node".to_string(), struct_value);

        let type_check_expr = Expression::TypeCheck {
            expr: Box::new(Expression::Identifier {
                name: "node".to_string(),
                is_public: false,
                pos: pos.clone(),
            }),
            target_type: Type::new("ExpressionStatement"),
            pos,
        };

        let result = interpreter
            .interpret_expression(&type_check_expr)
            .expect("type check should evaluate");

        match result {
            Value::Boolean(flag) => assert!(flag),
            _ => panic!("expected boolean from type check"),
        }
    }

    #[test]
    fn actor_handlers_mutate_state_and_return_values() {
        let mut interpreter = Interpreter::new();
        let pos = Position::start();

        let inc_body = Expression::Block {
            expressions: vec![
                Expression::Assignment {
                    target: Box::new(Expression::Identifier {
                        name: "count".to_string(),
                        is_public: false,
                        pos,
                    }),
                    value: Box::new(Expression::BinaryOp {
                        left: Box::new(Expression::Identifier {
                            name: "count".to_string(),
                            is_public: false,
                            pos,
                        }),
                        op: BinaryOperator::Add,
                        right: Box::new(Expression::Identifier {
                            name: "__message_payload".to_string(),
                            is_public: false,
                            pos,
                        }),
                        pos,
                    }),
                    op: AssignmentOperator::Assign,
                    pos,
                },
                Expression::Identifier {
                    name: "count".to_string(),
                    is_public: false,
                    pos,
                },
            ],
            pos,
        };

        let get_body = Expression::Identifier {
            name: "count".to_string(),
            is_public: false,
            pos,
        };

        let actor_expr = Expression::Actor {
            name: "Counter".to_string(),
            fields: vec![(
                "count".to_string(),
                Type {
                    name: "Int".to_string(),
                    is_nullable: false,
                    generics: vec![],
                },
            )],
            handlers: vec![
                MessageHandler {
                    message_type: "Inc".to_string(),
                    params: vec![],
                    body: Box::new(inc_body),
                },
                MessageHandler {
                    message_type: "Get".to_string(),
                    params: vec![],
                    body: Box::new(get_body),
                },
            ],
            pos,
        };

        let actor_value = interpreter
            .interpret_expression(&actor_expr)
            .expect("actor creation should succeed");
        let actor_ref = match &actor_value {
            Value::Actor(actor_ref) => actor_ref.clone(),
            _ => panic!("expected actor reference"),
        };
        interpreter
            .runtime
            .define_variable("counter".to_string(), actor_value);

        {
            let actor_system = interpreter.runtime.actor_system();
            let system = actor_system.lock().expect("actor system lock poisoned");
            let actor_instance = system
                .get_actor(actor_ref.id())
                .expect("actor should exist");
            assert_eq!(
                actor_instance
                    .state
                    .get("count")
                    .cloned()
                    .unwrap_or(AsyncValue::Integer(-1)),
                AsyncValue::Integer(0)
            );
        }

        let counter_ident = Expression::Identifier {
            name: "counter".to_string(),
            is_public: false,
            pos,
        };

        let build_request = |message: &str, payload: Option<i64>| -> Expression {
            let mut elements = vec![Expression::StringLiteral {
                value: message.to_string(),
                pos,
            }];
            if let Some(value) = payload {
                elements.push(Expression::IntegerLiteral { value, pos });
            } else {
                elements.push(Expression::NullLiteral { pos });
            }

            Expression::Await {
                expr: Box::new(Expression::Request {
                    message: Box::new(Expression::ArrayLiteral { elements, pos }),
                    source: Box::new(counter_ident.clone()),
                    pos,
                }),
                pos,
            }
        };

        let first = interpreter
            .interpret_expression(&build_request("Inc", Some(5)))
            .expect("request should succeed");
        assert_eq!(first, Value::Integer(5));

        let state_after_first = interpreter
            .interpret_expression(&build_request("Get", None))
            .expect("get request should succeed");
        assert_eq!(state_after_first, Value::Integer(5));

        {
            let actor_system = interpreter.runtime.actor_system();
            let system = actor_system.lock().expect("actor system lock poisoned");
            let actor_instance = system
                .get_actor(actor_ref.id())
                .expect("actor should exist");
            assert_eq!(
                actor_instance
                    .state
                    .get("count")
                    .cloned()
                    .unwrap_or(AsyncValue::Integer(-1)),
                AsyncValue::Integer(5)
            );
        }

        let second = interpreter
            .interpret_expression(&build_request("Inc", Some(2)))
            .expect("request should succeed");
        assert_eq!(second, Value::Integer(7));

        let final_value = interpreter
            .interpret_expression(&build_request("Get", None))
            .expect("get request should succeed");
        assert_eq!(final_value, Value::Integer(7));
    }

    #[test]
    fn reactive_property_assignment_updates_runtime_manager() {
        let mut interpreter = Interpreter::new();
        let pos = Position::start();

        let prop_expr = Expression::ReactiveProperty {
            name: "Username".to_string(),
            value: Box::new(Expression::StringLiteral {
                value: "Alice".to_string(),
                pos,
            }),
            is_computed: false,
            pos,
        };

        interpreter
            .interpret_expression(&prop_expr)
            .expect("define reactive property");

        let assign_expr = Expression::Assignment {
            target: Box::new(Expression::Identifier {
                name: "Username".to_string(),
                is_public: false,
                pos,
            }),
            value: Box::new(Expression::StringLiteral {
                value: "Bob".to_string(),
                pos,
            }),
            op: AssignmentOperator::Assign,
            pos,
        };

        interpreter
            .interpret_expression(&assign_expr)
            .expect("assignment should succeed");

        let reactive_runtime = interpreter.runtime.reactive_runtime();
        let runtime = reactive_runtime
            .lock()
            .expect("reactive runtime lock should be available");
        let property = runtime
            .property_manager
            .find_property_by_name("Username")
            .expect("property should be registered");
        assert_eq!(property.get(), &AsyncValue::String("Bob".to_string()));
    }

    #[test]
    fn flow_creation_captures_runtime_emissions() {
        let mut interpreter = Interpreter::new();
        let pos = Position::start();

        let flow_body = Expression::Block {
            expressions: vec![
                Expression::Let {
                    name: "i".to_string(),
                    type_annotation: None,
                    value: Box::new(Expression::IntegerLiteral { value: 0, pos }),
                    is_mutable: true,
                    delegation: None,
                    pos,
                },
                Expression::While {
                    condition: Box::new(Expression::BinaryOp {
                        left: Box::new(Expression::Identifier {
                            name: "i".to_string(),
                            is_public: false,
                            pos,
                        }),
                        op: BinaryOperator::Less,
                        right: Box::new(Expression::IntegerLiteral { value: 3, pos }),
                        pos,
                    }),
                    body: Box::new(Expression::Block {
                        expressions: vec![
                            Expression::Call {
                                callee: Box::new(Expression::Identifier {
                                    name: "emit".to_string(),
                                    is_public: false,
                                    pos,
                                }),
                                args: vec![Expression::Identifier {
                                    name: "i".to_string(),
                                    is_public: false,
                                    pos,
                                }],
                                pos,
                            },
                            Expression::Assignment {
                                target: Box::new(Expression::Identifier {
                                    name: "i".to_string(),
                                    is_public: false,
                                    pos,
                                }),
                                value: Box::new(Expression::BinaryOp {
                                    left: Box::new(Expression::Identifier {
                                        name: "i".to_string(),
                                        is_public: false,
                                        pos,
                                    }),
                                    op: BinaryOperator::Add,
                                    right: Box::new(Expression::IntegerLiteral { value: 1, pos }),
                                    pos,
                                }),
                                op: AssignmentOperator::Assign,
                                pos,
                            },
                        ],
                        pos,
                    }),
                    pos,
                },
            ],
            pos,
        };

        let flow_expr = Expression::FlowCreation {
            body: Box::new(flow_body),
            pos,
        };

        let flow_value = interpreter
            .interpret_expression(&flow_expr)
            .expect("flow creation should succeed");

        let values = match flow_value {
            Value::Flow(flow_arc) => {
                let concrete = flow_arc
                    .downcast::<Flow<Value>>()
                    .expect("expected Flow<Value>");
                let mut flow = Arc::try_unwrap(concrete).expect("flow still referenced");
                flow.collect_all()
            }
            other => panic!("expected Flow value, got {:?}", other),
        };

        let emitted: Vec<i64> = values
            .into_iter()
            .map(|value| match value {
                Value::Integer(i) => i,
                other => panic!("expected integer emission, got {:?}", other),
            })
            .collect();

        assert_eq!(emitted, vec![0, 1, 2]);
    }

    #[test]
    fn class_methods_mutate_instance_fields() {
        let mut interpreter = Interpreter::new();
        let pos = Position::start();

        let counter_field = ClassField {
            name: "value".to_string(),
            field_type: Type {
                name: "Int".to_string(),
                is_nullable: false,
                generics: vec![],
            },
            is_public: false,
            is_mutable: true,
            default_value: Some(Expression::IntegerLiteral { value: 0, pos }),
            annotations: vec![],
        };

        let value_ident = || Expression::Identifier {
            name: "value".to_string(),
            is_public: false,
            pos,
        };

        let increment_body = Expression::Block {
            expressions: vec![
                Expression::Assignment {
                    target: Box::new(value_ident()),
                    value: Box::new(Expression::BinaryOp {
                        left: Box::new(value_ident()),
                        op: BinaryOperator::Add,
                        right: Box::new(Expression::IntegerLiteral { value: 1, pos }),
                        pos,
                    }),
                    op: AssignmentOperator::Assign,
                    pos,
                },
                value_ident(),
            ],
            pos,
        };

        let increment_method = Method {
            name: "inc".to_string(),
            parameters: vec![],
            return_type: Some(Type {
                name: "Int".to_string(),
                is_nullable: false,
                generics: vec![],
            }),
            body: increment_body,
            is_public: true,
            is_static: false,
            receiver: None,
            annotations: vec![],
            pos,
        };

        let class_def = Expression::ClassDefinition {
            name: "Counter".to_string(),
            generics: vec![],
            superclass: None,
            fields: vec![counter_field],
            methods: vec![increment_method],
            is_sealed: false,
            doc_comment: None,
            pos,
        };

        let instantiate_counter = Expression::StructLiteral {
            name: "Counter".to_string(),
            fields: vec![],
            pos,
        };

        let let_counter = Expression::Let {
            name: "counter".to_string(),
            type_annotation: None,
            value: Box::new(instantiate_counter),
            is_mutable: true,
            delegation: None,
            pos,
        };

        let call_inc = Expression::Call {
            callee: Box::new(Expression::MemberAccess {
                object: Box::new(Expression::Identifier {
                    name: "counter".to_string(),
                    is_public: false,
                    pos,
                }),
                member: "inc".to_string(),
                is_safe: false,
                pos,
            }),
            args: vec![],
            pos,
        };

        let block = Expression::Block {
            expressions: vec![class_def, let_counter, call_inc],
            pos,
        };

        let result = interpreter
            .interpret_expression(&block)
            .expect("class method should execute");
        assert_eq!(result, Value::Integer(1));
    }

    #[test]
    fn channel_send_waits_until_capacity_is_available() {
        let mut interpreter = Interpreter::new();
        let channel = Channel::new(ChannelId::allocate(), Some(1));
        let background_channel = channel.clone();

        interpreter
            .runtime
            .define_variable("tx".to_string(), Value::Channel(channel));

        let pos = Position::start();

        let first_send = Expression::Await {
            expr: Box::new(send_expression(1, pos.clone())),
            pos: pos.clone(),
        };
        let first_result = interpreter
            .interpret_expression(&first_send)
            .expect("first send should resolve");
        match first_result {
            Value::Boolean(true) => {}
            other => panic!("expected awaited send to yield true, got {:?}", other),
        }

        let second_send = Expression::Await {
            expr: Box::new(send_expression(2, pos.clone())),
            pos: pos.clone(),
        };

        let drain_handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            let _ = background_channel.try_recv_with_status();
        });

        let second_result = interpreter
            .interpret_expression(&second_send)
            .expect("second send should resolve once capacity frees");

        drain_handle.join().expect("drain thread should succeed");

        match second_result {
            Value::Boolean(true) => {}
            other => panic!(
                "expected awaited send to yield true after draining, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn select_waits_until_channel_receives() {
        let mut interpreter = Interpreter::new();
        let channel = Channel::new(ChannelId::allocate(), Some(1));
        let sender_channel = channel.clone();

        interpreter
            .runtime
            .define_variable("rx".to_string(), Value::Channel(channel));
        interpreter
            .runtime
            .define_variable("val".to_string(), Value::Null);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            let _ = sender_channel.send_with_status(AsyncValue::Integer(42));
        });

        let select_expr = Expression::Select {
            cases: vec![seen_parser::ast::SelectCase {
                channel: Box::new(Expression::Identifier {
                    name: "rx".to_string(),
                    is_public: false,
                    pos: Position::start(),
                }),
                pattern: Pattern::Identifier("val".to_string()),
                handler: Box::new(Expression::Identifier {
                    name: "val".to_string(),
                    is_public: false,
                    pos: Position::start(),
                }),
            }],
            pos: Position::start(),
        };

        let result = interpreter
            .interpret_expression(&select_expr)
            .expect("select should resolve once value is available");

        assert_eq!(result, Value::Integer(42));
    }

    #[derive(Debug, Clone)]
    struct DelayedAsyncFunction {
        gate: Channel,
        position: Position,
    }

    impl AsyncFunctionTrait for DelayedAsyncFunction {
        fn execute(
            &self,
            _context: &mut AsyncExecutionContext,
        ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
            let gate = self.gate.clone();
            let pos = self.position.clone();

            Box::pin(async move {
                match gate.receive_future().await {
                    Ok(value) => Ok(value),
                    Err(AsyncError::ChannelError { reason, .. }) => Err(AsyncError::ChannelError {
                        reason,
                        position: pos.clone(),
                    }),
                    Err(other) => Err(other),
                }
            })
        }

        fn name(&self) -> &str {
            "delayed-channel-receive"
        }
    }

    #[test]
    fn jobs_scope_waits_for_spawned_tasks_before_unwinding() {
        let mut interpreter = Interpreter::new();
        interpreter.runtime.push_task_scope();

        let gate = Channel::new(ChannelId::allocate(), Some(1));
        let runtime_gate = gate.clone();
        let check_gate = gate.clone();
        let sender_gate = gate.clone();
        let pos = Position::start();

        let delayed_task = DelayedAsyncFunction {
            gate: runtime_gate,
            position: pos.clone(),
        };

        let async_runtime = interpreter.runtime.async_runtime();
        let mut runtime = async_runtime
            .lock()
            .expect("async runtime lock should be available");
        let task_handle = runtime.spawn_task(Box::new(delayed_task), TaskPriority::Normal);
        let task_id = task_handle
            .task_id()
            .expect("spawned task should yield an id");
        drop(runtime);

        interpreter.runtime.register_scope_task(task_id);
        let tasks = interpreter
            .runtime
            .pop_task_scope()
            .expect("scope stack should contain the registered task");

        let delay = Duration::from_millis(40);
        let send_delay = delay;
        let worker = thread::spawn(move || {
            thread::sleep(send_delay);
            let _ = sender_gate.send_with_status(AsyncValue::Integer(7));
        });

        let start = Instant::now();
        interpreter
            .join_scope_tasks(tasks, pos.clone())
            .expect("jobs.scope should join spawned task");
        let elapsed = start.elapsed();

        worker
            .join()
            .expect("sender thread should complete without panicking");

        assert!(
            elapsed >= delay,
            "jobs.scope returned too early (elapsed {:?} < {:?})",
            elapsed,
            delay
        );

        match check_gate.try_recv_with_status() {
            ChannelReceiveStatus::WouldBlock => {}
            other => panic!("expected gate channel to be drained, got {:?}", other),
        }
    }

    #[test]
    fn effect_handler_invokes_custom_body() {
        use seen_parser::ast::{EffectHandler, EffectOperation, Type};

        let pos = Position::start();
        let handler = EffectHandler {
            operation: "Read".to_string(),
            params: Vec::new(),
            body: Box::new(Expression::StringLiteral {
                value: "handled".to_string(),
                pos,
            }),
        };

        let call_expr = Expression::Call {
            callee: Box::new(Expression::MemberAccess {
                object: Box::new(Expression::Identifier {
                    name: "IO".to_string(),
                    is_public: false,
                    pos,
                }),
                member: "Read".to_string(),
                is_safe: false,
                pos,
            }),
            args: Vec::new(),
            pos,
        };

        let effect_def = Expression::Effect {
            name: "IO".to_string(),
            operations: vec![EffectOperation {
                name: "Read".to_string(),
                params: Vec::new(),
                return_type: Some(Type::new("String")),
            }],
            pos,
        };

        let handle_expr = Expression::Handle {
            body: Box::new(call_expr),
            effect: "IO".to_string(),
            handlers: vec![handler],
            pos,
        };

        let program = Expression::Block {
            expressions: vec![effect_def, handle_expr],
            pos,
        };

        let mut interpreter = Interpreter::new();
        let result = interpreter
            .interpret_expression(&program)
            .expect("effect handler should run");

        match result {
            Value::String(s) => assert_eq!(s, "handled"),
            other => panic!("expected handled string, got {:?}", other),
        }
    }

    #[test]
    fn effect_call_without_handler_reports_error() {
        use seen_parser::ast::EffectOperation;

        let pos = Position::start();
        let effect_def = Expression::Effect {
            name: "IO".to_string(),
            operations: vec![EffectOperation {
                name: "Read".to_string(),
                params: Vec::new(),
                return_type: None,
            }],
            pos,
        };

        let call_expr = Expression::Call {
            callee: Box::new(Expression::MemberAccess {
                object: Box::new(Expression::Identifier {
                    name: "IO".to_string(),
                    is_public: false,
                    pos,
                }),
                member: "Read".to_string(),
                is_safe: false,
                pos,
            }),
            args: Vec::new(),
            pos,
        };

        let program = Expression::Block {
            expressions: vec![effect_def, call_expr],
            pos,
        };

        let mut interpreter = Interpreter::new();
        let err = interpreter
            .interpret_expression(&program)
            .expect_err("missing handler should produce runtime error");
        match err {
            InterpreterError::RuntimeError { message, .. } => {
                assert!(
                    message.contains("No handler found"),
                    "unexpected error message: {}",
                    message
                );
            }
            other => panic!("expected runtime error, got {:?}", other),
        }
    }

    #[test]
    fn effect_trace_emits_breadcrumbs() {
        use seen_parser::ast::{EffectHandler, EffectOperation, Type};

        let pos = Position::start();
        let handler = EffectHandler {
            operation: "Write".to_string(),
            params: vec![Parameter {
                name: "value".to_string(),
                type_annotation: Some(Type::new("String")),
                default_value: None,
                memory_modifier: None,
            }],
            body: Box::new(Expression::Identifier {
                name: "value".to_string(),
                is_public: false,
                pos,
            }),
        };

        let call_expr = Expression::Call {
            callee: Box::new(Expression::MemberAccess {
                object: Box::new(Expression::Identifier {
                    name: "IO".to_string(),
                    is_public: false,
                    pos,
                }),
                member: "Write".to_string(),
                is_safe: false,
                pos,
            }),
            args: vec![Expression::StringLiteral {
                value: "trace-me".to_string(),
                pos,
            }],
            pos,
        };

        let effect_def = Expression::Effect {
            name: "IO".to_string(),
            operations: vec![EffectOperation {
                name: "Write".to_string(),
                params: vec![Parameter {
                    name: "value".to_string(),
                    type_annotation: Some(Type::new("String")),
                    default_value: None,
                    memory_modifier: None,
                }],
                return_type: Some(Type::new("String")),
            }],
            pos,
        };

        let handle_expr = Expression::Handle {
            body: Box::new(call_expr),
            effect: "IO".to_string(),
            handlers: vec![handler],
            pos,
        };

        let program = Expression::Block {
            expressions: vec![effect_def, handle_expr],
            pos,
        };

        let metadata = RuntimeTraceMetadata {
            program: "trace_test".to_string(),
            opt_level: 0,
            cli_profile: "default".to_string(),
            args: Vec::new(),
            seen_version: "test".to_string(),
            host: "test-host".to_string(),
        };
        let trace_handle = RuntimeTraceHandle::new(metadata);

        let mut interpreter = Interpreter::new();
        interpreter.set_trace_handle(trace_handle.clone());

        let result = interpreter
            .interpret_expression(&program)
            .expect("effect handler should succeed");
        assert!(matches!(result, Value::String(_)));

        let trace = trace_handle.snapshot();
        assert!(
            trace.events.iter().any(|event| matches!(
                event,
                RuntimeTraceEvent::EffectRegistered { effect, .. }
                if effect == "IO"
            )),
            "expected EffectRegistered event"
        );
        assert!(
            trace.events.iter().any(|event| matches!(
                event,
                RuntimeTraceEvent::EffectOperationInvoke {
                    effect,
                    operation,
                    via_handler,
                    ..
                } if effect == "IO" && operation == "Write" && *via_handler
            )),
            "expected EffectOperationInvoke via handler"
        );
        assert!(
            trace.events.iter().any(|event| matches!(
                event,
                RuntimeTraceEvent::EffectHandleExit { error, .. }
                if error.is_none()
            )),
            "expected EffectHandleExit without errors"
        );
    }
}
