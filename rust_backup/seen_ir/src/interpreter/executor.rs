//! Instruction executor for the IR interpreter
//!
//! This module contains the main execution loop and handlers for each
//! IR instruction type.

use super::error::{InterpreterError, InterpreterResult, ErrorLocation, StackFrame};
use super::memory::Address;
use super::runtime::{Runtime, RuntimeConfig, CallFrame};
use super::value::{InterpreterValue, ValueType};
use super::{InterpreterConfig, MemoryStats};

use crate::instruction::{Instruction, BinaryOp, UnaryOp, Label, BasicBlock};
use crate::module::IRModule;
use crate::function::IRFunction;
use crate::value::{IRValue, IRType};

use std::collections::HashMap;

/// Result of executing a function
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub return_value: InterpreterValue,
    pub instructions_executed: u64,
    pub memory_stats: MemoryStats,
}

/// Result of stepping through one instruction
#[derive(Debug, Clone)]
pub enum StepResult {
    /// Continue execution
    Continue,
    /// Returned from function with value
    Return(InterpreterValue),
    /// Hit a breakpoint
    Breakpoint,
    /// Execution complete
    Done,
}

/// The instruction executor
pub struct Executor<'a> {
    config: &'a InterpreterConfig,
    runtime: &'a mut Runtime,
    /// Label to instruction index mapping for current function
    label_map: HashMap<String, usize>,
    /// Current module being executed
    current_module: Option<&'a IRModule>,
}

impl<'a> Executor<'a> {
    pub fn new(config: &'a InterpreterConfig, runtime: &'a mut Runtime) -> Self {
        Self {
            config,
            runtime,
            label_map: HashMap::new(),
            current_module: None,
        }
    }

    /// Execute a function and return its result
    pub fn execute_function(
        mut self,
        module: &'a IRModule,
        function_name: &str,
        args: Vec<InterpreterValue>,
    ) -> Result<InterpreterValue, InterpreterError> {
        self.current_module = Some(module);

        // Find the function
        let function = module.get_function(function_name)
            .ok_or_else(|| InterpreterError::undefined_function(function_name))?;

        // Check argument count
        if args.len() != function.parameters.len() {
            return Err(InterpreterError::argument_count_mismatch(
                function_name,
                function.parameters.len(),
                args.len(),
            ));
        }

        // Create call frame
        let mut frame = CallFrame::new(function_name.to_string(), None);

        // Bind arguments to parameters
        for (param, arg) in function.parameters.iter().zip(args.into_iter()) {
            frame.set_local(param.name.clone(), arg);
        }

        self.runtime.push_frame(frame)?;

        // Build label map for this function
        self.build_label_map(function);

        // Execute the function
        let result = self.execute_function_body(function)?;

        self.runtime.pop_frame();

        Ok(result)
    }

    /// Build a map of labels to instruction indices
    fn build_label_map(&mut self, function: &IRFunction) {
        self.label_map.clear();
        
        // Flatten all instructions and track labels
        let mut index = 0;
        for block in function.cfg.block_order.iter() {
            if let Some(bb) = function.cfg.get_block(block) {
                // The label is at this index
                self.label_map.insert(bb.label.0.clone(), index);
                index += bb.instructions.len();
                if bb.terminator.is_some() {
                    index += 1;
                }
            }
        }
    }

    /// Get all instructions in order from a function (cloned to avoid borrow issues)
    fn get_instructions(&self, function: &IRFunction) -> Vec<Instruction> {
        let mut instructions = Vec::new();
        for block_name in &function.cfg.block_order {
            if let Some(block) = function.cfg.get_block(block_name) {
                for inst in &block.instructions {
                    instructions.push(inst.clone());
                }
                if let Some(ref term) = block.terminator {
                    instructions.push(term.clone());
                }
            }
        }
        instructions
    }

