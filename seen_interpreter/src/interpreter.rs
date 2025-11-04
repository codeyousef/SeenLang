//! Main interpreter implementation for the Seen programming language

use crate::builtins::BuiltinRegistry;
use crate::errors::{InterpreterError, InterpreterResult};
use crate::runtime::Runtime;
use crate::value::Value;
use seen_concurrency::{
    async_runtime::AsyncExecutionContext,
    async_runtime::AsyncFunction as AsyncFunctionTrait,
    types::{
        AsyncError, AsyncResult, AsyncValue, ChannelReceiveStatus, ChannelSendStatus, TaskId,
        TaskPriority,
    },
};
use seen_effects::{
    effects::{
        EffectCost, EffectMetadata, EffectOperationId, EffectOperationMetadata, EffectParameter,
        EffectSafetyLevel,
    },
    EffectDefinition, EffectId, EffectOperation as EffectOp,
};
use seen_parser::{
    BinaryOperator, Expression, InterpolationKind, InterpolationPart, MatchArm, Pattern, Position,
    Program, UnaryOperator,
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Wrapper for Seen expressions to be executed as async functions
#[derive(Debug, Clone)]
struct SeenAsyncFunction {
    expression: Expression,
    position: Position,
}

impl AsyncFunctionTrait for SeenAsyncFunction {
    fn execute(
        &self,
        _context: &mut AsyncExecutionContext,
    ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        let expr = self.expression.clone();
        let pos = self.position.clone();

        Box::pin(async move {
            // Create a new interpreter instance for this task
            let mut interpreter = Interpreter::new();

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

/// The main interpreter for Seen programs
pub struct Interpreter {
    /// Runtime environment
    runtime: Runtime,
    /// Built-in functions registry
    builtins: BuiltinRegistry,
    /// Break/Continue flags for loop control
    break_flag: bool,
    continue_flag: bool,
    /// Return flag and value
    return_flag: bool,
    return_value: Option<Value>,
    /// Task counter for generating unique task IDs
    task_counter: std::sync::atomic::AtomicU64,
}

impl Interpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        Self {
            runtime: Runtime::new(),
            builtins: BuiltinRegistry::new(),
            break_flag: false,
            continue_flag: false,
            return_flag: false,
            return_value: None,
            task_counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Interpret a complete program
    pub fn interpret(&mut self, program: &Program) -> InterpreterResult<Value> {
        let mut last_value = Value::Unit;

        for expr in &program.expressions {
            last_value = self.interpret_expression(expr)?;

            // Check for early return
            if self.return_flag {
                return Ok(self.return_value.take().unwrap_or(Value::Unit));
            }
        }

        // Execute any deferred work registered at the top level. Keep draining until empty
        loop {
            let deferred = self
                .runtime
                .take_current_deferred()
                .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))?;
            if deferred.is_empty() {
                break;
            }
            self.run_deferred(deferred)?;
        }

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
                variable,
                iterable,
                body,
                ..
            } => self.interpret_for(variable, iterable, body),

            Expression::Loop { body, .. } => {
                let mut last_value = Value::Unit;
                loop {
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

            // Assignment
            Expression::Assignment { target, value, pos } => {
                let val = self.interpret_expression(value)?;

                match target.as_ref() {
                    Expression::Identifier { name, .. } => {
                        self.runtime
                            .set_variable(name, val.clone())
                            .map_err(|e| InterpreterError::runtime(e.to_string(), *pos))?;
                        Ok(val)
                    }
                    _ => Err(InterpreterError::runtime("Invalid assignment target", *pos)),
                }
            }

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
                Ok(Value::Array(values))
            }

            Expression::IndexAccess { object, index, pos } => {
                let arr_val = self.interpret_expression(object)?;
                let idx_val = self.interpret_expression(index)?;

                match arr_val {
                    Value::Array(arr) => {
                        if let Some(idx) = idx_val.as_integer() {
                            let idx = idx as usize;
                            if idx < arr.len() {
                                Ok(arr[idx].clone())
                            } else {
                                Err(InterpreterError::runtime("Array index out of bounds", *pos))
                            }
                        } else {
                            Err(InterpreterError::type_error(
                                "Array index must be an integer",
                                *pos,
                            ))
                        }
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

            // Structs
            Expression::StructLiteral { name, fields, .. } => {
                let mut field_map = HashMap::new();
                for (field_name, field_expr) in fields {
                    let value = self.interpret_expression(field_expr)?;
                    field_map.insert(field_name.clone(), value);
                }
                Ok(Value::Struct {
                    name: name.clone(),
                    fields: field_map,
                })
            }

            Expression::MemberAccess {
                object,
                member,
                is_safe,
                pos,
            } => {
                let obj_val = self.interpret_expression(object)?;

                // Handle safe navigation
                if *is_safe && matches!(obj_val, Value::Null) {
                    return Ok(Value::Null);
                }

                match obj_val {
                    Value::Struct { fields, .. } => fields.get(member).cloned().ok_or_else(|| {
                        InterpreterError::runtime(format!("Field '{}' not found", member), *pos)
                    }),
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
                name, fields, pos, ..
            } => self.interpret_actor_definition(name, fields, *pos),

            // Send expressions
            Expression::Send {
                target,
                message,
                pos,
            } => self.interpret_send(target, message, *pos),

            // Receive expressions
            Expression::Receive {
                pattern: _,
                handler,
                pos,
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

            // Additional expressions that need proper implementation
            Expression::FloatLiteral { value, .. } => Ok(Value::Float(*value)),

            Expression::InterpolatedString { parts, .. } => {
                let mut result = String::new();
                for part in parts {
                    match &part.kind {
                        seen_parser::InterpolationKind::Text(text) => result.push_str(text),
                        seen_parser::InterpolationKind::Expression(expr) => {
                            let value = self.interpret_expression(expr)?;
                            result.push_str(&value.to_string());
                        }
                    }
                }
                Ok(Value::String(result))
            }

            Expression::UnaryOp { op, operand, pos } => {
                let val = self.interpret_expression(operand)?;
                match op {
                    seen_parser::UnaryOperator::Negate => match val {
                        Value::Integer(i) => Ok(Value::Integer(-i)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => Err(InterpreterError::runtime(
                            "Invalid unary negation operand",
                            *pos,
                        )),
                    },
                    seen_parser::UnaryOperator::Not => Ok(Value::Boolean(!val.is_truthy())),
                }
            }

            Expression::Elvis {
                nullable,
                default,
                pos,
            } => {
                let nullable_val = self.interpret_expression(nullable)?;
                if matches!(nullable_val, Value::Null) {
                    self.interpret_expression(default)
                } else {
                    Ok(nullable_val)
                }
            }

            Expression::IndexAccess {
                object, index, pos, ..
            } => {
                let obj_val = self.interpret_expression(object)?;
                let idx_val = self.interpret_expression(index)?;

                match (obj_val, idx_val) {
                    (Value::Array(arr), Value::Integer(i)) => {
                        let idx = if i < 0 { arr.len() as i64 + i } else { i } as usize;

                        if idx < arr.len() {
                            Ok(arr[idx].clone())
                        } else {
                            Err(InterpreterError::runtime("Array index out of bounds", *pos))
                        }
                    }
                    _ => Err(InterpreterError::runtime("Invalid index access", *pos)),
                }
            }

            Expression::Lambda { params, body, .. } => {
                // Create a lambda value that can be called later
                let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                Ok(Value::Function {
                    name: "<lambda>".to_string(),
                    parameters: param_names,
                    body: body.clone(),
                    closure: HashMap::new(), // For now, empty closure
                })
            }

            Expression::Try {
                body,
                catch_clauses,
                finally,
                pos,
                ..
            } => {
                // Execute try block
                let result = self.interpret_expression(body);

                match result {
                    Ok(value) => {
                        // Success - execute finally if present and return value
                        if let Some(finally_block) = finally {
                            self.interpret_expression(finally_block)?;
                        }
                        Ok(value)
                    }
                    Err(error) => {
                        // Error occurred - try catch handlers
                        for catch in catch_clauses {
                            // In a full implementation, would match error types
                            // For now, just execute the first catch handler
                            let catch_result = self.interpret_expression(&catch.body);
                            if let Some(finally_block) = finally {
                                self.interpret_expression(finally_block)?;
                            }
                            return catch_result;
                        }

                        // No catch handler matched, execute finally and re-throw
                        if let Some(finally_block) = finally {
                            self.interpret_expression(finally_block)?;
                        }
                        Err(error)
                    }
                }
            }

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
            _ => {
                let expr_name = std::any::type_name::<Expression>();
                Err(InterpreterError::runtime(
                    &format!("Expression type not yet implemented: {}", expr_name),
                    Position::new(0, 0, 0),
                ))
            }
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
                _ => Ok(Value::Unit), // Other operators not implemented yet
            }
        }
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
        }
    }

    /// Interpret a for loop
    fn interpret_for(
        &mut self,
        variable: &str,
        iterable: &Expression,
        body: &Expression,
    ) -> InterpreterResult<Value> {
        let iter_val = self.interpret_expression(iterable)?;

        // Push new scope for loop variable
        self.runtime.push_environment(false);

        let result = match iter_val {
            Value::Array(arr) => {
                let mut last_value = Value::Unit;
                for item in arr {
                    self.runtime.define_variable(variable.to_string(), item);
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
                    self.runtime
                        .define_variable(variable.to_string(), Value::Character(ch));
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
        // Method-call lowering for common string/array operations
        if let Expression::MemberAccess { object, member, .. } = callee {
            let recv = self.interpret_expression(object)?;
            match (member.as_str(), args.len(), recv) {
                ("length", 0, Value::String(s)) | ("size", 0, Value::String(s)) => {
                    return Ok(Value::Integer(s.chars().count() as i64));
                }
                ("length", 0, Value::Array(arr)) | ("size", 0, Value::Array(arr)) => {
                    return Ok(Value::Integer(arr.len() as i64));
                }
                ("push", 1, Value::Array(mut arr)) => {
                    let elem = self.interpret_expression(&args[0])?;
                    arr.push(elem);
                    return Ok(Value::Array(arr));
                }
                ("pop", 0, Value::Array(mut arr)) => {
                    if !arr.is_empty() {
                        arr.pop();
                    }
                    return Ok(Value::Array(arr));
                }
                ("slice", 2, Value::Array(arr)) => {
                    let start = self
                        .interpret_expression(&args[0])?
                        .as_integer()
                        .ok_or_else(|| {
                            InterpreterError::type_error("slice start must be int", pos)
                        })? as usize;
                    let end = self
                        .interpret_expression(&args[1])?
                        .as_integer()
                        .ok_or_else(|| InterpreterError::type_error("slice end must be int", pos))?
                        as usize;
                    if start > end || end > arr.len() {
                        return Err(InterpreterError::runtime("slice out of bounds", pos));
                    }
                    let slice = arr[start..end].to_vec();
                    return Ok(Value::Array(slice));
                }
                ("endsWith", 1, Value::String(s)) => {
                    let suffix = self.interpret_expression(&args[0])?.to_string();
                    return Ok(Value::Boolean(s.ends_with(&suffix)));
                }
                ("substring", 2, Value::String(s)) => {
                    let start = self
                        .interpret_expression(&args[0])?
                        .as_integer()
                        .ok_or_else(|| {
                            InterpreterError::type_error("substring start must be int", pos)
                        })? as usize;
                    let end = self
                        .interpret_expression(&args[1])?
                        .as_integer()
                        .ok_or_else(|| {
                            InterpreterError::type_error("substring end must be int", pos)
                        })? as usize;
                    let chars: Vec<char> = s.chars().collect();
                    if start > end || end > chars.len() {
                        return Err(InterpreterError::runtime("substring out of bounds", pos));
                    }
                    let sub: String = chars[start..end].iter().collect();
                    return Ok(Value::String(sub));
                }
                (_, _, recv_val) => {
                    // Fallback: treat as free function with receiver as first arg
                    if let Expression::MemberAccess { member, .. } = callee {
                        let mut arg_values = Vec::new();
                        arg_values.push(recv_val);
                        for a in args {
                            arg_values.push(self.interpret_expression(a)?);
                        }
                        if self.builtins.is_builtin(member) {
                            return self.builtins.call(member, &arg_values, pos);
                        }
                    }
                }
            }
        }
        // Check if it's a built-in function call
        if let Expression::Identifier { name, .. } = callee {
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

            // Evaluate arguments
            let mut arg_values = Vec::new();
            for arg in args {
                arg_values.push(self.interpret_expression(arg)?);
            }

            // Push function environment
            self.runtime.push_environment(true);

            // Bind parameters
            for (param, value) in parameters.iter().zip(arg_values) {
                self.runtime.define_variable(param.clone(), value);
            }

            // Save current flags
            let prev_break = self.break_flag;
            let prev_continue = self.continue_flag;
            self.break_flag = false;
            self.continue_flag = false;

            // Execute function body
            let result = self.interpret_expression(&body);

            // Run any deferred expressions registered in this scope
            let deferred = self
                .runtime
                .take_current_deferred()
                .map_err(|e| InterpreterError::runtime(e.to_string(), pos.clone()))?;
            let defer_result = self.run_deferred(deferred);

            // Get return value if any
            let return_value = if self.return_flag {
                self.return_flag = false;
                self.return_value.take()
            } else {
                None
            };

            // Restore flags
            self.break_flag = prev_break;
            self.continue_flag = prev_continue;

            // Pop function environment
            self.runtime
                .pop_environment()
                .map_err(|e| InterpreterError::runtime(e.to_string(), pos))?;

            match result {
                Ok(val) => {
                    if let Err(err) = defer_result {
                        Err(err)
                    } else {
                        Ok(return_value.unwrap_or(val))
                    }
                }
                Err(e) => Err(e),
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
                // Execute the promise using the async runtime
                let async_runtime = self.runtime.async_runtime();
                let mut runtime = async_runtime.lock().unwrap();

                // Check if we can execute the promise synchronously
                // For promises that are already resolved, get their value
                // For pending promises, run a single iteration to try to resolve them
                runtime.run_single_iteration().map_err(|e| {
                    InterpreterError::runtime(
                        format!("Async execution failed: {:?}", e),
                        pos.clone(),
                    )
                })?;

                // In a real async context, promises would contain their resolved values
                // For now, we'll use the async value conversion system
                let async_result = runtime.run_until_complete().map_err(|e| {
                    InterpreterError::runtime(
                        format!("Async completion failed: {:?}", e),
                        pos.clone(),
                    )
                })?;
                Ok(self.async_value_to_value(&async_result))
            }
            Value::Task(task_id) => {
                // Execute and wait for task completion using async runtime
                let async_runtime = self.runtime.async_runtime();
                let mut runtime = async_runtime.lock().unwrap();

                // Run the async runtime until all pending tasks complete
                let async_result = runtime.run_until_complete().map_err(|e| {
                    InterpreterError::runtime(
                        format!("Task execution failed: {:?}", e),
                        pos.clone(),
                    )
                })?;

                // Convert the async result to an interpreter value
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

        // Create async function wrapper for the expression
        let async_function = Box::new(SeenAsyncFunction {
            expression: expr.clone(),
            position: pos.clone(),
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
                let job_system = self.runtime.job_system();
                let mut job_error: Option<InterpreterError> = None;
                let mut first_iteration = true;

                job_system.parallel_for_sequential(0, items.len(), |idx| {
                    if job_error.is_some() || self.return_flag {
                        return;
                    }

                    let item = items[idx].clone();

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

        // Try each case in order (simplified deterministic selection)
        for case in cases {
            // Evaluate the channel expression
            let channel_value = self.interpret_expression(&case.channel)?;

            match channel_value {
                Value::Channel(channel) => {
                    match channel.try_recv_with_status() {
                        ChannelReceiveStatus::Received(async_value) => {
                            let received_value = match async_value {
                                AsyncValue::Integer(i) => Value::Integer(i),
                                AsyncValue::String(s) => Value::String(s),
                                AsyncValue::Boolean(b) => Value::Boolean(b),
                                AsyncValue::Unit => Value::Unit,
                                other => self.async_value_to_value(&other),
                            };

                            if self.match_pattern(&case.pattern, &received_value) {
                                return self.interpret_expression(&case.handler);
                            }
                        }
                        ChannelReceiveStatus::WouldBlock => {
                            // No messages available; try next case
                            continue;
                        }
                        ChannelReceiveStatus::Closed => {
                            // Treat closed channels as non-ready in select
                            continue;
                        }
                        ChannelReceiveStatus::Error(err) => {
                            return Err(InterpreterError::runtime(
                                format!("Channel receive failed: {}", err),
                                pos.clone(),
                            ));
                        }
                    }
                }
                _ => {
                    return Err(InterpreterError::runtime(
                        "Select case must be a channel",
                        pos,
                    ));
                }
            }
        }

        // If no case matched, return Unit (in a real implementation, this would block)
        Ok(Value::Unit)
    }

    /// Extract emission values from flow body
    fn extract_flow_emissions(&mut self, body: &Expression) -> InterpreterResult<Vec<i32>> {
        let mut emissions = Vec::new();

        // Execute the flow body and collect emit() calls
        match body {
            Expression::Block { expressions, .. } => {
                for expr in expressions {
                    if let Expression::Call { callee, args, .. } = expr {
                        if let Expression::Identifier { name, .. } = callee.as_ref() {
                            if name == "emit" && !args.is_empty() {
                                // Extract the emission value
                                let emission_value = self.interpret_expression(&args[0])?;
                                if let Value::Integer(i) = emission_value {
                                    emissions.push(i as i32);
                                }
                            }
                        }
                    }
                }
            }
            Expression::Call { callee, args, .. } => {
                if let Expression::Identifier { name, .. } = callee.as_ref() {
                    if name == "emit" && !args.is_empty() {
                        let emission_value = self.interpret_expression(&args[0])?;
                        if let Value::Integer(i) = emission_value {
                            emissions.push(i as i32);
                        }
                    }
                }
            }
            _ => {
                // For other expressions, try to interpret as a single value
                let result = self.interpret_expression(body)?;
                if let Value::Integer(i) = result {
                    emissions.push(i as i32);
                }
            }
        }

        // If no emissions found, default to empty flow
        if emissions.is_empty() {
            emissions.push(0); // Default single emission
        }

        Ok(emissions)
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
        _fields: &[(String, seen_parser::ast::Type)],
        pos: Position,
    ) -> InterpreterResult<Value> {
        // Generate unique actor ID
        let actor_id = seen_concurrency::types::ActorId::allocate();
        let mailbox = Arc::new(seen_concurrency::types::Mailbox {
            messages: std::sync::Mutex::new(std::collections::VecDeque::new()),
            capacity: None,
        });
        let actor_ref = seen_concurrency::types::ActorRef::new(actor_id, name.to_string(), mailbox);

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
                // Send message to actor mailbox
                let message = seen_concurrency::types::ActorMessage {
                    sender: None, // Anonymous sender for now
                    content: match message_value {
                        Value::Integer(i) => AsyncValue::Integer(i),
                        Value::String(s) => AsyncValue::String(s),
                        Value::Boolean(b) => AsyncValue::Boolean(b),
                        Value::Unit => AsyncValue::Unit,
                        _ => AsyncValue::Error, // Fallback for unsupported types
                    },
                    timestamp: std::time::SystemTime::now(),
                    priority: TaskPriority::Normal,
                };

                // Add message to actor's mailbox
                match actor_ref.mailbox() {
                    Ok(mailbox_arc) => {
                        if let Ok(mut mailbox) = mailbox_arc.messages.lock() {
                            mailbox.push_back(message);
                            Ok(Value::Boolean(true)) // Success
                        } else {
                            Err(InterpreterError::runtime(
                                "Failed to access actor mailbox",
                                pos,
                            ))
                        }
                    }
                    Err(err) => Err(InterpreterError::runtime(&err, pos)),
                }
            }
            Value::Channel(channel) => {
                // Send to channel
                let async_value = match message_value {
                    Value::Integer(i) => AsyncValue::Integer(i),
                    Value::String(s) => AsyncValue::String(s),
                    Value::Boolean(b) => AsyncValue::Boolean(b),
                    Value::Unit => AsyncValue::Unit,
                    _ => AsyncValue::Error, // Fallback for unsupported types
                };

                match channel.send_with_status(async_value) {
                    ChannelSendStatus::Sent => Ok(Value::Boolean(true)),
                    ChannelSendStatus::WouldBlock => Err(InterpreterError::runtime(
                        "Channel send would block (buffer full)",
                        pos.clone(),
                    )),
                    ChannelSendStatus::Closed => Err(InterpreterError::runtime(
                        "Channel send failed: channel is closed",
                        pos.clone(),
                    )),
                    ChannelSendStatus::Error(err) => Err(InterpreterError::runtime(
                        format!("Channel send failed: {}", err),
                        pos,
                    )),
                }
            }
            _ => Err(InterpreterError::runtime(
                "Can only send to actors or channels",
                pos,
            )),
        }
    }

    /// Convert AsyncValue to interpreter Value
    fn async_value_to_value(&self, async_value: &AsyncValue) -> Value {
        match async_value {
            AsyncValue::Unit => Value::Unit,
            AsyncValue::Integer(i) => Value::Integer(*i),
            AsyncValue::Float(f) => Value::Float(*f),
            AsyncValue::Boolean(b) => Value::Boolean(*b),
            AsyncValue::String(s) => Value::String(s.clone()),
            AsyncValue::Array(arr) => {
                let values: Vec<Value> = arr.iter().map(|v| self.async_value_to_value(v)).collect();
                Value::Array(values)
            }
            AsyncValue::Promise(promise) => Value::Promise(Arc::clone(promise)),
            AsyncValue::Channel(channel) => Value::Channel(channel.clone()),
            AsyncValue::Actor(actor) => Value::Actor(actor.clone()),
            AsyncValue::Error => Value::Null, // Map error to null for now
            AsyncValue::Pending => Value::Unit, // Map pending to unit
        }
    }

    /// Interpret effect definition
    fn interpret_effect_definition(
        &mut self,
        name: &str,
        operations: &[seen_parser::ast::EffectOperation],
        pos: Position,
    ) -> InterpreterResult<Value> {
        // Create effect definition
        let mut effect_operations = HashMap::new();

        for (idx, op) in operations.iter().enumerate() {
            let effect_op = EffectOp {
                id: EffectOperationId::new(idx as u64),
                name: op.name.clone(),
                parameters: op
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
                    .collect(),
                return_type: op
                    .return_type
                    .clone()
                    .unwrap_or(seen_parser::ast::Type::new("Unit")),
                is_pure: false,
                metadata: EffectOperationMetadata {
                    position: pos,
                    documentation: None,
                    performance_cost: EffectCost::Constant,
                    can_fail: false,
                },
            };
            effect_operations.insert(op.name.clone(), effect_op);
        }

        let effect_def = Arc::new(EffectDefinition {
            id: EffectId::new(1), // Simplified ID generation
            name: name.to_string(),
            operations: effect_operations,
            metadata: EffectMetadata {
                is_public: name.chars().next().map_or(false, |c| c.is_uppercase()),
                position: pos,
                documentation: None,
                is_composable: true,
                safety_level: EffectSafetyLevel::Safe,
            },
            type_parameters: Vec::new(),
        });

        // Register effect with runtime
        let advanced_runtime = self.runtime.advanced_runtime();
        // For now, just return the effect definition

        Ok(Value::Effect(effect_def))
    }

    /// Interpret handle expression for effects
    fn interpret_handle(
        &mut self,
        body: &Expression,
        effect: &str,
        handlers: &[seen_parser::ast::EffectHandler],
        pos: Position,
    ) -> InterpreterResult<Value> {
        // Set up effect handlers
        let mut handler_map = HashMap::new();

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

        // Create effect handle context
        let effect_handle = Value::EffectHandle {
            effect_id: EffectId::new(1), // Simplified
            handlers: handler_map,
        };

        // Push effect handle to runtime
        // Execute body with effect handlers in scope
        let result = self.interpret_expression(body)?;

        // Pop effect handle from runtime

        Ok(result)
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
                    // Convert Value array to AsyncValue array for reactive runtime
                    let async_values: Vec<seen_concurrency::types::AsyncValue> = values
                        .iter()
                        .map(|v| self.value_to_async_value(v))
                        .collect();
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
        pos: Position,
    ) -> InterpreterResult<Value> {
        let reactive_runtime = self.runtime.reactive_runtime();
        let mut runtime = reactive_runtime.lock().unwrap();

        // Parse the flow body to extract emit() and delay() calls
        let flow_values = self.extract_flow_emissions(body)?;
        let flow = runtime.create_flow_from_vec(flow_values);
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
            Value::Observable(obs) => {
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
        match value {
            Value::Unit => AsyncValue::Unit,
            Value::Integer(i) => AsyncValue::Integer(*i),
            Value::Float(f) => AsyncValue::Float(*f),
            Value::Boolean(b) => AsyncValue::Boolean(*b),
            Value::String(s) => AsyncValue::String(s.clone()),
            Value::Array(arr) => {
                let async_values: Vec<AsyncValue> =
                    arr.iter().map(|v| self.value_to_async_value(v)).collect();
                AsyncValue::Array(async_values)
            }
            Value::Promise(promise) => AsyncValue::Promise(Arc::clone(promise)),
            Value::Channel(channel) => AsyncValue::Channel(channel.clone()),
            Value::Actor(actor) => AsyncValue::Actor(actor.clone()),
            _ => AsyncValue::Unit, // Other types map to Unit for now
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_concurrency::types::{Channel, ChannelId};

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
    fn channel_send_would_block_emits_runtime_error() {
        let mut interpreter = Interpreter::new();
        let channel = Channel::new(ChannelId::allocate(), Some(1));
        interpreter
            .runtime
            .define_variable("tx".to_string(), Value::Channel(channel));

        let pos = Position::start();
        let send_ok = send_expression(1, pos.clone());
        interpreter
            .interpret_expression(&send_ok)
            .expect("first send should succeed");

        let send_err = send_expression(2, pos.clone());
        let error = interpreter
            .interpret_expression(&send_err)
            .expect_err("second send should fail");

        match error {
            InterpreterError::RuntimeError { message, .. } => {
                assert!(
                    message.contains("would block"),
                    "expected would-block message, got {message}"
                );
            }
            other => panic!("expected runtime error, got {:?}", other),
        }
    }
}
