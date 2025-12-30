//! Call instruction handler for the LLVM backend.
//!
//! This module handles the `Instruction::Call` variant, which includes:
//! - Direct function calls
//! - Intrinsic/builtin function dispatch
//! - Method calls on types (String, Array, Result, etc.)
//!
//! The Call instruction is the largest handler (~2000 lines) because it
//! includes dispatch logic for many built-in operations.

use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, InstructionValue, PointerValue, BasicMetadataValueEnum};
use inkwell::AddressSpace;
use inkwell::types::{BasicType, BasicTypeEnum, BasicMetadataTypeEnum};
use inkwell::IntPredicate;
use inkwell::basic_block::BasicBlock as LlvmBasicBlock;

use crate::instruction::Instruction;
use crate::value::{IRValue, IRType};
use crate::llvm_backend::LlvmBackend;
use crate::llvm::string_ops::RuntimeStringOps;
use crate::llvm::type_cast::TypeCastOps;
use crate::llvm::runtime_fns::RuntimeFunctions;
use crate::llvm::concurrency::ConcurrencyOps;
use crate::llvm::c_library::CLibraryOps;
use crate::llvm::type_builders::TypeBuilders;

type HashMap<K, V> = IndexMap<K, V>;

/// Operations for handling function calls
pub trait CallOps<'ctx> {
    fn emit_call(
        &mut self,
        target: &IRValue,
        args: &[IRValue],
        result: &Option<IRValue>,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;

    fn auto_declare_runtime_function(&mut self, name: &str, arg_count: usize) -> Result<FunctionValue<'ctx>>;
}