    /// Execute the body of a function
    fn execute_function_body(&mut self, function: &IRFunction) -> Result<InterpreterValue, InterpreterError> {
        let instructions = self.get_instructions(function);
        
        if instructions.is_empty() {
            return Ok(InterpreterValue::Void);
        }

        let mut ip = 0; // Instruction pointer
        let instruction_count = instructions.len();

        while ip < instruction_count {
            // Check instruction limit
            if let Some(limit) = self.config.max_instructions {
                if self.runtime.instructions_executed() >= limit {
                    return Err(InterpreterError::instruction_limit(limit));
                }
            }

            // Check stack depth
            if self.runtime.call_stack_depth() > self.config.max_stack_depth {
                return Err(InterpreterError::stack_overflow(
                    self.runtime.call_stack_depth(),
                    self.config.max_stack_depth,
                ));
            }

            let instruction = &instructions[ip];
            
            // Trace if enabled
            if self.config.trace_execution {
                eprintln!("[TRACE] ip={}: {:?}", ip, instruction);
            }

            // Update instruction pointer in frame
            if let Some(frame) = self.runtime.current_frame_mut() {
                frame.instruction_pointer = ip;
            }

            self.runtime.increment_instructions();

            // Execute the instruction
            match self.execute_instruction(instruction)? {
                InstructionResult::Continue => {
                    ip += 1;
                }
                InstructionResult::Jump(label) => {
                    ip = *self.label_map.get(&label)
                        .ok_or_else(|| InterpreterError::new(
                            super::error::InterpreterErrorKind::MissingLabel,
                            format!("Label not found: {}", label),
                        ))?;
                }
                InstructionResult::Return(value) => {
                    return Ok(value);
                }
            }
        }

        // Reached end without return
        Ok(InterpreterValue::Void)
    }

