//! Type inference for register allocation in LLVM backend.
//!
//! This module handles scanning IR instructions to infer register types
//! and pre-allocating slots with the correct LLVM types.

use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use inkwell::types::BasicTypeEnum;
use inkwell::values::FunctionValue;

use crate::function::IRFunction;
use crate::instruction::{BinaryOp, Instruction, UnaryOp};
use crate::llvm_backend::LlvmBackend;
use crate::value::{IRType, IRValue};

type HashMap<K, V> = IndexMap<K, V>;

/// Extension trait for type inference operations on LlvmBackend
pub trait TypeInference<'ctx> {
    /// Scan all instructions in a function and allocate typed slots for registers
    fn scan_and_allocate_registers(&mut self, func: &IRFunction) -> Result<()>;
    
    /// Infer the result type of an instruction (slow path with full lookups)
    fn infer_instruction_result_type(
        &self,
        inst: &Instruction,
        reg_types: &HashMap<usize, BasicTypeEnum<'ctx>>,
    ) -> Option<(usize, BasicTypeEnum<'ctx>)>;
    
    /// Fast version using pre-built caches
    fn infer_instruction_result_type_fast(
        &self,
        inst: &Instruction,
        reg_types: &HashMap<usize, BasicTypeEnum<'ctx>>,
        field_type_index: &HashMap<String, BasicTypeEnum<'ctx>>,
        fn_return_cache: &mut HashMap<String, Option<BasicTypeEnum<'ctx>>>,
    ) -> Option<(usize, BasicTypeEnum<'ctx>)>;
    
    /// Get the LLVM type of an IR value
    fn get_value_type(
        &self,
        val: &IRValue,
        reg_types: &HashMap<usize, BasicTypeEnum<'ctx>>,
    ) -> Option<BasicTypeEnum<'ctx>>;
    
    /// Collect all registers defined by an instruction
    fn collect_defined_registers(
        &self,
        inst: &Instruction,
        reg_types: &mut HashMap<usize, BasicTypeEnum<'ctx>>,
    );
}

impl<'ctx> TypeInference<'ctx> for LlvmBackend<'ctx> {
    fn scan_and_allocate_registers(&mut self, func: &IRFunction) -> Result<()> {
        let mut reg_types: HashMap<usize, BasicTypeEnum<'ctx>> = HashMap::new();
        
        let is_target_fn = func.name == "createLexer" || func.name == "lastSlashIndex" || func.name.contains("LexerError_getPosition");
        
        // Initialize with parameters
        for (i, param) in func.parameters.iter().enumerate() {
            let ty = self.ir_type_to_llvm(&param.param_type);
            reg_types.insert(i, ty);

            // Track struct/array element info for parameter registers and variables
            if let IRType::Struct { name, .. } = &param.param_type {
                self.reg_struct_types.insert(i as u32, name.clone());
                self.var_struct_types.insert(param.name.clone(), name.clone());
            }
            if let IRType::Pointer(inner) | IRType::Reference(inner) = &param.param_type {
                match inner.as_ref() {
                    IRType::Struct { name, .. } => {
                        self.reg_struct_types.insert(i as u32, name.clone());
                        self.var_struct_types.insert(param.name.clone(), name.clone());
                    }
                    IRType::Array(element_type) => {
                        match element_type.as_ref() {
                            IRType::Struct { name, .. } => {
                                self.reg_array_element_struct.insert(i as u32, name.clone());
                                self.var_array_element_struct.insert(param.name.clone(), name.clone());
                            }
                            IRType::String => {
                                self.reg_array_element_struct.insert(i as u32, "String".to_string());
                                self.var_array_element_struct.insert(param.name.clone(), "String".to_string());
                            }
                            _ => {}
                        }
                        if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                            self.reg_is_int_array.insert(i as u32);
                            self.var_is_int_array.insert(param.name.clone());
                        }
                    }
                    _ => {}
                }
            }
            if let IRType::Array(element_type) = &param.param_type {
                match element_type.as_ref() {
                    IRType::Struct { name, .. } => {
                        self.reg_array_element_struct.insert(i as u32, name.clone());
                        self.var_array_element_struct.insert(param.name.clone(), name.clone());
                    }
                    IRType::String => {
                        self.reg_array_element_struct.insert(i as u32, "String".to_string());
                        self.var_array_element_struct.insert(param.name.clone(), "String".to_string());
                    }
                    _ => {}
                }
                if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                    self.reg_is_int_array.insert(i as u32);
                    self.var_is_int_array.insert(param.name.clone());
                }
            }
        }

        // Initialize with locals
        for local in func.locals_iter() {
            // Debug: show all locals for normalize function
            if func.name == "normalize" {
            }
            if let IRType::Struct { name, .. } = &local.var_type {
                self.var_struct_types.insert(local.name.clone(), name.clone());
            }
            if let IRType::Pointer(inner) | IRType::Reference(inner) = &local.var_type {
                match inner.as_ref() {
                    IRType::Struct { name, .. } => {
                        self.var_struct_types.insert(local.name.clone(), name.clone());
                    }
                    IRType::Array(element_type) => {
                        match element_type.as_ref() {
                            IRType::Struct { name, .. } => {
                                self.var_array_element_struct.insert(local.name.clone(), name.clone());
                            }
                            IRType::String => {
                                self.var_array_element_struct.insert(local.name.clone(), "String".to_string());
                            }
                            _ => {}
                        }
                        if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                            self.var_is_int_array.insert(local.name.clone());
                        }
                    }
                    _ => {}
                }
            }
            if let IRType::Array(element_type) = &local.var_type {
                match element_type.as_ref() {
                    IRType::Struct { name, .. } => {
                        self.var_array_element_struct.insert(local.name.clone(), name.clone());
                    }
                    IRType::String => {
                        self.var_array_element_struct.insert(local.name.clone(), "String".to_string());
                    }
                    _ => {}
                }
                if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                    self.var_is_int_array.insert(local.name.clone());
                }
            }
        }

        // Pre-pass to track array element struct types for FieldAccess results
        for block in func.cfg.blocks_iter() {
            for inst in &block.instructions {
                if let Instruction::FieldAccess { struct_val, field, result, .. } = inst {
                    eprintln!("DEBUG type_inference: FieldAccess struct_val={:?}, field={}, result={:?}", struct_val, field, result);
                    if let IRValue::Register(reg_id) = result {
                        for (struct_name, def_fields) in self.struct_definitions.iter() {
                            for (field_name, field_ty) in def_fields.iter() {
                                if field_name == field {
                                    eprintln!("DEBUG type_inference: Found field '{}' in struct '{}' with type {:?}", field, struct_name, field_ty);
                                    if let IRType::Array(inner) = field_ty {
                                        if let IRType::Struct { name: inner_struct_name, .. } = &**inner {
                                            eprintln!("DEBUG type_inference: Setting reg {} to array element type '{}'", reg_id, inner_struct_name);
                                            self.reg_array_element_struct.insert(*reg_id, inner_struct_name.clone());
                                        }
                                        if matches!(inner.as_ref(), IRType::String) {
                                            self.reg_array_element_struct.insert(*reg_id, "String".to_string());
                                        }
                                        if matches!(inner.as_ref(), IRType::Integer | IRType::Char) {
                                            self.reg_is_int_array.insert(*reg_id);
                                        }
                                        if let IRType::Struct { name, .. } = &**inner {
                                            if name == "T" || name == "E" {
                                                self.reg_array_element_struct.insert(*reg_id, name.clone());
                                            }
                                        }
                                        // Handle Array(Optional(T)) - common pattern for Vec<Option<T>>
                                        if let IRType::Optional(opt_inner) = &**inner {
                                            // Mark the register as Vec type (since Array = Vec in our type system)
                                            self.reg_struct_types.insert(*reg_id, "Vec".to_string());
                                            // Mark the element type as "Option"
                                            self.reg_array_element_struct.insert(*reg_id, "Option".to_string());
                                            // Also track the inner type of the Option
                                            if let IRType::Struct { name: inner_struct_name, .. } = &**opt_inner {
                                                self.reg_option_inner_type.insert(*reg_id, inner_struct_name.clone());
                                            } else if matches!(&**opt_inner, IRType::String) {
                                                self.reg_option_inner_type.insert(*reg_id, "String".to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Build field-to-type index for O(1) lookups
        let mut field_type_index: HashMap<String, BasicTypeEnum<'ctx>> = HashMap::new();
        for (_, def_fields) in self.struct_definitions.iter() {
            for (field_name, field_ty) in def_fields.iter() {
                field_type_index.entry(field_name.clone()).or_insert_with(|| self.ir_type_to_llvm(field_ty));
            }
        }

        // Cache LLVM function return types
        let mut fn_return_cache: HashMap<String, Option<BasicTypeEnum<'ctx>>> = HashMap::new();

        let mut changed = true;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10;

        // Iterative type inference with caching
        while changed && iterations < MAX_ITERATIONS {
            changed = false;
            iterations += 1;
            for block in func.cfg.blocks_iter() {
                for inst in &block.instructions {
                    if let Some((reg_id, ty)) = self.infer_instruction_result_type_fast(inst, &reg_types, &field_type_index, &mut fn_return_cache) {
                        let existing_ty = reg_types.get(&reg_id).copied();
                        if existing_ty.map_or(true, |e| e != ty) {
                            reg_types.insert(reg_id, ty);
                            changed = true;
                        }
                    }
                }
            }
        }

        // Fallback: ensure all defined registers have a slot
        for block in func.cfg.blocks_iter() {
            for inst in &block.instructions {
                self.collect_defined_registers(inst, &mut reg_types);
            }
        }

        // Allocate slots in entry block
        let function = self.current_fn.ok_or_else(|| anyhow!("Current function not set"))?;
        let entry_block = function
            .get_first_basic_block()
            .ok_or_else(|| anyhow!("No entry block"))?;

        let builder = &self.builder;
        let saved_block = builder.get_insert_block();

        if let Some(first_inst) = entry_block.get_first_instruction() {
            builder.position_before(&first_inst);
        } else {
            builder.position_at_end(entry_block);
        }

        for (reg_id, ty) in reg_types {
            let ptr = builder.build_alloca(ty, &format!("r{}", reg_id)).map_err(|e| anyhow!("{e:?}"))?;
            self.reg_slots.insert(reg_id as u32, ptr);
            self.reg_slot_types.insert(reg_id as u32, ty);
        }

        if let Some(block) = saved_block {
            builder.position_at_end(block);
        }

        Ok(())
    }

    fn infer_instruction_result_type(
        &self,
        inst: &Instruction,
        reg_types: &HashMap<usize, BasicTypeEnum<'ctx>>,
    ) -> Option<(usize, BasicTypeEnum<'ctx>)> {
        match inst {
            Instruction::Binary {
                op,
                left,
                right,
                result,
            } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                match op {
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::LessThan
                    | BinaryOp::LessEqual
                    | BinaryOp::GreaterThan
                    | BinaryOp::GreaterEqual => Some((reg_id, self.bool_t.into())),
                    _ => self
                        .get_value_type(left, reg_types)
                        .or_else(|| self.get_value_type(right, reg_types))
                        .map(|ty| (reg_id, ty)),
                }
            }
            Instruction::Unary {
                op,
                operand,
                result,
            } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                match op {
                    UnaryOp::Not => Some((reg_id, self.bool_t.into())),
                    _ => self
                        .get_value_type(operand, reg_types)
                        .map(|ty| (reg_id, ty)),
                }
            }
            Instruction::Call {
                target,
                result: Some(result),
                ..
            } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                if let IRValue::Variable(name) = target {
                    // Handle known builtin methods that return String
                    if name == "substring" {
                        return Some((reg_id, self.ty_string().into()));
                    }
                    if name == "charAt" {
                        return Some((reg_id, self.ctx.i8_type().into()));
                    }
                    if name == "toInt" {
                        return Some((reg_id, self.i64_t.into()));
                    }
                    
                    // Check if this is a constructor
                    if name.ends_with("_new") {
                        let class_name = &name[..name.len() - 4];
                        if let Some((struct_ty, _)) = self.struct_types.get(class_name) {
                            return Some((reg_id, (*struct_ty).into()));
                        }
                    }
                    
                    if let Some(struct_name) = self.fn_return_struct_types.get(name) {
                        if let Some((struct_ty, _)) = self.struct_types.get(struct_name) {
                            return Some((reg_id, (*struct_ty).into()));
                        }
                    }
                    
                    if let Some(func) = self.module.get_function(name) {
                        if name == "__ExecuteCommand" {
                             let ir_struct_ty = self.ctx.struct_type(&[self.bool_t.into(), self.ty_string().into()], false);
                             return Some((reg_id, ir_struct_ty.into()));
                        }
                        let ret_ty = func.get_type().get_return_type();
                        ret_ty.map(|ty| (reg_id, ty))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Instruction::Load { source, dest } => {
                let reg_id = if let IRValue::Register(id) = dest {
                    *id as usize
                } else {
                    return None;
                };
                if let IRValue::Variable(name) = source {
                    self.var_slot_types.get(name).map(|ty| (reg_id, *ty))
                } else {
                    None
                }
            }
            Instruction::Move { source, dest } => {
                let reg_id = if let IRValue::Register(id) = dest {
                    *id as usize
                } else {
                    return None;
                };
                self.get_value_type(source, reg_types)
                    .map(|ty| (reg_id, ty))
            }
            Instruction::Cast {
                target_type,
                result,
                ..
            } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                Some((reg_id, self.ir_type_to_llvm(target_type)))
            }
            Instruction::ArrayAccess { array, result, .. } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                if let Some(arr_ty) = self.get_value_type(array, reg_types) {
                    if arr_ty == self.ty_string().into() {
                        return Some((reg_id, self.ctx.i8_type().into()));
                    }
                }
                
                if let IRValue::Register(arr_reg_id) = array {
                    if let Some(struct_name) = self.reg_array_element_struct.get(arr_reg_id) {
                        if let Some((llvm_struct_ty, _)) = self.struct_types.get(struct_name) {
                            return Some((reg_id, (*llvm_struct_ty).into()));
                        }
                    }
                }
                if let IRValue::Variable(name) = array {
                    if let Some(struct_name) = self.var_array_element_struct.get(name) {
                        if let Some((llvm_struct_ty, _)) = self.struct_types.get(struct_name) {
                            return Some((reg_id, (*llvm_struct_ty).into()));
                        }
                    }
                }
                
                None
            }
            Instruction::FieldAccess {
                struct_val,
                field,
                result,
                ..
            } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                let struct_name: Option<&String> = match struct_val {
                    IRValue::Variable(name) => self.var_struct_types.get(name),
                    IRValue::Register(id) => self.reg_struct_types.get(id),
                    _ => None,
                };

                if let Some(name) = struct_name {
                    if let Some((_, fields)) = self.struct_types.get(name) {
                        if let Some(idx) = fields.iter().position(|f| f == field) {
                            if let Some(def_fields) = self.struct_definitions.get(name) {
                                if let Some((_, field_ty)) = def_fields.get(idx) {
                                    return Some((reg_id, self.ir_type_to_llvm(field_ty)));
                                }
                            }
                        }
                    }
                }
                
                // Fallback: search all struct definitions
                for (_, def_fields) in self.struct_definitions.iter() {
                    for (field_name, field_ty) in def_fields.iter() {
                        if field_name == field {
                            return Some((reg_id, self.ir_type_to_llvm(field_ty)));
                        }
                    }
                }
                
                None
            }
            Instruction::StringConcat { result, .. } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                Some((reg_id, self.ty_string().into()))
            }
            Instruction::StringLength { result, .. } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                Some((reg_id, self.i64_t.into()))
            }
            Instruction::ConstructObject { class_name, result, .. } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                if let Some((st, _)) = self.struct_types.get(class_name) {
                     return Some((reg_id, (*st).into()));
                }
                Some((reg_id, self.i8_ptr_t.into()))
            }
            Instruction::GetEnumTag { result, .. } => {
                let reg_id = if let IRValue::Register(id) = result {
                    *id as usize
                } else {
                    return None;
                };
                Some((reg_id, self.i64_t.into()))
            }
            _ => None,
        }
    }

    fn infer_instruction_result_type_fast(
        &self,
        inst: &Instruction,
        reg_types: &HashMap<usize, BasicTypeEnum<'ctx>>,
        field_type_index: &HashMap<String, BasicTypeEnum<'ctx>>,
        fn_return_cache: &mut HashMap<String, Option<BasicTypeEnum<'ctx>>>,
    ) -> Option<(usize, BasicTypeEnum<'ctx>)> {
        match inst {
            Instruction::Binary { result, left, op, right } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                match op {
                    BinaryOp::Equal | BinaryOp::NotEqual | BinaryOp::LessThan | BinaryOp::LessEqual 
                    | BinaryOp::GreaterThan | BinaryOp::GreaterEqual => Some((reg_id, self.bool_t.into())),
                    _ => {
                        let ty = self.get_value_type(left, reg_types)
                            .or_else(|| self.get_value_type(right, reg_types));
                        Some((reg_id, ty.unwrap_or(self.i64_t.into())))
                    }
                }
            }
            Instruction::Unary { result, operand, op } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                match op {
                    UnaryOp::Not => Some((reg_id, self.bool_t.into())),
                    _ => {
                        let op_ty = self.get_value_type(operand, reg_types);
                        Some((reg_id, op_ty.unwrap_or(self.i64_t.into())))
                    }
                }
            }
            Instruction::Call { target, result: Some(result), .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                if let IRValue::Variable(name) = target {
                    if let Some(cached) = fn_return_cache.get(name) {
                        return cached.map(|ty| (reg_id, ty));
                    }
                    let ret_ty = if let Some(func) = self.module.get_function(name) {
                        func.get_type().get_return_type().and_then(|t| t.try_into().ok())
                    } else {
                        None
                    };
                    fn_return_cache.insert(name.clone(), ret_ty);
                    return ret_ty.map(|ty| (reg_id, ty));
                }
                None
            }
            Instruction::Load { dest, source, .. } => {
                let reg_id = if let IRValue::Register(id) = dest { *id as usize } else { return None; };
                let src_ty = self.get_value_type(source, reg_types);
                Some((reg_id, src_ty.unwrap_or(self.i64_t.into())))
            }
            Instruction::Move { dest, source } => {
                let reg_id = if let IRValue::Register(id) = dest { *id as usize } else { return None; };
                let src_ty = self.get_value_type(source, reg_types);
                Some((reg_id, src_ty.unwrap_or(self.i64_t.into())))
            }
            Instruction::Cast { result, target_type, .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                let ty = self.ir_type_to_llvm(target_type);
                Some((reg_id, ty))
            }
            Instruction::ArrayAccess { result, array, .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                if let IRValue::Register(arr_reg_id) = array {
                    if let Some(struct_name) = self.reg_array_element_struct.get(arr_reg_id) {
                        if let Some((llvm_struct_ty, _)) = self.struct_types.get(struct_name) {
                            return Some((reg_id, (*llvm_struct_ty).into()));
                        }
                    }
                }
                if let IRValue::Variable(name) = array {
                    if let Some(struct_name) = self.var_array_element_struct.get(name) {
                        if let Some((llvm_struct_ty, _)) = self.struct_types.get(struct_name) {
                            return Some((reg_id, (*llvm_struct_ty).into()));
                        }
                    }
                }
                Some((reg_id, self.i64_t.into()))
            }
            Instruction::FieldAccess { result, field, struct_val, .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                if let Some(ty) = field_type_index.get(field) {
                    return Some((reg_id, *ty));
                }
                let struct_name: Option<&String> = match struct_val {
                    IRValue::Variable(name) => self.var_struct_types.get(name),
                    IRValue::Register(id) => self.reg_struct_types.get(id),
                    _ => None,
                };
                if let Some(name) = struct_name {
                    if let Some((_, fields)) = self.struct_types.get(name) {
                        if let Some(idx) = fields.iter().position(|f| f == field) {
                            if let Some((struct_ty, _)) = self.struct_types.get(name) {
                                if let Some(field_ty) = struct_ty.get_field_type_at_index(idx as u32) {
                                    return Some((reg_id, field_ty));
                                }
                            }
                        }
                    }
                }
                Some((reg_id, self.i64_t.into()))
            }
            Instruction::StringConcat { result, .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                Some((reg_id, self.ty_string().into()))
            }
            Instruction::StringLength { result, .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                Some((reg_id, self.i64_t.into()))
            }
            Instruction::ConstructObject { class_name, result, .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                if let Some((st, _)) = self.struct_types.get(class_name) {
                    return Some((reg_id, (*st).into()));
                }
                Some((reg_id, self.i8_ptr_t.into()))
            }
            Instruction::GetEnumTag { result, .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                Some((reg_id, self.i64_t.into()))
            }
            Instruction::ArrayLength { result, .. } => {
                let reg_id = if let IRValue::Register(id) = result { *id as usize } else { return None; };
                Some((reg_id, self.i64_t.into()))
            }
            _ => None,
        }
    }

    fn get_value_type(
        &self,
        val: &IRValue,
        reg_types: &HashMap<usize, BasicTypeEnum<'ctx>>,
    ) -> Option<BasicTypeEnum<'ctx>> {
        match val {
            IRValue::Integer(_) => Some(self.i64_t.into()),
            IRValue::Float(_) => Some(self.ctx.f64_type().into()),
            IRValue::Boolean(_) => Some(self.bool_t.into()),
            IRValue::Register(id) => reg_types.get(&(*id as usize)).cloned(),
            IRValue::Variable(name) => self.var_slot_types.get(name).cloned(),
            IRValue::String(_) | IRValue::StringConstant(_) => Some(self.ty_string().into()),
            _ => None,
        }
    }

    fn collect_defined_registers(
        &self,
        inst: &Instruction,
        reg_types: &mut HashMap<usize, BasicTypeEnum<'ctx>>,
    ) {
        let mut add_if_missing = |val: &IRValue| {
            if let IRValue::Register(id) = val {
                reg_types
                    .entry(*id as usize)
                    .or_insert_with(|| self.i64_t.into());
            }
        };

        match inst {
            Instruction::Binary { result, .. } => add_if_missing(result),
            Instruction::Unary { result, .. } => add_if_missing(result),
            Instruction::Call {
                result: Some(result),
                ..
            } => add_if_missing(result),
            Instruction::Load { dest, .. } => add_if_missing(dest),
            Instruction::Move { dest, .. } => add_if_missing(dest),
            Instruction::Allocate { result, .. } => add_if_missing(result),
            Instruction::ArrayAccess { result, .. } => add_if_missing(result),
            Instruction::ArrayLength { result, .. } => add_if_missing(result),
            Instruction::FieldAccess { result, .. } => add_if_missing(result),
            Instruction::GetEnumTag { result, .. } => add_if_missing(result),
            Instruction::GetEnumField { result, .. } => add_if_missing(result),
            Instruction::Cast { result, .. } => add_if_missing(result),
            Instruction::TypeCheck { result, .. } => add_if_missing(result),
            Instruction::StringConcat { result, .. } => add_if_missing(result),
            Instruction::StringLength { result, .. } => add_if_missing(result),
            Instruction::SimdSplat { result, .. } => add_if_missing(result),
            Instruction::SimdReduceAdd { result, .. } => add_if_missing(result),
            Instruction::VirtualCall {
                result: Some(result),
                ..
            } => add_if_missing(result),
            Instruction::StaticCall {
                result: Some(result),
                ..
            } => add_if_missing(result),
            Instruction::ConstructObject { result, .. } => add_if_missing(result),
            Instruction::ConstructEnum { result, .. } => add_if_missing(result),
            Instruction::ChannelSelect {
                payload_result,
                index_result,
                status_result,
                ..
            } => {
                add_if_missing(payload_result);
                add_if_missing(index_result);
                add_if_missing(status_result);
            }
            Instruction::Spawn { result, .. } => add_if_missing(result),
            Instruction::Scoped { result, .. } => add_if_missing(result),
            _ => {}
        }
    }
}