impl<'ctx> CallOps<'ctx> for LlvmBackend<'ctx> {
    fn emit_call(
        &mut self,
        target: &IRValue,
        args: &[IRValue],
        result: &Option<IRValue>,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        // Debug: log Option_Unwrap calls
        if let IRValue::Variable(name) = target {
            if name == "Option_Unwrap" || name == "Option_unwrap" || name == "Unwrap" || name == "unwrap" {
                eprintln!("DEBUG LLVM: emit_call for '{}' with args: {:?}, result: {:?}", name, args, result);
            }
            if name.contains("unwrap") || name.contains("Unwrap") {
                eprintln!("DEBUG LLVM: emit_call checking function '{}' for unwrap handling", name);
            }
        }
        
        // Handle Vec methods specially to convert float<->i64
        let func_name = match target {
            IRValue::Variable(name) => Some(name.clone()),
            IRValue::Function { name, .. } => Some(name.clone()),
            _ => None,
        };
        
        // Debug all function calls
        if let Some(ref fn_name) = func_name {
            if fn_name.contains("toString") || fn_name.contains("ToString") {
                eprintln!("DEBUG LLVM emit_call: func_name={}", fn_name);
            }
        }
        
        let is_vec_push = func_name.as_deref() == Some("Vec_push");
        let is_vec_get = func_name.as_deref() == Some("Vec_get");
        let is_vec_set = func_name.as_deref() == Some("Vec_set");
        
        // Track which Vec variables store floats (from push calls)
        let mut is_float_vec_call = false;
        if is_vec_push || is_vec_set {
            // Check if the value being pushed/set is a float
            let value_arg = if is_vec_push { args.get(1) } else { args.get(2) };
            if let Some(v) = value_arg {
                let val = self.eval_value(v, fn_map)?;
                if val.is_float_value() {
                    is_float_vec_call = true;
                    // Track the Vec variable as storing floats
                    if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                        self.var_is_float_vec.insert(vec_var.clone());
                    }
                }
                // Track String element type from push calls
                let is_string_type = val.is_struct_value() && val.get_type() == self.ty_string().into();
                eprintln!("DEBUG Vec_push: value_type={:?}, is_struct={}, is_string_type={}, vec_var={:?}, value={:?}", 
                    val.get_type(), val.is_struct_value(), is_string_type, args.get(0), v);
                if is_string_type {
                    if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                        eprintln!("DEBUG: Vec_push tracking String element for vec '{}'", vec_var);
                        self.var_array_element_struct.insert(vec_var.clone(), "String".to_string());
                    }
                }
                // Track struct element type from push calls (for IRValue::Variable pointing to a struct)
                if let IRValue::Variable(pushed_var) = v {
                    if let Some(struct_name) = self.var_struct_types.get(pushed_var).cloned() {
                        if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                            self.var_array_element_struct.insert(vec_var.clone(), struct_name.clone());
                            
                            // If pushing an Option to a Vec, track the Option's inner type for the Vec
                            if struct_name == "Option" {
                                if let Some(inner_type) = self.var_option_inner_type.get(pushed_var).cloned() {
                                    eprintln!("DEBUG: Vec_push tracking Option inner type '{}' for vec '{}'", inner_type, vec_var);
                                    self.var_option_inner_type.insert(vec_var.clone(), inner_type);
                                }
                            }
                        }
                    }
                }
                // Track Option inner types when pushing from a register (e.g., Some(value) result)
                if let IRValue::Register(pushed_reg) = v {
                    if let Some(struct_name) = self.reg_struct_types.get(pushed_reg).cloned() {
                        if struct_name == "Option" {
                            if let Some(inner_type) = self.reg_option_inner_type.get(pushed_reg).cloned() {
                                if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                                    eprintln!("DEBUG: Vec_push tracking Option inner type '{}' for vec '{}' from reg {}", inner_type, vec_var, pushed_reg);
                                    self.var_option_inner_type.insert(vec_var.clone(), inner_type);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Check if this is a get from a float Vec
        if is_vec_get {
            if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                if self.var_is_float_vec.contains(vec_var) {
                    is_float_vec_call = true;
                }
            }
        }

        // Ensure alias shims exist so any stray calls to unprefixed Result methods resolve
        self.ensure_result_alias_shims()?;

        // Handle Result/File method aliases even when the call target is an IRValue::Function
        // BUT NOT for Option_ prefixed functions - those should use Option_* directly
        if let Some(name) = func_name.as_ref() {
            // Skip Result alias handling for Option methods - they have their own implementations
            let is_option_method = name.starts_with("Option_");
            let base_normalized = normalize_method_name(name);
            if !is_option_method {
                if let Some(method) = get_result_method_alias(base_normalized) {
                let result_fn_name = format!("Result_{}", method);
                let func_opt = fn_map.get(&result_fn_name).copied()
                    .or_else(|| self.module.get_function(&result_fn_name));

                // If the function isn't available yet (forward call), declare a stub so we never
                // fall back to emitting an unresolved "unwrapErr" symbol.
                let func = if let Some(f) = func_opt {
                    f
                } else {
                    let param_types: Vec<BasicMetadataTypeEnum> = (0..args.len())
                        .map(|_| self.i8_ptr_t.into())
                        .collect();
                    let ret_ty = match method {
                        "isOk" | "isErr" => self.bool_t.fn_type(&param_types, false),
                        _ => self.i8_ptr_t.fn_type(&param_types, false),
                    };
                    self.module.add_function(&result_fn_name, ret_ty, None)
                };

                // Call the Result method directly
                let fn_type = func.get_type();
                let param_types: Vec<_> = fn_type.get_param_types();
                let mut call_args = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let val = self.eval_value(arg, fn_map)?;
                    let expected_ty = param_types.get(i).copied();

                    // Coerce argument type to match function signature
                    let arg_val = if let Some(expected) = expected_ty {
                        let expected_is_ptr = matches!(expected, inkwell::types::BasicMetadataTypeEnum::PointerType(_));

                        if expected_is_ptr {
                            if val.is_pointer_value() {
                                val
                            } else if val.is_struct_value() {
                                let tmp = self.alloca_for_type(val.get_type().as_basic_type_enum(), "result_arg")?;
                                self.builder.build_store(tmp, val)?;
                                tmp.as_basic_value_enum()
                            } else if val.is_int_value() {
                                let iv = val.into_int_value();
                                let ptr = self.builder.build_int_to_ptr(iv, self.i8_ptr_t, "int_to_ptr")?;
                                ptr.as_basic_value_enum()
                            } else {
                                val
                            }
                        } else {
                            val
                        }
                    } else {
                        val
                    };
                    call_args.push(arg_val.into());
                }

                let call = self.builder.build_call(func, &call_args, "result_call")?;
                if let Some(r) = result {
                    if let Some(val) = call.try_as_basic_value().left() {
                        self.assign_value(r, val)?;
                    }
                }
                return Ok(());
            }
            }
        }

        // Handle known intrinsics
        if let IRValue::Variable(name) = target {
            // Normalize method names using helper from instructions module
            let base_normalized = normalize_method_name(name);

            match base_normalized {
                "toFloat" => {
                    // Convert integer to float
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        let float_val = if val.is_float_value() {
                            val.into_float_value()
                        } else if val.is_int_value() {
                            self.builder.build_signed_int_to_float(
                                val.into_int_value(),
                                self.ctx.f64_type(),
                                "toFloat"
                            )?
                        } else {
                            self.ctx.f64_type().const_zero()
                        };
                        if let Some(r) = result {
                            self.assign_value(r, float_val.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "Result_isOk" | "isOk" => {
                    // Result.isOk() checks if the result is Ok (success)
                    // Result struct layout: { bool is_ok, i8* ok_value, i8* err_value }
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        
                        // If it's a struct, extract the is_ok field (field 0)
                        let is_ok = if val.is_struct_value() {
                            let sv = val.into_struct_value();
                            self.builder.build_extract_value(sv, 0, "is_ok")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.bool_t.const_zero())
                        } else if val.is_int_value() {
                            // Bool value directly
                            val.into_int_value()
                        } else if val.is_pointer_value() {
                            // Pointer to Result struct - load is_ok field
                            let ptr = val.into_pointer_value();
                            let is_ok_ptr = self.builder.build_struct_gep(
                                self.bool_t,
                                ptr,
                                0,
                                "is_ok_ptr"
                            ).unwrap_or(ptr);
                            self.builder.build_load(self.bool_t, is_ok_ptr, "is_ok")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.bool_t.const_zero())
                        } else {
                            self.bool_t.const_zero()
                        };
                        
                        if let Some(r) = result {
                            self.assign_value(r, is_ok.as_basic_value_enum())?;
                        }
                    } else {
                        // No argument - return false
                        if let Some(r) = result {
                            self.assign_value(r, self.bool_t.const_zero().as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "Option_unwrapOr" | "unwrapOr" => {
                    // Option.unwrapOr(default) returns the value if Some, otherwise the default
                    // In Seen, Option<T> is represented as { bool is_some, T value }
                    // We need to return the inner value if is_some, else the default
                    if args.len() >= 2 {
                        let opt_val = self.eval_value(&args[0], fn_map)?;
                        let default_val = self.eval_value(&args[1], fn_map)?;
                        
                        eprintln!("DEBUG: unwrapOr opt_val type: {:?}", opt_val.get_type());
                        
                        let result_val = if opt_val.is_struct_value() {
                            eprintln!("DEBUG: unwrapOr path: struct value");
                            let sv = opt_val.into_struct_value();
                            let is_some = self.builder.build_extract_value(sv, 0, "is_some")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.bool_t.const_zero());
                            
                            // Debug print is_some
                            // We can't print runtime value easily without printf, but we can check if it's const
                            // But it's likely not const.
                            
                            // Extract inner value - ensure type matches default
                            let inner_val = self.builder.build_extract_value(sv, 1, "inner_val")
                                .ok();
                            
                            if let Some(inner) = inner_val {
                                eprintln!("DEBUG: unwrapOr inner type: {:?}", inner.get_type());
                                eprintln!("DEBUG: unwrapOr default type: {:?}", default_val.get_type());
                                
                                // Types must match for select - if they do, use select
                                if inner.get_type() == default_val.get_type() {
                                    self.builder.build_select(is_some, inner, default_val, "unwrap_or_result")?
                                } else {
                                    eprintln!("DEBUG: unwrapOr type mismatch, using phi");
                                    // Type mismatch - need to cast or use conditional blocks
                                    // For now, if is_some use inner, else use default via phi
                                    let current_fn = self.builder.get_insert_block()
                                        .and_then(|b| b.get_parent())
                                        .ok_or_else(|| anyhow!("No current function"))?;
                                    
                                    let then_block = self.ctx.append_basic_block(current_fn, "unwrap_some");
                                    let else_block = self.ctx.append_basic_block(current_fn, "unwrap_none");
                                    let merge_block = self.ctx.append_basic_block(current_fn, "unwrap_merge");
                                    
                                    self.builder.build_conditional_branch(is_some, then_block, else_block)?;
                                    
                                    // Then block - return inner as pointer (generic)
                                    self.builder.position_at_end(then_block);
                                    let inner_as_ptr = if inner.is_pointer_value() {
                                        inner.into_pointer_value()
                                    } else if inner.is_int_value() {
                                        self.builder.build_int_to_ptr(inner.into_int_value(), self.i8_ptr_t, "inner_ptr")?
                                    } else {
                                        self.i8_ptr_t.const_null()
                                    };
                                    self.builder.build_unconditional_branch(merge_block)?;
                                    let then_block_end = self.builder.get_insert_block().unwrap();
                                    
                                    // Else block - return default as pointer
                                    self.builder.position_at_end(else_block);
                                    let default_as_ptr = if default_val.is_pointer_value() {
                                        default_val.into_pointer_value()
                                    } else if default_val.is_int_value() {
                                        self.builder.build_int_to_ptr(default_val.into_int_value(), self.i8_ptr_t, "default_ptr")?
                                    } else {
                                        self.i8_ptr_t.const_null()
                                    };
                                    self.builder.build_unconditional_branch(merge_block)?;
                                    let else_block_end = self.builder.get_insert_block().unwrap();
                                    
                                    // Merge with phi
                                    self.builder.position_at_end(merge_block);
                                    let phi = self.builder.build_phi(self.i8_ptr_t, "unwrap_result")?;
                                    phi.add_incoming(&[(&inner_as_ptr, then_block_end), (&default_as_ptr, else_block_end)]);
                                    phi.as_basic_value()
                                }
                            } else {
                                // Couldn't extract inner - return default
                                default_val
                            }
                        } else if opt_val.is_pointer_value() {
                            // Pointer to Option struct
                            let ptr = opt_val.into_pointer_value();
                            
                            // Option layout is { bool, i64 } (generic slot)
                            // This assumes all generics are boxed/cast to i64
                            let option_ty = self.ctx.struct_type(&[self.bool_t.into(), self.i64_t.into()], false);
                            
                            // Load is_some (index 0)
                            let is_some_ptr = unsafe { 
                                self.builder.build_struct_gep(option_ty, ptr, 0, "is_some_ptr")
                                    .unwrap_or(ptr) 
                            };
                            let is_some = self.builder.build_load(self.bool_t, is_some_ptr, "is_some")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.bool_t.const_zero());
                            
                            // Load value slot (index 1, i64)
                            let value_slot_ptr = unsafe { 
                                self.builder.build_struct_gep(option_ty, ptr, 1, "value_slot_ptr")
                                    .unwrap() 
                            };
                            let value_slot = self.builder.build_load(self.i64_t, value_slot_ptr, "value_slot")?
                                .into_int_value();
                            
                            // Convert value_slot to T
                            let inner_ty = default_val.get_type();
                            
                            if inner_ty.is_struct_type() {
                                // Structs are boxed. value_slot is pointer to struct.
                                // We must use control flow to avoid loading from invalid pointer if is_some is false.
                                
                                let current_fn = self.current_fn.unwrap();
                                let entry_bb = self.builder.get_insert_block().unwrap();
                                let then_bb = self.ctx.append_basic_block(current_fn, "unwrap_some");
                                let merge_bb = self.ctx.append_basic_block(current_fn, "unwrap_merge");
                                
                                self.builder.build_conditional_branch(is_some, then_bb, merge_bb)?;
                                
                                // Then block: load struct
                                self.builder.position_at_end(then_bb);
                                let struct_ptr = self.builder.build_int_to_ptr(value_slot, self.i8_ptr_t, "struct_ptr")?;
                                let inner_val = self.builder.build_load(inner_ty, struct_ptr, "inner_val")?;
                                self.builder.build_unconditional_branch(merge_bb)?;
                                let then_bb_end = self.builder.get_insert_block().unwrap();
                                
                                // Merge block: phi
                                self.builder.position_at_end(merge_bb);
                                let phi = self.builder.build_phi(inner_ty, "unwrap_result")?;
                                phi.add_incoming(&[(&inner_val, then_bb_end), (&default_val, entry_bb)]);
                                phi.as_basic_value()
                            } else {
                                // For non-struct types (Ptr, Int, Float), the value is stored directly in i64 slot (or cast).
                                // It is safe to decode it unconditionally because we don't dereference it.
                                
                                let inner_val = if inner_ty.is_pointer_type() {
                                    // Pointers are stored as i64
                                    let ptr_val = self.builder.build_int_to_ptr(value_slot, inner_ty.into_pointer_type(), "inner_ptr")?;
                                    ptr_val.as_basic_value_enum()
                                } else if inner_ty.is_int_type() {
                                    // Ints are stored as i64. Cast if needed
                                    if inner_ty.into_int_type().get_bit_width() != 64 {
                                         self.builder.build_int_cast(value_slot, inner_ty.into_int_type(), "int_cast")?.as_basic_value_enum()
                                    } else {
                                         value_slot.as_basic_value_enum()
                                    }
                                } else if inner_ty.is_float_type() {
                                    // Floats are bitcast to i64
                                    let val_cast = self.builder.build_bit_cast(value_slot, self.ctx.f64_type(), "float_cast")?;
                                    val_cast.as_basic_value_enum()
                                } else {
                                    // Fallback
                                    default_val
                                };

                                // Select
                                if inner_val.get_type() == default_val.get_type() {
                                    self.builder.build_select(is_some, inner_val, default_val, "unwrap_or_result")?
                                } else {
                                    default_val
                                }
                            }
                        } else {
                            // Unknown type - return default
                            default_val
                        };
                        
                        if let Some(r) = result {
                            self.assign_value(r, result_val)?;
                        }
                    } else if let Some(default_arg) = args.get(0) {
                        // Single argument - just return it as the default
                        let default_val = self.eval_value(default_arg, fn_map)?;
                        if let Some(r) = result {
                            self.assign_value(r, default_val)?;
                        }
                    } else if let Some(r) = result {
                        self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                // Handle Type_toString methods (enum/struct toString)
                name if name.ends_with("_toString") => {
                    // Extract type name (e.g., "TokenType" from "TokenType_toString")
                    let type_name = &name[..name.len() - "_toString".len()];
                    
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        
                        // Check if this is a known enum type with variants
                        if let Some(variants) = self.enum_types.get(type_name).cloned() {
                            // Enum toString - build a switch on the tag to return variant name
                            if val.is_int_value() {
                                let tag = val.into_int_value();
                                
                                // Create string globals for each variant name
                                let current_fn = self.builder.get_insert_block()
                                    .and_then(|b| b.get_parent())
                                    .ok_or_else(|| anyhow!("No current function"))?;
                                
                                let merge_block = self.ctx.append_basic_block(current_fn, "tostring_merge");
                                
                                // Build switch with variant name strings
                                let mut cases = Vec::new();
                                let mut variant_strings = Vec::new();
                                
                                for (i, variant) in variants.iter().enumerate() {
                                    let variant_block = self.ctx.append_basic_block(current_fn, &format!("variant_{}", i));
                                    cases.push((self.i64_t.const_int(i as u64, false), variant_block));
                                    
                                    // Create string for this variant
                                    let variant_str = self.builder.build_global_string_ptr(variant, &format!("enum_str_{}", i))?;
                                    variant_strings.push((variant_block, variant_str.as_pointer_value()));
                                }
                                
                                // Default case returns the type name
                                let default_block = self.ctx.append_basic_block(current_fn, "default_variant");
                                let default_str = self.builder.build_global_string_ptr(type_name, "enum_default_str")?;
                                
                                self.builder.build_switch(tag, default_block, &cases)?;
                                
                                // Build each variant block to jump to merge
                                for (block, _str_ptr) in &variant_strings {
                                    self.builder.position_at_end(*block);
                                    self.builder.build_unconditional_branch(merge_block)?;
                                }
                                
                                // Default block
                                self.builder.position_at_end(default_block);
                                self.builder.build_unconditional_branch(merge_block)?;
                                
                                // Merge block with phi node
                                self.builder.position_at_end(merge_block);
                                let phi = self.builder.build_phi(self.i8_ptr_t, "variant_str")?;
                                
                                for (block, str_ptr) in &variant_strings {
                                    phi.add_incoming(&[(str_ptr, *block)]);
                                }
                                phi.add_incoming(&[(&default_str.as_pointer_value(), default_block)]);
                                
                                if let Some(r) = result {
                                    self.assign_value(r, phi.as_basic_value())?;
                                }
                                return Ok(());
                            }
                        }
                        
                        // Fallback: convert value to string representation
                        let str_val = if val.is_int_value() {
                            // Convert integer (enum tag) to string number
                            let func = self.ensure_int_to_string_fn();
                            let call = self.builder.build_call(func, &[val.into()], "int2s")?;
                            call.try_as_basic_value().left().unwrap_or_else(|| {
                                // Return type name as fallback
                                self.builder.build_global_string_ptr(type_name, "type_str")
                                    .map(|g| g.as_pointer_value().as_basic_value_enum())
                                    .unwrap_or_else(|_| self.i8_ptr_t.const_null().as_basic_value_enum())
                            })
                        } else if val.is_pointer_value() {
                            // Assume it's already a string
                            val
                        } else if val.is_struct_value() {
                            // Struct toString - return type name
                            self.builder.build_global_string_ptr(type_name, "struct_type_str")
                                .map(|g| g.as_pointer_value().as_basic_value_enum())
                                .unwrap_or_else(|_| self.i8_ptr_t.const_null().as_basic_value_enum())
                        } else {
                            // Return type name as fallback
                            self.builder.build_global_string_ptr(type_name, "fallback_type_str")
                                .map(|g| g.as_pointer_value().as_basic_value_enum())
                                .unwrap_or_else(|_| self.i8_ptr_t.const_null().as_basic_value_enum())
                        };
                        
                        if let Some(r) = result {
                            self.assign_value(r, str_val)?;
                        }
                    } else if let Some(r) = result {
                        // No argument - return type name
                        let type_str = self.builder.build_global_string_ptr(type_name, "type_name_str")
                            .map(|g| g.as_pointer_value().as_basic_value_enum())
                            .unwrap_or_else(|_| self.i8_ptr_t.const_null().as_basic_value_enum());
                        self.assign_value(r, type_str)?;
                    }
                    return Ok(());
                }
                // Handle super() calls - pass through the this pointer for simple inheritance
                "super" => {
                    // Base class constructor call - in Seen's simple class model,
                    // super() initializes base fields. Pass through 'this' if provided.
                    if let Some(arg) = args.get(0) {
                        let this_val = self.eval_value(arg, fn_map)?;
                        if let Some(r) = result {
                            self.assign_value(r, this_val)?;
                        }
                    } else if let Some(r) = result {
                        // No this pointer - return zero (void/unit)
                        self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                // Handle T_* generic method calls by resolving to concrete File type
                // In Seen, T is typically File when used with Result<T, E> in I/O operations
                "T_readToString" => {
                    // T.readToString() on a File - forward to File.readToString
                    let file_methods = ["File.readToString", "File_readToString"];
                    for method_name in &file_methods {
                        if let Some(func) = fn_map.get(*method_name).copied()
                            .or_else(|| self.module.get_function(method_name)) {
                            let mut call_args: Vec<BasicMetadataValueEnum> = Vec::new();
                            for arg in args {
                                let val = self.eval_value(arg, fn_map)?;
                                call_args.push(val.into());
                            }
                            let call = self.builder.build_call(func, &call_args, "file_read")?;
                            if let Some(r) = result {
                                if let Some(val) = call.try_as_basic_value().left() {
                                    self.assign_value(r, val)?;
                                }
                            }
                            return Ok(());
                        }
                    }
                    // Fallback: call __ReadFile runtime intrinsic directly
                    if let Some(arg) = args.get(0) {
                        let file_val = self.eval_value(arg, fn_map)?;
                        // Extract fd from File struct (field 0)
                        let fd = if file_val.is_struct_value() {
                            self.builder.build_extract_value(file_val.into_struct_value(), 0, "fd")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.i64_t.const_zero())
                        } else if file_val.is_int_value() {
                            file_val.into_int_value()
                        } else {
                            // Pointer to File - load fd
                            let ptr = if file_val.is_pointer_value() {
                                file_val.into_pointer_value()
                            } else {
                                self.builder.build_int_to_ptr(file_val.into_int_value(), self.i8_ptr_t, "file_ptr")?
                            };
                            self.builder.build_load(self.i64_t, ptr, "fd")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.i64_t.const_zero())
                        };
                        // Call __ReadFile(fd)
                        if let Some(read_fn) = self.module.get_function("__ReadFile") {
                            let call = self.builder.build_call(read_fn, &[fd.into()], "content")?;
                            // Wrap in Result<String, String>
                            let content = call.try_as_basic_value().left()
                                .unwrap_or_else(|| self.i8_ptr_t.const_null().as_basic_value_enum());
                            let result_ty = self.ctx.struct_type(&[self.bool_t.into(), self.i8_ptr_t.into()], false);
                            let mut ok_result = result_ty.get_undef();
                            ok_result = self.builder.build_insert_value(ok_result, self.bool_t.const_int(1, false), 0, "is_ok")?.into_struct_value();
                            ok_result = self.builder.build_insert_value(ok_result, content, 1, "value")?.into_struct_value();
                            if let Some(r) = result {
                                self.assign_value(r, ok_result.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                    }
                    // Last resort: return error result
                    if let Some(r) = result {
                        let result_ty = self.ctx.struct_type(&[self.bool_t.into(), self.i8_ptr_t.into()], false);
                        let mut err = result_ty.get_undef();
                        err = self.builder.build_insert_value(err, self.bool_t.const_zero(), 0, "is_ok")?.into_struct_value();
                        let msg = self.builder.build_global_string_ptr("File read failed", "err")?;
                        err = self.builder.build_insert_value(err, msg.as_pointer_value(), 1, "msg")?.into_struct_value();
                        self.assign_value(r, err.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "T_write" => {
                    // T.write(content) on a File - forward to __WriteFile
                    if args.len() >= 2 {
                        let file_val = self.eval_value(&args[0], fn_map)?;
                        let content_val = self.eval_value(&args[1], fn_map)?;
                        // Extract fd from File struct
                        let fd = if file_val.is_struct_value() {
                            self.builder.build_extract_value(file_val.into_struct_value(), 0, "fd")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.i64_t.const_zero())
                        } else if file_val.is_int_value() {
                            file_val.into_int_value()
                        } else {
                            let ptr = if file_val.is_pointer_value() {
                                file_val.into_pointer_value()
                            } else {
                                self.builder.build_int_to_ptr(file_val.into_int_value(), self.i8_ptr_t, "file_ptr")?
                            };
                            self.builder.build_load(self.i64_t, ptr, "fd")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.i64_t.const_zero())
                        };
                        // Call __WriteFile(fd, content)
                        if let Some(write_fn) = self.module.get_function("__WriteFile") {
                            let call = self.builder.build_call(write_fn, &[fd.into(), content_val.into()], "written")?;
                            let written = call.try_as_basic_value().left()
                                .unwrap_or_else(|| self.i64_t.const_int(u64::MAX, true).as_basic_value_enum());
                            // Wrap in Result<Int, String>
                            let result_ty = self.ctx.struct_type(&[self.bool_t.into(), self.i64_t.into()], false);
                            let mut ok_result = result_ty.get_undef();
                            ok_result = self.builder.build_insert_value(ok_result, self.bool_t.const_int(1, false), 0, "is_ok")?.into_struct_value();
                            ok_result = self.builder.build_insert_value(ok_result, written, 1, "value")?.into_struct_value();
                            if let Some(r) = result {
                                self.assign_value(r, ok_result.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                    }
                    // Return error result
                    if let Some(r) = result {
                        let result_ty = self.ctx.struct_type(&[self.bool_t.into(), self.i64_t.into()], false);
                        let mut err = result_ty.get_undef();
                        err = self.builder.build_insert_value(err, self.bool_t.const_zero(), 0, "is_ok")?.into_struct_value();
                        err = self.builder.build_insert_value(err, self.i64_t.const_int(u64::MAX, true), 1, "val")?.into_struct_value();
                        self.assign_value(r, err.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "T_close" => {
                    // T.close() on a File - forward to __CloseFile
                    if let Some(arg) = args.get(0) {
                        let file_val = self.eval_value(arg, fn_map)?;
                        // Extract fd from File struct
                        let fd = if file_val.is_struct_value() {
                            self.builder.build_extract_value(file_val.into_struct_value(), 0, "fd")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.i64_t.const_zero())
                        } else if file_val.is_int_value() {
                            file_val.into_int_value()
                        } else {
                            let ptr = if file_val.is_pointer_value() {
                                file_val.into_pointer_value()
                            } else {
                                self.builder.build_int_to_ptr(file_val.into_int_value(), self.i8_ptr_t, "file_ptr")?
                            };
                            self.builder.build_load(self.i64_t, ptr, "fd")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.i64_t.const_zero())
                        };
                        // Call __CloseFile(fd)
                        if let Some(close_fn) = self.module.get_function("__CloseFile") {
                            let call = self.builder.build_call(close_fn, &[fd.into()], "close_result")?;
                            let close_result = call.try_as_basic_value().left()
                                .unwrap_or_else(|| self.i64_t.const_zero().as_basic_value_enum());
                            // Wrap in Result<Int, String>
                            let result_ty = self.ctx.struct_type(&[self.bool_t.into(), self.i64_t.into()], false);
                            let mut ok_result = result_ty.get_undef();
                            ok_result = self.builder.build_insert_value(ok_result, self.bool_t.const_int(1, false), 0, "is_ok")?.into_struct_value();
                            ok_result = self.builder.build_insert_value(ok_result, close_result, 1, "value")?.into_struct_value();
                            if let Some(r) = result {
                                self.assign_value(r, ok_result.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                    }
                    // Return success (close is often optional)
                    if let Some(r) = result {
                        let result_ty = self.ctx.struct_type(&[self.bool_t.into(), self.i64_t.into()], false);
                        let mut ok = result_ty.get_undef();
                        ok = self.builder.build_insert_value(ok, self.bool_t.const_int(1, false), 0, "is_ok")?.into_struct_value();
                        ok = self.builder.build_insert_value(ok, self.i64_t.const_zero(), 1, "val")?.into_struct_value();
                        self.assign_value(r, ok.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                // Handle any other T_* generic method calls
                name if name.starts_with("T_") => {
                    // Try to find a concrete implementation by checking common types
                    let method_name = &name[2..]; // Strip "T_" prefix
                    let concrete_types = ["File", "String", "Vec", "Map", "Result", "Option"];
                    
                    for concrete_type in concrete_types {
                        let underscore_name = format!("{}_{}", concrete_type, method_name);
                        let dot_name = format!("{}.{}", concrete_type, method_name);
                        
                        if let Some(func) = fn_map.get(&underscore_name).copied()
                            .or_else(|| fn_map.get(&dot_name).copied())
                            .or_else(|| self.module.get_function(&underscore_name))
                            .or_else(|| self.module.get_function(&dot_name)) {
                            let mut call_args: Vec<BasicMetadataValueEnum> = Vec::new();
                            for arg in args {
                                let val = self.eval_value(arg, fn_map)?;
                                call_args.push(val.into());
                            }
                            let call = self.builder.build_call(func, &call_args, "generic_call")?;
                            if let Some(r) = result {
                                if let Some(val) = call.try_as_basic_value().left() {
                                    self.assign_value(r, val)?;
                                }
                            }
                            return Ok(());
                        }
                    }
                    
                    // No concrete implementation found - return zero as fallback
                    eprintln!("WARNING: No concrete implementation found for generic method '{}'", name);
                    if let Some(r) = result {
                        self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "__default" => {
                    // Legacy __default without type suffix - try to infer type from result register
                    if let Some(r) = result {
                        // Check if we know the type of the result register
                        let inferred_type = match r {
                            IRValue::Register(reg_id) => {
                                // Try reg_struct_types first
                                if let Some(struct_name) = self.reg_struct_types.get(reg_id).cloned() {
                                    Some(struct_name)
                                } else if let Some(slot_ty) = self.reg_slot_types.get(reg_id) {
                                    // Check LLVM type
                                    if slot_ty.is_struct_type() {
                                        let st = slot_ty.into_struct_type();
                                        // Check if it's a String type (i64, ptr)
                                        if st.count_fields() == 2 {
                                            Some("String".to_string())
                                        } else {
                                            None
                                        }
                                    } else if slot_ty.is_float_type() {
                                        Some("Float".to_string())
                                    } else if slot_ty.is_int_type() {
                                        let it = slot_ty.into_int_type();
                                        if it.get_bit_width() == 1 {
                                            Some("Bool".to_string())
                                        } else {
                                            Some("Int".to_string())
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        
                        let default_val = match inferred_type.as_deref() {
                            Some("String") => {
                                // Empty string: SeenString { len: 0, data: null }
                                let string_ty = self.ty_string();
                                let zero_len = self.i64_t.const_zero();
                                let null_ptr = self.i8_ptr_t.const_null();
                                string_ty.const_named_struct(&[zero_len.into(), null_ptr.into()]).as_basic_value_enum()
                            }
                            Some("Float") => {
                                self.ctx.f64_type().const_zero().as_basic_value_enum()
                            }
                            Some("Bool") => {
                                self.ctx.bool_type().const_zero().as_basic_value_enum()
                            }
                            Some(struct_name) => {
                                // For known struct types, create zeroed struct
                                if let Some((struct_ty, _field_names)) = self.struct_types.get(struct_name).cloned() {
                                    struct_ty.const_zero().as_basic_value_enum()
                                } else {
                                    // Unknown struct - return i64 zero as pointer
                                    self.i64_t.const_zero().as_basic_value_enum()
                                }
                            }
                            None => {
                                // No type info - return 0 (i64) as fallback
                                self.i64_t.const_zero().as_basic_value_enum()
                            }
                        };
                        self.assign_value(r, default_val)?;
                    }
                    return Ok(());
                }
                name if name.starts_with("__default_") => {
                    // __default_TypeName - generate appropriate default value based on type
                    let type_name = &name[10..]; // Skip "__default_"
                    
                    if let Some(r) = result {
                        let default_val = match type_name {
                            "Int" | "i64" | "Integer" => {
                                self.i64_t.const_zero().as_basic_value_enum()
                            }
                            "Float" | "f64" => {
                                self.ctx.f64_type().const_zero().as_basic_value_enum()
                            }
                            "Bool" | "Boolean" => {
                                self.ctx.bool_type().const_zero().as_basic_value_enum()
                            }
                            "String" => {
                                // Empty string: SeenString { len: 0, data: null }
                                let string_ty = self.ty_string();
                                let zero_len = self.i64_t.const_zero();
                                let null_ptr = self.i8_ptr_t.const_null();
                                string_ty.const_named_struct(&[zero_len.into(), null_ptr.into()]).as_basic_value_enum()
                            }
                            _ => {
                                // For struct types, try to create a zeroed struct
                                if let Some((struct_ty, _field_names)) = self.struct_types.get(type_name).cloned() {
                                    // Create zeroed struct
                                    struct_ty.const_zero().as_basic_value_enum()
                                } else {
                                    // Unknown type - return 0 as fallback
                                    self.i64_t.const_zero().as_basic_value_enum()
                                }
                            }
                        };
                        self.assign_value(r, default_val)?;
                    }
                    return Ok(());
                }
                "abort" => {
                    // Print message and exit
                    if let Some(arg) = args.get(0) {
                        let msg_val = self.eval_value(arg, fn_map)?;
                        let msg_ptr = self.as_cstr_ptr(msg_val)?;
                        self.call_printf(&[msg_ptr.into()])?;
                        // Print newline
                        let newline = self.builder.build_global_string_ptr("\n", "newline")?;
                        self.call_printf(&[newline.as_pointer_value().into()])?;
                    }
                    
                    let exit_fn = self.declare_c_void_fn(
                        "exit",
                        &[self.ctx.i32_type().into()],
                        false,
                    );
                    self.builder.build_call(exit_fn, &[self.ctx.i32_type().const_int(1, false).into()], "exit")?;
                    // self.builder.build_unreachable()?;
                    return Ok(());
                }
                "__ArrayNew" => {
                    // Create a new dynamic array with given capacity
                    // Array layout: { i64 len, i64 capacity, i8* data_ptr }
                    // args: [element_size, capacity]
                    
                    // Use capacity from second argument if available, otherwise first (legacy/fallback)
                    let cap_arg = if args.len() >= 2 {
                        &args[1]
                    } else {
                        args.get(0).ok_or_else(|| anyhow!("__ArrayNew requires arguments"))?
                    };

                    let capacity = self.eval_value(cap_arg, fn_map)?;
                    let cap_i64 = self.as_i64(capacity)?;

                    // Ensure minimum capacity of 4 to avoid malloc(0) and reduce reallocs
                    let min_cap = self.i64_t.const_int(4, false);
                    let is_zero = self.builder.build_int_compare(inkwell::IntPredicate::EQ, cap_i64, self.i64_t.const_zero(), "is_zero")?;
                    let alloc_cap = self.builder.build_select(is_zero, min_cap.as_basic_value_enum(), cap_i64.as_basic_value_enum(), "alloc_cap")?.into_int_value();

                        // Allocate header (24 bytes)
                        let header_size = self.i64_t.const_int(24, false);
                        let malloc = self.get_malloc();
                        let header_ptr = self.builder.build_call(malloc, &[header_size.into()], "arr_header_alloc")?
                            .try_as_basic_value().left()
                            .ok_or_else(|| anyhow!("malloc returned void"))?
                            .into_pointer_value();
                        
                        // Allocate data buffer
                        let elem_size = if args.len() >= 2 {
                            let sz_val = self.eval_value(&args[0], fn_map)?;
                            self.as_i64(sz_val)?
                        } else {
                            self.i64_t.const_int(8, false)
                        };
                        let data_size = self.builder.build_int_mul(alloc_cap, elem_size, "data_size")?;
                        let data_ptr = self.builder.build_call(malloc, &[data_size.into()], "arr_data_alloc")?
                            .try_as_basic_value().left()
                            .ok_or_else(|| anyhow!("malloc returned void"))?
                            .into_pointer_value();

                        // Store length = 0
                        let len_ptr = self.builder.build_pointer_cast(
                            header_ptr,
                            self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "len_ptr"
                        )?;
                        self.builder.build_store(len_ptr, self.i64_t.const_zero())?;
                        
                        // Store capacity
                        let cap_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                len_ptr,
                                &[self.i64_t.const_int(1, false)],
                                "cap_ptr"
                            )?
                        };
                        self.builder.build_store(cap_ptr, alloc_cap)?;

                        // Store data pointer
                        let data_ptr_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                len_ptr,
                                &[self.i64_t.const_int(2, false)],
                                "data_ptr_ptr"
                            )?
                        };
                        let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                            data_ptr_ptr,
                            self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "data_ptr_ptr_casted"
                        )?;
                        self.builder.build_store(data_ptr_ptr_casted, data_ptr)?;
                        
                        if let Some(r) = result {
                            self.assign_value(r, header_ptr.as_basic_value_enum())?;
                        }
                        return Ok(());
                }
                // Note: Vec_push must NOT be handled here - Vec has a different layout
                // than Array ({ chunks, capacities, usage, length, totalCapacity, nextChunkSize }
                // vs { length, capacity, data }). Vec_push must call the actual Vec_push function.
                // Check the ORIGINAL name to avoid catching "Vec_push" when it's normalized to "push".
                "push" | "List_push" | "Array_push" if !name.starts_with("Vec_") => {
                    // push(array, value) - append value to dynamic array
                    // CRITICAL: For push to modify the original array, we need the SLOT POINTER
                    // of the array variable, NOT a loaded copy of the array value.
                    if args.len() == 2 {
                        let value = self.eval_value(&args[1], fn_map)?;
                        eprintln!("DEBUG Array_push: args[0]={:?}, value_type={}", args[0], 
                            if value.is_struct_value() { "struct" } 
                            else if value.is_int_value() { "int" }
                            else if value.is_pointer_value() { "pointer" }
                            else if value.is_float_value() { "float" }
                            else { "unknown" });
                        
                        // Try to get the slot pointer for the array variable directly
                        // This is necessary because push needs to modify the array in-place
                        let arr_ptr = match &args[0] {
                            IRValue::Variable(var_name) => {
                                // Use the variable's slot pointer directly
                                if let Some(slot) = self.var_slots.get(var_name).copied() {
                                    eprintln!("DEBUG Array_push: using var_slot for {}", var_name);
                                    slot
                                } else {
                                    // Fallback to eval_value if no slot (shouldn't happen for local arrays)
                                    let arr_val = self.eval_value(&args[0], fn_map)?;
                                    if arr_val.is_pointer_value() {
                                        arr_val.into_pointer_value()
                                    } else if arr_val.is_struct_value() {
                                        // This path LOSES the modification!
                                        eprintln!("WARNING: Array_push on struct value - modification may be lost!");
                                        let sv = arr_val.into_struct_value();
                                        let spill = self.builder.build_alloca(sv.get_type(), "vec_spill")?;
                                        self.builder.build_store(spill, sv)?;
                                        spill
                                    } else {
                                        self.builder.build_int_to_ptr(
                                            arr_val.into_int_value(),
                                            self.i8_ptr_t,
                                            "arr_ptr"
                                        )?
                                    }
                                }
                            }
                            IRValue::Register(reg_id) => {
                                // Check if this register came from a FieldAccess on an Array field
                                // If so, use the field pointer directly to enable in-place modification
                                if let Some((struct_ptr, field_idx, struct_ty)) = self.reg_field_access_info.get(reg_id).copied() {
                                    eprintln!("DEBUG: Array_push using field pointer for Register({}) - struct_ptr={:?}, field_idx={}", reg_id, struct_ptr, field_idx);
                                    // Get a pointer to the array field in the struct
                                    self.builder.build_struct_gep(struct_ty, struct_ptr, field_idx, "arr_field_ptr")?
                                } else {
                                    eprintln!("DEBUG: Array_push fallback for Register({}) - no field access info", reg_id);
                                    // Not from a field access - evaluate normally
                                    let arr_val = self.eval_value(&args[0], fn_map)?;
                                    if arr_val.is_pointer_value() {
                                        arr_val.into_pointer_value()
                                    } else if arr_val.is_struct_value() {
                                        // This path LOSES the modification!
                                        eprintln!("WARNING: Array_push on Register struct value - modification may be lost! reg={}", reg_id);
                                        let sv = arr_val.into_struct_value();
                                        let spill = self.builder.build_alloca(sv.get_type(), "vec_spill")?;
                                        self.builder.build_store(spill, sv)?;
                                        spill
                                    } else {
                                        self.builder.build_int_to_ptr(
                                            arr_val.into_int_value(),
                                            self.i8_ptr_t,
                                            "arr_ptr"
                                        )?
                                    }
                                }
                            }
                            _ => {
                                // Not a variable - evaluate normally
                                let arr_val = self.eval_value(&args[0], fn_map)?;
                                if arr_val.is_pointer_value() {
                                    arr_val.into_pointer_value()
                                } else if arr_val.is_struct_value() {
                                    // This path LOSES the modification!
                                    eprintln!("WARNING: Array_push on non-variable struct - modification may be lost!");
                                    let sv = arr_val.into_struct_value();
                                    let spill = self.builder.build_alloca(sv.get_type(), "vec_spill")?;
                                    self.builder.build_store(spill, sv)?;
                                    spill
                                } else {
                                    self.builder.build_int_to_ptr(
                                        arr_val.into_int_value(),
                                        self.i8_ptr_t,
                                        "arr_ptr"
                                    )?
                                }
                            }
                        };
                        
                        // Load current length
                        let len_ptr = self.builder.build_pointer_cast(
                            arr_ptr,
                            self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "len_ptr"
                        )?;
                        let len = self.builder.build_load(self.i64_t, len_ptr, "len")?.into_int_value();

                        // Load capacity
                        let cap_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                len_ptr,
                                &[self.i64_t.const_int(1, false)],
                                "cap_ptr"
                            )?
                        };
                        let cap = self.builder.build_load(self.i64_t, cap_ptr, "cap")?.into_int_value();

                        // Load data pointer
                        let data_ptr_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                len_ptr,
                                &[self.i64_t.const_int(2, false)],
                                "data_ptr_ptr"
                            )?
                        };
                        let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                            data_ptr_ptr,
                            self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "data_ptr_ptr_casted"
                        )?;
                        let mut data_ptr = self.builder.build_load(self.i8_ptr_t, data_ptr_ptr_casted, "data_ptr")?.into_pointer_value();

                        // Check if resize needed
                        let needs_resize = self.builder.build_int_compare(inkwell::IntPredicate::EQ, len, cap, "needs_resize")?;
                        
                        let current_bb = self.builder.get_insert_block().unwrap();
                        let resize_bb = self.ctx.append_basic_block(self.current_fn.unwrap(), "resize");
                        let cont_bb = self.ctx.append_basic_block(self.current_fn.unwrap(), "push_cont");
                        
                        self.builder.build_conditional_branch(needs_resize, resize_bb, cont_bb)?;
                        
                        // Resize block
                        self.builder.position_at_end(resize_bb);
                        
                        // Handle cap=0 case
                        let cap_is_zero = self.builder.build_int_compare(inkwell::IntPredicate::EQ, cap, self.i64_t.const_zero(), "cap_is_zero")?;
                        let new_cap = self.builder.build_select(
                            cap_is_zero,
                            self.i64_t.const_int(4, false),
                            self.builder.build_int_mul(cap, self.i64_t.const_int(2, false), "mul")?,
                            "new_cap"
                        )?.into_int_value();

                        // Get element size without consuming the value
                        let elem_byte_size = match &value {
                            BasicValueEnum::StructValue(sv) => {
                                // Get actual struct size from LLVM
                                let struct_ty = sv.get_type();
                                let size_of = struct_ty.size_of().map(|sz| {
                                    // Try to extract constant value
                                    if let Some(const_val) = sz.get_zero_extended_constant() {
                                        const_val
                                    } else {
                                        // If not constant, use a safe default for typical structs
                                        // Token has ~56 bytes, String has 16 bytes
                                        64
                                    }
                                }).unwrap_or(64);
                                size_of
                            }
                            BasicValueEnum::FloatValue(_) => 8,
                            _ => 8,  // Default to 8 bytes for i64/pointers
                        };
                        let elem_size = self.i64_t.const_int(elem_byte_size, false);
                        let new_size = self.builder.build_int_mul(new_cap, elem_size, "new_size")?;
                        
                        let realloc = self.get_realloc();
                        let new_data_ptr = self.builder.build_call(realloc, &[data_ptr.into(), new_size.into()], "realloc")?
                            .try_as_basic_value().left()
                            .ok_or_else(|| anyhow!("realloc returned void"))?
                            .into_pointer_value();
                        
                        self.builder.build_store(cap_ptr, new_cap)?;
                        self.builder.build_store(data_ptr_ptr_casted, new_data_ptr)?;
                        self.builder.build_unconditional_branch(cont_bb)?;
                        
                        // Continue block
                        self.builder.position_at_end(cont_bb);
                        let phi = self.builder.build_phi(self.i8_ptr_t, "data_ptr_phi")?;
                        phi.add_incoming(&[(&data_ptr, current_bb), (&new_data_ptr, resize_bb)]);
                        data_ptr = phi.as_basic_value().into_pointer_value();
                        
                        // Handle different value types
                        if value.is_float_value() {
                            // Float array
                            let f64_ptr_ty = self.ctx.f64_type().ptr_type(inkwell::AddressSpace::from(0u16));
                            let data_f64_ptr = self.builder.build_pointer_cast(data_ptr, f64_ptr_ty, "data_f64_ptr")?;
                            let elem_ptr = unsafe {
                                self.builder.build_gep(
                                    self.ctx.f64_type(),
                                    data_f64_ptr,
                                    &[len],
                                    "elem_ptr"
                                )?
                            };
                            self.builder.build_store(elem_ptr, value.into_float_value())?;
                        } else if value.is_int_value() {
                            // Could be integer or pointer-as-int (struct pointer)
                            // Treat as pointer array (for struct arrays)
                            let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                            let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
                            let elem_ptr = unsafe {
                                self.builder.build_gep(
                                    self.i64_t,
                                    data_i64_ptr,
                                    &[len],
                                    "elem_ptr"
                                )?
                            };
                            let val_to_store = if value.into_int_value().get_type().get_bit_width() < 64 {
                                self.builder.build_int_z_extend(value.into_int_value(), self.i64_t, "zext")?
                            } else {
                                value.into_int_value()
                            };
                            self.builder.build_store(elem_ptr, val_to_store)?;
                        } else if value.is_pointer_value() {
                            // Pointer value - this IS the value for reference types (class instances)
                            // Store the pointer as i64 (reference semantics)
                            let ptr = value.into_pointer_value();
                            
                            // Convert pointer to i64 and store it
                            let ptr_as_int = self.builder.build_ptr_to_int(ptr, self.i64_t, "ptr_to_int")?;
                            let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                            let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
                            let elem_ptr = unsafe {
                                self.builder.build_gep(
                                    self.i64_t,
                                    data_i64_ptr,
                                    &[len],
                                    "elem_ptr"
                                )?
                            };
                            self.builder.build_store(elem_ptr, ptr_as_int)?;
                        } else if value.is_struct_value() {
                            let sv = value.into_struct_value();
                            let st = sv.get_type();
                            let ptr_ty = st.ptr_type(inkwell::AddressSpace::from(0u16));
                            let data_struct_ptr = self.builder.build_pointer_cast(data_ptr, ptr_ty, "data_struct_ptr")?;
                            let elem_ptr = unsafe {
                                self.builder.build_gep(
                                    st,
                                    data_struct_ptr,
                                    &[len],
                                    "elem_ptr"
                                )?
                            };
                            self.builder.build_store(elem_ptr, sv)?;
                        } else {
                            return Err(anyhow!("push: unsupported value type"));
                        }
                        
                        // Increment length
                        let new_len = self.builder.build_int_add(len, self.i64_t.const_int(1, false), "new_len")?;
                        self.builder.build_store(len_ptr, new_len)?;
                        
                        if let Some(r) = result {
                            self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                        }
                        return Ok(());
                    }
                }
                "__Print" => {
                    // Print a string
                    if let Some(arg0) = args.get(0) {
                        let val = self.eval_value(arg0, fn_map)?;
                        let s = self.as_cstr_ptr(val)?;
                        // Use printf with %s format
                        let fmt = self.builder.build_global_string_ptr("%s", "fmt_str")?;
                        self.call_printf(&[fmt.as_pointer_value().into(), s.into()])?;
                        if let Some(r) = result {
                            self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                        }
                        return Ok(());
                    }
                }
                "__PrintInt" => {
                    // Print an integer
                    if let Some(arg0) = args.get(0) {
                        let val = self.eval_value(arg0, fn_map)?;
                        let int_val = self.as_i64(val)?;
                        let fmt = self.builder.build_global_string_ptr("%ld", "fmt_int")?;
                        self.call_printf(&[fmt.as_pointer_value().into(), int_val.into()])?;
                        if let Some(r) = result {
                            self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                        }
                        return Ok(());
                    }
                }
                "__GetTime" => {
                    // Get current time in seconds as float (high-resolution timer)
                    // Use clock_gettime(CLOCK_MONOTONIC) for precise timing
                    let clock_gettime = self.get_or_declare_clock_gettime();
                    
                    // Allocate timespec struct on stack: { i64 tv_sec, i64 tv_nsec }
                    let timespec_ty = self.ctx.struct_type(&[
                        self.i64_t.into(),
                        self.i64_t.into(),
                    ], false);
                    let timespec_ptr = self.builder.build_alloca(timespec_ty, "timespec")?;
                    
                    // CLOCK_MONOTONIC = 1
                    let clock_id = self.ctx.i32_type().const_int(1, false);
                    self.builder.build_call(
                        clock_gettime,
                        &[clock_id.into(), timespec_ptr.into()],
                        "gettime_call"
                    )?;
                    
                    // Load tv_sec and tv_nsec
                    let sec_ptr = self.builder.build_struct_gep(timespec_ty, timespec_ptr, 0, "sec_ptr")?;
                    let nsec_ptr = self.builder.build_struct_gep(timespec_ty, timespec_ptr, 1, "nsec_ptr")?;
                    let tv_sec = self.builder.build_load(self.i64_t, sec_ptr, "tv_sec")?.into_int_value();
                    let tv_nsec = self.builder.build_load(self.i64_t, nsec_ptr, "tv_nsec")?.into_int_value();
                    
                    // Convert to float: sec + nsec * 1e-9
                    let sec_f = self.builder.build_signed_int_to_float(tv_sec, self.ctx.f64_type(), "sec_f")?;
                    let nsec_f = self.builder.build_signed_int_to_float(tv_nsec, self.ctx.f64_type(), "nsec_f")?;
                    let nano = self.ctx.f64_type().const_float(1e-9);
                    let nsec_sec = self.builder.build_float_mul(nsec_f, nano, "nsec_sec")?;
                    let time_f = self.builder.build_float_add(sec_f, nsec_sec, "time_f")?;
                    
                    if let Some(r) = result {
                        self.assign_value(r, time_f.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "__PrintFloat" => {
                    if let Some(arg0) = args.get(0) {
                        let val = self.eval_value(arg0, fn_map)?;
                        let float_val = if val.is_float_value() {
                            val.into_float_value()
                        } else if val.is_int_value() {
                            self.builder.build_signed_int_to_float(
                                val.into_int_value(),
                                self.ctx.f64_type(),
                                "i2f"
                            )?
                        } else if val.is_pointer_value() {
                            let int_val = self.builder.build_ptr_to_int(
                                val.into_pointer_value(),
                                self.i64_t,
                                "ptr2i"
                            )?;
                            self.builder.build_signed_int_to_float(
                                int_val,
                                self.ctx.f64_type(),
                                "i2f"
                            )?
                        } else {
                            return Err(anyhow!("__PrintFloat: unsupported value type"));
                        };
                        let fmt = self.builder.build_global_string_ptr("%f\n", "fmt_float")?;
                        self.call_printf(&[fmt.as_pointer_value().into(), float_val.into()])?;
                        if let Some(r) = result {
                            self.assign_value(
                                r,
                                self.i64_t.const_zero().as_basic_value_enum(),
                            )?;
                        }
                        return Ok(());
                    }
                }
                "println" => {
                    if let Some(arg0) = args.get(0) {
                        let a0 = self.eval_value(arg0, fn_map)?;
                        
                        if a0.is_struct_value() {
                            // Safe print for SeenString { len, ptr }
                            let sv = a0.into_struct_value();
                            // Extract len (index 0)
                            let len_i64 = self.builder.build_extract_value(sv, 0, "len")?.into_int_value();
                            // Extract ptr (index 1)
                            let ptr = self.builder.build_extract_value(sv, 1, "ptr")?.into_pointer_value();
                            
                            // Cast len to i32 for printf precision
                            let len_i32 = self.builder.build_int_cast(len_i64, self.ctx.i32_type(), "len_i32")?;
                            
                            // Create format string "%.*s\n"
                            let fmt_str = self.builder.build_global_string_ptr("%.*s\n", "fmt_println_safe")?;
                            let fmt_ptr = self.builder.build_pointer_cast(
                                fmt_str.as_pointer_value(),
                                self.i8_ptr_t,
                                "fmt_ptr"
                            )?;
                            
                            self.call_printf(&[
                                fmt_ptr.into(),
                                len_i32.into(),
                                ptr.into()
                            ])?;
                        } else {
                            // Fallback for C-strings (pointers)
                            let s = self.as_cstr_ptr(a0)?;
                            let fmt_str = self.builder.build_global_string_ptr("%s\n", "fmt_println_cstr")?;
                            let fmt_ptr = self.builder.build_pointer_cast(
                                fmt_str.as_pointer_value(),
                                self.i8_ptr_t,
                                "fmt_ptr"
                            )?;
                            self.call_printf(&[fmt_ptr.into(), s.into()])?;
                        }

                        if let Some(r) = result {
                            self.assign_value(
                                r,
                                self.i64_t.const_zero().as_basic_value_enum(),
                            )?;
                        }
                        return Ok(());
                    }
                }
                "endsWith" => {
                    // endsWith(string, suffix) -> bool
                    if args.len() == 2 {
                        let s_val = self.eval_value(&args[0], fn_map)?;
                        let suf_val = self.eval_value(&args[1], fn_map)?;
                        let s = self.as_cstr_ptr(s_val)?;
                        let suf = self.as_cstr_ptr(suf_val)?;
                        let res = self.runtime_endswith(s, suf)?;
                        if let Some(r) = result {
                            self.assign_value(r, res.as_basic_value_enum())?;
                        }
                        return Ok(());
                    }
                }
                "substring" => {
                    // substring(string, start, end) -> string
                    if args.len() == 3 {
                        let s_val = self.eval_value(&args[0], fn_map)?;
                        let s = self.as_cstr_ptr(s_val)?;
                        let start_v = self.eval_value(&args[1], fn_map)?;
                        let start = self.as_i64(start_v)?;
                        let end_v = self.eval_value(&args[2], fn_map)?;
                        let end = self.as_i64(end_v)?;
                        let res = self.runtime_substring(s, start, end)?;
                        if let Some(r) = result {
                            self.assign_value(r, res.as_basic_value_enum())?;
                        }
                        return Ok(());
                    }
                }
                "charAt" => {
                    // charAt(string, index) -> Char (i8)
                    if args.len() == 2 {
                        let s_val = self.eval_value(&args[0], fn_map)?;
                        let s = self.as_cstr_ptr(s_val)?;
                        let idx_val = self.eval_value(&args[1], fn_map)?;
                        let idx = self.as_i64(idx_val)?;
                        
                        // Get pointer to character at index
                        let char_ptr = unsafe {
                            self.builder.build_gep(self.ctx.i8_type(), s, &[idx], "char_ptr")?
                        };
                        
                        // Load the character as i8
                        let char_val = self.builder.build_load(self.ctx.i8_type(), char_ptr, "char_val")?.into_int_value();
                        
                        if let Some(r) = result {
                            self.assign_value(r, char_val.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "toInt" => {
                    // toInt(char) -> int - convert Char (i8) to Int (i64)
                    if args.len() == 1 {
                        let char_val = self.eval_value(&args[0], fn_map)?;
                        eprintln!("DEBUG toInt: char_val is_int={}, type={:?}", char_val.is_int_value(), char_val.get_type());
                        if char_val.is_int_value() {
                            let iv = char_val.into_int_value();
                            eprintln!("DEBUG toInt: int value bit_width={}, is_const={}, const_val={:?}", 
                                iv.get_type().get_bit_width(),
                                iv.is_const(),
                                if iv.is_const() { Some(iv.get_zero_extended_constant()) } else { None });
                        }
                        let char_val = self.eval_value(&args[0], fn_map)?;
                        let char_i8 = if char_val.is_int_value() {
                            char_val.into_int_value()
                        } else if char_val.is_struct_value() {
                            // Handle Char struct wrapper {i64, ptr} - extract first field
                            let sv = char_val.into_struct_value();
                            if let Ok(first_field) = self.builder.build_extract_value(sv, 0, "char_int") {
                                if first_field.is_int_value() {
                                    first_field.into_int_value()
                                } else {
                                    return Err(anyhow!("toInt: struct first field is not int"));
                                }
                            } else {
                                return Err(anyhow!("toInt: could not extract struct field"));
                            }
                        } else {
                            return Err(anyhow!("toInt expects a Char (int) argument, got {:?}", char_val));
                        };
                        // Sign-extend to i64 if needed
                        let as_i64 = if char_i8.get_type() == self.i64_t {
                            char_i8
                        } else {
                            self.builder.build_int_s_extend(char_i8, self.i64_t, "char_to_int")?
                        };
                        if let Some(r) = result {
                            self.assign_value(r, as_i64.as_basic_value_enum())?;
                        }
                        return Ok(());
                    }
                }
                "__GetCommandLineArgCount" => {
                    let (_g_argc, _g_argv) = self.ensure_arg_globals();
                    let argc = self.builder.build_load(
                        self.ctx.i32_type(),
                        self.g_argc.unwrap().as_pointer_value(),
                        "argc",
                    )?;
                    let argc_int = argc.into_int_value();
                    let argc_i64 = self.builder.build_int_z_extend(argc_int, self.i64_t, "argc_i64")?;
                    if let Some(r) = result {
                        self.assign_value(r, argc_i64.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "__GetCommandLineArg" => {
                    let index_val = self.eval_value(&args[0], fn_map)?;
                    let index_int = self.as_i64(index_val)?;
                    
                    let (_g_argc, _g_argv) = self.ensure_arg_globals();
                    let argv = self.builder.build_load(
                        self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                        self.g_argv.unwrap().as_pointer_value(),
                        "argv",
                    )?;
                    let argv_ptr = argv.into_pointer_value();
                    
                    unsafe {
                        let arg_ptr_ptr = self.builder.build_in_bounds_gep(
                            self.i8_ptr_t,
                            argv_ptr,
                            &[index_int],
                            "arg_ptr_ptr"
                        )?;
                        let arg_cstr = self.builder.build_load(
                            self.i8_ptr_t,
                            arg_ptr_ptr,
                            "arg_cstr"
                        )?;
                        
                        let strlen_fn = self.get_strlen();
                        let len_call = self.builder.build_call(strlen_fn, &[arg_cstr.into()], "len")?;
                        let len = len_call.try_as_basic_value().left().unwrap().into_int_value();
                        let len64 = self.builder.build_int_z_extend(len, self.i64_t, "len64")?;
                        
                        let str_ty = self.ty_string();
                        let mut str_val = str_ty.get_undef();
                        str_val = self.builder.build_insert_value(str_val, len64, 0, "str_len")?.into_struct_value();
                        str_val = self.builder.build_insert_value(str_val, arg_cstr, 1, "str_ptr")?.into_struct_value();
                        
                        if let Some(r) = result {
                            self.assign_value(r, str_val.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "__GetCommandLineArgs" => {
                    let (_g_argc, _g_argv) = self.ensure_arg_globals();
                    let argc = self.builder.build_load(
                        self.ctx.i32_type(),
                        self.g_argc.unwrap().as_pointer_value(),
                        "argc",
                    )?;
                    let argv = self.builder.build_load(
                        self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                        self.g_argv.unwrap().as_pointer_value(),
                        "argv",
                    )?;
                    
                    let helper = self.declare_c_fn(
                        "__GetCommandLineArgsHelper",
                        self.i8_ptr_t.into(),
                        &[self.ctx.i32_type().into(), self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)).into()],
                        false
                    );
                    
                    let call = self.builder.build_call(helper, &[argc.into(), argv.into()], "get_args")?;
                    let ptr = call.try_as_basic_value().left().unwrap().into_pointer_value();
                    
                    if let Some(r) = result {
                        self.assign_value(r, ptr.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "__GetTimestamp" => {
                    // time(NULL) -> seconds -> sprintf into buffer and return char*
                    let time_t = self.i64_t; // treat as i64
                    let time_fn = self.declare_c_fn(
                        "time",
                        time_t.into(),
                        &[self.i8_ptr_t.into()],
                        false,
                    );
                    let null = self.i8_ptr_t.const_null();
                    let secs = self.builder.build_call(time_fn, &[null.into()], "time")?;
                    let secs_val = secs.try_as_basic_value().left().unwrap();
                    // allocate buffer 32 bytes
                    let malloc = self.get_malloc();
                    let sz = self.i64_t.const_int(32, false);
                    let buf = self.builder.build_call(malloc, &[sz.into()], "malloc_ts")?;
                    let bufp = buf
                        .try_as_basic_value()
                        .left()
                        .unwrap()
                        .into_pointer_value();
                    // sprintf(buf, "%lld", secs)
                    let sprintf = self.declare_c_fn(
                        "sprintf",
                        self.i64_t.into(),
                        &[
                            self.i8_ptr_t.into(),
                            self.i8_ptr_t.into(),
                            self.i64_t.into(),
                        ],
                        true,
                    );
                    let fmt = self.builder.build_global_string_ptr("%lld", "fmt_ts")?;
                    let _ = self.builder.build_call(
                        sprintf,
                        &[bufp.into(), fmt.as_pointer_value().into(), secs_val.into()],
                        "sprintf_ts",
                    );
                    if let Some(r) = result {
                        self.assign_value(r, bufp.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "__IntToString" => {
                    // Convert integer to string using sprintf
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        let int_val = self.as_i64(val)?;
                        
                        // Allocate buffer (32 bytes for largest i64)
                        let malloc = self.get_malloc();
                        let sz = self.i64_t.const_int(32, false);
                        let buf = self.builder.build_call(malloc, &[sz.into()], "malloc_inttostr")?;
                        let bufp = buf
                            .try_as_basic_value()
                            .left()
                            .unwrap()
                            .into_pointer_value();
                        
                        // sprintf(buf, "%lld", int_val)
                        let sprintf = self.declare_c_fn(
                            "sprintf",
                            self.ctx.i32_type().into(),
                            &[
                                self.i8_ptr_t.into(),
                                self.i8_ptr_t.into(),
                            ],
                            true,
                        );
                        let fmt = self.builder.build_global_string_ptr("%lld", "fmt_inttostr")?;
                        let _ = self.builder.build_call(
                            sprintf,
                            &[bufp.into(), fmt.as_pointer_value().into(), int_val.into()],
                            "sprintf_inttostr",
                        );
                        
                        if let Some(r) = result {
                            // Convert C string to Seen String struct { i64 len, i8* ptr }
                            let str_struct = self.cstr_to_string_struct(bufp)?;
                            self.assign_value(r, str_struct)?;
                        }
                    }
                    return Ok(());
                }
                "__FloatToString" => {
                    // Convert float to string using sprintf
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        let float_val = if val.is_float_value() {
                            val.into_float_value()
                        } else if val.is_int_value() {
                            self.builder.build_signed_int_to_float(
                                val.into_int_value(),
                                self.ctx.f64_type(),
                                "i2f_fts",
                            )?
                        } else {
                            return Err(anyhow!("__FloatToString requires numeric argument"));
                        };
                        
                        // Allocate buffer (64 bytes for float formatting)
                        let malloc = self.get_malloc();
                        let sz = self.i64_t.const_int(64, false);
                        let buf = self.builder.build_call(malloc, &[sz.into()], "malloc_floattostr")?;
                        let bufp = buf
                            .try_as_basic_value()
                            .left()
                            .unwrap()
                            .into_pointer_value();
                        
                        // sprintf(buf, "%g", float_val)
                        let sprintf = self.declare_c_fn(
                            "sprintf",
                            self.ctx.i32_type().into(),
                            &[
                                self.i8_ptr_t.into(),
                                self.i8_ptr_t.into(),
                            ],
                            true,
                        );
                        let fmt = self.builder.build_global_string_ptr("%g", "fmt_floattostr")?;
                        let _ = self.builder.build_call(
                            sprintf,
                            &[bufp.into(), fmt.as_pointer_value().into(), float_val.into()],
                            "sprintf_floattostr",
                        );
                        
                        if let Some(r) = result {
                            // Convert C string to Seen String struct { i64 len, i8* ptr }
                            let str_struct = self.cstr_to_string_struct(bufp)?;
                            self.assign_value(r, str_struct)?;
                        }
                    }
                    return Ok(());
                }
                "__HasEnv" => {
                    // Check if environment variable exists: getenv(name) != NULL
                    if let Some(arg) = args.get(0) {
                        let name_val = self.eval_value(arg, fn_map)?;
                        let name_ptr = self.as_cstr_ptr(name_val)?;
                        
                        let getenv_fn = self.declare_c_fn(
                            "getenv",
                            self.i8_ptr_t.into(),
                            &[self.i8_ptr_t.into()],
                            false,
                        );
                        let result_ptr = self.builder.build_call(getenv_fn, &[name_ptr.into()], "getenv_result")?
                            .try_as_basic_value().left().unwrap().into_pointer_value();
                        
                        let null = self.i8_ptr_t.const_null();
                        let has_val = self.builder.build_int_compare(
                            inkwell::IntPredicate::NE,
                            result_ptr,
                            null,
                            "has_env",
                        )?;
                        
                        if let Some(r) = result {
                            self.assign_value(r, has_val.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "__GetEnv" => {
                    // Get environment variable: getenv(name) -> string struct
                    if let Some(arg) = args.get(0) {
                        let name_val = self.eval_value(arg, fn_map)?;
                        let name_ptr = self.as_cstr_ptr(name_val)?;
                        
                        let getenv_fn = self.declare_c_fn(
                            "getenv",
                            self.i8_ptr_t.into(),
                            &[self.i8_ptr_t.into()],
                            false,
                        );
                        let result_ptr = self.builder.build_call(getenv_fn, &[name_ptr.into()], "getenv_result")?
                            .try_as_basic_value().left().unwrap().into_pointer_value();
                        
                        // Build a String struct from the C string
                        let str_struct = self.cstr_to_string_struct(result_ptr)?;
                        
                        if let Some(r) = result {
                            self.assign_value(r, str_struct)?;
                        }
                    }
                    return Ok(());
                }
                "__SetEnv" => {
                    // Set environment variable: setenv(name, value, 1) -> 0 on success
                    if args.len() >= 2 {
                        let name_val = self.eval_value(&args[0], fn_map)?;
                        let name_ptr = self.as_cstr_ptr(name_val)?;
                        let value_val = self.eval_value(&args[1], fn_map)?;
                        let value_ptr = self.as_cstr_ptr(value_val)?;
                        
                        let setenv_fn = self.declare_c_fn(
                            "setenv",
                            self.ctx.i32_type().into(),
                            &[self.i8_ptr_t.into(), self.i8_ptr_t.into(), self.ctx.i32_type().into()],
                            false,
                        );
                        let overwrite = self.ctx.i32_type().const_int(1, false);
                        let ret = self.builder.build_call(setenv_fn, &[name_ptr.into(), value_ptr.into(), overwrite.into()], "setenv_result")?
                            .try_as_basic_value().left().unwrap().into_int_value();
                        
                        // Return true if setenv returned 0
                        let success = self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            ret,
                            self.ctx.i32_type().const_zero(),
                            "setenv_success",
                        )?;
                        
                        if let Some(r) = result {
                            self.assign_value(r, success.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "__RemoveEnv" => {
                    // Remove environment variable: unsetenv(name) -> 0 on success
                    if let Some(arg) = args.get(0) {
                        let name_val = self.eval_value(arg, fn_map)?;
                        let name_ptr = self.as_cstr_ptr(name_val)?;
                        
                        let unsetenv_fn = self.declare_c_fn(
                            "unsetenv",
                            self.ctx.i32_type().into(),
                            &[self.i8_ptr_t.into()],
                            false,
                        );
                        let ret = self.builder.build_call(unsetenv_fn, &[name_ptr.into()], "unsetenv_result")?
                            .try_as_basic_value().left().unwrap().into_int_value();
                        
                        // Return true if unsetenv returned 0
                        let success = self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            ret,
                            self.ctx.i32_type().const_zero(),
                            "unsetenv_success",
                        )?;
                        
                        if let Some(r) = result {
                            self.assign_value(r, success.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                // Handle getMessage for error types - return empty string for now
                // The actual message is stored in the error struct but we'd need to know the field offset
                _ if base_normalized.ends_with("_getMessage") || base_normalized == "getMessage" => {
                    // Return empty string as placeholder
                    let empty = self.builder.build_global_string_ptr("", "empty_msg")?;
                    let empty_ptr = empty.as_pointer_value();
                    let mut str_val = self.ty_string().get_undef();
                    str_val = self.builder.build_insert_value(str_val, self.i64_t.const_zero(), 0, "empty_len")?.into_struct_value();
                    str_val = self.builder.build_insert_value(str_val, empty_ptr, 1, "empty_ptr")?.into_struct_value();
                    if let Some(r) = result {
                        self.assign_value(r, str_val.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                // Handle size/length/len as getting array length
                // BUT only for actual Array types - for Vec<T>, call Vec_len instead
                // IMPORTANT: First check if the original function (e.g. Map_size) exists
                "size" | "length" | "len" if args.len() == 1 => {
                    eprintln!("DEBUG LLVM: len/length/size call with arg {:?}, original name={}", args[0], name);
                    
                    // First, try to call the original function if it exists (e.g., Map_size)
                    if let Some(original_fn) = fn_map.get(name).copied()
                        .or_else(|| self.module.get_function(name))
                    {
                        eprintln!("DEBUG LLVM: Found original function '{}', calling it directly", name);
                        let this_val = self.eval_value(&args[0], fn_map)?;
                        let this_ptr = if this_val.is_pointer_value() {
                            this_val.into_pointer_value()
                        } else if this_val.is_int_value() {
                            self.builder.build_int_to_ptr(
                                this_val.into_int_value(),
                                self.i8_ptr_t,
                                "method_this"
                            )?
                        } else {
                            // Spill to stack
                            let tmp = self.alloca_for_type(this_val.get_type().as_basic_type_enum(), "method_this_spill")?;
                            self.builder.build_store(tmp, this_val)?;
                            tmp
                        };
                        
                        let call_val = self.builder.build_call(
                            original_fn,
                            &[this_ptr.into()],
                            "method_result"
                        )?;
                        
                        if let Some(r) = result {
                            if let Some(ret) = call_val.try_as_basic_value().left() {
                                self.assign_value(r, ret)?;
                            }
                        }
                        return Ok(());
                    }
                    
                    // Check if Vec_len exists - if so, this might be a Vec, not an Array
                    // For Vec types, we need to call Vec_len method, not read array header
                    if let Some(vec_len_fn) = fn_map.get("Vec_len").copied()
                        .or_else(|| self.module.get_function("Vec_len")) 
                    {
                        // Check if arg looks like a Vec (tracked in var_is_int_array would be false)
                        let is_likely_vec = match &args[0] {
                            IRValue::Variable(v) => {
                                // If it's not tracked as an int array, it might be a Vec
                                let result = !self.var_is_int_array.contains(v);
                                eprintln!("DEBUG LLVM: Variable({}) is_likely_vec={}, var_is_int_array={:?}", v, result, self.var_is_int_array);
                                result
                            }
                            IRValue::Register(r) => {
                                // Check if register is tracked as array element type
                                let result = self.reg_array_element_struct.get(r).is_none();
                                eprintln!("DEBUG LLVM: Register({}) is_likely_vec={}, reg_array_element_struct={:?}", r, result, self.reg_array_element_struct.get(r));
                                result
                            }
                            _ => {
                                eprintln!("DEBUG LLVM: Other type, is_likely_vec=false");
                                false
                            }
                        };
                        
                        eprintln!("DEBUG LLVM: is_likely_vec={}", is_likely_vec);
                        
                        if is_likely_vec {
                            let cur_fn_name = self.current_fn.map(|f| f.get_name().to_string_lossy().into_owned()).unwrap_or_else(|| "none".to_string());
                            eprintln!("DEBUG LLVM: Taking Vec_len path in fn {}", cur_fn_name);
                            // Call Vec_len instead of inline array length
                            let arr_val = self.eval_value(&args[0], fn_map)?;
                            eprintln!("DEBUG LLVM: arr_val type = {:?}", arr_val);
                            
                            // Check if this is actually a String struct - if so, length is field 0
                            if arr_val.is_struct_value() {
                                let sv = arr_val.into_struct_value();
                                let sv_type = sv.get_type();
                                eprintln!("DEBUG LLVM: struct value with {} fields", sv_type.count_fields());
                                // String is { i64 len, ptr data }
                                if sv_type.count_fields() == 2 {
                                    // Extract length from field 0
                                    let len = self.builder.build_extract_value(sv, 0, "str_len")?;
                                    if len.is_int_value() {
                                        if let Some(r) = result {
                                            self.assign_value(r, len)?;
                                        }
                                        return Ok(());
                                    }
                                }
                            }
                            
                            let arr_ptr = if arr_val.is_pointer_value() {
                                eprintln!("DEBUG LLVM: is_pointer_value");
                                arr_val.into_pointer_value()
                            } else if arr_val.is_int_value() {
                                eprintln!("DEBUG LLVM: is_int_value");
                                self.builder.build_int_to_ptr(
                                    arr_val.into_int_value(), 
                                    self.i8_ptr_t, 
                                    "vec_ptr"
                                )?
                            } else {
                                eprintln!("DEBUG LLVM: FALLTHROUGH - using inline arr_len");
                                // Fall through to default for other types
                                let arr_val = self.eval_value(&args[0], fn_map)?;
                                let arr_ptr = if arr_val.is_pointer_value() {
                                    arr_val.into_pointer_value()
                                } else {
                                    self.builder.build_int_to_ptr(
                                        arr_val.into_int_value(),
                                        self.i8_ptr_t,
                                        "arr_ptr"
                                    )?
                                };
                                let len_ptr = arr_ptr;
                                let len = self.builder.build_load(self.i64_t, len_ptr, "arr_len")?;
                                if let Some(r) = result {
                                    self.assign_value(r, len)?;
                                }
                                return Ok(());
                            };
                            
                            eprintln!("DEBUG LLVM: Calling Vec_len! builder_block={:?}", self.builder.get_insert_block().map(|b| b.get_name().to_string_lossy().into_owned()));
                            let call_val = self.builder.build_call(
                                vec_len_fn,
                                &[arr_ptr.into()],
                                "vec_len"
                            )?;
                            
                            if let Some(r) = result {
                                if let Some(ret) = call_val.try_as_basic_value().left() {
                                    self.assign_value(r, ret)?;
                                }
                            }
                            return Ok(());
                        }
                    }
                    
                    // For actual arrays, use inline length access
                    let arr_val = self.eval_value(&args[0], fn_map)?;
                    // For arrays/vecs, length is stored at offset 0
                    let arr_ptr = if arr_val.is_pointer_value() {
                        arr_val.into_pointer_value()
                    } else if arr_val.is_int_value() {
                        let iv = arr_val.into_int_value();
                        self.builder.build_int_to_ptr(iv, self.i8_ptr_t, "arr_ptr")?
                    } else if arr_val.is_struct_value() {
                        // Spill struct to stack and get pointer
                        let tmp = self.alloca_for_type(arr_val.get_type().as_basic_type_enum(), "size_arg")?;
                        self.builder.build_store(tmp, arr_val)?;
                        tmp
                    } else {
                        // Fall through to default call handling
                        // Don't error - let the auto-declare logic handle it
                        let _ = arr_val; // silence warning
                        // Can't inline, use default path
                        // Don't return Ok here, let it fall through
                        unreachable!()
                    };
                    let len_ptr = arr_ptr;
                    let len = self.builder.build_load(self.i64_t, len_ptr, "arr_len")?;
                    if let Some(r) = result {
                        self.assign_value(r, len)?;
                    }
                    return Ok(());
                }
                // Handle push for List/Vec - forward to Vec_push
                // NOTE: Do NOT forward Vec_push to itself!
                "push" | "List_push" if args.len() == 2 && !name.starts_with("Vec_") => {
                    eprintln!("DEBUG: Forward-to-Vec_push handler for name='{}', normalized='{}'", name, base_normalized);
                    // Try to call Vec_push if it exists
                    if let Some(vec_push) = fn_map.get("Vec_push").copied()
                        .or_else(|| self.module.get_function("Vec_push")) {
                        let arr_val = self.eval_value(&args[0], fn_map)?;
                        let item_val = self.eval_value(&args[1], fn_map)?;
                        
                        // Convert args to pointers if needed
                        // CRITICAL: Vec ptr is stored as i64, must be converted to ptr for function call
                        let arr_ptr = if arr_val.is_pointer_value() {
                            arr_val
                        } else if arr_val.is_struct_value() {
                            let tmp = self.alloca_for_type(arr_val.get_type().as_basic_type_enum(), "push_arr")?;
                            self.builder.build_store(tmp, arr_val)?;
                            tmp.as_basic_value_enum()
                        } else if arr_val.is_int_value() {
                            // i64 -> ptr conversion for Vec pointer
                            self.builder.build_int_to_ptr(
                                arr_val.into_int_value(),
                                self.i8_ptr_t,
                                "vec_ptr"
                            )?.as_basic_value_enum()
                        } else {
                            arr_val
                        };
                        
                        let item_ptr = if item_val.is_pointer_value() {
                            item_val
                        } else if item_val.is_struct_value() {
                            let tmp = self.alloca_for_type(item_val.get_type().as_basic_type_enum(), "push_item")?;
                            self.builder.build_store(tmp, item_val)?;
                            tmp.as_basic_value_enum()
                        } else if item_val.is_int_value() {
                            let iv = item_val.into_int_value();
                            self.builder.build_int_to_ptr(iv, self.i8_ptr_t, "item_ptr")?.as_basic_value_enum()
                        } else {
                            item_val
                        };
                        
                        let call = self.builder.build_call(vec_push, &[arr_ptr.into(), item_ptr.into()], "vec_push")?;
                        if let Some(r) = result {
                            if let Some(val) = call.try_as_basic_value().left() {
                                self.assign_value(r, val)?;
                            }
                        }
                        return Ok(());
                    }
                }
                // Handle close - call fclose or __CloseFile
                "close" if args.len() == 1 => {
                    let file_val = self.eval_value(&args[0], fn_map)?;
                    let fd = self.as_i64(file_val)?;
                    
                    // Try calling File_close if it exists
                    if let Some(file_close) = fn_map.get("File_close").copied()
                        .or_else(|| self.module.get_function("File_close")) {
                        let file_ptr = if file_val.is_pointer_value() {
                            file_val.into_pointer_value()
                        } else {
                            self.builder.build_int_to_ptr(fd, self.i8_ptr_t, "file_ptr")?
                        };
                        let call = self.builder.build_call(file_close, &[file_ptr.into()], "file_close")?;
                        if let Some(r) = result {
                            if let Some(val) = call.try_as_basic_value().left() {
                                self.assign_value(r, val)?;
                            }
                        }
                    } else {
                        // Fallback: use C's close()
                        let close_fn = self.declare_c_fn(
                            "close",
                            self.ctx.i32_type().into(),
                            &[self.ctx.i32_type().into()],
                            false,
                        );
                        let fd_i32 = self.builder.build_int_truncate(fd, self.ctx.i32_type(), "fd_i32")?;
                        let call = self.builder.build_call(close_fn, &[fd_i32.into()], "close")?;
                        if let Some(r) = result {
                            if let Some(val) = call.try_as_basic_value().left() {
                                self.assign_value(r, val)?;
                            }
                        }
                    }
                    return Ok(());
                }
                // Handle readToString - forward to File_readToString
                "readToString" if args.len() == 1 => {
                    if let Some(read_fn) = fn_map.get("File_readToString").copied()
                        .or_else(|| self.module.get_function("File_readToString")) {
                        let file_val = self.eval_value(&args[0], fn_map)?;
                        let file_ptr = if file_val.is_pointer_value() {
                            file_val
                        } else if file_val.is_int_value() {
                            let iv = file_val.into_int_value();
                            self.builder.build_int_to_ptr(iv, self.i8_ptr_t, "file_ptr")?.as_basic_value_enum()
                        } else {
                            file_val
                        };
                        
                        let call = self.builder.build_call(read_fn, &[file_ptr.into()], "readToString")?;
                        if let Some(r) = result {
                            if let Some(val) = call.try_as_basic_value().left() {
                                self.assign_value(r, val)?;
                            }
                        }
                        return Ok(());
                    }
                }
                "__BoolToString" => {
                    // Convert bool to "true" or "false" - returns string struct { i64 len, ptr data }
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        let bool_val = self.as_i64(val)?;
                        
                        // Compare to 0
                        let is_true = self.builder.build_int_compare(
                            inkwell::IntPredicate::NE,
                            bool_val,
                            self.i64_t.const_zero(),
                            "is_true",
                        )?;
                        
                        // Create global strings for "true" and "false"
                        let true_str = self.builder.build_global_string_ptr("true", "str_true")?;
                        let false_str = self.builder.build_global_string_ptr("false", "str_false")?;
                        
                        // Select pointer based on condition
                        let result_ptr = self.builder.build_select(
                            is_true,
                            true_str.as_pointer_value(),
                            false_str.as_pointer_value(),
                            "bool_str_ptr",
                        )?.into_pointer_value();
                        
                        // Select length: "true" = 4, "false" = 5
                        let true_len = self.i64_t.const_int(4, false);
                        let false_len = self.i64_t.const_int(5, false);
                        let result_len = self.builder.build_select(
                            is_true,
                            true_len,
                            false_len,
                            "bool_str_len",
                        )?.into_int_value();
                        
                        // Build string struct { len, ptr }
                        let str_ty = self.ty_string();
                        let mut str_struct = str_ty.get_undef();
                        str_struct = self.builder.build_insert_value(str_struct, result_len, 0, "str_len")?.into_struct_value();
                        str_struct = self.builder.build_insert_value(str_struct, result_ptr, 1, "str_ptr")?.into_struct_value();
                        
                        if let Some(r) = result {
                            self.assign_value(r, str_struct.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "__Sqrt" => {
                    // Call llvm.sqrt.f64 intrinsic
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        let f64_val = if val.is_float_value() {
                            val.into_float_value()
                        } else if val.is_int_value() {
                            self.builder.build_signed_int_to_float(
                                val.into_int_value(),
                                self.ctx.f64_type(),
                                "i2f_sqrt",
                            )?
                        } else {
                            return Err(anyhow!("__Sqrt requires a numeric argument"));
                        };
                        
                        // Use LLVM sqrt intrinsic for optimal codegen
                        let sqrt_intrinsic = inkwell::intrinsics::Intrinsic::find("llvm.sqrt.f64")
                            .ok_or_else(|| anyhow!("Failed to find llvm.sqrt.f64 intrinsic"))?;
                        let sqrt_fn = sqrt_intrinsic.get_declaration(&self.module, &[self.ctx.f64_type().into()])
                            .ok_or_else(|| anyhow!("Failed to get sqrt declaration"))?;
                        
                        let sqrt_result = self.builder.build_call(
                            sqrt_fn,
                            &[f64_val.into()],
                            "sqrt_result",
                        )?;
                        
                        if let Some(r) = result {
                            if let Some(ret) = sqrt_result.try_as_basic_value().left() {
                                self.assign_value(r, ret)?;
                            }
                        }
                    }
                    return Ok(());
                }
                "toFloat" | "__toFloat" => {
                    // Convert Int to Float
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        let f64_val = if val.is_float_value() {
                            val.into_float_value()
                        } else if val.is_int_value() {
                            self.builder.build_signed_int_to_float(
                                val.into_int_value(),
                                self.ctx.f64_type(),
                                "i2f_toFloat",
                            )?
                        } else if val.is_pointer_value() {
                            let int_val = self.builder.build_ptr_to_int(
                                val.into_pointer_value(),
                                self.i64_t,
                                "ptr2i_toFloat",
                            )?;
                            self.builder.build_signed_int_to_float(
                                int_val,
                                self.ctx.f64_type(),
                                "i2f_toFloat",
                            )?
                        } else {
                            return Err(anyhow!("toFloat requires a numeric argument"));
                        };
                        
                        if let Some(r) = result {
                            self.assign_value(r, f64_val.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "toString" | "__toString" | "Int_toString" | "Float_toString" | "Bool_toString" | "Char_toString" => {
                    eprintln!("DEBUG toString handler: name={}", name);
                    // Convert value to String
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        let str_ptr = if val.is_int_value() {
                            let iv = val.into_int_value();
                            let bit_width = iv.get_type().get_bit_width();
                            eprintln!("DEBUG toString: name={}, is_int=true, bit_width={}", name, bit_width);

                            let call_site = if name == "Bool_toString" || bit_width == 1 {
                                eprintln!("DEBUG toString: using BoolToString");
                                let func = self.ensure_bool_to_string_fn();
                                let int_val = self.builder.build_int_z_extend(iv, self.i64_t, "bool_ext")?;
                                self.builder.build_call(func, &[int_val.into()], "bool_str")?
                            } else if name == "Char_toString" || bit_width == 8 {
                                eprintln!("DEBUG toString: using CharToString");
                                let func = self.ensure_char_to_string_fn();
                                let int_val = self.builder.build_int_z_extend(iv, self.i64_t, "char_ext")?;
                                self.builder.build_call(func, &[int_val.into()], "char_str")?
                            } else {
                                eprintln!("DEBUG toString: using IntToString");
                                let func = self.ensure_int_to_string_fn();
                                let int_val = if bit_width < 64 {
                                    self.builder.build_int_s_extend(iv, self.i64_t, "int_ext")?
                                } else {
                                    iv
                                };
                                self.builder.build_call(func, &[int_val.into()], "int_str")?
                            };
                            call_site.try_as_basic_value().left().unwrap_or_else(|| self.i8_ptr_t.const_null().as_basic_value_enum())
                        } else if val.is_float_value() {
                            let func = self.ensure_float_to_string_fn();
                            let call = self.builder.build_call(func, &[val.into()], "f2s")?;
                            call.try_as_basic_value().left().unwrap_or_else(|| self.i8_ptr_t.const_null().as_basic_value_enum())
                        } else if val.is_pointer_value() {
                            // Assume it's already a string
                            val
                        } else {
                            // Default: return empty string
                            self.i8_ptr_t.const_null().as_basic_value_enum()
                        };
                        
                        if let Some(r) = result {
                            self.assign_value(r, str_ptr)?;
                        }
                    }
                    return Ok(());
                }
                "__ReadFile_LEGACY" => {
                    // Removed legacy implementation
                    return Ok(());
                }

                "__CreateDirectory" => {
                    let mkdir = self.declare_c_fn(
                        "mkdir",
                        self.ctx.i32_type().into(),
                        &[self.i8_ptr_t.into(), self.ctx.i32_type().into()],
                        false,
                    );
                    let path_v = self.eval_value(&args[0], fn_map)?;
                    let path = self.as_cstr_ptr(path_v)?;
                    let mode = self.ctx.i32_type().const_int(0o755, false);
                    let rc = self.builder.build_call(
                        mkdir,
                        &[path.into(), mode.into()],
                        "mkdir",
                    )?;
                    let ok = self.builder.build_int_compare(
                        inkwell::IntPredicate::EQ,
                        rc.try_as_basic_value().left().unwrap().into_int_value(),
                        self.ctx.i32_type().const_zero(),
                        "ok",
                    )?;
                    if let Some(r) = result {
                        self.assign_value(r, ok.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "__DeleteFile" => {
                    let rm = self.declare_c_fn(
                        "remove",
                        self.ctx.i32_type().into(),
                        &[self.i8_ptr_t.into()],
                        false,
                    );
                    let path_v = self.eval_value(&args[0], fn_map)?;
                    let path = self.as_cstr_ptr(path_v)?;
                    let rc = self.builder.build_call(rm, &[path.into()], "rm")?;
                    let ok = self.builder.build_int_compare(
                        inkwell::IntPredicate::EQ,
                        rc.try_as_basic_value().left().unwrap().into_int_value(),
                        self.ctx.i32_type().const_zero(),
                        "ok",
                    )?;
                    if let Some(r) = result {
                        self.assign_value(r, ok.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "__ExecuteProgram" => {
                    let system = self.declare_c_fn(
                        "system",
                        self.ctx.i32_type().into(),
                        &[self.i8_ptr_t.into()],
                        false,
                    );
                    let path_v = self.eval_value(&args[0], fn_map)?;
                    let path = self.as_cstr_ptr(path_v)?;
                    let rc = self.builder.build_call(system, &[path.into()], "system")?;
                    let rcv = rc.try_as_basic_value().left().unwrap().into_int_value();
                    let r64 = self.builder.build_int_s_extend(rcv, self.i64_t, "rc64")?;
                    if let Some(r) = result {
                        self.assign_value(r, r64.as_basic_value_enum())?;
                    }
                    return Ok(());
                }
                "__FormatSeenCode" => {
                    // Identity: return input
                    if let Some(arg0) = args.get(0) {
                        let s = self.eval_value(arg0, fn_map)?;
                        if let Some(r) = result {
                            self.assign_value(r, s)?;
                        }
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        if let IRValue::Function { name, .. } = target {
            match name.as_str() {
                "__channel_send_future" => {
                    if args.len() >= 2 {
                        let chan_val = self.eval_value(&args[0], fn_map)?;
                        let chan_ptr = self.to_i8_ptr(chan_val, "send_chan")?;
                        let msg_val = self.eval_value(&args[1], fn_map)?;
                        let boxed = self.box_runtime_value(msg_val)?;
                        let send_fn = self.ensure_channel_send_fn();
                        self.builder
                            .build_call(
                                send_fn,
                                &[
                                    chan_ptr.as_basic_value_enum().into(),
                                    boxed.as_basic_value_enum().into(),
                                ],
                                "channel_send",
                            )
                            .map_err(|e| anyhow!("{e:?}"))?;
                    }
                    let handle_fn = self.ensure_task_handle_new_fn();
                    let kind = self.ctx.i32_type().const_int(3, false);
                    let call = self.builder.build_call(
                        handle_fn,
                        &[kind.into()],
                        "channel_future_handle",
                    )?;
                    if let Some(r) = result {
                        if let Some(val) = call.try_as_basic_value().left() {
                            self.assign_value(r, val)?;
                        }
                    }
                    return Ok(());
                }
                "__spawn_task" | "__spawn_detached" | "__spawn_actor" => {
                    if let Some(arg0) = args.get(0) {
                        let _ = self.eval_value(arg0, fn_map)?;
                    }
                    let spawn_fn = self.ensure_spawn_fn(name.as_str());
                    let call =
                        self.builder
                            .build_call(spawn_fn, &[], "spawn_handle_call")?;
                    if let Some(r) = result {
                        if let Some(val) = call.try_as_basic_value().left() {
                            self.assign_value(r, val)?;
                        }
                    }
                    return Ok(());
                }
                "__await" => {
                    if let Some(arg0) = args.get(0) {
                        let handle_val = self.eval_value(arg0, fn_map)?;
                        let handle_ptr =
                            self.cast_handle_ptr(handle_val, "await_handle_ptr")?;
                        let await_fn = self.ensure_await_fn();
                        let call = self.builder.build_call(
                            await_fn,
                            &[handle_ptr.as_basic_value_enum().into()],
                            "await_call",
                        )?;
                        if let Some(r) = result {
                            if let Some(val) = call.try_as_basic_value().left() {
                                let ok = self
                                    .builder
                                    .build_int_compare(
                                        inkwell::IntPredicate::NE,
                                        val.into_int_value(),
                                        self.ctx.i32_type().const_zero(),
                                        "await_ok",
                                    )
                                    .map_err(|e| anyhow!("{e:?}"))?;
                                self.assign_value(r, ok.as_basic_value_enum())?;
                            }
                        }
                    }
                    return Ok(());
                }
                "__scope_push" | "__scope_pop" => {
                    let scope_fn = self.ensure_scope_fn(name.as_str());
                    let kind = args
                        .get(0)
                        .and_then(|v| {
                            if let IRValue::Integer(k) = v {
                                Some(*k as u64)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    let kind_const = self.ctx.i32_type().const_int(kind, false);
                    self.builder.build_call(
                        scope_fn,
                        &[kind_const.into()],
                        "scope_call",
                    )?;
                    return Ok(());
                }
                _ => {}
            }
        }

        // Handle __ReadFile specially (manual sret)
        if func_name.as_deref() == Some("__ReadFile") {
            // Ensure function is declared with correct signature: void(SeenString*, i64)
            let f = if let Some(f) = self.module.get_function("__ReadFile_SRET") {
                f
            } else {
                let ret_ty = self.ctx.void_type();
                let param_types = &[self.i8_ptr_t.into(), self.i64_t.into()];
                let fn_ty = ret_ty.fn_type(param_types, false);
                self.module.add_function("__ReadFile_SRET", fn_ty, None)
            };

            let fd_arg = args.get(0).expect("missing fd arg");
            let fd_val = self.eval_value(fd_arg, fn_map)?;
            let fd_int = if fd_val.is_int_value() {
                fd_val.into_int_value()
            } else {
                return Err(anyhow!("Invalid fd value type: {:?}", fd_val));
            };

            // Allocate result struct on stack
            let result_ty = self.ty_string().as_basic_type_enum();
            let result_ptr = self.alloca_for_type(result_ty, "read_result")?;

            // Call: __ReadFile_SRET(result_ptr, fd)
            self.builder.build_call(f, &[result_ptr.into(), fd_int.into()], "call_read")?;

            // Load result from stack
            let loaded = self.builder.build_load(result_ty, result_ptr, "loaded_result")?;

            if let Some(r) = result {
                self.assign_value(r, loaded)?;
            }
            return Ok(());
        }

        // Handle __FileError specially (manual sret) - returns String struct
        if func_name.as_deref() == Some("__FileError") {
            // Ensure function is declared with correct signature: void(SeenString*, i64)
            let f = if let Some(f) = self.module.get_function("__FileError_SRET") {
                f
            } else {
                let ret_ty = self.ctx.void_type();
                let param_types = &[self.i8_ptr_t.into(), self.i64_t.into()];
                let fn_ty = ret_ty.fn_type(param_types, false);
                self.module.add_function("__FileError_SRET", fn_ty, None)
            };

            let fd_arg = args.get(0).expect("missing fd arg");
            let fd_val = self.eval_value(fd_arg, fn_map)?;
            let fd_int = if fd_val.is_int_value() {
                fd_val.into_int_value()
            } else {
                return Err(anyhow!("Invalid fd value type: {:?}", fd_val));
            };

            // Allocate result struct on stack
            let result_ty = self.ty_string().as_basic_type_enum();
            let result_ptr = self.alloca_for_type(result_ty, "error_result")?;

            // Call: __FileError_SRET(result_ptr, fd)
            self.builder.build_call(f, &[result_ptr.into(), fd_int.into()], "call_error")?;

            // Load result from stack
            let loaded = self.builder.build_load(result_ty, result_ptr, "loaded_error")?;

            if let Some(r) = result {
                self.assign_value(r, loaded)?;
            }
            return Ok(());
        }

        // Handle __ExecuteCommand specially (manual sret)
        if func_name.as_deref() == Some("__ExecuteCommand") {
            let f = self.module.get_function("__ExecuteCommand").expect("__ExecuteCommand not found");
            let cmd_arg = args.get(0).expect("missing command arg");
            let cmd_val = self.eval_value(cmd_arg, fn_map)?;
            
            // Allocate result struct on stack
            let result_ty = self.ty_cmd_result().as_basic_type_enum();
            let result_ptr = self.alloca_for_type(result_ty, "cmd_result")?;
            
            // Spill command string to stack to pass by pointer
            let cmd_ptr = if cmd_val.is_pointer_value() {
                cmd_val.into_pointer_value()
            } else if cmd_val.is_struct_value() {
                let tmp = self.alloca_for_type(self.ty_string().as_basic_type_enum(), "cmd_spill")?;
                self.builder.build_store(tmp, cmd_val)?;
                tmp
            } else {
                return Err(anyhow!("Invalid command value type: {:?}", cmd_val));
            };
            
            // Call: __ExecuteCommand(result_ptr, cmd_ptr)
            self.builder.build_call(f, &[result_ptr.into(), cmd_ptr.into()], "call_exec")?;
            
            // Load result from stack
            let loaded = self.builder.build_load(result_ty, result_ptr, "loaded_result")?;
            
            if let Some(r) = result {
                // Convert ABI struct {i8, String} to IR struct {i1, String}
                let loaded_sv = loaded.into_struct_value();
                let success_i8 = self.builder.build_extract_value(loaded_sv, 0, "success_i8")?.into_int_value();
                let success_i1 = self.builder.build_int_truncate(success_i8, self.bool_t, "success_i1")?;
                let output = self.builder.build_extract_value(loaded_sv, 1, "output")?;
                
                let ir_struct_ty = self.ctx.struct_type(&[self.bool_t.into(), self.ty_string().into()], false);
                let mut val = ir_struct_ty.get_undef();
                val = self.builder.build_insert_value(val, success_i1, 0, "s_success")?.into_struct_value();
                val = self.builder.build_insert_value(val, output, 1, "s_output")?.into_struct_value();
                
                self.assign_value(r, val.as_basic_value_enum())?;
                
                // Propagate return struct type info
                if let IRValue::Register(reg_id) = r {
                    self.reg_struct_types.insert(*reg_id, "CommandResult".to_string());
                }
            }
            // Skip normal call handling
            return Ok(());
        }

        // Normal call by name - try both underscore and dot naming conventions
        let (f_opt, actual_name) = match target {
            IRValue::Variable(name) => {
                // First try exact name
                if let Some(f) = fn_map.get(name).cloned() {
                    (Some(f), name.clone())
                } else {
                    // Try alternate naming: Type_method <-> Type.method
                    let alt_name = if name.contains('_') && !name.starts_with("__") {
                        // Type_method -> Type.method
                        if let Some(pos) = name.find('_') {
                            let (type_part, method_part) = name.split_at(pos);
                            format!("{}.{}", type_part, &method_part[1..])
                        } else {
                            name.clone()
                        }
                    } else if name.contains('.') {
                        // Type.method -> Type_method
                        name.replace('.', "_")
                    } else {
                        name.clone()
                    };
                    
                    if let Some(f) = fn_map.get(&alt_name).cloned() {
                        (Some(f), alt_name)
                    } else {
                        (None, name.clone())
                    }
                }
            }
            IRValue::Function { name, .. } => {
                if let Some(f) = fn_map.get(name).cloned() {
                    (Some(f), name.clone())
                } else {
                    // Try alternate naming
                    let alt_name = if name.contains('_') && !name.starts_with("__") {
                        if let Some(pos) = name.find('_') {
                            let (type_part, method_part) = name.split_at(pos);
                            format!("{}.{}", type_part, &method_part[1..])
                        } else {
                            name.clone()
                        }
                    } else if name.contains('.') {
                        name.replace('.', "_")
                    } else {
                        name.clone()
                    };
                    
                    if let Some(f) = fn_map.get(&alt_name).cloned() {
                        (Some(f), alt_name)
                    } else {
                        (None, name.clone())
                    }
                }
            }
            _ => (None, String::new()),
        };
        let f = match f_opt {
            Some(func) => func,
            None => {
                // Try to auto-declare external runtime functions starting with __
                if let IRValue::Variable(name) = target {
                    // First check if function already exists in module
                    if let Some(existing) = self.module.get_function(name) {
                        existing
                    } else if name.starts_with("__") {
                        let func = self.auto_declare_runtime_function(name, args.len())?;
                        func
                    } else if name.ends_with("_getMessage") || name.ends_with("_toString") {
                        // Auto-declare error/class message methods as returning String
                        let func_ty = self.ty_string().fn_type(&[self.i8_ptr_t.into()], false);
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else if name.ends_with("_new") && args.len() <= 2 {
                        // Auto-declare error constructors
                        let param_types: Vec<BasicMetadataTypeEnum> = (0..args.len())
                            .map(|_| self.i8_ptr_t.into())
                            .collect();
                        let func_ty = self.i8_ptr_t.fn_type(&param_types, false);
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else if name == "size" || name == "length" || name == "len" || name == "capacity" 
                           || name.ends_with("_size") || name.ends_with("_length") || name.ends_with("_len") {
                        // Auto-declare size/length methods as returning i64
                        let param_types: Vec<BasicMetadataTypeEnum> = (0..args.len())
                            .map(|_| self.i8_ptr_t.into())
                            .collect();
                        let func_ty = self.i64_t.fn_type(&param_types, false);
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else if name == "write" || name == "set" {
                        // C library functions that return int
                        // write: file write, returns bytes written
                        // set: array/map set, returns success
                        // NOTE: "remove" excluded because Map_remove returns Option, not i32
                        let param_types: Vec<BasicMetadataTypeEnum> = (0..args.len())
                            .map(|_| self.i8_ptr_t.into())
                            .collect();
                        let func_ty = self.ctx.i32_type().fn_type(&param_types, false);
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else if name == "isEmpty" || name == "isValid" || name == "contains" 
                           || name == "isOk" || name == "isErr" || name == "isNone" || name == "isSome"
                           || name.ends_with("_isEmpty") || name.ends_with("_contains")
                           || name.ends_with("_isOk") || name.ends_with("_isErr")
                           || name.ends_with("_isNone") || name.ends_with("_isSome") {
                        // Auto-declare boolean-returning methods (including Result/Option)
                        let param_types: Vec<BasicMetadataTypeEnum> = (0..args.len())
                            .map(|_| self.i8_ptr_t.into())
                            .collect();
                        let func_ty = self.bool_t.fn_type(&param_types, false);
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else if name == "unwrap" || name == "unwrapOr" || name == "unwrapErr" 
                           || name.ends_with("_unwrap") || name.ends_with("_unwrapOr") || name.ends_with("_unwrapErr") {
                        // Auto-declare unwrap methods as returning i8_ptr (generic)
                        let param_types: Vec<BasicMetadataTypeEnum> = (0..args.len())
                            .map(|_| self.i8_ptr_t.into())
                            .collect();
                        let func_ty = self.i8_ptr_t.fn_type(&param_types, false);
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else if name == "run_frontend" {
                        // Special handling for run_frontend to ensure correct ABI (String structs by value)
                        let string_ty = self.ty_string();
                        let param_types = vec![
                            string_ty.into(),
                            string_ty.into(),
                            string_ty.into(),
                        ];
                        
                        // Try to find FrontendResult type
                        let ret_ty = self.struct_types.get("FrontendResult")
                            .map(|(st, _)| st.as_basic_type_enum())
                            .or_else(|| self.struct_types.get("bootstrap_frontend_FrontendResult").map(|(st, _)| st.as_basic_type_enum()))
                            .unwrap_or_else(|| self.i8_ptr_t.into());

                        let func_ty = if ret_ty.is_struct_type() {
                             ret_ty.fn_type(&param_types, false)
                        } else {
                             self.i8_ptr_t.fn_type(&param_types, false)
                        };
                        
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else if name == "CompileSeenProgram" {
                        // Ensure correct ABI for CompileSeenProgram (String structs by value)
                        let string_ty = self.ty_string();
                        let param_types = vec![
                            string_ty.into(),
                            string_ty.into(),
                        ];
                        let func_ty = self.bool_t.fn_type(&param_types, false);
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else if name.ends_with("String") || name.starts_with("read") || name.starts_with("get")
                           || name.starts_with("to") || name.contains("String") {
                        // Methods likely returning String
                        let param_types: Vec<BasicMetadataTypeEnum> = (0..args.len())
                            .map(|_| self.i8_ptr_t.into())
                            .collect();
                        let func_ty = self.ty_string().fn_type(&param_types, false);
                        let func = self.module.add_function(name, func_ty, None);
                        func
                    } else {
                        // Fallback: assume generic function returning i8_ptr  
                        let param_types: Vec<BasicMetadataTypeEnum> = (0..args.len())
                            .map(|_| self.i8_ptr_t.into())
                            .collect();
                        let func_ty = self.i8_ptr_t.fn_type(&param_types, false);
                        let func = self.module.add_function(name, func_ty, None);
                        eprintln!("WARNING: Auto-declaring unknown function '{}' with {} args", name, args.len());
                        func
                    }
                } else {
                    return Err(anyhow!("Unknown call target {:?}", target));
                }
            }
        };

        let mut call_args: Vec<BasicMetadataValueEnum> = Vec::new();
        let params = f.get_params();
        let fn_name = func_name.as_deref().unwrap_or("");

        // Handle argument count mismatch for *_new constructors
        // If function expects 0 args but we have args (or vice versa), adjust
        let expected_arg_count = params.len();
        let actual_arg_count = args.len();
        
        // Determine which args to use
        let args_to_process: Vec<&IRValue> = if expected_arg_count == 0 && actual_arg_count > 0 && fn_name.ends_with("_new") {
            // Constructor expects 0 args but was called with args - skip args
            vec![]
        } else if expected_arg_count > actual_arg_count && fn_name.ends_with("_new") {
            // Constructor expects more args (e.g., self) - prepend null pointers
            args.iter().collect()
        } else {
            args.iter().collect()
        };
        
        // If function expects more args than provided, add null/default values
        let mut extra_args_needed = if expected_arg_count > args_to_process.len() {
            expected_arg_count - args_to_process.len()
        } else {
            0
        };

        for (i, a) in args_to_process.iter().enumerate() {
            // Skip if we've already filled all expected params
            if i >= expected_arg_count && expected_arg_count > 0 {
                break;
            }
            
            let v = self.eval_value(a, fn_map)?;
            
            // Debug output for Vec_push call
            if fn_name == "Vec_push" {
                eprintln!("DEBUG Vec_push generic path: i={}, arg={:?}, v_type={:?}, params.len()={}", 
                    i, a, v.get_type(), params.len());
                if let Some(param) = params.get(i) {
                    eprintln!("DEBUG Vec_push generic path: param[{}] type={:?}", i, param.get_type());
                }
            }
            
            // General argument coercion logic
            // This handles:
            // 1. Passing structs as pointers (spilling) when function expects pointer (e.g. generic T)
            // 2. Loading structs from pointers when function expects struct (e.g. String)
            // 3. Int <-> Pointer conversions
            // 4. Legacy i64 conversion for Vec methods (if they still expect i64)
            
            let mut pushed = false;
            if let Some(param) = params.get(i) {
                let expected_ty = param.get_type();
                
                // Case 1: Function expects Pointer
                if expected_ty.is_pointer_type() {
                    if v.is_struct_value() {
                        // Struct -> Pointer (Spill)
                        let sv = v.into_struct_value();
                        let tmp = self.alloca_for_type(sv.get_type().as_basic_type_enum(), "arg_spill")?;
                        self.builder.build_store(tmp, sv)?;
                        call_args.push(tmp.into());
                        pushed = true;
                    } else if v.is_int_value() {
                        // Int -> Pointer
                        // If it's a generic function (T), we might need to spill int to stack?
                        // Or cast int to pointer?
                        // For Vec_push(T), T is generic. If we push int, we should spill it.
                        // But if function expects i8* as "opaque pointer to object", casting int to ptr is wrong if int IS the object.
                        // However, in Seen, Int is a value type. T can hold Int.
                        // If T is represented as i8*, we must box Int?
                        // Or spill Int to stack and pass pointer to it?
                        // The current ABI for generics seems to be "pointer to data".
                        // So we should spill Int.
                        
                        // BUT, legacy code might expect Int cast to Ptr?
                        // Let's check if it's a collection method
                        let is_collection = fn_name.starts_with("Vec_") || fn_name.starts_with("Map_") || fn_name.starts_with("List_");
                        
                        // CRITICAL: The first argument to collection methods is the 'this' pointer,
                        // which is stored as i64 but represents a heap pointer. This should be cast
                        // to ptr, NOT spilled to stack. Only subsequent arguments (generic T values)
                        // should be spilled.
                        let is_this_arg = i == 0 && is_collection;
                        eprintln!("DEBUG: Vec arg i={}, is_collection={}, is_this_arg={}, v.is_int_value={}", i, is_collection, is_this_arg, v.is_int_value());
                        
                        if is_collection && !is_this_arg {
                            // Spill Int to stack for generic T value arguments
                            eprintln!("DEBUG: Collection arg {} spill to stack", i);
                            let iv = v.into_int_value();
                            let tmp = self.alloca_for_type(self.i64_t.into(), "int_spill")?;
                            self.builder.build_store(tmp, iv)?;
                            call_args.push(tmp.into());
                            pushed = true;
                        } else {
                            // Standard Int -> Ptr cast for 'this' pointer or non-collection functions
                            eprintln!("DEBUG: Int to ptr cast for arg {}", i);
                            let ptr = self.builder.build_int_to_ptr(
                                v.into_int_value(), 
                                expected_ty.into_pointer_type(), 
                                "arg_cast"
                            )?;
                            eprintln!("DEBUG: Created arg_cast ptr {:?}", ptr);
                            let ptr_meta: BasicMetadataValueEnum = ptr.into();
                            eprintln!("DEBUG: ptr_meta = {:?}", ptr_meta);
                            call_args.push(ptr_meta);
                            eprintln!("DEBUG: call_args after push = {:?}", call_args.last());
                            pushed = true;
                        }
                    } else if v.is_float_value() {
                        // Float -> Pointer (Spill)
                        let fv = v.into_float_value();
                        let tmp = self.alloca_for_type(self.ctx.f64_type().into(), "float_spill")?;
                        self.builder.build_store(tmp, fv)?;
                        call_args.push(tmp.into());
                        pushed = true;
                    }
                }
                // Case 2: Function expects Struct (e.g. String)
                else if expected_ty.is_struct_type() {
                    if v.is_pointer_value() {
                        // Pointer -> Struct (Load)
                        let ptr = v.into_pointer_value();
                        let loaded = self.builder.build_load(expected_ty, ptr, "struct_load")?;
                        call_args.push(loaded.into());
                        pushed = true;
                    }
                }
                // Case 3: Function expects Int (Legacy Vec support or standard int arg)
                else if expected_ty.is_int_type() {
                    let expected_int_ty = expected_ty.into_int_type();
                    let expected_bits = expected_int_ty.get_bit_width();
                    
                    if v.is_struct_value() {
                        // Struct -> Int (Extract first field or Spill & Cast)
                        let sv = v.into_struct_value();
                        // Try to extract first field if it's an int (optimization)
                        let first = self.builder.build_extract_value(sv, 0, "struct_first");
                        if let Ok(first_val) = first {
                            if first_val.is_int_value() {
                                let int_val = first_val.into_int_value();
                                let actual_bits = int_val.get_type().get_bit_width();
                                // Truncate or extend to match expected size
                                if actual_bits > expected_bits {
                                    let truncated = self.builder.build_int_truncate(int_val, expected_int_ty, "int_trunc")?;
                                    call_args.push(truncated.into());
                                } else if actual_bits < expected_bits {
                                    let extended = self.builder.build_int_z_extend(int_val, expected_int_ty, "int_ext")?;
                                    call_args.push(extended.into());
                                } else {
                                    call_args.push(int_val.into());
                                }
                                pushed = true;
                            }
                        }
                        if !pushed {
                            // Fallback: spill and cast pointer to int
                            let tmp = self.alloca_for_type(sv.get_type().as_basic_type_enum(), "elem_spill")?;
                            self.builder.build_store(tmp, sv)?;
                            let as_i64 = self.builder.build_ptr_to_int(tmp, self.i64_t, "elem_ptr_i64")?;
                            // Truncate if needed
                            if expected_bits < 64 {
                                let truncated = self.builder.build_int_truncate(as_i64, expected_int_ty, "ptr_trunc")?;
                                call_args.push(truncated.into());
                            } else {
                                call_args.push(as_i64.into());
                            }
                            pushed = true;
                        }
                    } else if v.is_int_value() {
                        // Int -> Int (truncate/extend to match expected bit width)
                        let int_val = v.into_int_value();
                        let actual_bits = int_val.get_type().get_bit_width();
                        if actual_bits > expected_bits {
                            let truncated = self.builder.build_int_truncate(int_val, expected_int_ty, "int_trunc")?;
                            call_args.push(truncated.into());
                        } else if actual_bits < expected_bits {
                            let extended = self.builder.build_int_z_extend(int_val, expected_int_ty, "int_ext")?;
                            call_args.push(extended.into());
                        } else {
                            call_args.push(int_val.into());
                        }
                        pushed = true;
                    } else if v.is_pointer_value() {
                        // Pointer -> Int
                        let ptr_val = v.into_pointer_value();
                        let as_i64 = self.builder.build_ptr_to_int(ptr_val, self.i64_t, "ptr2i")?;
                        // Truncate if needed
                        if expected_bits < 64 {
                            let truncated = self.builder.build_int_truncate(as_i64, expected_int_ty, "ptr_trunc")?;
                            call_args.push(truncated.into());
                        } else {
                            call_args.push(as_i64.into());
                        }
                        pushed = true;
                    } else if v.is_float_value() {
                        // Float -> Int (Bitcast)
                        let f64_val = v.into_float_value();
                        let as_i64 = self.builder.build_bit_cast(f64_val, self.i64_t, "f2i_bitcast")?.into_int_value();
                        // Truncate if needed
                        if expected_bits < 64 {
                            let truncated = self.builder.build_int_truncate(as_i64, expected_int_ty, "float_trunc")?;
                            call_args.push(truncated.into());
                        } else {
                            call_args.push(as_i64.into());
                        }
                        pushed = true;
                    }
                }
            }
            
            if !pushed {
                eprintln!("DEBUG: generic call fallback - pushing v={:?} as-is for fn {}", v.get_type(), fn_name);
                call_args.push(v.into());
            }

        }
        
        // Fill in missing arguments with null/default values
        while extra_args_needed > 0 && call_args.len() < expected_arg_count {
            let param_idx = call_args.len();
            if let Some(param) = params.get(param_idx) {
                let expected_ty = param.get_type();
                if expected_ty.is_pointer_type() {
                    call_args.push(expected_ty.into_pointer_type().const_null().into());
                } else if expected_ty.is_int_type() {
                    call_args.push(expected_ty.into_int_type().const_zero().into());
                } else if expected_ty.is_float_type() {
                    call_args.push(expected_ty.into_float_type().const_zero().into());
                } else if expected_ty.is_struct_type() {
                    // For struct types (like String { i64, ptr }), create a zero-initialized struct
                    let struct_ty = expected_ty.into_struct_type();
                    let zero_struct = struct_ty.const_zero();
                    call_args.push(zero_struct.into());
                } else {
                    // Default to i64 zero
                    call_args.push(self.i64_t.const_zero().into());
                }
            }
            extra_args_needed -= 1;
        }
        
        // Debug the final call_args
        if fn_name == "Vec_push" {
            eprintln!("DEBUG Vec_push final call_args before build_call:");
            for (i, arg) in call_args.iter().enumerate() {
                eprintln!("  arg[{}] = {:?}", i, arg);
            }
            eprintln!("DEBUG: Calling build_call with f={:?}, call_args len={}", f.get_name().to_str(), call_args.len());
        }
        
        let call = self.builder.build_call(f, &call_args, "call")?;
        
        if fn_name == "Vec_push" {
            eprintln!("DEBUG Vec_push: call result = {:?}", call);
        }
        // Debug: check the return type of the call
        let target_name = match target {
            IRValue::Variable(name) | IRValue::Function { name, .. } => name.as_str(),
            _ => "unknown",
        };
        if target_name.contains("unwrap") || target_name.contains("Unwrap") {
            eprintln!("DEBUG: After build_call for '{}', f.get_type().get_return_type() = {:?}", 
                target_name, f.get_type().get_return_type());
        }
        if target_name == "SeenLexer_new" {
    //                     println!("DEBUG: Call to {} returned {:?}", target_name, call.try_as_basic_value().left().map(|v| v.get_type()));
    //                     println!("DEBUG:   Function f return type: {:?}", f.get_type().get_return_type());
    //                     println!("DEBUG:   result register: {:?}", result);
        }
        if let Some(r) = result {
            let call_result = call.try_as_basic_value().left();
            if target_name.contains("unwrap") || target_name.contains("Unwrap") {
                eprintln!("DEBUG: {} call result = {:?}, ret type = {:?}", target_name, call_result.is_some(), f.get_type().get_return_type());
            }
            if let Some(ret) = call_result {
                // For Vec_get from float Vec, DON'T bitcast here - keep as i64
                // but mark the register as containing float bits
                if is_vec_get && is_float_vec_call {
                    if let IRValue::Register(reg_id) = r {
                        self.reg_is_float.insert(*reg_id);
                    }
                }
                
                // Special handling for Vec_get returning Option: propagate the Option's inner type
                // If Vec stores Option<T> and we know T, propagate T as the Option's inner type
                if is_vec_get {
                    let vec_arg = args.get(0);
                    let (elem_type, inner_type) = match vec_arg {
                        Some(IRValue::Variable(vec_var)) => {
                            let elem = self.var_array_element_struct.get(vec_var).cloned();
                            let inner = self.var_option_inner_type.get(vec_var).cloned();
                            (elem, inner)
                        }
                        Some(IRValue::Register(vec_reg)) => {
                            let elem = self.reg_array_element_struct.get(vec_reg).cloned();
                            let inner = self.reg_option_inner_type.get(vec_reg).cloned();
                            (elem, inner)
                        }
                        _ => (None, None)
                    };
                    
                    eprintln!("DEBUG: Vec_get on {:?}, elem_type={:?}, inner_type={:?}", vec_arg, elem_type, inner_type);
                    
                    if let Some(elem_type) = elem_type {
                        // If element type is "Option", check for nested inner type
                        if elem_type == "Option" {
                            if let Some(inner_type) = inner_type {
                                if let IRValue::Register(reg_id) = r {
                                    eprintln!("DEBUG: Vec_get returning Option with inner type '{}' to reg {}", inner_type, reg_id);
                                    self.reg_option_inner_type.insert(*reg_id, inner_type);
                                    // Also set reg_struct_types to Option so store propagation works
                                    self.reg_struct_types.insert(*reg_id, "Option".to_string());
                                }
                            }
                        } else {
                            // Element type is not "Option" but something else (like HashEntry)
                            // If Vec_get returns Option<T>, the T is this elem_type
                            // But we need to check if the return type is actually Option
                            if let IRValue::Register(reg_id) = r {
                                // The Vec stores non-Option elements, so get() returns the element directly
                                // In this case, elem_type IS the Option's inner type
                                // (This handles Vec<Option<HashEntry>> where elem_type tracked as HashEntry via push)
                                eprintln!("DEBUG: Vec_get returning element type '{}' to reg {}", elem_type, reg_id);
                                // Track this as the Option's inner type since get() wraps in Option
                                self.reg_option_inner_type.insert(*reg_id, elem_type);
                            }
                        }
                    }
                }
                
                self.assign_value(r, ret.clone())?;
                
                // Debug: trace char return values
                if ret.is_int_value() && ret.into_int_value().get_type().get_bit_width() == 8 {
                    eprintln!("DEBUG: Assigning i8 return value to {:?}, ret_type={:?}", r, ret.get_type());
                }

                // Propagate return struct type info
                eprintln!("DEBUG: emit_call struct type propagation for target={:?}", target);
                let func_name = match target {
                    IRValue::Variable(name) => Some(name),
                    IRValue::Function { name, .. } => Some(name),
                    _ => None,
                };
                eprintln!("DEBUG: emit_call func_name={:?}", func_name);
                
                if let Some(name) = func_name {
                    // Try both underscore and dot naming conventions
                    // Calls use TypeName_method but definitions might use TypeName.method
                    let alt_name = if name.contains('_') {
                        // Convert underscore to dot: TypeName_method -> TypeName.method
                        let parts: Vec<&str> = name.splitn(2, '_').collect();
                        if parts.len() == 2 {
                            Some(format!("{}.{}", parts[0], parts[1]))
                        } else {
                            None
                        }
                    } else if name.contains('.') {
                        // Convert dot to underscore: TypeName.method -> TypeName_method
                        let parts: Vec<&str> = name.splitn(2, '.').collect();
                        if parts.len() == 2 {
                            Some(format!("{}_{}", parts[0], parts[1]))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    
                    // Try to find struct return type using both naming conventions
                    let struct_name = self.fn_return_struct_types.get(name).cloned()
                        .or_else(|| alt_name.as_ref().and_then(|alt| self.fn_return_struct_types.get(alt).cloned()));
                    
                    if name.contains("ImportSymbol") || name.contains("getLocation") || name.contains("TypeError") {
                        eprintln!("DEBUG: Call to '{}', fn_return_struct_types.get = {:?}, alt_name = {:?}, r = {:?}", 
                            name, self.fn_return_struct_types.get(name), alt_name.as_ref().and_then(|alt| self.fn_return_struct_types.get(alt)), r);
                    }
                    
                    if let Some(struct_name) = struct_name {
                        if let IRValue::Register(reg_id) = r {
                            // Don't overwrite a more specific type with a generic 'T'
                            // If we already have a concrete type (like Option) from Vec_get handling, keep it
                            let existing = self.reg_struct_types.get(reg_id).cloned();
                            if existing.is_none() || (struct_name != "T" && struct_name != "V" && struct_name != "E") {
                                eprintln!("DEBUG: Setting reg {} struct type to '{}' from call to '{}'", reg_id, struct_name, name);
                                self.reg_struct_types.insert(*reg_id, struct_name.clone());
                            } else {
                                eprintln!("DEBUG: Keeping existing reg {} struct type '{}' (not overwriting with '{}')", reg_id, existing.as_ref().unwrap_or(&String::new()), struct_name);
                            }
                        }
                    }
                    
                    // Try to find array element struct return type using both naming conventions
                    let elem_struct = self.fn_return_array_element_struct.get(name).cloned()
                        .or_else(|| alt_name.as_ref().and_then(|alt| self.fn_return_array_element_struct.get(alt).cloned()));
                    if let Some(elem_struct) = elem_struct {
                        if let IRValue::Register(reg_id) = r {
                            self.reg_array_element_struct.insert(*reg_id, elem_struct.clone());
                        }
                    }
                    
                    // Special handling for Vec_toArray: propagate element type from Vec to resulting Array
                    if name == "Vec_toArray" {
                        eprintln!("DEBUG: Vec_toArray call detected, args[0] = {:?}", args.get(0));
                        if let Some(vec_arg) = args.get(0) {
                            // Try to get element type from the Vec variable
                            let elem_type = match vec_arg {
                                IRValue::Variable(var_name) => {
                                    eprintln!("DEBUG: Vec_toArray checking var_array_element_struct for '{}'", var_name);
                                    self.var_array_element_struct.get(var_name).cloned()
                                }
                                IRValue::Register(reg_id) => {
                                    self.reg_array_element_struct.get(reg_id).cloned()
                                }
                                _ => None,
                            };
                            eprintln!("DEBUG: Vec_toArray elem_type = {:?}", elem_type);
                            if let Some(elem_type) = elem_type {
                                if let IRValue::Register(reg_id) = r {
                                    eprintln!("DEBUG: Vec_toArray propagating element type '{}' to reg {}", elem_type, reg_id);
                                    self.reg_array_element_struct.insert(*reg_id, elem_type);
                                }
                            }
                        }
                    }
                    
                    // Special handling for Option/Result unwrap: propagate inner type
                    if name == "Option_unwrap" || name == "Option_Unwrap" || name == "Result_unwrap" || name == "unwrap" || name == "Unwrap" {
                        eprintln!("DEBUG: unwrap special handling triggered for '{}', args: {:?}", name, args);
                        if let Some(container_arg) = args.get(0) {
                            eprintln!("DEBUG: unwrap call to '{}' with arg {:?}", name, container_arg);
                            // Try to get inner type from the Option tracking first, then fall back to array element struct
                            let inner_type = match container_arg {
                                IRValue::Variable(var_name) => {
                                    eprintln!("DEBUG: unwrap checking var_option_inner_type for '{}', all keys: {:?}", 
                                        var_name, self.var_option_inner_type.keys().collect::<Vec<_>>());
                                    self.var_option_inner_type.get(var_name).cloned()
                                        .or_else(|| self.var_array_element_struct.get(var_name).cloned())
                                }
                                IRValue::Register(reg_id) => {
                                    eprintln!("DEBUG: unwrap checking reg_option_inner_type for reg {}", reg_id);
                                    self.reg_option_inner_type.get(reg_id).cloned()
                                        .or_else(|| self.reg_array_element_struct.get(reg_id).cloned())
                                }
                                _ => None,
                            };
                            eprintln!("DEBUG: unwrap inner_type = {:?}", inner_type);
                            if let Some(inner_type) = inner_type {
                                if let IRValue::Register(reg_id) = r {
                                    eprintln!("DEBUG: unwrap propagating inner type '{}' to reg {}", inner_type, reg_id);
                                    self.reg_struct_types.insert(*reg_id, inner_type);
                                }
                            }
                        }
                    }
                    
                    // Track Option inner types when Some() is called
                    if name == "Some" {
                        if let Some(value_arg) = args.get(0) {
                            // Get the type of the value being wrapped in Some
                            let inner_type = match value_arg {
                                IRValue::Variable(var_name) => self.var_struct_types.get(var_name).cloned(),
                                IRValue::Register(reg_id) => self.reg_struct_types.get(reg_id).cloned(),
                                _ => None,
                            };
                            if let Some(inner_type) = inner_type.clone() {
                                if let IRValue::Register(reg_id) = r {
                                    eprintln!("DEBUG: Some() tracking inner type '{}' for result reg {}", inner_type, reg_id);
                                    self.reg_option_inner_type.insert(*reg_id, inner_type);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Auto-declare an external runtime function with a generic signature.
    /// All parameters and return type default to i64 for simplicity.
    /// This allows calling runtime functions that aren't specifically handled.
    fn auto_declare_runtime_function(&mut self, name: &str, arg_count: usize) -> Result<FunctionValue<'ctx>> {
        // Check if already declared
        if let Some(f) = self.module.get_function(name) {
            return Ok(f);
        }
        
        // Build parameter types based on argument count - all i64
        let param_types: Vec<BasicMetadataTypeEnum> = (0..arg_count)
            .map(|_| self.i64_t.into())
            .collect();
        
        // Return type is i64 by default (works for most int/pointer results)
        let fn_type = self.i64_t.fn_type(&param_types, false);
        let func = self.module.add_function(name, fn_type, None);
        
        Ok(func)
    }
}

/// Intrinsic function names that have special handling
pub const INTRINSIC_FUNCTIONS: &[&str] = &[
    // Conversion
    "toFloat",
    "toInt",
    "toString",
    // Array operations
    "__ArrayNew",
    "push",
    "pop",
    "get",
    "set",
    "len",
    "size",
    "capacity",
    "clear",
    "remove",
    "insert",
    "contains",
    "indexOf",
    "lastIndexOf",
    "isEmpty",
    "reverse",
    "slice",
    "join",
    "sort",
    // String operations  
    "charAt",
    "substring",
    "startsWith",
    "endsWith",
    "trim",
    "trimStart",
    "trimEnd",
    "split",
    "replace",
    "replaceAll",
    "toUpperCase",
    "toLowerCase",
    "repeat",
    "padStart",
    "padEnd",
    // Result/Option operations
    "isOk",
    "isOkay",
    "isErr",
    "unwrap",
    "unwrapErr",
    "unwrapOr",
    "isSome",
    "isNone",
    // I/O operations
    "print",
    "println",
    "readLine",
    "readFile",
    "writeFile",
    "fileExists",
    "createDirectory",
    // System operations
    "abort",
    "exit",
    "getenv",
    "setenv",
    "executeCommand",
    "getTimestamp",
    // Math operations
    "abs",
    "sqrt",
    "floor",
    "ceil",
    "round",
    "sin",
    "cos",
    "tan",
    "pow",
    "log",
    "exp",
    "min",
    "max",
    // Type operations
];

/// Helper to normalize method names (e.g. "String_length" -> "length")
pub fn normalize_method_name(name: &str) -> &str {
    if let Some(idx) = name.find('_') {
        // Check if it's a known type prefix
        let prefix = &name[..idx];
        match prefix {
            "String" | "Array" | "Vec" | "List" | "Map" | "Result" | "Option" | "File" 
            | "Int" | "Char" | "Float" | "Bool" => {
                &name[idx + 1..]
            }
            _ => name,
        }
    } else {
        name
    }
}

impl<'ctx> LlvmBackend<'ctx> {
    /// Create thin wrapper functions so unprefixed Result method calls (e.g. `unwrapErr`) resolve
    /// even if the call lowering missed the alias path. The wrapper simply forwards to the
    /// prefixed implementation (e.g. `Result_unwrapErr`).
    fn ensure_result_alias_shims(&mut self) -> Result<()> {
        self.ensure_result_alias_shim("unwrap", "Result_unwrap")?;
        self.ensure_result_alias_shim("unwrapErr", "Result_unwrapErr")?;
        self.ensure_result_alias_shim("unwrapOr", "Result_unwrapOr")?;
        self.ensure_result_alias_shim("isOk", "Result_isOkay")?;
        self.ensure_result_alias_shim("isOkay", "Result_isOkay")?;
        self.ensure_result_alias_shim("isErr", "Result_isErr")?;
        Ok(())
    }

    fn ensure_result_alias_shim(&mut self, alias: &str, target: &str) -> Result<()> {
        let target_fn = match self.module.get_function(target) {
            Some(f) => f,
            None => return Ok(()),
        };

        // If alias exists and already has a body, we're done; if it is only declared, we will
        // materialize a wrapper body below.
        let wrapper = match self.module.get_function(alias) {
            Some(existing) => {
                if existing.count_basic_blocks() > 0 {
                    return Ok(());
                }
                existing
            }
            None => self.module.add_function(alias, target_fn.get_type(), None),
        };

        // Preserve current insertion point so we don't disturb ongoing codegen
        let saved_block = self.builder.get_insert_block();

        let fn_type = target_fn.get_type();
        let entry = self.ctx.append_basic_block(wrapper, "entry");
        self.builder.position_at_end(entry);

        let params: Vec<BasicMetadataValueEnum> = wrapper
            .get_param_iter()
            .map(|p| p.into())
            .collect();
        let call = self.builder.build_call(target_fn, &params, "alias_call")?;

        if fn_type.get_return_type().is_some() {
            if let Some(val) = call.try_as_basic_value().left() {
                self.builder.build_return(Some(&val))?;
            } else {
                self.builder.build_return(None)?;
            }
        } else {
            self.builder.build_return(None)?;
        }

        // Restore insertion point
        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(())
    }
}

/// Helper to map Result/File method aliases to their actual implementation names
pub fn get_result_method_alias(name: &str) -> Option<&'static str> {
    match name {
        // Direct Result methods
        "isOk" | "isOkay" => Some("isOkay"),
        "isErr" | "isError" => Some("isErr"),
        "unwrap" | "getValue" => Some("unwrap"),
        "unwrapErr" | "getError" => Some("unwrapErr"),
        "unwrapOr" | "getValueOrDefault" => Some("unwrapOr"),
        _ => None,
    }
}

/// Check if a function name corresponds to a known intrinsic
pub fn is_intrinsic(name: &str) -> bool {
    let normalized = normalize_method_name(name);
    INTRINSIC_FUNCTIONS.contains(&normalized) || 
    name.starts_with("__") || 
    get_result_method_alias(normalized).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallCategory {
    Intrinsic,
    Direct,
    Indirect,
    Method,
}

/// Categorize a call target for dispatch
pub fn categorize_call(target: &IRValue) -> CallCategory {
    match target {
        IRValue::Variable(name) => {
            if is_intrinsic(name) {
                CallCategory::Intrinsic
            } else if name.contains('_') {
                CallCategory::Method
            } else {
                CallCategory::Direct
            }
        }
        IRValue::Function { .. } => CallCategory::Direct,
        _ => CallCategory::Indirect,
    }
}
