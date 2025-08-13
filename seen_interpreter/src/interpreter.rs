//! Main interpreter implementation for the Seen programming language

use std::collections::HashMap;
use seen_parser::{Program, Expression, BinaryOperator, UnaryOperator, Pattern, MatchArm, InterpolationPart, InterpolationKind, Position};
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