    /// Execute a single instruction
    fn execute_instruction(&mut self, instruction: &Instruction) -> Result<InstructionResult, InterpreterError> {
        match instruction {
            // Control flow
            Instruction::Label(_) => Ok(InstructionResult::Continue),
            
            Instruction::Jump(label) => {
                Ok(InstructionResult::Jump(label.0.clone()))
            }
            
            Instruction::JumpIf { condition, target } => {
                let cond_value = self.resolve_value(condition)?;
                if cond_value.is_truthy() {
                    Ok(InstructionResult::Jump(target.0.clone()))
                } else {
                    Ok(InstructionResult::Continue)
                }
            }
            
            Instruction::JumpIfNot { condition, target } => {
                let cond_value = self.resolve_value(condition)?;
                if !cond_value.is_truthy() {
                    Ok(InstructionResult::Jump(target.0.clone()))
                } else {
                    Ok(InstructionResult::Continue)
                }
            }
            
            Instruction::Return(value) => {
                let ret_val = match value {
                    Some(v) => self.resolve_value(v)?,
                    None => InterpreterValue::Void,
                };
                Ok(InstructionResult::Return(ret_val))
            }

            // Data operations
            Instruction::Load { source, dest } => {
                let addr = self.resolve_value(source)?.as_pointer()?;
                let value = self.runtime.read_i64(addr)?;
                self.store_to_dest(dest, InterpreterValue::integer(value))?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::Store { value, dest } => {
                let val = self.resolve_value(value)?;
                // Store can either be to a variable/register or to memory via pointer
                match dest {
                    IRValue::Variable(_) | IRValue::Register(_) | IRValue::GlobalVariable(_) => {
                        self.store_to_dest(dest, val)?;
                    }
                    _ => {
                        // Treat as memory store via pointer
                        let addr = self.resolve_value(dest)?.as_pointer()?;
                        let int_val = val.as_integer()?;
                        self.runtime.write_i64(addr, int_val)?;
                    }
                }
                Ok(InstructionResult::Continue)
            }
            
            Instruction::Move { source, dest } => {
                let value = self.resolve_value(source)?;
                self.store_to_dest(dest, value)?;
                Ok(InstructionResult::Continue)
            }

            // Arithmetic and logic
            Instruction::Binary { op, left, right, result } => {
                let lhs = self.resolve_value(left)?;
                let rhs = self.resolve_value(right)?;
                let res = self.execute_binary_op(op, &lhs, &rhs)?;
                self.store_to_dest(result, res)?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::Unary { op, operand, result } => {
                let val = self.resolve_value(operand)?;
                let res = self.execute_unary_op(op, &val)?;
                self.store_to_dest(result, res)?;
                Ok(InstructionResult::Continue)
            }

            // Memory management
            Instruction::Allocate { size, result } => {
                let size_val = self.resolve_value(size)?.as_integer()? as usize;
                let addr = self.runtime.allocate(size_val, None)?;
                self.store_to_dest(result, InterpreterValue::pointer(addr, ValueType::Void))?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::Deallocate { pointer } => {
                let addr = self.resolve_value(pointer)?.as_pointer()?;
                self.runtime.free(addr)?;
                Ok(InstructionResult::Continue)
            }

            // Array operations
            Instruction::ArrayAccess { array, index, result, .. } => {
                let arr = self.resolve_value(array)?;
                let idx = self.resolve_value(index)?.as_integer()?;
                
                match arr {
                    InterpreterValue::Array { elements, .. } => {
                        if idx < 0 || idx as usize >= elements.len() {
                            return Err(InterpreterError::out_of_bounds(idx, elements.len()));
                        }
                        let value = elements[idx as usize].clone();
                        self.store_to_dest(result, value)?;
                    }
                    InterpreterValue::Pointer { address, .. } => {
                        // Array as pointer - calculate offset and load
                        let elem_addr = address.offset(idx * 8); // Assume 8-byte elements
                        let value = self.runtime.read_i64(elem_addr)?;
                        self.store_to_dest(result, InterpreterValue::integer(value))?;
                    }
                    _ => return Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::TypeMismatch {
                            expected: ValueType::Array(Box::new(ValueType::Void)),
                            found: arr.get_type(),
                        },
                        "Expected array for array access",
                    )),
                }
                Ok(InstructionResult::Continue)
            }
            
            Instruction::ArraySet { array, index, value, .. } => {
                let idx = self.resolve_value(index)?.as_integer()?;
                let val = self.resolve_value(value)?;
                let arr = self.resolve_value(array)?;
                
                match arr {
                    InterpreterValue::Pointer { address, .. } => {
                        let elem_addr = address.offset(idx * 8);
                        let int_val = val.as_integer()?;
                        self.runtime.write_i64(elem_addr, int_val)?;
                    }
                    _ => return Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::InvalidOperation,
                        "Cannot modify non-pointer array",
                    )),
                }
                Ok(InstructionResult::Continue)
            }
            
            Instruction::ArrayLength { array, result } => {
                let arr = self.resolve_value(array)?;
                let len = match arr {
                    InterpreterValue::Array { elements, .. } => elements.len() as i64,
                    InterpreterValue::String(s) => s.len() as i64,
                    _ => return Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::TypeMismatch {
                            expected: ValueType::Array(Box::new(ValueType::Void)),
                            found: arr.get_type(),
                        },
                        "Expected array or string for length",
                    )),
                };
                self.store_to_dest(result, InterpreterValue::integer(len))?;
                Ok(InstructionResult::Continue)
            }

            // Struct operations
            Instruction::FieldAccess { struct_val, field, result, .. } => {
                let st = self.resolve_value(struct_val)?;
                match st {
                    InterpreterValue::Struct { fields, .. } => {
                        let value = fields.get(field)
                            .ok_or_else(|| InterpreterError::new(
                                super::error::InterpreterErrorKind::InvalidFieldAccess,
                                format!("Field '{}' not found", field),
                            ))?;
                        self.store_to_dest(result, value.clone())?;
                    }
                    _ => return Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::TypeMismatch {
                            expected: ValueType::Struct { name: "?".to_string(), fields: vec![] },
                            found: st.get_type(),
                        },
                        "Expected struct for field access",
                    )),
                }
                Ok(InstructionResult::Continue)
            }
            
            Instruction::FieldSet { struct_val, field, value, .. } => {
                // Note: This would need mutable access to the struct
                // For now, we'll handle this through pointers
                let _val = self.resolve_value(value)?;
                let _st = self.resolve_value(struct_val)?;
                // TODO: Implement proper struct field mutation
                Ok(InstructionResult::Continue)
            }

            // String operations
            Instruction::StringConcat { left, right, result } => {
                let l = self.resolve_value(left)?;
                let r = self.resolve_value(right)?;
                
                let l_str = match &l {
                    InterpreterValue::String(s) => s.clone(),
                    other => format!("{}", other),
                };
                let r_str = match &r {
                    InterpreterValue::String(s) => s.clone(),
                    other => format!("{}", other),
                };
                
                self.store_to_dest(result, InterpreterValue::string(l_str + &r_str))?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::StringLength { string, result } => {
                let s = self.resolve_value(string)?;
                let len = match s {
                    InterpreterValue::String(s) => s.len() as i64,
                    _ => return Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::TypeMismatch {
                            expected: ValueType::String,
                            found: s.get_type(),
                        },
                        "Expected string",
                    )),
                };
                self.store_to_dest(result, InterpreterValue::integer(len))?;
                Ok(InstructionResult::Continue)
            }

            // Type operations
            Instruction::Cast { value, target_type, result } => {
                let val = self.resolve_value(value)?;
                let casted = self.cast_value(&val, target_type)?;
                self.store_to_dest(result, casted)?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::TypeCheck { value, target_type, result } => {
                let val = self.resolve_value(value)?;
                let val_type = val.get_type();
                let target = ValueType::from_ir_type(target_type);
                let matches = val_type.is_compatible_with(&target);
                self.store_to_dest(result, InterpreterValue::boolean(matches))?;
                Ok(InstructionResult::Continue)
            }

            // Debug operations
            Instruction::Print(value) => {
                let val = self.resolve_value(value)?;
                self.runtime.println(&format!("{}", val));
                Ok(InstructionResult::Continue)
            }
            
            Instruction::Debug { message, value } => {
                if let Some(v) = value {
                    let val = self.resolve_value(v)?;
                    self.runtime.println(&format!("[DEBUG] {}: {}", message, val));
                } else {
                    self.runtime.println(&format!("[DEBUG] {}", message));
                }
                Ok(InstructionResult::Continue)
            }

            // Function calls
            Instruction::Call { target, args, result, .. } => {
                self.execute_call(target, args, result.as_ref())
            }
            
            Instruction::VirtualCall { receiver, method_name, args, result, .. } => {
                // For now, treat as a regular call
                // TODO: Implement proper virtual dispatch
                let _ = self.resolve_value(receiver)?;
                self.runtime.println(&format!("[WARN] VirtualCall not fully implemented: {}", method_name));
                if let Some(res) = result {
                    self.store_to_dest(res, InterpreterValue::Void)?;
                }
                Ok(InstructionResult::Continue)
            }
            
            Instruction::StaticCall { class_name, method_name, args, result, .. } => {
                // Look up the static method
                let full_name = format!("{}::{}", class_name, method_name);
                self.runtime.println(&format!("[WARN] StaticCall not fully implemented: {}", full_name));
                if let Some(res) = result {
                    self.store_to_dest(res, InterpreterValue::Void)?;
                }
                Ok(InstructionResult::Continue)
            }

            // Object construction
            Instruction::ConstructObject { class_name, args, result, .. } => {
                // Allocate space and call constructor
                // For now, create an empty struct
                let fields = HashMap::new();
                self.store_to_dest(result, InterpreterValue::struct_value(class_name.clone(), fields))?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::ConstructEnum { enum_name, variant_name, fields, result, .. } => {
                let field_values: Vec<InterpreterValue> = fields
                    .iter()
                    .map(|f| self.resolve_value(f))
                    .collect::<Result<_, _>>()?;
                
                self.store_to_dest(result, InterpreterValue::Enum {
                    type_name: enum_name.clone(),
                    variant_name: variant_name.clone(),
                    tag: 0, // Would need proper tag computation
                    payload: if field_values.is_empty() { None } else { Some(field_values) },
                })?;
                Ok(InstructionResult::Continue)
            }

            // Enum operations
            Instruction::GetEnumTag { enum_value, result } => {
                let val = self.resolve_value(enum_value)?;
                match val {
                    InterpreterValue::Enum { tag, .. } => {
                        self.store_to_dest(result, InterpreterValue::integer(tag as i64))?;
                    }
                    _ => return Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::TypeMismatch {
                            expected: ValueType::Enum { name: "?".to_string(), variants: vec![] },
                            found: val.get_type(),
                        },
                        "Expected enum for GetEnumTag",
                    )),
                }
                Ok(InstructionResult::Continue)
            }
            
            Instruction::GetEnumField { enum_value, field_index, result } => {
                let val = self.resolve_value(enum_value)?;
                match val {
                    InterpreterValue::Enum { payload, .. } => {
                        if let Some(fields) = payload {
                            let idx = *field_index as usize;
                            if idx < fields.len() {
                                self.store_to_dest(result, fields[idx].clone())?;
                            } else {
                                return Err(InterpreterError::out_of_bounds(idx as i64, fields.len()));
                            }
                        } else {
                            self.store_to_dest(result, InterpreterValue::Void)?;
                        }
                    }
                    _ => return Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::TypeMismatch {
                            expected: ValueType::Enum { name: "?".to_string(), variants: vec![] },
                            found: val.get_type(),
                        },
                        "Expected enum for GetEnumField",
                    )),
                }
                Ok(InstructionResult::Continue)
            }

            // Frame management
            Instruction::PushFrame => {
                // Already handled by call mechanism
                Ok(InstructionResult::Continue)
            }
            
            Instruction::PopFrame => {
                // Already handled by return mechanism
                Ok(InstructionResult::Continue)
            }

            // SIMD (simplified)
            Instruction::SimdSplat { scalar, lanes, result, .. } => {
                let val = self.resolve_value(scalar)?;
                let elements = vec![val; *lanes as usize];
                self.store_to_dest(result, InterpreterValue::array(ValueType::Integer, elements))?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::SimdReduceAdd { vector, result, .. } => {
                let vec_val = self.resolve_value(vector)?;
                match vec_val {
                    InterpreterValue::Array { elements, .. } => {
                        let sum: i64 = elements.iter()
                            .filter_map(|e| e.as_integer().ok())
                            .sum();
                        self.store_to_dest(result, InterpreterValue::integer(sum))?;
                    }
                    _ => return Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::TypeMismatch {
                            expected: ValueType::Array(Box::new(ValueType::Integer)),
                            found: vec_val.get_type(),
                        },
                        "Expected vector for SimdReduceAdd",
                    )),
                }
                Ok(InstructionResult::Continue)
            }

            // Concurrency (not supported in interpreter)
            Instruction::Scoped { result, .. } => {
                self.runtime.println("[WARN] Scoped blocks not supported in interpreter");
                self.store_to_dest(result, InterpreterValue::Void)?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::Spawn { result, .. } => {
                self.runtime.println("[WARN] Spawn not supported in interpreter");
                self.store_to_dest(result, InterpreterValue::Void)?;
                Ok(InstructionResult::Continue)
            }
            
            Instruction::ChannelSelect { payload_result, index_result, status_result, .. } => {
                self.runtime.println("[WARN] ChannelSelect not supported in interpreter");
                self.store_to_dest(payload_result, InterpreterValue::Void)?;
                self.store_to_dest(index_result, InterpreterValue::integer(-1))?;
                self.store_to_dest(status_result, InterpreterValue::integer(0))?;
                Ok(InstructionResult::Continue)
            }

            Instruction::Nop => Ok(InstructionResult::Continue),
        }
    }

    /// Resolve an IRValue to an InterpreterValue
    fn resolve_value(&self, ir_value: &IRValue) -> Result<InterpreterValue, InterpreterError> {
        match ir_value {
            IRValue::Void => Ok(InterpreterValue::Void),
            IRValue::Integer(i) => Ok(InterpreterValue::integer(*i)),
            IRValue::Float(f) => Ok(InterpreterValue::float(*f)),
            IRValue::Boolean(b) => Ok(InterpreterValue::boolean(*b)),
            IRValue::Char(c) => Ok(InterpreterValue::Char(*c)),
            IRValue::String(s) => Ok(InterpreterValue::string(s.clone())),
            IRValue::Null => Ok(InterpreterValue::Null),
            IRValue::Undefined => Ok(InterpreterValue::Undefined),
            
            IRValue::Variable(name) => {
                self.runtime.get_variable(name).cloned()
            }
            
            IRValue::Register(n) => {
                let var_name = format!("_r{}", n);
                self.runtime.get_variable(&var_name).cloned()
            }
            
            IRValue::GlobalVariable(name) => {
                self.runtime.get_global(name)
                    .cloned()
                    .ok_or_else(|| InterpreterError::undefined_variable(name))
            }
            
            IRValue::StringConstant(idx) => {
                self.runtime.get_interned_string(*idx)
                    .map(|s| InterpreterValue::string(s))
                    .ok_or_else(|| InterpreterError::internal("Invalid string constant index"))
            }
            
            IRValue::Array(elements) => {
                let values: Vec<InterpreterValue> = elements
                    .iter()
                    .map(|e| self.resolve_value(e))
                    .collect::<Result<_, _>>()?;
                let elem_type = if values.is_empty() {
                    ValueType::Void
                } else {
                    values[0].get_type()
                };
                Ok(InterpreterValue::array(elem_type, values))
            }
            
            IRValue::Struct { type_name, fields } => {
                let mut resolved: HashMap<String, InterpreterValue> = HashMap::new();
                for (name, value) in fields {
                    resolved.insert(name.clone(), self.resolve_value(value)?);
                }
                Ok(InterpreterValue::struct_value(type_name.clone(), resolved))
            }
            
            IRValue::Function { name, .. } => {
                Ok(InterpreterValue::Function {
                    name: name.clone(),
                    parameter_types: vec![],
                    return_type: Box::new(ValueType::Void),
                })
            }
            
            IRValue::Label(name) => {
                // Labels shouldn't be resolved as values
                Ok(InterpreterValue::Void)
            }
            
            IRValue::AddressOf(inner) => {
                // Would need to get actual memory address
                // For now, return a placeholder
                Ok(InterpreterValue::Null)
            }
            
            IRValue::ByteArray(bytes) => {
                let elements: Vec<InterpreterValue> = bytes
                    .iter()
                    .map(|b| InterpreterValue::integer(*b as i64))
                    .collect();
                Ok(InterpreterValue::array(ValueType::Integer, elements))
            }
        }
    }

    /// Store a value to a destination (variable or register)
    fn store_to_dest(&mut self, dest: &IRValue, value: InterpreterValue) -> Result<(), InterpreterError> {
        match dest {
            IRValue::Variable(name) => {
                self.runtime.set_local(name.clone(), value)?;
            }
            IRValue::Register(n) => {
                let name = format!("_r{}", n);
                self.runtime.set_local(name, value)?;
            }
            IRValue::GlobalVariable(name) => {
                self.runtime.set_global(name.clone(), value);
            }
            _ => {
                return Err(InterpreterError::internal(format!(
                    "Cannot store to {:?}",
                    dest
                )));
            }
        }
        Ok(())
    }

    /// Execute a binary operation
    fn execute_binary_op(
        &self,
        op: &BinaryOp,
        lhs: &InterpreterValue,
        rhs: &InterpreterValue,
    ) -> Result<InterpreterValue, InterpreterError> {
        match op {
            // Arithmetic
            BinaryOp::Add => {
                if let (Ok(l), Ok(r)) = (lhs.as_float(), rhs.as_float()) {
                    // Check if both are actually integers
                    if matches!(lhs, InterpreterValue::Integer(_)) && 
                       matches!(rhs, InterpreterValue::Integer(_)) {
                        return Ok(InterpreterValue::integer(lhs.as_integer()? + rhs.as_integer()?));
                    }
                    return Ok(InterpreterValue::float(l + r));
                }
                // String concatenation
                if let (InterpreterValue::String(l), InterpreterValue::String(r)) = (lhs, rhs) {
                    return Ok(InterpreterValue::string(format!("{}{}", l, r)));
                }
                Err(InterpreterError::new(
                    super::error::InterpreterErrorKind::InvalidOperation,
                    format!("Cannot add {:?} and {:?}", lhs.get_type(), rhs.get_type()),
                ))
            }
            
            BinaryOp::Subtract => {
                if matches!(lhs, InterpreterValue::Integer(_)) && 
                   matches!(rhs, InterpreterValue::Integer(_)) {
                    return Ok(InterpreterValue::integer(lhs.as_integer()? - rhs.as_integer()?));
                }
                let l = lhs.as_float()?;
                let r = rhs.as_float()?;
                Ok(InterpreterValue::float(l - r))
            }
            
            BinaryOp::Multiply => {
                if matches!(lhs, InterpreterValue::Integer(_)) && 
                   matches!(rhs, InterpreterValue::Integer(_)) {
                    return Ok(InterpreterValue::integer(lhs.as_integer()? * rhs.as_integer()?));
                }
                let l = lhs.as_float()?;
                let r = rhs.as_float()?;
                Ok(InterpreterValue::float(l * r))
            }
            
            BinaryOp::Divide => {
                let r_int = rhs.as_integer().unwrap_or(1);
                let r_float = rhs.as_float()?;
                if r_int == 0 || r_float == 0.0 {
                    return Err(InterpreterError::division_by_zero());
                }
                if matches!(lhs, InterpreterValue::Integer(_)) && 
                   matches!(rhs, InterpreterValue::Integer(_)) {
                    return Ok(InterpreterValue::integer(lhs.as_integer()? / rhs.as_integer()?));
                }
                let l = lhs.as_float()?;
                Ok(InterpreterValue::float(l / r_float))
            }
            
            BinaryOp::Modulo => {
                let r = rhs.as_integer()?;
                if r == 0 {
                    return Err(InterpreterError::division_by_zero());
                }
                let l = lhs.as_integer()?;
                Ok(InterpreterValue::integer(l % r))
            }

            // Comparison
            BinaryOp::Equal => {
                let eq = match (lhs, rhs) {
                    (InterpreterValue::Integer(l), InterpreterValue::Integer(r)) => l == r,
                    (InterpreterValue::Float(l), InterpreterValue::Float(r)) => l == r,
                    (InterpreterValue::Boolean(l), InterpreterValue::Boolean(r)) => l == r,
                    (InterpreterValue::String(l), InterpreterValue::String(r)) => l == r,
                    (InterpreterValue::Null, InterpreterValue::Null) => true,
                    _ => false,
                };
                Ok(InterpreterValue::boolean(eq))
            }
            
            BinaryOp::NotEqual => {
                let eq = match (lhs, rhs) {
                    (InterpreterValue::Integer(l), InterpreterValue::Integer(r)) => l != r,
                    (InterpreterValue::Float(l), InterpreterValue::Float(r)) => l != r,
                    (InterpreterValue::Boolean(l), InterpreterValue::Boolean(r)) => l != r,
                    (InterpreterValue::String(l), InterpreterValue::String(r)) => l != r,
                    _ => true,
                };
                Ok(InterpreterValue::boolean(eq))
            }
            
            BinaryOp::LessThan => {
                if matches!(lhs, InterpreterValue::Integer(_)) && 
                   matches!(rhs, InterpreterValue::Integer(_)) {
                    return Ok(InterpreterValue::boolean(lhs.as_integer()? < rhs.as_integer()?));
                }
                let l = lhs.as_float()?;
                let r = rhs.as_float()?;
                Ok(InterpreterValue::boolean(l < r))
            }
            
            BinaryOp::LessEqual => {
                if matches!(lhs, InterpreterValue::Integer(_)) && 
                   matches!(rhs, InterpreterValue::Integer(_)) {
                    return Ok(InterpreterValue::boolean(lhs.as_integer()? <= rhs.as_integer()?));
                }
                let l = lhs.as_float()?;
                let r = rhs.as_float()?;
                Ok(InterpreterValue::boolean(l <= r))
            }
            
            BinaryOp::GreaterThan => {
                if matches!(lhs, InterpreterValue::Integer(_)) && 
                   matches!(rhs, InterpreterValue::Integer(_)) {
                    return Ok(InterpreterValue::boolean(lhs.as_integer()? > rhs.as_integer()?));
                }
                let l = lhs.as_float()?;
                let r = rhs.as_float()?;
                Ok(InterpreterValue::boolean(l > r))
            }
            
            BinaryOp::GreaterEqual => {
                if matches!(lhs, InterpreterValue::Integer(_)) && 
                   matches!(rhs, InterpreterValue::Integer(_)) {
                    return Ok(InterpreterValue::boolean(lhs.as_integer()? >= rhs.as_integer()?));
                }
                let l = lhs.as_float()?;
                let r = rhs.as_float()?;
                Ok(InterpreterValue::boolean(l >= r))
            }

            // Logical
            BinaryOp::And => {
                Ok(InterpreterValue::boolean(lhs.is_truthy() && rhs.is_truthy()))
            }
            
            BinaryOp::Or => {
                Ok(InterpreterValue::boolean(lhs.is_truthy() || rhs.is_truthy()))
            }

            // Bitwise
            BinaryOp::BitwiseAnd => {
                let l = lhs.as_integer()?;
                let r = rhs.as_integer()?;
                Ok(InterpreterValue::integer(l & r))
            }
            
            BinaryOp::BitwiseOr => {
                let l = lhs.as_integer()?;
                let r = rhs.as_integer()?;
                Ok(InterpreterValue::integer(l | r))
            }
            
            BinaryOp::BitwiseXor => {
                let l = lhs.as_integer()?;
                let r = rhs.as_integer()?;
                Ok(InterpreterValue::integer(l ^ r))
            }
            
            BinaryOp::LeftShift => {
                let l = lhs.as_integer()?;
                let r = rhs.as_integer()?;
                Ok(InterpreterValue::integer(l << r))
            }
            
            BinaryOp::RightShift => {
                let l = lhs.as_integer()?;
                let r = rhs.as_integer()?;
                Ok(InterpreterValue::integer(l >> r))
            }
        }
    }

    /// Execute a unary operation
    fn execute_unary_op(
        &self,
        op: &UnaryOp,
        operand: &InterpreterValue,
    ) -> Result<InterpreterValue, InterpreterError> {
        match op {
            UnaryOp::Negate => {
                match operand {
                    InterpreterValue::Integer(i) => Ok(InterpreterValue::integer(-i)),
                    InterpreterValue::Float(f) => Ok(InterpreterValue::float(-f)),
                    _ => Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::InvalidOperation,
                        format!("Cannot negate {:?}", operand.get_type()),
                    )),
                }
            }
            
            UnaryOp::Not => {
                Ok(InterpreterValue::boolean(!operand.is_truthy()))
            }
            
            UnaryOp::BitwiseNot => {
                let i = operand.as_integer()?;
                Ok(InterpreterValue::integer(!i))
            }
            
            UnaryOp::Reference => {
                // Would need actual memory allocation
                Ok(InterpreterValue::Null)
            }
            
            UnaryOp::Dereference => {
                match operand {
                    InterpreterValue::Pointer { address, .. } => {
                        // Would need to read from memory
                        Ok(InterpreterValue::Void)
                    }
                    _ => Err(InterpreterError::new(
                        super::error::InterpreterErrorKind::InvalidOperation,
                        "Cannot dereference non-pointer",
                    )),
                }
            }
        }
    }

    /// Cast a value to a target type
    fn cast_value(&self, value: &InterpreterValue, target: &IRType) -> Result<InterpreterValue, InterpreterError> {
        match target {
            IRType::Integer => {
                let i = value.as_integer()?;
                Ok(InterpreterValue::integer(i))
            }
            IRType::Float => {
                let f = value.as_float()?;
                Ok(InterpreterValue::float(f))
            }
            IRType::Boolean => {
                Ok(InterpreterValue::boolean(value.is_truthy()))
            }
            IRType::String => {
                Ok(InterpreterValue::string(format!("{}", value)))
            }
            _ => Ok(value.clone()), // Identity cast
        }
    }

    /// Execute a function call
    fn execute_call(
        &mut self,
        target: &IRValue,
        args: &[IRValue],
        result: Option<&IRValue>,
    ) -> Result<InstructionResult, InterpreterError> {
        let func_name = match target {
            IRValue::Function { name, .. } => name.clone(),
            IRValue::GlobalVariable(name) => name.clone(),
            IRValue::Variable(name) => name.clone(),
            _ => return Err(InterpreterError::internal("Invalid call target")),
        };

        // Evaluate arguments
        let arg_values: Vec<InterpreterValue> = args
            .iter()
            .map(|a| self.resolve_value(a))
            .collect::<Result<_, _>>()?;

        // Handle built-in functions
        let ret_val = match func_name.as_str() {
            "print" | "println" => {
                for arg in &arg_values {
                    self.runtime.print(&format!("{}", arg));
                }
                if func_name == "println" {
                    self.runtime.println("");
                }
                InterpreterValue::Void
            }
            
            "len" | "length" => {
                if let Some(arr) = arg_values.first() {
                    match arr {
                        InterpreterValue::Array { elements, .. } => {
                            InterpreterValue::integer(elements.len() as i64)
                        }
                        InterpreterValue::String(s) => {
                            InterpreterValue::integer(s.len() as i64)
                        }
                        _ => InterpreterValue::integer(0),
                    }
                } else {
                    InterpreterValue::integer(0)
                }
            }
            
            "malloc" | "alloc" => {
                let size = arg_values.first()
                    .and_then(|v| v.as_integer().ok())
                    .unwrap_or(0) as usize;
                let addr = self.runtime.allocate(size, None)?;
                InterpreterValue::pointer(addr, ValueType::Void)
            }
            
            "free" => {
                if let Some(ptr) = arg_values.first() {
                    let addr = ptr.as_pointer()?;
                    self.runtime.free(addr)?;
                }
                InterpreterValue::Void
            }
            
            _ => {
                // Look up function in module and call recursively
                if let Some(module) = self.current_module {
                    if module.has_function(&func_name) {
                        // TODO: Implement recursive function calls
                        // This would require restructuring to avoid borrowing issues
                        self.runtime.println(&format!("[WARN] Recursive call to {} not yet supported", func_name));
                        InterpreterValue::Void
                    } else {
                        self.runtime.println(&format!("[WARN] Unknown function: {}", func_name));
                        InterpreterValue::Void
                    }
                } else {
                    self.runtime.println(&format!("[WARN] Unknown function: {}", func_name));
                    InterpreterValue::Void
                }
            }
        };

        // Store result
        if let Some(dest) = result {
            self.store_to_dest(dest, ret_val)?;
        }

        Ok(InstructionResult::Continue)
    }

    /// Step through one instruction (for debugging)
    pub fn step(&mut self, module: &IRModule) -> Result<StepResult, InterpreterError> {
        // Would need to track execution state
        // This is a simplified implementation
        Ok(StepResult::Done)
    }
}

