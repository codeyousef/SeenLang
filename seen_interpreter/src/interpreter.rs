//! Main interpreter implementation for the Seen programming language

use std::collections::HashMap;
use std::sync::Arc;
use seen_parser::{Program, Expression, BinaryOperator, UnaryOperator, Pattern, MatchArm, InterpolationPart, InterpolationKind, Position};
use seen_concurrency::{
    types::{Promise, TaskPriority, AsyncValue},
    async_runtime::AsyncFunction as AsyncFunctionTrait,
};
use seen_effects::{
    EffectDefinition, EffectOperation as EffectOp, 
    EffectId, effects::{EffectParameter, EffectMetadata, EffectOperationMetadata, EffectOperationId, EffectCost, EffectSafetyLevel},
};
use seen_reactive::{
    Observable, Flow, ReactiveProperty,
    properties::PropertyId,
};
use crate::value::Value;
use crate::runtime::Runtime;
use crate::errors::{InterpreterError, InterpreterResult};
use crate::builtins::BuiltinRegistry;

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
        
        Ok(last_value)
    }

    /// Interpret an expression
    pub fn interpret_expression(&mut self, expr: &Expression) -> InterpreterResult<Value> {
        // Check for break/continue/return flags
        if self.break_flag || self.continue_flag || self.return_flag {
            return Ok(Value::Unit);
        }
        
        match expr {
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
                    self.runtime.get_variable(name)
                        .map_err(|e| InterpreterError::runtime(e.to_string(), *pos))
                }
            }
            
            // Binary operations
            Expression::BinaryOp { left, op, right, pos } => {
                self.interpret_binary_op(left, op.clone(), right, *pos)
            }
            
            // Unary operations
            Expression::UnaryOp { op, operand, pos } => {
                self.interpret_unary_op(op.clone(), operand, *pos)
            }
            
            // Control flow
            Expression::If { condition, then_branch, else_branch, .. } => {
                let cond_value = self.interpret_expression(condition)?;
                if cond_value.is_truthy() {
                    self.interpret_expression(then_branch)
                } else if let Some(else_expr) = else_branch {
                    self.interpret_expression(else_expr)
                } else {
                    Ok(Value::Null)
                }
            }
            
            Expression::While { condition, body, .. } => {
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
            
            Expression::For { variable, iterable, body, .. } => {
                self.interpret_for(variable, iterable, body)
            }
            
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
            Expression::Block { expressions, .. } => {
                self.interpret_block(expressions)
            }
            
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
                        self.runtime.set_variable(name, val.clone())
                            .map_err(|e| InterpreterError::runtime(e.to_string(), *pos))?;
                        Ok(val)
                    }
                    _ => Err(InterpreterError::runtime("Invalid assignment target", *pos))
                }
            }
            
            // Function definition
            Expression::Function { name, params, body, .. } => {
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
            Expression::Call { callee, args, pos } => {
                self.interpret_call(callee, args, *pos)
            }
            
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
                            Err(InterpreterError::type_error("Array index must be an integer", *pos))
                        }
                    }
                    Value::String(s) => {
                        if let Some(idx) = idx_val.as_integer() {
                            let idx = idx as usize;
                            let chars: Vec<char> = s.chars().collect();
                            if idx < chars.len() {
                                Ok(Value::Character(chars[idx]))
                            } else {
                                Err(InterpreterError::runtime("String index out of bounds", *pos))
                            }
                        } else {
                            Err(InterpreterError::type_error("String index must be an integer", *pos))
                        }
                    }
                    _ => Err(InterpreterError::type_error(
                        format!("Cannot index {}", arr_val.type_name()),
                        *pos,
                    ))
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
            
            Expression::MemberAccess { object, member, is_safe, pos } => {
                let obj_val = self.interpret_expression(object)?;
                
                // Handle safe navigation
                if *is_safe && matches!(obj_val, Value::Null) {
                    return Ok(Value::Null);
                }
                
                match obj_val {
                    Value::Struct { fields, .. } => {
                        fields.get(member)
                            .cloned()
                            .ok_or_else(|| InterpreterError::runtime(
                                format!("Field '{}' not found", member),
                                *pos,
                            ))
                    }
                    _ => Err(InterpreterError::type_error(
                        format!("Cannot access field on {}", obj_val.type_name()),
                        *pos,
                    ))
                }
            }
            
            // Pattern matching
            Expression::Match { expr, arms, .. } => {
                self.interpret_match(expr, arms)
            }
            
            // String interpolation
            Expression::InterpolatedString { parts, .. } => {
                self.interpret_interpolated_string(parts)
            }
            
            Expression::Elvis { nullable, default, .. } => {
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
            Expression::Await { expr, pos } => {
                self.interpret_await(expr, *pos)
            }
            
            // Spawn expressions for concurrency
            Expression::Spawn { expr, pos } => {
                self.interpret_spawn(expr, *pos)
            }
            
            // Select expressions for channel operations
            Expression::Select { cases, pos } => {
                self.interpret_select(cases, *pos)
            }
            
            // Actor definitions
            Expression::Actor { name, fields, pos, .. } => {
                self.interpret_actor_definition(name, fields, *pos)
            }
            
            // Send expressions
            Expression::Send { target, message, pos } => {
                self.interpret_send(target, message, *pos)
            }
            
            // Receive expressions
            Expression::Receive { pattern: _, handler, pos } => {
                // Simplified receive implementation
                self.interpret_expression(handler)
            }
            
            // Effect definition
            Expression::Effect { name, operations, pos } => {
                self.interpret_effect_definition(name, operations, *pos)
            }
            
            // Handle expression for effects
            Expression::Handle { body, effect, handlers, pos } => {
                self.interpret_handle(body, effect, handlers, *pos)
            }
            
            // Contract-annotated function
            Expression::ContractedFunction { function, requires, ensures, invariants, pos } => {
                self.interpret_contracted_function(function, requires, ensures, invariants, *pos)
            }
            
            // Observable creation (Seen syntax: Observable.Range(1, 10))
            Expression::ObservableCreation { source, pos } => {
                self.interpret_observable_creation(source, *pos)
            }
            
            // Flow creation (Seen syntax: flow { emit(1); delay(100ms) })
            Expression::FlowCreation { body, pos } => {
                self.interpret_flow_creation(body, *pos)
            }
            
            // Reactive property (Seen syntax: @Reactive var Username = "")
            Expression::ReactiveProperty { name, value, is_computed, pos } => {
                self.interpret_reactive_property(name, value, *is_computed, *pos)
            }
            
            // Stream operations (Map, Filter, etc.)
            Expression::StreamOperation { stream, operation, pos } => {
                self.interpret_stream_operation(stream, operation, *pos)
            }
            
            // All other expressions return Unit for now
            _ => Ok(Value::Unit),
        }
    }

    /// Interpret a binary operation
    fn interpret_binary_op(&mut self, left: &Expression, op: BinaryOperator, right: &Expression, pos: Position) -> InterpreterResult<Value> {
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
                BinaryOperator::Add => left_val.add(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::Subtract => left_val.subtract(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::Multiply => left_val.multiply(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::Divide => left_val.divide(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::Modulo => {
                    match (&left_val, &right_val) {
                        (Value::Integer(a), Value::Integer(b)) if *b != 0 => {
                            Ok(Value::Integer(a % b))
                        }
                        (Value::Integer(_), Value::Integer(0)) => {
                            Err(InterpreterError::division_by_zero(pos))
                        }
                        _ => Err(InterpreterError::type_error(
                            "Modulo requires integer operands",
                            pos,
                        ))
                    }
                }
                BinaryOperator::Equal => Ok(Value::Boolean(left_val.equals(&right_val))),
                BinaryOperator::NotEqual => Ok(Value::Boolean(!left_val.equals(&right_val))),
                BinaryOperator::Less => left_val.less_than(&right_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::LessEqual => {
                    match left_val.less_than(&right_val) {
                        Ok(Value::Boolean(lt)) => Ok(Value::Boolean(lt || left_val.equals(&right_val))),
                        Ok(_) => Err(InterpreterError::runtime("Invalid comparison result", pos)),
                        Err(e) => Err(InterpreterError::runtime(e, pos)),
                    }
                }
                BinaryOperator::Greater => right_val.less_than(&left_val)
                    .map_err(|e| InterpreterError::runtime(e, pos)),
                BinaryOperator::GreaterEqual => {
                    match right_val.less_than(&left_val) {
                        Ok(Value::Boolean(gt)) => Ok(Value::Boolean(gt || left_val.equals(&right_val))),
                        Ok(_) => Err(InterpreterError::runtime("Invalid comparison result", pos)),
                        Err(e) => Err(InterpreterError::runtime(e, pos)),
                    }
                }
                _ => Ok(Value::Unit), // Other operators not implemented yet
            }
        }
    }

    /// Interpret a unary operation
    fn interpret_unary_op(&mut self, op: UnaryOperator, operand: &Expression, pos: Position) -> InterpreterResult<Value> {
        let val = self.interpret_expression(operand)?;
        
        match op {
            UnaryOperator::Negate => val.negate()
                .map_err(|e| InterpreterError::runtime(e, pos)),
            UnaryOperator::Not => Ok(val.logical_not()),
        }
    }

    /// Interpret a for loop
    fn interpret_for(&mut self, variable: &str, iterable: &Expression, body: &Expression) -> InterpreterResult<Value> {
        let iter_val = self.interpret_expression(iterable)?;
        let mut last_value = Value::Unit;
        
        // Push new scope for loop variable
        self.runtime.push_environment(false);
        
        let result = match iter_val {
            Value::Array(arr) => {
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
                for ch in s.chars() {
                    self.runtime.define_variable(variable.to_string(), Value::Character(ch));
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
        
        // Pop loop scope
        self.runtime.pop_environment()
            .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))?;
        
        result
    }

    /// Interpret a block expression
    fn interpret_block(&mut self, expressions: &[Expression]) -> InterpreterResult<Value> {
        self.runtime.push_environment(false);
        
        let mut last_value = Value::Unit;
        for expr in expressions {
            last_value = self.interpret_expression(expr)?;
            
            // Check for early exit
            if self.break_flag || self.continue_flag || self.return_flag {
                break;
            }
        }
        
        self.runtime.pop_environment()
            .map_err(|e| InterpreterError::runtime(e.to_string(), Position::start()))?;
        
        Ok(last_value)
    }

    /// Interpret a function/method call
    fn interpret_call(&mut self, callee: &Expression, args: &[Expression], pos: Position) -> InterpreterResult<Value> {
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
        
        if let Value::Function { name, parameters, body, .. } = func_val {
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
            self.runtime.pop_environment()
                .map_err(|e| InterpreterError::runtime(e.to_string(), pos))?;
            
            match result {
                Ok(val) => Ok(return_value.unwrap_or(val)),
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
    fn interpret_match(&mut self, value: &Expression, arms: &[MatchArm]) -> InterpreterResult<Value> {
        let val = self.interpret_expression(value)?;
        
        for arm in arms {
            if self.pattern_matches(&val, &arm.pattern) {
                return self.interpret_expression(&arm.body);
            }
        }
        
        Err(InterpreterError::runtime("No matching pattern", Position::start()))
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
    fn interpret_interpolated_string(&mut self, parts: &[InterpolationPart]) -> InterpreterResult<Value> {
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
                // Block until promise is resolved
                while promise.is_pending() {
                    // In a real implementation, this would yield to the async runtime
                    // For now, we'll return a placeholder
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                
                if promise.is_resolved() {
                    if let Some(async_value) = promise.value() {
                        // Convert AsyncValue to Value
                        Ok(self.async_value_to_value(async_value))
                    } else {
                        Ok(Value::Unit)
                    }
                } else {
                    Err(InterpreterError::runtime("Promise was rejected", pos))
                }
            }
            Value::Task(task_id) => {
                // Wait for task completion
                let runtime = self.runtime.async_runtime();
                // For now, return the task ID as a placeholder
                Ok(Value::Task(task_id))
            }
            _ => Err(InterpreterError::runtime("Cannot await non-promise value", pos))
        }
    }
    
    /// Interpret spawn expression
    fn interpret_spawn(&mut self, expr: &Expression, pos: Position) -> InterpreterResult<Value> {
        // Create a simple async function from the expression
        let async_runtime = self.runtime.async_runtime();
        
        // For now, we'll create a basic task
        // In a full implementation, this would create an actual async task
        let task_id = seen_concurrency::types::TaskId::new(1);
        
        // Store the expression to execute later (simplified)
        Ok(Value::Task(task_id))
    }
    
    /// Interpret select expression for channel operations
    fn interpret_select(&mut self, cases: &[seen_parser::ast::SelectCase], pos: Position) -> InterpreterResult<Value> {
        // Simplified select implementation
        // In a real implementation, this would handle channel operations
        for _case in cases {
            // Process each case
        }
        Ok(Value::Unit)
    }
    
    /// Interpret actor definition
    fn interpret_actor_definition(&mut self, name: &str, _fields: &[(String, seen_parser::ast::Type)], pos: Position) -> InterpreterResult<Value> {
        // Simplified actor definition
        let actor_system = self.runtime.actor_system();
        
        // Create a placeholder actor
        let actor_id = seen_concurrency::types::ActorId::new(1);
        let actor_ref = seen_concurrency::types::ActorRef {
            id: actor_id,
            actor_type: name.to_string(),
            mailbox: Arc::new(seen_concurrency::types::Mailbox {
                messages: std::sync::Mutex::new(std::collections::VecDeque::new()),
                capacity: None,
            }),
        };
        
        Ok(Value::Actor(actor_ref))
    }
    
    /// Interpret send expression
    fn interpret_send(&mut self, target: &Expression, message: &Expression, pos: Position) -> InterpreterResult<Value> {
        let target_value = self.interpret_expression(target)?;
        let message_value = self.interpret_expression(message)?;
        
        match target_value {
            Value::Actor(actor_ref) => {
                // Send message to actor
                // For now, just return success
                Ok(Value::Unit)
            }
            Value::Channel(channel) => {
                // Send to channel
                // For now, just return success
                Ok(Value::Unit)
            }
            _ => Err(InterpreterError::runtime("Can only send to actors or channels", pos))
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
                let values: Vec<Value> = arr.iter()
                    .map(|v| self.async_value_to_value(v))
                    .collect();
                Value::Array(values)
            }
            AsyncValue::Promise(promise) => Value::Promise(Arc::clone(promise)),
            AsyncValue::Channel(channel) => Value::Channel(Arc::clone(channel)),
            AsyncValue::Actor(actor) => Value::Actor(actor.clone()),
            AsyncValue::Error => Value::Null, // Map error to null for now
            AsyncValue::Pending => Value::Unit, // Map pending to unit
        }
    }
    
    /// Interpret effect definition
    fn interpret_effect_definition(&mut self, name: &str, operations: &[seen_parser::ast::EffectOperation], pos: Position) -> InterpreterResult<Value> {
        // Create effect definition
        let mut effect_operations = HashMap::new();
        
        for (idx, op) in operations.iter().enumerate() {
            let effect_op = EffectOp {
                id: EffectOperationId::new(idx as u64),
                name: op.name.clone(),
                parameters: op.params.iter().map(|p| EffectParameter {
                    name: p.name.clone(),
                    param_type: p.type_annotation.clone().unwrap_or(seen_parser::ast::Type::new("Any")),
                    is_mutable: false,
                    default_value: None,
                }).collect(),
                return_type: op.return_type.clone().unwrap_or(seen_parser::ast::Type::new("Unit")),
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
    fn interpret_handle(&mut self, body: &Expression, effect: &str, handlers: &[seen_parser::ast::EffectHandler], pos: Position) -> InterpreterResult<Value> {
        // Set up effect handlers
        let mut handler_map = HashMap::new();
        
        for handler in handlers {
            // Store handler implementation
            handler_map.insert(handler.operation.clone(), Value::Unit); // Simplified
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
    fn interpret_observable_creation(&mut self, source: &seen_parser::ast::ObservableSource, pos: Position) -> InterpreterResult<Value> {
        let reactive_runtime = self.runtime.reactive_runtime();
        let mut runtime = reactive_runtime.lock().unwrap();
        
        match source {
            seen_parser::ast::ObservableSource::Range { start, end, step } => {
                // Create observable from range
                let start_val = self.interpret_expression(start)?;
                let end_val = self.interpret_expression(end)?;
                let step_val = step.as_ref()
                    .map(|s| self.interpret_expression(s))
                    .transpose()?
                    .unwrap_or(Value::Integer(1));
                
                if let (Some(s), Some(e), Some(st)) = (
                    start_val.as_integer(),
                    end_val.as_integer(),
                    step_val.as_integer()
                ) {
                    let observable = runtime.create_observable_range(s as i32, e as i32, st as i32);
                    // Box the observable and wrap in Arc
                    let boxed: Box<dyn std::any::Any + Send + Sync> = Box::new(observable);
                    Ok(Value::Observable(Arc::new(boxed)))
                } else {
                    Err(InterpreterError::runtime("Observable.Range requires integer arguments", pos))
                }
            }
            seen_parser::ast::ObservableSource::FromArray(array_expr) => {
                // Create observable from array
                let array_val = self.interpret_expression(array_expr)?;
                if let Value::Array(values) = array_val {
                    // Convert Value array to AsyncValue array for reactive runtime
                    let async_values: Vec<seen_concurrency::types::AsyncValue> = values.iter()
                        .map(|v| self.value_to_async_value(v))
                        .collect();
                    let observable = runtime.create_observable_from_vec(async_values);
                    let boxed: Box<dyn std::any::Any + Send + Sync> = Box::new(observable);
                    Ok(Value::Observable(Arc::new(boxed)))
                } else {
                    Err(InterpreterError::runtime("Observable.FromArray requires an array", pos))
                }
            }
            _ => Ok(Value::Unit), // Other sources not implemented yet
        }
    }
    
    /// Interpret flow creation
    fn interpret_flow_creation(&mut self, body: &Expression, pos: Position) -> InterpreterResult<Value> {
        let reactive_runtime = self.runtime.reactive_runtime();
        let mut runtime = reactive_runtime.lock().unwrap();
        
        // For now, create a simple flow
        // In a full implementation, this would parse the flow body for emit() and delay() calls
        let flow = runtime.create_flow_from_vec(vec![1, 2, 3]);
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
                // Apply operation to observable
                // This is simplified - real implementation would transform the observable
                match operation {
                    seen_parser::ast::StreamOp::Map(_) => {
                        // Map operation
                        Ok(Value::Observable(obs))
                    }
                    seen_parser::ast::StreamOp::Filter(_) => {
                        // Filter operation
                        Ok(Value::Observable(obs))
                    }
                    seen_parser::ast::StreamOp::Throttle(_duration) => {
                        // Throttle operation
                        Ok(Value::Observable(obs))
                    }
                    _ => Ok(Value::Observable(obs)),
                }
            }
            Value::Flow(flow) => {
                // Apply operation to flow
                Ok(Value::Flow(flow))
            }
            _ => Err(InterpreterError::runtime("Stream operations require Observable or Flow", pos)),
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
                let async_values: Vec<AsyncValue> = arr.iter()
                    .map(|v| self.value_to_async_value(v))
                    .collect();
                AsyncValue::Array(async_values)
            }
            Value::Promise(promise) => AsyncValue::Promise(Arc::clone(promise)),
            Value::Channel(channel) => AsyncValue::Channel(Arc::clone(channel)),
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
    
    #[test]
    fn test_interpreter_creation() {
        let interpreter = Interpreter::new();
        let _ = interpreter; // Use the value
    }
}