/// Internal result of executing an instruction
enum InstructionResult {
    Continue,
    Jump(String),
    Return(InterpreterValue),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_add() {
        let config = InterpreterConfig::default();
        let mut runtime = Runtime::new(RuntimeConfig::default());
        let executor = Executor::new(&config, &mut runtime);
        
        let result = executor.execute_binary_op(
            &BinaryOp::Add,
            &InterpreterValue::integer(10),
            &InterpreterValue::integer(20),
        ).unwrap();
        
        assert_eq!(result.as_integer().unwrap(), 30);
    }

    #[test]
    fn test_binary_divide_by_zero() {
        let config = InterpreterConfig::default();
        let mut runtime = Runtime::new(RuntimeConfig::default());
        let executor = Executor::new(&config, &mut runtime);
        
        let result = executor.execute_binary_op(
            &BinaryOp::Divide,
            &InterpreterValue::integer(10),
            &InterpreterValue::integer(0),
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_comparison_ops() {
        let config = InterpreterConfig::default();
        let mut runtime = Runtime::new(RuntimeConfig::default());
        let executor = Executor::new(&config, &mut runtime);
        
        let lt = executor.execute_binary_op(
            &BinaryOp::LessThan,
            &InterpreterValue::integer(5),
            &InterpreterValue::integer(10),
        ).unwrap();
        assert!(lt.as_boolean().unwrap());
        
        let eq = executor.execute_binary_op(
            &BinaryOp::Equal,
            &InterpreterValue::integer(42),
            &InterpreterValue::integer(42),
        ).unwrap();
        assert!(eq.as_boolean().unwrap());
    }
}
