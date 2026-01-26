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
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, BasicMetadataValueEnum};
use inkwell::AddressSpace;
use inkwell::types::{BasicType, BasicMetadataTypeEnum};

use crate::value::IRValue;
use crate::llvm_backend::LlvmBackend;
use crate::llvm::string_ops::RuntimeStringOps;
use crate::llvm::type_cast::TypeCastOps;
use crate::llvm::runtime_fns::RuntimeFunctions;
use crate::llvm::concurrency::ConcurrencyOps;
use crate::llvm::c_library::CLibraryOps;
use crate::llvm::type_builders::TypeBuilders;
use crate::llvm::types_ir::ir_type_to_llvm;

type HashMap<K, V> = IndexMap<K, V>;

/// Check if a type name represents a primitive type that should be unboxed
/// when returned from generic functions like Option.unwrap()
fn is_primitive_type(type_name: &str) -> bool {
    matches!(type_name, "Int" | "Bool" | "Char" | "Float" | "Integer" | "Boolean" | "i64" | "i1" | "i8" | "f64")
}

/// Check if a type name is a generic type parameter placeholder.
/// Handles qualified names like "Result::E", "Option::T" by extracting the base name.
fn is_generic_type_param(type_name: &str) -> bool {
    // Extract the base name after any path separators (. or ::)
    let base = type_name
        .rsplit(&['.', ':'][..])
        .find(|part| !part.is_empty())
        .unwrap_or(type_name);
    matches!(base, "T" | "K" | "V" | "E" | "T1" | "T2" | "U" | "R" | "A" | "B")
}

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
        if self.trace_options.trace_boxing {
            eprintln!("[BOXING] emit_call target={:?}", target);
            // Debug: log Option_Unwrap calls
            if let IRValue::Variable(name) = target {
                if name == "Option_Unwrap" || name == "Option_unwrap" || name == "Unwrap" || name == "unwrap" {
                    eprintln!("[BOXING] emit_call for '{}' with args: {:?}, result: {:?}", name, args, result);
                }
                if name.contains("unwrap") || name.contains("Unwrap") {
                    eprintln!("[BOXING] emit_call checking function '{}' for unwrap handling", name);
                }
            }
        }
        
        // Handle Vec methods specially to convert float<->i64
        let func_name = match target {
            IRValue::Variable(name) => Some(name.clone()),
            IRValue::Function { name, .. } => Some(name.clone()),
            _ => None,
        };
        
        // Debug all function calls
        if self.trace_options.trace_boxing {
            if let Some(ref fn_name) = func_name {
                if fn_name.contains("toString") || fn_name.contains("ToString") {
                    eprintln!("[BOXING] emit_call: func_name={}", fn_name);
                }
            }
        }
        
        let is_vec_push = func_name.as_deref() == Some("Vec_push");
        let is_vec_get = func_name.as_deref() == Some("Vec_get");
        let is_vec_set = func_name.as_deref() == Some("Vec_set");
        let is_map_put = func_name.as_deref() == Some("Map_put");
        let is_map_get = func_name.as_deref() == Some("Map_get");

        // Track Map value types from Map.put calls
        // For Map<K, V>, args[0]=this, args[1]=key, args[2]=value
        if is_map_put {
            if let Some(value_arg) = args.get(2) {
                // Track enum value types
                if let IRValue::Variable(value_var) = value_arg {
                    if let Some(struct_name) = self.var_struct_types.get(value_var).cloned() {
                        if let Some(IRValue::Variable(map_var)) = args.get(0) {
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Map_put tracking value type '{}' for map '{}'", struct_name, map_var);
                            }
                            self.var_array_element_struct.insert(map_var.clone(), struct_name);
                        }
                    }
                    // Check if it's an enum type by looking up the struct type name
                    if let Some(type_name) = self.var_struct_types.get(value_var).cloned() {
                        if self.enum_types.contains_key(&type_name) {
                            if let Some(IRValue::Variable(map_var)) = args.get(0) {
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] Map_put tracking enum value type '{}' for map '{}'", type_name, map_var);
                                }
                                self.var_array_element_struct.insert(map_var.clone(), type_name);
                            }
                        }
                    }
                }
                // Track from register if value comes from expression result
                if let IRValue::Register(value_reg) = value_arg {
                    if let Some(struct_name) = self.reg_struct_types.get(value_reg).cloned() {
                        if let Some(IRValue::Variable(map_var)) = args.get(0) {
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Map_put tracking value type '{}' (from reg) for map '{}'", struct_name, map_var);
                            }
                            self.var_array_element_struct.insert(map_var.clone(), struct_name);
                        }
                    }
                }
            }
        }

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
                        // Also track as element type for unwrap unboxing
                        self.var_array_element_struct.insert(vec_var.clone(), "Float".to_string());
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] Vec_push tracking Float element for vec '{}'", vec_var);
                        }
                    }
                }
                // Track Int element type from push calls (for primitive int values)
                if val.is_int_value() {
                    let int_val = val.into_int_value();
                    let bit_width = int_val.get_type().get_bit_width();
                    if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                        let type_name = match bit_width {
                            64 => "Int",
                            32 => "Int",  // i32 also treated as Int
                            8 => "Char",
                            1 => "Bool",
                            _ => "Int",  // Default to Int for other int widths
                        };
                        // Only track if not already tracked (don't overwrite String etc.)
                        if !self.var_array_element_struct.contains_key(vec_var) {
                            self.var_array_element_struct.insert(vec_var.clone(), type_name.to_string());
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Vec_push tracking {} element for vec '{}' (bit_width={})", type_name, vec_var, bit_width);
                            }
                        }
                    }
                }
                // Track String element type from push calls
                let is_string_type = val.get_type() == self.ctx.ptr_type(AddressSpace::default()).into();
                let is_string_struct = if val.is_struct_value() {
                    let st = val.into_struct_value().get_type();
                    // Check if it matches String struct layout {i64, i8*}
                    st.count_fields() == 2 && 
                    st.get_field_type_at_index(0).map_or(false, |t| t.is_int_type()) &&
                    st.get_field_type_at_index(1).map_or(false, |t| t.is_pointer_type())
                } else {
                    false
                };

                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] Vec_push: value_type={:?}, is_struct={}, is_string_type={}, is_string_struct={}, vec_var={:?}, value={:?}",
                        val.get_type(), val.is_struct_value(), is_string_type, is_string_struct, args.get(0), v);
                }

                if is_string_type || is_string_struct {
                    if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] Vec_push tracking String element for vec '{}'", vec_var);
                        }
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
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] Vec_push tracking Option inner type '{}' for vec '{}'", inner_type, vec_var);
                                    }
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
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] Vec_push tracking Option inner type '{}' for vec '{}' from reg {}", inner_type, vec_var, pushed_reg);
                                    }
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
                                // Check if we need to heap allocate this struct (for collections)
                                let needs_heap_alloc = if let Some(name) = func_name.as_deref() {
                                    match name {
                                        "Vec_push" => i == 1,
                                        "Vec_set" => i == 2,
                                        "Map_set" | "Map_insert" => i == 1 || i == 2,
                                        _ => false
                                    }
                                } else {
                                    false
                                };

                                if needs_heap_alloc {
                                    // Allocate on heap to ensure lifetime persists beyond this call
                                    let ty = val.get_type().as_basic_type_enum();
                                    let size = ty.size_of().unwrap();
                                    let malloc = self.get_malloc();
                                    let ptr = self.builder.build_call(malloc, &[size.into()], "struct_heap_alloc")?
                                        .try_as_basic_value().left()
                                        .ok_or_else(|| anyhow!("malloc returned void"))?
                                        .into_pointer_value();
                                    
                                    // Cast pointer to correct type and store
                                    let typed_ptr = self.builder.build_pointer_cast(
                                        ptr,
                                        self.ctx.ptr_type(AddressSpace::default()),
                                        "struct_ptr_cast"
                                    )?;
                                    self.builder.build_store(typed_ptr, val)?;
                                    
                                    // Return the pointer (cast to expected type if needed, usually i8*)
                                    typed_ptr.as_basic_value_enum()
                                } else {
                                    // Stack allocation for temporary reference
                                    let tmp = self.alloca_for_type(val.get_type().as_basic_type_enum(), "result_arg")?;
                                    self.builder.build_store(tmp, val)?;
                                    tmp.as_basic_value_enum()
                                }
                            } else if val.is_int_value() {
                                let iv = val.into_int_value();
                                let ptr = self.builder.build_int_to_ptr(iv, self.i8_ptr_t, "int_to_ptr")?;
                                ptr.as_basic_value_enum()
                            } else {
                                val
                            }
                        } else {
                            // Expected is not pointer (likely int)
                            if val.is_struct_value() {
                                // If function expects int but we have struct, box it and pass pointer as int
                                // This handles generic functions (taking i64) receiving structs
                                
                                // Heap allocate
                                let ty = val.get_type().as_basic_type_enum();
                                let size = ty.size_of().unwrap();
                                let malloc = self.get_malloc();
                                let ptr = self.builder.build_call(malloc, &[size.into()], "struct_box_alloc")?
                                    .try_as_basic_value().left()
                                    .ok_or_else(|| anyhow!("malloc returned void"))?
                                    .into_pointer_value();
                                
                                let typed_ptr = self.builder.build_pointer_cast(ptr, self.ctx.ptr_type(AddressSpace::default()), "box_ptr_cast")?;
                                self.builder.build_store(typed_ptr, val)?;
                                
                                // Convert pointer to int
                                let ptr_int = self.builder.build_ptr_to_int(ptr, self.i64_t, "box_ptr_int")?;
                                ptr_int.as_basic_value_enum()
                            } else {
                                val
                            }
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

            // Debug: log all function calls that start with __Array
            if name.starts_with("__Array") {
                eprintln!("[DEBUG CALL] name='{}', base_normalized='{}'", name, base_normalized);
            }

            // Handle generic method calls before normalizing
            // Patterns: Int_isNone, E_isSome, ParamNode_unwrap, etc.
            // These are methods on Option<T> where T has been substituted
            if name.contains('_') {
                let parts: Vec<&str> = name.splitn(2, '_').collect();
                if parts.len() == 2 {
                    let method = parts[1];
                    // Check if this is a known Option/Result method
                    // Note: unwrapOr and UnwrapOr are excluded because they have complex generic parameter handling
                    if ["isSome", "isNone", "unwrap", "Unwrap", "IsSome", "IsNone", "expect", "Expect"].contains(&method) {
                        // Try to route to Option_method
                        let concrete_types = ["Option", "Result"];
                        for concrete_type in concrete_types {
                            let underscore_name = format!("{}_{}", concrete_type, method);
                            if let Some(func) = fn_map.get(&underscore_name).copied()
                                .or_else(|| self.module.get_function(&underscore_name)) {
                                let mut call_args: Vec<BasicMetadataValueEnum> = Vec::new();
                                let params = func.get_params();
                                for (i, arg) in args.iter().enumerate() {
                                    let val = self.eval_value(arg, fn_map)?;
                                    if let Some(param) = params.get(i) {
                                        let expected_ty = param.get_type();
                                        if expected_ty.is_pointer_type() && val.is_int_value() {
                                            let ptr = self.builder.build_int_to_ptr(
                                                val.into_int_value(),
                                                expected_ty.into_pointer_type(),
                                                "generic_method_arg_cast"
                                            )?;
                                            call_args.push(ptr.into());
                                        } else {
                                            call_args.push(val.into());
                                        }
                                    } else {
                                        call_args.push(val.into());
                                    }
                                }
                                let call = self.builder.build_call(func, &call_args, "generic_method_call")?;
                                if let Some(r) = result {
                                    if let Some(val) = call.try_as_basic_value().left() {
                                        self.assign_value(r, val)?;
                                    }
                                    // Propagate inner type for unwrap
                                    if method == "unwrap" || method == "Unwrap" {
                                        if let Some(container_arg) = args.get(0) {
                                            let inner_type = match container_arg {
                                                IRValue::Variable(var_name) => {
                                                    self.var_option_inner_type.get(var_name).cloned()
                                                }
                                                IRValue::Register(reg_id) => {
                                                    self.reg_option_inner_type.get(reg_id).cloned()
                                                }
                                                _ => None,
                                            };
                                            if let Some(inner_type) = inner_type {
                                                if let IRValue::Register(reg_id) = r {
                                                    if self.trace_options.trace_boxing {
                                                        eprintln!("[BOXING] Generic unwrap propagating inner type '{}' to reg {}", inner_type, reg_id);
                                                    }
                                                    self.reg_struct_types.insert(*reg_id, inner_type);
                                                }
                                            }
                                        }
                                    }
                                }
                                return Ok(());
                            }
                        }
                    }
                }
            }

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
                            self.assign_value(r, float_val.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "startsWith" | "String_startsWith" => {
                    // Inline implementation: if prefix.len > text.len return false; else memcmp == 0
                    if args.len() >= 2 {
                        let text_raw = self.eval_value(&args[0], fn_map)?;
                        let prefix_raw = self.eval_value(&args[1], fn_map)?;

                        // Convert arguments to SeenString structs
                        // String literals from eval_value are pointers to heap-allocated SeenString structs
                        let text = if text_raw.is_pointer_value() {
                            self.load_seen_string_from_ptr(text_raw.into_pointer_value())?
                        } else {
                            text_raw
                        };
                        let prefix = if prefix_raw.is_pointer_value() {
                            self.load_seen_string_from_ptr(prefix_raw.into_pointer_value())?
                        } else {
                            prefix_raw
                        };

                        // Extract length and data from both strings
                        let text_sv = text.into_struct_value();
                        let prefix_sv = prefix.into_struct_value();
                        let text_len = self.builder.build_extract_value(text_sv, 0, "text_len")?.into_int_value();
                        let text_data = self.builder.build_extract_value(text_sv, 1, "text_data")?.into_pointer_value();
                        let prefix_len = self.builder.build_extract_value(prefix_sv, 0, "prefix_len")?.into_int_value();
                        let prefix_data = self.builder.build_extract_value(prefix_sv, 1, "prefix_data")?.into_pointer_value();

                        // if prefix_len > text_len, return false
                        let len_cmp = self.builder.build_int_compare(
                            inkwell::IntPredicate::UGT,
                            prefix_len,
                            text_len,
                            "prefix_longer",
                        )?;

                        // Get or declare memcmp
                        let memcmp_fn = if let Some(f) = self.module.get_function("memcmp") {
                            f
                        } else {
                            let fn_ty = self.ctx.i32_type().fn_type(
                                &[self.i8_ptr_t.into(), self.i8_ptr_t.into(), self.i64_t.into()],
                                false,
                            );
                            self.module.add_function("memcmp", fn_ty, None)
                        };

                        // Call memcmp(text_data, prefix_data, prefix_len)
                        let memcmp_result = self.builder.build_call(
                            memcmp_fn,
                            &[text_data.into(), prefix_data.into(), prefix_len.into()],
                            "memcmp_result",
                        )?;
                        let cmp_val = memcmp_result.try_as_basic_value().left().unwrap().into_int_value();
                        let cmp_zero = self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            cmp_val,
                            self.ctx.i32_type().const_zero(),
                            "cmp_zero",
                        )?;

                        // Result: !len_cmp && cmp_zero (prefix not longer AND memcmp == 0)
                        let not_longer = self.builder.build_not(len_cmp, "not_longer")?;
                        let starts_with = self.builder.build_and(not_longer, cmp_zero, "starts_with")?;

                        if let Some(r) = result {
                            self.assign_value(r, starts_with.as_basic_value_enum())?;
                        }
                    }
                    return Ok(());
                }
                "endsWith" | "String_endsWith" => {
                    // Runtime: endsWith(SeenString text, SeenString suffix) -> bool
                    if args.len() >= 2 {
                        let text_raw = self.eval_value(&args[0], fn_map)?;
                        let suffix_raw = self.eval_value(&args[1], fn_map)?;

                        // Convert arguments to SeenString structs
                        // String literals from eval_value are pointers to heap-allocated SeenString structs
                        let text = if text_raw.is_pointer_value() {
                            self.load_seen_string_from_ptr(text_raw.into_pointer_value())?
                        } else {
                            text_raw
                        };
                        let suffix = if suffix_raw.is_pointer_value() {
                            self.load_seen_string_from_ptr(suffix_raw.into_pointer_value())?
                        } else {
                            suffix_raw
                        };

                        let func = if let Some(f) = self.module.get_function("endsWith") {
                            f
                        } else {
                            let fn_ty = self.bool_t.fn_type(&[self.ty_string().into(), self.ty_string().into()], false);
                            self.module.add_function("endsWith", fn_ty, None)
                        };

                        let call = self.builder.build_call(func, &[text.into(), suffix.into()], "ends")?;
                        if let Some(r) = result {
                            if let Some(val) = call.try_as_basic_value().left() {
                                self.assign_value(r, val)?;
                            }
                        }
                    }
                    return Ok(());
                }
                "contains" | "String_contains" => {
                    // Runtime: contains(SeenString text, SeenString needle) -> bool
                    if args.len() >= 2 {
                        let text_raw = self.eval_value(&args[0], fn_map)?;
                        let needle_raw = self.eval_value(&args[1], fn_map)?;

                        // Convert arguments to SeenString structs
                        // String literals from eval_value are pointers to heap-allocated SeenString structs
                        let text = if text_raw.is_pointer_value() {
                            self.load_seen_string_from_ptr(text_raw.into_pointer_value())?
                        } else {
                            text_raw
                        };
                        let needle = if needle_raw.is_pointer_value() {
                            self.load_seen_string_from_ptr(needle_raw.into_pointer_value())?
                        } else {
                            needle_raw
                        };

                        let func = if let Some(f) = self.module.get_function("contains") {
                            f
                        } else {
                            let fn_ty = self.bool_t.fn_type(&[self.ty_string().into(), self.ty_string().into()], false);
                            self.module.add_function("contains", fn_ty, None)
                        };

                        let call = self.builder.build_call(func, &[text.into(), needle.into()], "has")?;
                        if let Some(r) = result {
                            if let Some(val) = call.try_as_basic_value().left() {
                                self.assign_value(r, val)?;
                            }
                        }
                    }
                    return Ok(());
                }
                "trim" | "String_trim" => {
                    // Runtime: trim(SeenString text) -> SeenString
                    if let Some(arg) = args.get(0) {
                        let text_raw = self.eval_value(arg, fn_map)?;

                        // Convert argument to SeenString struct
                        // String literals from eval_value are pointers to heap-allocated SeenString structs
                        let text = if text_raw.is_pointer_value() {
                            self.load_seen_string_from_ptr(text_raw.into_pointer_value())?
                        } else {
                            text_raw
                        };

                        let func = if let Some(f) = self.module.get_function("trim") {
                            f
                        } else {
                            let fn_ty = self.ty_string().fn_type(&[self.ty_string().into()], false);
                            self.module.add_function("trim", fn_ty, None)
                        };

                        let call = self.builder.build_call(func, &[text.into()], "trimmed")?;
                        if let Some(r) = result {
                            if let Some(val) = call.try_as_basic_value().left() {
                                self.assign_value(r, val)?;
                            }
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

                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] unwrapOr opt_val type: {:?}", opt_val.get_type());
                        }

                        let result_val = if opt_val.is_struct_value() {
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] unwrapOr path: struct value");
                            }
                            let sv = opt_val.into_struct_value();
                            let is_some = self.builder.build_extract_value(sv, 0, "is_some")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.bool_t.const_zero());

                            // Extract inner value - ensure type matches default
                            let inner_val = self.builder.build_extract_value(sv, 1, "inner_val")
                                .ok();

                            if let Some(inner) = inner_val {
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] unwrapOr inner type: {:?}", inner.get_type());
                                    eprintln!("[BOXING] unwrapOr default type: {:?}", default_val.get_type());
                                }

                                // Types must match for select - if they do, use select
                                if inner.get_type() == default_val.get_type() {
                                    self.builder.build_select(is_some, inner, default_val, "unwrap_or_result")?
                                } else {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] unwrapOr type mismatch, using phi");
                                    }
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
                            let is_some_ptr = 
                                self.builder.build_struct_gep(option_ty, ptr, 0, "is_some_ptr")
                                    .unwrap_or(ptr);
                            let is_some = self.builder.build_load(self.bool_t, is_some_ptr, "is_some")
                                .map(|v| v.into_int_value())
                                .unwrap_or_else(|_| self.bool_t.const_zero());
                            
                            // Load value slot (index 1, i64)
                            let value_slot_ptr = 
                                self.builder.build_struct_gep(option_ty, ptr, 1, "value_slot_ptr")
                                    .unwrap();
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
                // Only use special handling if the function is NOT defined in fn_map
                // This allows user-defined toString methods (like StringBuilder.toString) to be called
                name if name.ends_with("_toString") && !fn_map.contains_key(name) => {
                    // Extract type name (e.g., "TokenType" from "TokenType_toString")
                    let type_name = &name[..name.len() - "_toString".len()];
                    let str_ty = self.ty_string();

                    eprintln!("[DEBUG toString] type_name='{}', enum_types contains: {:?}", type_name, self.enum_types.keys().collect::<Vec<_>>());
                    
                    // Helper to build a String struct from a C string pointer
                    let build_string_struct = |builder: &inkwell::builder::Builder<'ctx>, ptr: inkwell::values::PointerValue<'ctx>, len: u64, str_ty: inkwell::types::StructType<'ctx>, i64_t: inkwell::types::IntType<'ctx>| -> Result<inkwell::values::BasicValueEnum<'ctx>, anyhow::Error> {
                        let len_val = i64_t.const_int(len, false);
                        let mut str_struct = str_ty.get_undef();
                        str_struct = builder.build_insert_value(str_struct, len_val, 0, "str_len")?.into_struct_value();
                        str_struct = builder.build_insert_value(str_struct, ptr, 1, "str_ptr")?.into_struct_value();
                        Ok(str_struct.as_basic_value_enum())
                    };
                    
                    if let Some(arg) = args.get(0) {
                        let val = self.eval_value(arg, fn_map)?;
                        
                        // Check if this is a known enum type with variants
                        if let Some(variants) = self.enum_types.get(type_name).cloned() {
                            eprintln!("[DEBUG toString] Found enum '{}' with {} variants", type_name, variants.len());
                            // Enum toString - build a switch on the tag to return variant name as String struct
                            if val.is_int_value() {
                                let tag = val.into_int_value();
                                
                                // Create string globals for each variant name
                                let current_fn = self.builder.get_insert_block()
                                    .and_then(|b| b.get_parent())
                                    .ok_or_else(|| anyhow!("No current function"))?;
                                
                                let merge_block = self.ctx.append_basic_block(current_fn, "tostring_merge");
                                
                                // Build switch with variant name strings - now as String structs
                                let mut cases = Vec::new();
                                let mut variant_structs: Vec<(inkwell::basic_block::BasicBlock<'ctx>, inkwell::values::BasicValueEnum<'ctx>)> = Vec::new();
                                
                                for (i, variant) in variants.iter().enumerate() {
                                    let variant_block = self.ctx.append_basic_block(current_fn, &format!("variant_{}", i));
                                    cases.push((self.i64_t.const_int(i as u64, false), variant_block));
                                    
                                    // Create string for this variant as a String struct { len, ptr }
                                    let variant_str = self.builder.build_global_string_ptr(variant, &format!("enum_str_{}", i))?;
                                    let variant_len = variant.len() as u64;
                                    let str_struct = build_string_struct(&self.builder, variant_str.as_pointer_value(), variant_len, str_ty, self.i64_t)?;
                                    variant_structs.push((variant_block, str_struct));
                                }
                                
                                // Default case returns the type name as String struct
                                let default_block = self.ctx.append_basic_block(current_fn, "default_variant");
                                let default_str = self.builder.build_global_string_ptr(type_name, "enum_default_str")?;
                                let default_len = type_name.len() as u64;
                                let default_struct = build_string_struct(&self.builder, default_str.as_pointer_value(), default_len, str_ty, self.i64_t)?;
                                
                                self.builder.build_switch(tag, default_block, &cases)?;
                                
                                // Build each variant block to jump to merge
                                for (block, _) in &variant_structs {
                                    self.builder.position_at_end(*block);
                                    self.builder.build_unconditional_branch(merge_block)?;
                                }
                                
                                // Default block
                                self.builder.position_at_end(default_block);
                                self.builder.build_unconditional_branch(merge_block)?;
                                
                                // Merge block with phi node for String struct
                                self.builder.position_at_end(merge_block);
                                let phi = self.builder.build_phi(str_ty, "variant_str_struct")?;
                                
                                for (block, str_struct) in &variant_structs {
                                    phi.add_incoming(&[(str_struct, *block)]);
                                }
                                phi.add_incoming(&[(&default_struct, default_block)]);
                                
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
                                // Return type name as fallback - as String struct
                                let ptr = self.builder.build_global_string_ptr(type_name, "type_str")
                                    .map(|g| g.as_pointer_value())
                                    .unwrap_or_else(|_| self.i8_ptr_t.const_null());
                                build_string_struct(&self.builder, ptr, type_name.len() as u64, str_ty, self.i64_t)
                                    .unwrap_or_else(|_| self.i8_ptr_t.const_null().as_basic_value_enum())
                            })
                        } else if val.is_pointer_value() {
                            // Assume it's already a string - try to build struct
                            let ptr = val.into_pointer_value();
                            // Use strlen to get length for C string pointers
                            let strlen_ty = self.i64_t.fn_type(&[self.i8_ptr_t.into()], false);
                            let strlen_fn = if let Some(f) = self.module.get_function("strlen") {
                                f
                            } else {
                                self.module.add_function("strlen", strlen_ty, None)
                            };
                            let len_call = self.builder.build_call(strlen_fn, &[ptr.into()], "cstr_len")?;
                            let len = len_call.try_as_basic_value().left().unwrap().into_int_value();
                            let mut str_struct = str_ty.get_undef();
                            str_struct = self.builder.build_insert_value(str_struct, len, 0, "str_len")?.into_struct_value();
                            str_struct = self.builder.build_insert_value(str_struct, ptr, 1, "str_ptr")?.into_struct_value();
                            str_struct.as_basic_value_enum()
                        } else if val.is_struct_value() {
                            // Struct toString - return type name as String struct
                            let ptr = self.builder.build_global_string_ptr(type_name, "struct_type_str")
                                .map(|g| g.as_pointer_value())
                                .unwrap_or_else(|_| self.i8_ptr_t.const_null());
                            build_string_struct(&self.builder, ptr, type_name.len() as u64, str_ty, self.i64_t)
                                .unwrap_or_else(|_| self.i8_ptr_t.const_null().as_basic_value_enum())
                        } else {
                            // Return type name as fallback - as String struct
                            let ptr = self.builder.build_global_string_ptr(type_name, "fallback_type_str")
                                .map(|g| g.as_pointer_value())
                                .unwrap_or_else(|_| self.i8_ptr_t.const_null());
                            build_string_struct(&self.builder, ptr, type_name.len() as u64, str_ty, self.i64_t)
                                .unwrap_or_else(|_| self.i8_ptr_t.const_null().as_basic_value_enum())
                        };
                        
                        if let Some(r) = result {
                            self.assign_value(r, str_val)?;
                        }
                    } else if let Some(r) = result {
                        // No argument - return type name as String struct
                        let ptr = self.builder.build_global_string_ptr(type_name, "type_name_str")
                            .map(|g| g.as_pointer_value())
                            .unwrap_or_else(|_| self.i8_ptr_t.const_null());
                        let str_struct = build_string_struct(&self.builder, ptr, type_name.len() as u64, str_ty, self.i64_t)
                            .unwrap_or_else(|_| self.i8_ptr_t.const_null().as_basic_value_enum());
                        self.assign_value(r, str_struct)?;
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
                // Handle generic method calls with various type prefixes
                // Patterns: T_*, E_*, K_*, V_*, Int_*, ParamNode_*, etc.
                name if name.contains('_') && {
                    // Match if it looks like a generic type prefix followed by a method name
                    let parts: Vec<&str> = name.splitn(2, '_').collect();
                    if parts.len() == 2 {
                        let prefix = parts[0];
                        let method = parts[1];
                        // Single uppercase letter (T, E, K, V), or known method patterns
                        (prefix.len() == 1 && prefix.chars().all(|c| c.is_ascii_uppercase())) ||
                        // Option/Result methods that might be prefixed with a type
                        ["isSome", "isNone", "unwrap", "Unwrap", "IsSome", "IsNone", "expect", "Expect", "unwrapOr", "UnwrapOr"].contains(&method)
                    } else {
                        false
                    }
                } => {
                    // Try to find a concrete implementation by checking common types
                    let parts: Vec<&str> = name.splitn(2, '_').collect();
                    let method_name = parts.get(1).copied().unwrap_or("");
                    let concrete_types = ["File", "String", "Vec", "Map", "Result", "Option"];
                    
                    for concrete_type in concrete_types {
                        let underscore_name = format!("{}_{}", concrete_type, method_name);
                        let dot_name = format!("{}.{}", concrete_type, method_name);
                        
                        if let Some(func) = fn_map.get(&underscore_name).copied()
                            .or_else(|| fn_map.get(&dot_name).copied())
                            .or_else(|| self.module.get_function(&underscore_name))
                            .or_else(|| self.module.get_function(&dot_name)) {
                            let mut call_args: Vec<BasicMetadataValueEnum> = Vec::new();
                            let params = func.get_params();
                            for (i, arg) in args.iter().enumerate() {
                                let val = self.eval_value(arg, fn_map)?;
                                // Check if we need to convert i64 to ptr for 'this' argument
                                if let Some(param) = params.get(i) {
                                    let expected_ty = param.get_type();
                                    if expected_ty.is_pointer_type() && val.is_int_value() {
                                        // Convert i64 (class pointer as int) to ptr
                                        let ptr = self.builder.build_int_to_ptr(
                                            val.into_int_value(),
                                            expected_ty.into_pointer_type(),
                                            "generic_arg_cast"
                                        )?;
                                        call_args.push(ptr.into());
                                    } else {
                                        call_args.push(val.into());
                                    }
                                } else {
                                    call_args.push(val.into());
                                }
                            }
                            let call = self.builder.build_call(func, &call_args, "generic_call")?;
                            if let Some(r) = result {
                                if let Some(val) = call.try_as_basic_value().left() {
                                    self.assign_value(r, val)?;
                                }

                                // Propagate inner type for unwrap calls through generic T_* handler
                                if method_name == "unwrap" || method_name == "Unwrap" {
                                    if let Some(container_arg) = args.get(0) {
                                        let inner_type = match container_arg {
                                            IRValue::Variable(var_name) => {
                                                self.var_option_inner_type.get(var_name).cloned()
                                            }
                                            IRValue::Register(reg_id) => {
                                                self.reg_option_inner_type.get(reg_id).cloned()
                                            }
                                            _ => None,
                                        };
                                        if let Some(inner_type) = inner_type {
                                            if let IRValue::Register(reg_id) = r {
                                                if self.trace_options.trace_boxing {
                                                    eprintln!("[BOXING] T_unwrap propagating inner type '{}' to reg {}", inner_type, reg_id);
                                                }
                                                self.reg_struct_types.insert(*reg_id, inner_type);
                                            }
                                        }
                                    }
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
                // Note: Vec_push must NOT be handled here - Vec has a different layout
                // than Array ({ chunks, capacities, usage, length, totalCapacity, nextChunkSize }
                // vs { length, capacity, data }). Vec_push must call the actual Vec_push function.
                // Check the ORIGINAL name to avoid catching "Vec_push" when it's normalized to "push".
                "List_push" if !name.starts_with("Vec_") => {
                    // push(array, value) - append value to dynamic array
                    // CRITICAL: For push to modify the original array, we need the SLOT POINTER
                    // of the array variable, NOT a loaded copy of the array value.
                    if args.len() == 2 {
                        let value = self.eval_value(&args[1], fn_map)?;
                        eprintln!("DEBUG List_push: args[0]={:?}, value_type={}", args[0],
                            if value.is_struct_value() { "struct" }
                            else if value.is_int_value() { "int" }
                            else if value.is_pointer_value() { "pointer" }
                            else if value.is_float_value() { "float" }
                            else { "unknown" });

                        // Try to get the slot pointer for the array variable directly
                        // This is necessary because push needs to modify the array in-place
                        // Track whether we have a slot (needs load) or direct pointer (no load)
                        let (arr_ptr, is_slot) = match &args[0] {
                            IRValue::Variable(var_name) => {
                                // Use the variable's slot pointer directly
                                if let Some(slot) = self.var_slots.get(var_name).copied() {
                                    eprintln!("DEBUG List_push: using var_slot for {}", var_name);
                                    (slot, true)  // slot contains array pointer - needs load
                                } else {
                                    // Fallback to eval_value if no slot (shouldn't happen for local arrays)
                                    let arr_val = self.eval_value(&args[0], fn_map)?;
                                    if arr_val.is_pointer_value() {
                                        (arr_val.into_pointer_value(), false)  // direct pointer - no load
                                    } else if arr_val.is_struct_value() {
                                        // This path LOSES the modification!
                                        eprintln!("WARNING: List_push on struct value - modification may be lost!");
                                        let sv = arr_val.into_struct_value();
                                        let spill = self.builder.build_alloca(sv.get_type(), "vec_spill")?;
                                        self.builder.build_store(spill, sv)?;
                                        (spill, false)
                                    } else {
                                        (self.builder.build_int_to_ptr(
                                            arr_val.into_int_value(),
                                            self.i8_ptr_t,
                                            "arr_ptr"
                                        )?, false)
                                    }
                                }
                            }
                            IRValue::Register(reg_id) => {
                                // Check if this register came from a FieldAccess on an Array field
                                // If so, use the field pointer directly to enable in-place modification
                                if let Some((struct_ptr, field_idx, struct_ty)) = self.reg_field_access_info.get(reg_id).copied() {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] List_push using field pointer for Register({}) - struct_ptr={:?}, field_idx={}", reg_id, struct_ptr, field_idx);
                                    }
                                    // Get a pointer to the array field in the struct
                                    (self.builder.build_struct_gep(struct_ty, struct_ptr, field_idx, "arr_field_ptr")?, true)  // field slot - needs load
                                } else {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] List_push fallback for Register({}) - no field access info", reg_id);
                                    }
                                    // Not from a field access - evaluate normally
                                    let arr_val = self.eval_value(&args[0], fn_map)?;
                                    if arr_val.is_pointer_value() {
                                        (arr_val.into_pointer_value(), false)  // direct pointer - no load
                                    } else if arr_val.is_struct_value() {
                                        // This path LOSES the modification!
                                        eprintln!("WARNING: List_push on Register struct value - modification may be lost! reg={}", reg_id);
                                        let sv = arr_val.into_struct_value();
                                        let spill = self.builder.build_alloca(sv.get_type(), "vec_spill")?;
                                        self.builder.build_store(spill, sv)?;
                                        (spill, false)
                                    } else {
                                        (self.builder.build_int_to_ptr(
                                            arr_val.into_int_value(),
                                            self.i8_ptr_t,
                                            "arr_ptr"
                                        )?, false)
                                    }
                                }
                            }
                            _ => {
                                // Not a variable - evaluate normally
                                let arr_val = self.eval_value(&args[0], fn_map)?;
                                if arr_val.is_pointer_value() {
                                    (arr_val.into_pointer_value(), false)  // direct pointer - no load
                                } else if arr_val.is_struct_value() {
                                    // This path LOSES the modification!
                                    eprintln!("WARNING: List_push on non-variable struct - modification may be lost!");
                                    let sv = arr_val.into_struct_value();
                                    let spill = self.builder.build_alloca(sv.get_type(), "vec_spill")?;
                                    self.builder.build_store(spill, sv)?;
                                    (spill, false)
                                } else {
                                    (self.builder.build_int_to_ptr(
                                        arr_val.into_int_value(),
                                        self.i8_ptr_t,
                                        "arr_ptr"
                                    )?, false)
                                }
                            }
                        };

                        // Get the actual array pointer:
                        // - If arr_ptr is a slot, load the pointer from it
                        // - If arr_ptr is already the array pointer, use it directly
                        let actual_arr_ptr = if is_slot {
                            eprintln!("[DEBUG List_push] loading actual_arr_ptr from slot");
                            self.builder.build_load(self.i8_ptr_t, arr_ptr, "actual_arr_ptr")?
                                .into_pointer_value()
                        } else {
                            eprintln!("[DEBUG List_push] using arr_ptr directly (not a slot)");
                            arr_ptr
                        };

                        // Load current length from the actual array
                        let len_ptr = self.builder.build_pointer_cast(
                            actual_arr_ptr,
                            self.ctx.ptr_type(inkwell::AddressSpace::from(0u16)),
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

                        // Load stored element_size from index 2
                        let elem_size_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                len_ptr,
                                &[self.i64_t.const_int(2, false)],
                                "elem_size_ptr"
                            )?
                        };
                        let stored_elem_size = self.builder.build_load(self.i64_t, elem_size_ptr, "stored_elem_size")?.into_int_value();

                        // Load data pointer from index 3
                        let data_ptr_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                len_ptr,
                                &[self.i64_t.const_int(3, false)],
                                "data_ptr_ptr"
                            )?
                        };
                        let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                            data_ptr_ptr,
                            self.ctx.ptr_type(inkwell::AddressSpace::from(0u16)),
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

                        // Use the stored element_size from the array struct
                        // This is CRITICAL for generic push operations where compile-time type info is lost
                        let elem_size = stored_elem_size;
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
                            let f64_ptr_ty = self.ctx.ptr_type(inkwell::AddressSpace::from(0u16));
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
                            let i64_ptr_ty = self.ctx.ptr_type(inkwell::AddressSpace::from(0u16));
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
                            let i64_ptr_ty = self.ctx.ptr_type(inkwell::AddressSpace::from(0u16));
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
                            let ptr_ty = self.ctx.ptr_type(inkwell::AddressSpace::from(0u16));
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
                    // Print a string (no newline)
                    if let Some(arg0) = args.get(0) {
                        let val = self.eval_value(arg0, fn_map)?;
                        let s = self.as_cstr_ptr(val)?;
                        // Use printf with %s format
                        let fmt = self.builder.build_global_string_ptr("%s", "fmt_str")?;
                        self.call_printf(&[fmt.as_pointer_value().into(), s.into()])?;
                        self.call_fflush()?;
                        if let Some(r) = result {
                            self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                        }
                        return Ok(());
                    }
                }
                "__Println" => {
                    // Print a string with newline
                    if let Some(arg0) = args.get(0) {
                        let val = self.eval_value(arg0, fn_map)?;
                        let s = self.as_cstr_ptr(val)?;
                        // Use puts which auto-appends newline
                        let puts_fn = self.module.get_function("puts").unwrap_or_else(|| {
                            self.module.add_function(
                                "puts",
                                self.ctx.i32_type().fn_type(&[self.i8_ptr_t.into()], false),
                                None,
                            )
                        });
                        self.builder.build_call(puts_fn, &[s.into()], "puts_call")?;
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
                        let fmt = self.builder.build_global_string_ptr("%ld\n", "fmt_int")?;
                        self.call_printf(&[fmt.as_pointer_value().into(), int_val.into()])?;
                        
                        // Flush stdout to ensure output is visible immediately
                        self.call_fflush()?;

                        if let Some(r) = result {
                            self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                        }
                        return Ok(());
                    }
                }
                "__PtrToInt" => {
                    // Convert a pointer to an integer (for debug printing addresses)
                    if let Some(arg0) = args.get(0) {
                        let val = self.eval_value(arg0, fn_map)?;
                        let int_val = if val.is_pointer_value() {
                            self.builder.build_ptr_to_int(
                                val.into_pointer_value(),
                                self.i64_t,
                                "ptr_to_int"
                            )?
                        } else if val.is_int_value() {
                            // Already an int (class pointers are stored as i64)
                            val.into_int_value()
                        } else {
                            return Err(anyhow!("__PtrToInt: unsupported value type"));
                        };
                        if let Some(r) = result {
                            self.assign_value(r, int_val.as_basic_value_enum())?;
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
                        self.ctx.ptr_type(inkwell::AddressSpace::from(0u16)),
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
                        self.ctx.ptr_type(inkwell::AddressSpace::from(0u16)),
                        self.g_argv.unwrap().as_pointer_value(),
                        "argv",
                    )?;
                    
                    let helper = self.declare_c_fn(
                        "__GetCommandLineArgsHelper",
                        self.i8_ptr_t.into(),
                        &[self.ctx.i32_type().into(), self.ctx.ptr_type(inkwell::AddressSpace::from(0u16)).into()],
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
                "Array_push" | "push" if name != "Vec_push" => {
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] Array_push handler for name='{}'", name);
                    }

                    // Get array pointer - handle Variables specially to get their address
                    // Track whether arr_ptr is a slot (needs load) or direct pointer (no load)
                    let (arr_ptr, is_slot) = if let IRValue::Variable(var_name) = &args[0] {
                        if let Some(slot) = self.var_slots.get(var_name).copied() {
                            (slot, true)  // slot contains the array pointer - needs load
                        } else {
                            let val = self.eval_value(&args[0], fn_map)?;
                            if val.is_pointer_value() {
                                (val.into_pointer_value(), false)  // direct pointer - no load
                            } else {
                                return Err(anyhow!("Array_push: variable '{}' is not a pointer", var_name));
                            }
                        }
                    } else if let IRValue::Register(reg_id) = &args[0] {
                        if let Some((struct_ptr, field_idx, struct_ty)) = self.reg_field_access_info.get(reg_id).copied() {
                            // Re-calculate GEP to get the pointer to the field
                            (self.builder.build_struct_gep(struct_ty, struct_ptr, field_idx, "arr_field_ptr")?, true)  // field slot - needs load
                        } else {
                            let arr_val = self.eval_value(&args[0], fn_map)?;
                            if arr_val.is_pointer_value() {
                                (arr_val.into_pointer_value(), false)  // direct pointer - no load
                            } else if arr_val.is_int_value() {
                                (self.builder.build_int_to_ptr(arr_val.into_int_value(), self.i8_ptr_t, "arr_ptr")?, false)  // direct pointer - no load
                            } else {
                                 return Err(anyhow!("Array_push: invalid array argument (register {})", reg_id));
                            }
                        }
                    } else {
                        let arr_val = self.eval_value(&args[0], fn_map)?;
                        if arr_val.is_pointer_value() {
                            (arr_val.into_pointer_value(), false)  // direct pointer - no load
                        } else if arr_val.is_int_value() {
                            (self.builder.build_int_to_ptr(arr_val.into_int_value(), self.i8_ptr_t, "arr_ptr")?, false)  // direct pointer - no load
                        } else {
                             return Err(anyhow!("Array_push: invalid array argument"));
                        }
                    };

                    let item_val = self.eval_value(&args[1], fn_map)?;

                    // Check if item is from a boxed generic parameter
                    let is_boxed_param = if let IRValue::Variable(name) = &args[1] {
                        self.var_is_boxed_generic.contains(name)
                    } else {
                        false
                    };

                    // Check if we're pushing a non-class struct (data struct)
                    // These should be stored by value, not as pointers
                    let mut element_struct_type = match &args[0] {
                        IRValue::Variable(var_name) => self.var_array_element_struct.get(var_name).cloned(),
                        IRValue::Register(reg_id) => self.reg_array_element_struct.get(reg_id).cloned(),
                        _ => None,
                    };
                    // If element type is unknown but we're pushing a String, record it now.
                    if element_struct_type.is_none() {
                        let is_string_arg = match &args[1] {
                            IRValue::String(_) => true,
                            IRValue::Variable(name) => {
                                self.var_struct_types.get(name).map(|t| t == "String").unwrap_or(false)
                            }
                            IRValue::Register(reg_id) => {
                                self.reg_struct_types.get(reg_id).map(|t| t == "String").unwrap_or(false)
                            }
                            _ => false,
                        };
                        if is_string_arg {
                            if let IRValue::Variable(var_name) = &args[0] {
                                self.var_array_element_struct.insert(var_name.clone(), "String".to_string());
                            }
                            if let IRValue::Register(reg_id) = &args[0] {
                                self.reg_array_element_struct.insert(*reg_id, "String".to_string());
                            }
                            element_struct_type = Some("String".to_string());
                        }
                    }
                    let is_generic_elem = element_struct_type.as_ref().map(|t| {
                        let base = t
                            .rsplit(&['.', ':'][..])
                            .find(|part| !part.is_empty())
                            .unwrap_or(t.as_str());
                        matches!(
                            base,
                            "T" | "K" | "V" | "E" | "T1" | "T2" | "U" | "R" | "A" | "B"
                        )
                    }).unwrap_or(false);
                    let is_non_class_struct = element_struct_type.as_ref()
                        .map(|t| !self.class_types.contains(t) && self.struct_types.contains_key(t))
                        .unwrap_or(false);

                    // Get item pointer and size
                    eprintln!("[DEBUG Array_push] element_struct_type={:?}, is_non_class_struct={}, item_val_type={}",
                        element_struct_type, is_non_class_struct,
                        if item_val.is_struct_value() { "struct" }
                        else if item_val.is_pointer_value() { "pointer" }
                        else if item_val.is_int_value() { "int" }
                        else { "other" });
                    let mut direct_item_ptr: Option<inkwell::values::PointerValue<'ctx>> = None;
                    let (item_ptr, item_size_val) = if item_val.is_struct_value() {
                         if is_generic_elem {
                             // Generic arrays store pointers-as-int. Box the struct and push the pointer.
                             let ty = item_val.get_type().as_basic_type_enum();
                             let size = ty.size_of().unwrap();
                             let malloc = self.get_malloc();
                             let heap_ptr = self.builder.build_call(malloc, &[size.into()], "generic_box_alloc")?
                                 .try_as_basic_value().left()
                                 .ok_or_else(|| anyhow!("malloc returned void"))?
                                 .into_pointer_value();
                             let typed_ptr = self.builder.build_pointer_cast(
                                 heap_ptr,
                                 self.ctx.ptr_type(AddressSpace::default()),
                                 "generic_box_ptr",
                             )?;
                             self.builder.build_store(typed_ptr, item_val)?;
                             let ptr_as_int = self.builder.build_ptr_to_int(heap_ptr, self.i64_t, "generic_box_ptr2i")?;
                             let tmp = self.alloca_for_type(self.i64_t.into(), "item_tmp")?;
                             self.builder.build_store(tmp, ptr_as_int)?;
                             (tmp, self.i64_t.const_int(8, false))
                         } else {
                             let size = item_val.get_type().size_of().unwrap();
                             eprintln!("[DEBUG Array_push] struct_value path, size={:?}", size);
                             let tmp = self.alloca_for_type(item_val.get_type().as_basic_type_enum(), "item_tmp")?;
                             self.builder.build_store(tmp, item_val)?;
                             (tmp, size)
                         }
                    } else if item_val.is_pointer_value() && is_non_class_struct {
                         // For non-class structs (data structs), load the struct from pointer
                         // and store by value
                         let struct_name = element_struct_type.as_ref().unwrap();
                         if let Some((struct_ty, _)) = self.struct_types.get(struct_name).cloned() {
                             eprintln!("[DEBUG] Array_push: non-class struct '{}', loading from pointer", struct_name);
                             let loaded = self.builder.build_load(struct_ty, item_val.into_pointer_value(), "load_struct")?;
                             let size = struct_ty.size_of().unwrap();
                             let tmp = self.alloca_for_type(struct_ty.into(), "item_tmp")?;
                             self.builder.build_store(tmp, loaded)?;
                             (tmp, size)
                         } else {
                             // Fallback: store pointer-as-int
                             let size = self.i64_t.const_int(8, false);
                             let tmp = self.alloca_for_type(self.i64_t.into(), "item_tmp")?;
                             let ptr_as_int = self.builder.build_ptr_to_int(item_val.into_pointer_value(), self.i64_t, "ptr2i")?;
                             self.builder.build_store(tmp, ptr_as_int)?;
                             (tmp, size)
                         }
                    } else if item_val.is_pointer_value() {
                         // Check if element type is String - String is 16 bytes ({ i64 len, ptr data })
                         // and should be stored by value, not as a pointer
                         let is_string_element = element_struct_type.as_ref()
                             .map(|t| t == "String")
                             .unwrap_or(false);

                         // Also detect String from pointer type when element_struct_type is unknown
                         // This handles the case inside generic functions like Vec<String>.push()
                         // where the element type tracking doesn't propagate through field accesses
                         let points_to_string = if !is_string_element && element_struct_type.is_none() {
                             // Check if the pointer points to a String-like struct
                             // by trying to load it as a String and seeing if it matches
                             // Heuristic: if arg[1] comes from a variable that was tracked as String
                             match &args[1] {
                                 IRValue::Variable(name) => {
                                     self.var_array_element_struct.get(name).map(|t| t == "String").unwrap_or(false)
                                         || self.var_struct_types.get(name).map(|t| t == "String").unwrap_or(false)
                                 }
                                 IRValue::Register(reg_id) => {
                                     self.reg_array_element_struct.get(reg_id).map(|t| t == "String").unwrap_or(false)
                                         || self.reg_struct_types.get(reg_id).map(|t| t == "String").unwrap_or(false)
                                 }
                                 _ => false,
                             }
                         } else {
                             false
                         };

                         if is_string_element || points_to_string {
                             // String elements: load the String struct and store by value (16 bytes)
                             let string_ty = self.ty_string();
                             let size = string_ty.size_of().unwrap();
                             eprintln!("[DEBUG Array_push] String element path (is_string_element={}, points_to_string={}), loading struct (size={:?})", is_string_element, points_to_string, size);
                             let loaded = self.builder.build_load(string_ty, item_val.into_pointer_value(), "load_string")?;
                             let tmp = self.alloca_for_type(string_ty.into(), "item_tmp")?;
                             self.builder.build_store(tmp, loaded)?;
                             (tmp, size)
                         } else if is_boxed_param {
                             // For boxed generic parameters, the pointer points to a box containing
                             // an i64 value. We need to load the i64 and store it.
                             let size = self.i64_t.const_int(8, false);
                             let tmp = self.alloca_for_type(self.i64_t.into(), "item_tmp")?;
                             if self.trace_options.trace_boxing {
                                 eprintln!("[BOXING] Array_push: loading i64 from boxed param");
                             }
                             let i64_val = self.builder.build_load(self.i64_t, item_val.into_pointer_value(), "unbox_load")?;
                             self.builder.build_store(tmp, i64_val)?;
                             (tmp, size)
                         } else {
                             if !is_boxed_param {
                                 direct_item_ptr = Some(self.builder.build_pointer_cast(
                                     item_val.into_pointer_value(),
                                     self.i8_ptr_t,
                                     "direct_item_ptr",
                                 )?);
                             }
                             // For direct pointer values (e.g., class types),
                             // convert the pointer to i64 using ptrtoint.
                             let size = self.i64_t.const_int(8, false);
                             let tmp = self.alloca_for_type(self.i64_t.into(), "item_tmp")?;
                             let ptr_as_int = self.builder.build_ptr_to_int(item_val.into_pointer_value(), self.i64_t, "ptr2i")?;
                             self.builder.build_store(tmp, ptr_as_int)?;
                             (tmp, size)
                         }
                    } else {
                         // Int/Float/Bool - or pointer-as-int for struct elements
                         // Check if this array holds struct elements and use actual struct size
                         // CRITICAL: CLASS types are stored as 8-byte pointers, not inline structs!
                         let elem_size = if let Some(elem_struct_name) = &element_struct_type {
                             // Check if this is a CLASS type - if so, use 8 bytes (pointer)
                             if self.class_types.contains(elem_struct_name) {
                                 eprintln!("[DEBUG Array_push] element type '{}' is CLASS, using 8-byte pointer size", elem_struct_name);
                                 self.i64_t.const_int(8, false)
                             } else if let Some((struct_ty, _)) = self.struct_types.get(elem_struct_name).cloned() {
                                 // Non-class struct: use actual struct size
                                 struct_ty.size_of().map(|sz| {
                                     self.builder.build_int_z_extend(sz, self.i64_t, "struct_sz").ok()
                                 }).flatten().unwrap_or_else(|| self.i64_t.const_int(8, false))
                             } else {
                                 self.i64_t.const_int(8, false)
                             }
                         } else {
                             self.i64_t.const_int(8, false)
                         };
                         let tmp = self.alloca_for_type(item_val.get_type().as_basic_type_enum(), "item_tmp")?;
                         self.builder.build_store(tmp, item_val)?;
                         (tmp, elem_size)
                    };
                    
                    // If element type is generic/unknown, box struct values when array stores pointers (size 8)
                    let mut boxed_item_ptr: Option<inkwell::values::PointerValue<'ctx>> = None;
                    if element_struct_type.is_none() && item_val.is_struct_value() {
                        let struct_val = item_val.into_struct_value();
                        let struct_ty = struct_val.get_type();
                        let struct_size = struct_ty.size_of().unwrap();
                        let struct_size_i64 = self.builder.build_int_z_extend(struct_size, self.i64_t, "boxed_sz")?;
                        let malloc = self.get_malloc();
                        let heap_ptr = self.builder
                            .build_call(malloc, &[struct_size_i64.into()], "boxed_alloc")?
                            .try_as_basic_value()
                            .left()
                            .ok_or_else(|| anyhow!("boxed_alloc returned void"))?
                            .into_pointer_value();
                        let heap_ptr_typed = self.builder.build_pointer_cast(
                            heap_ptr,
                            struct_ty.ptr_type(AddressSpace::default()),
                            "boxed_ptr",
                        )?;
                        self.builder.build_store(heap_ptr_typed, struct_val)?;
                        let ptr_as_int = self.builder.build_ptr_to_int(heap_ptr, self.i64_t, "boxed_ptr2i")?;
                        let tmp = self.alloca_for_type(self.i64_t.into(), "boxed_tmp")?;
                        self.builder.build_store(tmp, ptr_as_int)?;
                        boxed_item_ptr = Some(self.builder.build_pointer_cast(tmp, self.i8_ptr_t, "boxed_item_ptr")?);
                    }

                    // Cast item_ptr to i8*
                    let item_ptr_i8 = self.builder.build_pointer_cast(item_ptr, self.i8_ptr_t, "item_ptr_i8")?;

                    // Get the actual array pointer:
                    // - If arr_ptr is a slot (alloca or field), we need to LOAD the array pointer from it
                    // - If arr_ptr is already the array pointer, use it directly
                    let actual_arr_ptr = if is_slot {
                        eprintln!("[DEBUG Array_push] loading actual_arr_ptr from variable slot");
                        self.builder.build_load(self.i8_ptr_t, arr_ptr, "actual_arr_ptr")?
                            .into_pointer_value()
                    } else {
                        eprintln!("[DEBUG Array_push] using arr_ptr directly (not a slot)");
                        arr_ptr
                    };

                    // Read element_size from the array struct at index 2
                    // This is CRITICAL for generic push operations where compile-time type info is lost
                    // Array layout: { i64 len, i64 cap, i64 element_size, ptr data }
                    let elem_size_ptr = unsafe {
                        self.builder.build_gep(
                            self.i64_t,
                            actual_arr_ptr,
                            &[self.i64_t.const_int(2, false)],  // index 2 = element_size
                            "elem_size_ptr"
                        )?
                    };
                    let stored_elem_size = self.builder.build_load(self.i64_t, elem_size_ptr, "stored_elem_size")?
                        .into_int_value();
                    eprintln!("[DEBUG Array_push] reading stored element_size from actual array at index 2");

                    // Use the STORED element size, not the compile-time computed one
                    // This fixes generic push operations where T evaluates to 8 bytes at compile time
                    // but the actual element (e.g., String) is 16 bytes
                    let final_elem_size = stored_elem_size;

                    let mut final_item_ptr = if let Some(boxed_ptr) = boxed_item_ptr {
                        let is_size_8 = self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            stored_elem_size,
                            self.i64_t.const_int(8, false),
                            "is_elem_size_8",
                        )?;
                        let selected = self.builder.build_select(
                            is_size_8,
                            boxed_ptr.as_basic_value_enum(),
                            item_ptr_i8.as_basic_value_enum(),
                            "item_ptr_sel",
                        )?;
                        selected.into_pointer_value()
                    } else {
                        item_ptr_i8
                    };

                    if let Some(direct_ptr) = direct_item_ptr {
                        let is_size_8 = self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            stored_elem_size,
                            self.i64_t.const_int(8, false),
                            "is_elem_size_8_direct",
                        )?;
                        let selected = self.builder.build_select(
                            is_size_8,
                            final_item_ptr.as_basic_value_enum(),
                            direct_ptr.as_basic_value_enum(),
                            "item_ptr_sel_direct",
                        )?;
                        final_item_ptr = selected.into_pointer_value();
                    }

                    // Route to type-specific push function based on element type
                    // This ensures ABI compatibility between Rust bootstrap and self-hosted compiler
                    let is_string_elem = element_struct_type.as_ref().map(|t| t == "String").unwrap_or(false);
                    let is_int_or_char = element_struct_type.as_ref()
                        .map(|t| t == "Int" || t == "Char" || t == "Integer" || t == "i64")
                        .unwrap_or(false);
                    let is_class_elem = element_struct_type.as_ref()
                        .map(|t| self.class_types.contains(t))
                        .unwrap_or(false);
                    let is_data_struct = element_struct_type.as_ref()
                        .map(|t| !self.class_types.contains(t) && self.struct_types.contains_key(t) && t != "String")
                        .unwrap_or(false);

                    eprintln!("[DEBUG Array_push] routing: elem_type={:?}, is_string={}, is_int_char={}, is_class={}, is_data_struct={}",
                        element_struct_type, is_string_elem, is_int_or_char, is_class_elem, is_data_struct);

                    if is_string_elem {
                        // String: use seen_arr_push_str(ptr, %SeenString)
                        let push_str_fn = self.module.get_function("seen_arr_push_str").unwrap_or_else(|| {
                            let string_ty = self.ty_string();
                            let fn_type = self.ctx.void_type().fn_type(&[
                                self.i8_ptr_t.into(),     // arr_ptr
                                string_ty.into(),         // string value
                            ], false);
                            self.module.add_function("seen_arr_push_str", fn_type, None)
                        });
                        // Load the String struct from the item pointer
                        let string_ty = self.ty_string();
                        let string_val = self.builder.build_load(string_ty, final_item_ptr, "push_str_val")?;
                        self.builder.build_call(push_str_fn, &[actual_arr_ptr.into(), string_val.into()], "push_str_res")?;
                        eprintln!("[DEBUG Array_push] used seen_arr_push_str");
                    } else if is_int_or_char {
                        // Int/Char: use seen_arr_push_i64(ptr, i64)
                        let push_i64_fn = self.module.get_function("seen_arr_push_i64").unwrap_or_else(|| {
                            let fn_type = self.ctx.void_type().fn_type(&[
                                self.i8_ptr_t.into(),     // arr_ptr
                                self.i64_t.into(),         // i64 value
                            ], false);
                            self.module.add_function("seen_arr_push_i64", fn_type, None)
                        });
                        // Load the i64 value from the item pointer
                        let i64_val = self.builder.build_load(self.i64_t, final_item_ptr, "push_i64_val")?;
                        self.builder.build_call(push_i64_fn, &[actual_arr_ptr.into(), i64_val.into()], "push_i64_res")?;
                        eprintln!("[DEBUG Array_push] used seen_arr_push_i64");
                    } else if is_data_struct {
                        // Data struct: use Array_push(ptr, ptr) - copies bytes based on element_size stored in array
                        let arr_push_fn = self.module.get_function("Array_push").unwrap_or_else(|| {
                            let fn_type = self.i64_t.fn_type(&[
                                self.i8_ptr_t.into(),     // arr_ptr
                                self.i8_ptr_t.into(),     // element_ptr
                            ], false);
                            self.module.add_function("Array_push", fn_type, None)
                        });
                        self.builder.build_call(arr_push_fn, &[actual_arr_ptr.into(), final_item_ptr.into()], "push_data_res")?;
                        eprintln!("[DEBUG Array_push] used Array_push for data struct");
                    } else if is_class_elem {
                        // Class/pointer: use seen_arr_push_ptr(ptr, ptr)
                        let push_ptr_fn = self.module.get_function("seen_arr_push_ptr").unwrap_or_else(|| {
                            let fn_type = self.ctx.void_type().fn_type(&[
                                self.i8_ptr_t.into(),     // arr_ptr
                                self.i8_ptr_t.into(),     // element_ptr (pointer to push)
                            ], false);
                            self.module.add_function("seen_arr_push_ptr", fn_type, None)
                        });
                        // For class types, the item is already a pointer (stored as ptr-as-int in final_item_ptr)
                        // Load the ptr-as-int and convert back to pointer
                        let ptr_as_int = self.builder.build_load(self.i64_t, final_item_ptr, "push_ptr_int")?;
                        let ptr_val = self.builder.build_int_to_ptr(ptr_as_int.into_int_value(), self.i8_ptr_t, "push_ptr_val")?;
                        self.builder.build_call(push_ptr_fn, &[actual_arr_ptr.into(), ptr_val.into()], "push_ptr_res")?;
                        eprintln!("[DEBUG Array_push] used seen_arr_push_ptr for class");
                    } else {
                        // Generic/unknown: fall back to __ArrayPush with element size
                        // This handles generic type parameters and unknown types
                        let array_push_fn = self.module.get_function("__ArrayPush").unwrap_or_else(|| {
                            let fn_type = self.ctx.i32_type().fn_type(&[
                                self.i8_ptr_t.into(), // arr_ptr
                                self.i8_ptr_t.into(), // element_ptr
                                self.i64_t.into()     // element_size
                            ], false);
                            self.module.add_function("__ArrayPush", fn_type, None)
                        });
                        self.builder.build_call(array_push_fn, &[actual_arr_ptr.into(), final_item_ptr.into(), final_elem_size.into()], "push_generic_res")?;
                        eprintln!("[DEBUG Array_push] used __ArrayPush for generic/unknown type");
                    }

                    return Ok(());
                }
                // Handle push for List/Vec - forward to Vec_push
                // NOTE: Do NOT forward Vec_push to itself!
                "push" | "List_push" if args.len() == 2 && !name.starts_with("Vec_") && !name.starts_with("Array_") => {
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] Forward-to-Vec_push handler for name='{}', normalized='{}'", name, base_normalized);
                    }
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

        // Handle __ExecuteCommand specially
        // C signature: SeenCommandResult* __ExecuteCommand(SeenString cmd)
        // - Takes SeenString by value (struct in registers on x86_64)
        // - Returns pointer to malloc'd SeenCommandResult
        if func_name.as_deref() == Some("__ExecuteCommand") {
            let f = self.module.get_function("__ExecuteCommand").expect("__ExecuteCommand not found");
            let cmd_arg = args.get(0).expect("missing command arg");
            let cmd_val = self.eval_value(cmd_arg, fn_map)?;

            // Get the command string value (by value, not by pointer)
            let cmd_struct = if cmd_val.is_struct_value() {
                cmd_val.into_struct_value()
            } else if cmd_val.is_pointer_value() {
                // If it's a pointer, load the struct
                let ptr = cmd_val.into_pointer_value();
                self.builder.build_load(self.ty_string(), ptr, "cmd_load")?.into_struct_value()
            } else {
                return Err(anyhow!("Invalid command value type: {:?}", cmd_val));
            };

            // Call: ptr = __ExecuteCommand(SeenString)
            let call_val = self.builder.build_call(f, &[cmd_struct.into()], "call_exec")?;
            let result_ptr = call_val.try_as_basic_value().left().expect("expected ptr return").into_pointer_value();

            // Load result struct from the returned pointer
            let result_ty = self.ty_cmd_result().as_basic_type_enum();
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
        let (f_opt, _actual_name) = match target {
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
                        
                        // Check if this is a generic container function that needs ptr-as-int boxing
                        let is_generic_map_fn = matches!(fn_name,
                            "Map_put" | "Map_get" | "Map_containsKey" | "Map_containsValue" | "Map_remove"
                        );
                        let is_generic_vec_fn = matches!(fn_name,
                            "Vec_push" | "Vec_set" | "Array_push" | "ArraySet" | "HashMap_get" | "HashMap_put"
                        );
                        let param_is_generic = self.fn_generic_param_indices
                            .get(fn_name)
                            .map(|indices| indices.contains(&i))
                            .unwrap_or(false);
                        
                        let needs_ptr_as_int = (is_generic_map_fn || is_generic_vec_fn || param_is_generic) && i > 0;
                        
                        if needs_ptr_as_int {
                            // For generic parameters, use ptr-as-int representation:
                            // 1. malloc for struct
                            // 2. store struct
                            // 3. ptrtoint of malloc_ptr
                            // 4. store ptrtoint into spill
                            // 5. pass pointer to spill
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Struct value to generic param: using ptr-as-int boxing for fn '{}' arg {}", fn_name, i);
                            }
                            let malloc_ptr = self.builder.build_malloc(sv.get_type(), "arg_box")?;
                            self.builder.build_store(malloc_ptr, sv)?;
                            let ptr_as_int = self.builder.build_ptr_to_int(malloc_ptr, self.i64_t, "arg_ptr2int")?;
                            let spill = self.alloca_for_type(self.i64_t.into(), "arg_box_spill")?;
                            self.builder.build_store(spill, ptr_as_int)?;
                            call_args.push(spill.into());
                            pushed = true;
                        } else {
                            // Use malloc instead of alloca to ensure value survives if stored (e.g. in Vec)
                            // This effectively boxes the struct on the heap
                            let malloc_ptr = self.builder.build_malloc(sv.get_type(), "arg_box")?;
                            self.builder.build_store(malloc_ptr, sv)?;
                            call_args.push(malloc_ptr.into());
                            pushed = true;
                        }
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
                        // Let's check if it's a collection/generic method that needs boxing
                        // Include Option_, Result_, StringHashMap_ since they are also class types
                        // whose 'this' pointer is stored as i64 but needs ptr conversion
                        let is_collection = fn_name.starts_with("Vec_")
                            || fn_name.starts_with("Map_")
                            || fn_name.starts_with("List_")
                            || fn_name.starts_with("Option_")
                            || fn_name.starts_with("Result_")
                            || fn_name.starts_with("StringHashMap_")
                            || fn_name.starts_with("HashEntry_");

                        // Some() and Option_Store take generic T values that need boxing
                        let is_generic_option_fn = fn_name == "Some" || fn_name == "Option_Store" || fn_name == "Option_Replace";

                        // Check if the target function has a generic parameter at this index
                        // This handles user-defined generic classes like SimpleVec<T>
                        let param_is_generic = self.fn_generic_param_indices
                            .get(fn_name)
                            .map(|indices| indices.contains(&i))
                            .unwrap_or(false);
                        if self.trace_options.trace_boxing && param_is_generic {
                            eprintln!("[BOXING] Target fn '{}' has generic param at index {} (int value case)", fn_name, i);
                        }

                        // CRITICAL: The first argument to collection methods is the 'this' pointer,
                        // which is stored as i64 but represents a heap pointer. This should be cast
                        // to ptr, NOT spilled to stack. Only subsequent arguments (generic T values)
                        // should be spilled.
                        // For Some(), ALL arguments are generic values (no 'this' pointer).
                        // For Option_Store/Replace, arg0 is 'this', arg1 is the value.
                        let is_this_arg = (i == 0 && is_collection) || (i == 0 && (fn_name == "Option_Store" || fn_name == "Option_Replace"));
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] Vec arg i={}, is_collection={}, is_generic_option_fn={}, param_is_generic={}, is_this_arg={}, v.is_int_value={}", i, is_collection, is_generic_option_fn, param_is_generic, is_this_arg, v.is_int_value());
                        }

                        if (is_collection || is_generic_option_fn || param_is_generic) && !is_this_arg {
                            // Spill Int to stack for generic T value arguments (collections and Option constructors)
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Collection arg {} spill to stack", i);
                            }
                            let iv = v.into_int_value();
                            let tmp = self.alloca_for_type(self.i64_t.into(), "int_spill")?;
                            self.builder.build_store(tmp, iv)?;
                            call_args.push(tmp.into());
                            pushed = true;
                        } else {
                            // Check if the int value came from a dereferenced boxed generic parameter
                            // If so, we need to re-box it for the callee
                            // But NOT for class instances (they are represented as i64 pointers)
                            let is_from_boxed_generic = if let IRValue::Variable(name) = a {
                                let in_boxed = self.var_is_boxed_generic.contains(name);
                                // Don't treat class instances as boxed generics
                                let is_class = self.var_struct_types.get(name)
                                    .map(|t| self.class_types.contains(t))
                                    .unwrap_or(false);
                                if in_boxed && !is_class {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] var '{}' is boxed generic, will re-box", name);
                                    }
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            if is_from_boxed_generic {
                                // Re-box: the value was dereferenced by load_from_slot,
                                // but the callee expects a boxed pointer
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] Re-boxing dereferenced boxed generic for arg {}", i);
                                }
                                let iv = v.into_int_value();
                                let tmp = self.alloca_for_type(self.i64_t.into(), "rebox_deref")?;
                                self.builder.build_store(tmp, iv)?;
                                call_args.push(tmp.into());
                                pushed = true;
                            } else {
                                // Standard Int -> Ptr cast for 'this' pointer or non-collection functions
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] Int to ptr cast for arg {}", i);
                                }
                                let ptr = self.builder.build_int_to_ptr(
                                    v.into_int_value(),
                                    expected_ty.into_pointer_type(),
                                    "arg_cast"
                                )?;
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] Created arg_cast ptr {:?}", ptr);
                                }
                                let ptr_meta: BasicMetadataValueEnum = ptr.into();
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] ptr_meta = {:?}", ptr_meta);
                                }
                                call_args.push(ptr_meta);
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] call_args after push = {:?}", call_args.last());
                                }
                                pushed = true;
                            }
                        }
                    } else if v.is_float_value() {
                        // Float -> Pointer (Spill)
                        let fv = v.into_float_value();
                        let tmp = self.alloca_for_type(self.ctx.f64_type().into(), "float_spill")?;
                        self.builder.build_store(tmp, fv)?;
                        call_args.push(tmp.into());
                        pushed = true;
                    } else if v.is_pointer_value() {
                        // Pointer -> Pointer
                        // If we are passing a pointer to a generic function (which expects ptr),
                        // and the argument is a Struct or String, we must ensure it's on the heap.
                        // If it's a stack pointer (e.g. to a local struct), storing it in a collection is unsafe.
                        // We "box" it by allocating heap memory and copying the struct there.
                        
                        let arg_type = a.get_type();
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] Checking if should box. arg_type={:?}", arg_type);
                        }
                        
                        let mut should_box = matches!(arg_type, crate::value::IRType::Struct { .. } | crate::value::IRType::String);
                        let mut target_type = arg_type.clone();
                        
                        // Check if this is a ptr-as-int representation (from Vec.get, Option.unwrap, etc.)
                        // These need to be re-boxed for generic function calls
                        let current_fn_name = self.current_fn.map(|f| f.get_name().to_string_lossy().into_owned()).unwrap_or_else(|| "?".to_string());
                        let is_ptr_as_int = if let IRValue::Variable(name) = a {
                            // If the variable has struct type "T" (generic), it might be ptr-as-int
                            let struct_type = self.var_struct_types.get(name);
                            let elem_type = self.var_array_element_struct.get(name);

                            // Check for generic type markers (handles qualified names like Result::E)
                            let is_generic = struct_type.map(|t| is_generic_type_param(t)).unwrap_or(false);

                            // Check for primitive from Vec.get (has elem_type set to primitive)
                            // Also check if struct_type is "Option" but elem_type is primitive - this happens
                            // when the IR type is Optional<T> but the runtime returns T directly (like Vec.get)
                            let is_vec_get_primitive = elem_type.map(|t| is_primitive_type(t)).unwrap_or(false);

                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] is_ptr_as_int check in '{}' for var '{}', struct_type={:?}, elem_type={:?}, is_generic={}, is_vec_get_primitive={}",
                                    current_fn_name, name, struct_type, elem_type, is_generic, is_vec_get_primitive);
                            }

                            is_generic || is_vec_get_primitive
                        } else if let IRValue::Register(reg_id) = a {
                            // If the register has struct type "T" (generic), it might be ptr-as-int
                            let struct_type = self.reg_struct_types.get(reg_id);
                            let elem_type = self.reg_array_element_struct.get(reg_id);

                            let is_generic = struct_type.map(|t| is_generic_type_param(t)).unwrap_or(false);
                            let is_vec_get_primitive = elem_type.map(|t| is_primitive_type(t)).unwrap_or(false);

                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] is_ptr_as_int check in '{}' for reg {}, struct_type={:?}, elem_type={:?}, is_generic={}, is_vec_get_primitive={}",
                                    current_fn_name, reg_id, struct_type, elem_type, is_generic, is_vec_get_primitive);
                            }

                            is_generic || is_vec_get_primitive
                        } else {
                            false
                        };

                        if is_ptr_as_int {
                            // Check if this is already a boxed generic parameter (ptr to i64)
                            let is_already_boxed = if let IRValue::Variable(name) = a {
                                self.var_is_boxed_generic.contains(name)
                            } else {
                                false
                            };

                            if is_already_boxed {
                                // Boxed generic parameter - LOAD the i64 value from it, then store in new rebox_spill
                                if self.trace_options.trace_boxing {
                                    let var_name = if let IRValue::Variable(name) = a { name.clone() } else { "?".to_string() };
                                    eprintln!("[BOXING] Re-boxing boxed generic parameter '{}' (loading from ptr to i64)", var_name);
                                }
                                let ptr_val = v.into_pointer_value();
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING]   ptr_val = {:?}", ptr_val);
                                }
                                let int_val = self.builder.build_load(self.i64_t, ptr_val, "load_boxed_rebox")?.into_int_value();
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING]   loaded int_val = {:?}", int_val);
                                }
                                let tmp = self.alloca_for_type(self.i64_t.into(), "rebox_spill")?;
                                self.builder.build_store(tmp, int_val)?;
                                call_args.push(tmp.into());
                                pushed = true;
                            } else {
                                // This is a ptr-as-int value (like from Vec.get) being passed to a generic function.
                                // Convert the pointer back to an integer and box it properly.
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] Re-boxing ptr-as-int value for generic function call");
                                }
                                let ptr_val = v.into_pointer_value();
                                let int_val = self.builder.build_ptr_to_int(ptr_val, self.i64_t, "ptr2int_rebox")?;
                                let tmp = self.alloca_for_type(self.i64_t.into(), "rebox_spill")?;
                                self.builder.build_store(tmp, int_val)?;
                                call_args.push(tmp.into());
                                pushed = true;
                            }
                        }
                        
                        if !pushed && !should_box {
                            if let IRValue::Variable(name) = a {
                                if self.var_is_string.contains(name) {
                                    should_box = true;
                                    target_type = crate::value::IRType::String;
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] Variable {} is string, boxing", name);
                                    }
                                } else if let Some(ty_name) = self.var_struct_types.get(name) {
                                    // Don't box class instances - they're already heap pointers
                                    let is_class = self.class_types.contains(ty_name);
                                    if !is_class {
                                        if let Some(fields) = self.struct_definitions.get(ty_name) {
                                            should_box = true;
                                            target_type = crate::value::IRType::Struct {
                                                name: ty_name.clone(),
                                                fields: fields.clone(),
                                            };
                                            if self.trace_options.trace_boxing {
                                                eprintln!("[BOXING] Variable {} is struct {}, boxing", name, ty_name);
                                            }
                                        }
                                    } else {
                                        if self.trace_options.trace_boxing {
                                            eprintln!("[BOXING] Variable {} is class {}, NOT boxing (already heap ptr)", name, ty_name);
                                        }
                                    }
                                }
                            }
                        }
                        
                        if !pushed && should_box {
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Boxing pointer argument for generic call. Type: {:?}", target_type);
                            }

                            let ptr_val = v.into_pointer_value();

                            // Check if this is a container storage function that uses ptr-as-int representation
                            // These functions expect generic parameters as ptr -> i64 (where i64 = ptrtoint of actual value ptr)
                            let is_container_storage_fn = matches!(fn_name,
                                "Vec_push" | "Vec_set" | "Array_push" | "ArraySet" |
                                "Map_put" | "Map_get" | "Map_containsKey" | "Map_containsValue" | "Map_remove"
                            );

                            // Check if the target function has a generic parameter at this index
                            // Generic params (T, K, V, E) expect ptr-as-int boxing for String arguments
                            let param_is_generic = self.fn_generic_param_indices
                                .get(fn_name)
                                .map(|indices| indices.contains(&i))
                                .unwrap_or(false);

                            if param_is_generic && self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Target fn '{}' has generic param at index {}", fn_name, i);
                            }

                            // For String type in container storage functions (like Vec<String>.push, Map<String, V>.put),
                            // OR when target function has a generic parameter at this index,
                            // we use ptr-as-int representation: the container stores i64 values
                            // that are ptrtoint of String pointers.
                            // So we box the pointer (as i64), not the struct contents.
                            if matches!(target_type, crate::value::IRType::String) && (is_container_storage_fn || param_is_generic) {
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] String arg to generic param in fn '{}' idx {}: boxing pointer-as-int", fn_name, i);
                                }
                                // Convert pointer to i64 and box that
                                let ptr_as_int = self.builder.build_ptr_to_int(ptr_val, self.i64_t, "str_ptr2int")?;
                                let tmp = self.alloca_for_type(self.i64_t.into(), "str_box")?;
                                self.builder.build_store(tmp, ptr_as_int)?;
                                call_args.push(tmp.into());
                                pushed = true;
                            } else {
                                let llvm_ty = ir_type_to_llvm(
                                    self.ctx,
                                    &target_type,
                                    self.i64_t,
                                    self.bool_t,
                                    self.i8_ptr_t,
                                    &self.struct_types
                                );

                                // Load the struct from the current pointer
                                let loaded = self.builder.build_load(llvm_ty, ptr_val, "box_load")?;
                                // Allocate new heap memory
                                let malloc_ptr = self.builder.build_malloc(llvm_ty, "box_malloc")?;
                                // Store the struct to the new heap memory
                                self.builder.build_store(malloc_ptr, loaded)?;

                                call_args.push(malloc_ptr.into());
                                pushed = true;
                            }
                        }
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
                    } else if v.is_int_value() {
                        // Int (ptr-as-int) -> Struct (Load)
                        let ptr = self.builder.build_int_to_ptr(
                            v.into_int_value(),
                            self.ctx.ptr_type(AddressSpace::default()),
                            "struct_ptr",
                        )?;
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
                        // Struct -> Int: Need to determine if this is a generic function parameter
                        // that requires ptr-as-int boxing, or a regular int parameter
                        let sv = v.into_struct_value();

                        // Check if this is a generic container function parameter
                        // These functions have i64 parameters but expect boxed structs (ptr-as-int)
                        let is_generic_map_fn = matches!(fn_name,
                            "Map_put" | "Map_get" | "Map_containsKey" | "Map_containsValue" | "Map_remove"
                        );
                        let is_generic_vec_fn = matches!(fn_name,
                            "Vec_push" | "Vec_set" | "Array_push" | "ArraySet" | "HashMap_get" | "HashMap_put"
                        );
                        let param_is_generic = self.fn_generic_param_indices
                            .get(fn_name)
                            .map(|indices| indices.contains(&i))
                            .unwrap_or(false);

                        // For generic parameters (not the 'this' pointer at i==0), use ptr-as-int boxing
                        let needs_generic_boxing = (is_generic_map_fn || is_generic_vec_fn || param_is_generic) && i > 0;

                        if needs_generic_boxing {
                            // Always print this for debugging the double free issue
                            let struct_ty = sv.get_type();
                            eprintln!("[BOXING-DEBUG] Struct to i64: fn='{}' arg={}, struct_fields={}, struct_type={:?}",
                                fn_name, i, struct_ty.count_fields(), struct_ty);
                            // Box the entire struct: malloc, store, ptrtoint
                            let malloc_ptr = self.builder.build_malloc(struct_ty, "gen_box")?;
                            self.builder.build_store(malloc_ptr, sv)?;
                            let as_i64 = self.builder.build_ptr_to_int(malloc_ptr, self.i64_t, "gen_ptr2int")?;
                            if expected_bits < 64 {
                                let truncated = self.builder.build_int_truncate(as_i64, expected_int_ty, "gen_trunc")?;
                                call_args.push(truncated.into());
                            } else {
                                call_args.push(as_i64.into());
                            }
                            pushed = true;
                        } else {
                            // Not a generic parameter: try to extract first field if it's an int
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
                        }
                        if !pushed {
                            // Fallback: spill and cast pointer to int
                            // Use malloc instead of alloca to ensure value survives if stored
                            let struct_ty = sv.get_type();
                            let malloc_ptr = self.builder.build_malloc(struct_ty, "elem_box")?;
                            self.builder.build_store(malloc_ptr, sv)?;
                            let as_i64 = self.builder.build_ptr_to_int(malloc_ptr, self.i64_t, "elem_ptr_i64")?;
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
            
            // Handle struct values (like String) being passed to generic parameters
            // These need ptr-as-int boxing but the value is already a struct, not a pointer
            if !pushed && v.is_struct_value() {
                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] struct value handling for fn '{}' arg {} - v.is_struct_value()=true", fn_name, i);
                }
                
                // Check if the function expects a generic parameter at this index
                let param_is_generic = self.fn_generic_param_indices
                    .get(fn_name)
                    .map(|indices| indices.contains(&i))
                    .unwrap_or(false);
                
                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] param_is_generic={}, fn_generic_param_indices for '{}' = {:?}", 
                        param_is_generic, fn_name, self.fn_generic_param_indices.get(fn_name));
                }
                
                // Also check for generic container storage functions (NOT StringHashMap which has concrete String key)
                // Map<K, V> functions that take generic K or V:
                let is_generic_map_fn = matches!(fn_name,
                    "Map_put" | "Map_get" | "Map_containsKey" | "Map_containsValue" | "Map_remove"
                );
                // Vec<T> and Array<T> functions that take generic T:
                let is_generic_vec_fn = matches!(fn_name,
                    "Vec_push" | "Vec_set" | "Array_push" | "ArraySet" | "HashMap_get" | "HashMap_put"
                );
                
                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] is_generic_map_fn={}, is_generic_vec_fn={}", is_generic_map_fn, is_generic_vec_fn);
                }
                
                let needs_boxing = param_is_generic || is_generic_map_fn || is_generic_vec_fn;
                
                if needs_boxing {
                    let sv = v.into_struct_value();
                    let sv_type = sv.get_type();
                    
                    // Check if this looks like a String struct { i64, ptr }
                    let is_string_like = sv_type.count_fields() == 2;
                    
                    if is_string_like {
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] Struct value arg to generic param in fn '{}' idx {}: boxing with ptr-as-int", fn_name, i);
                        }
                        // Allocate heap memory for the struct, store it, convert ptr to i64, store that in a spill
                        let malloc_ptr = self.builder.build_malloc(sv_type, "arg_box")?;
                        self.builder.build_store(malloc_ptr, sv)?;
                        let ptr_as_int = self.builder.build_ptr_to_int(malloc_ptr, self.i64_t, "arg_ptr2int")?;
                        let tmp = self.alloca_for_type(self.i64_t.into(), "arg_box_spill")?;
                        self.builder.build_store(tmp, ptr_as_int)?;
                        call_args.push(tmp.into());
                        pushed = true;
                    } else {
                        // For non-String structs, just malloc and pass pointer
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] Non-string struct value arg to generic param in fn '{}' idx {}: malloc and pass ptr", fn_name, i);
                        }
                        let malloc_ptr = self.builder.build_malloc(sv_type, "arg_box")?;
                        self.builder.build_store(malloc_ptr, sv)?;
                        call_args.push(malloc_ptr.into());
                        pushed = true;
                    }
                }
            }
            
            if !pushed {
                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] generic call fallback - pushing v={:?} as-is for fn {}", v.get_type(), fn_name);
                }
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
        if self.trace_options.trace_boxing && fn_name == "Vec_push" {
            eprintln!("[BOXING] Vec_push final call_args before build_call:");
            for (i, arg) in call_args.iter().enumerate() {
                eprintln!("  arg[{}] = {:?}", i, arg);
            }
            eprintln!("[BOXING] Calling build_call with f={:?}, call_args len={}", f.get_name().to_str(), call_args.len());
        }

        let call = self.builder.build_call(f, &call_args, "call")?;

        if self.trace_options.trace_boxing && fn_name == "Vec_push" {
            eprintln!("[BOXING] Vec_push: call result = {:?}", call);
        }
        // Debug: check the return type of the call
        let target_name = match target {
            IRValue::Variable(name) | IRValue::Function { name, .. } => name.as_str(),
            _ => "unknown",
        };
        if self.trace_options.trace_boxing && (target_name.contains("unwrap") || target_name.contains("Unwrap")) {
            eprintln!("[BOXING] After build_call for '{}', f.get_type().get_return_type() = {:?}",
                target_name, f.get_type().get_return_type());
        }
        if target_name == "SeenLexer_new" {
    //                     println!("DEBUG: Call to {} returned {:?}", target_name, call.try_as_basic_value().left().map(|v| v.get_type()));
    //                     println!("DEBUG:   Function f return type: {:?}", f.get_type().get_return_type());
    //                     println!("DEBUG:   result register: {:?}", result);
        }
        if let Some(r) = result {
            let call_result = call.try_as_basic_value().left();
            if self.trace_options.trace_boxing && (target_name.contains("unwrap") || target_name.contains("Unwrap")) {
                eprintln!("[BOXING] {} call result = {:?}, ret type = {:?}", target_name, call_result.is_some(), f.get_type().get_return_type());
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
                    
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] Vec_get on {:?}, elem_type={:?}, inner_type={:?}", vec_arg, elem_type, inner_type);
                    }

                    if let Some(elem_type) = elem_type {
                        // If element type is "String", unbox the pointer
                        if elem_type == "String" {
                             let ptr = if ret.is_pointer_value() {
                                 Some(ret.into_pointer_value())
                             } else if ret.is_int_value() {
                                 // Cast i64 to pointer
                                 Some(self.builder.build_int_to_ptr(ret.into_int_value(), self.ctx.ptr_type(inkwell::AddressSpace::from(0u16)), "i64_to_ptr")?)
                             } else {
                                 None
                             };

                             if let Some(ptr) = ptr {
                                 if self.trace_options.trace_boxing {
                                     eprintln!("[BOXING] Vec_get unboxing String from ptr {:?}", ptr);
                                 }
                                 let struct_ptr = self.builder.build_pointer_cast(ptr, self.ctx.ptr_type(inkwell::AddressSpace::from(0u16)), "str_ptr")?;
                                 let struct_val = self.builder.build_load(self.ty_string(), struct_ptr, "str_load")?;
                                 self.assign_value(r, struct_val)?;
                                 return Ok(());
                             }
                        }

                        // If element type is "Option", check for nested inner type
                        if elem_type == "Option" {
                            if let Some(inner_type) = inner_type {
                                if let IRValue::Register(reg_id) = r {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] Vec_get returning Option with inner type '{}' to reg {}", inner_type, reg_id);
                                    }
                                    self.reg_option_inner_type.insert(*reg_id, inner_type);
                                    // Also set reg_struct_types to Option so store propagation works
                                    self.reg_struct_types.insert(*reg_id, "Option".to_string());
                                }
                            }
                        } else {
                            // Element type is not "Option" - this is the actual element type (Int, String, etc.)
                            // Vec.get() in Seen returns T directly (not Option<T>), aborting on out-of-bounds
                            // So we track the element type as the register's struct type (if applicable)
                            if let IRValue::Register(reg_id) = r {
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] Vec_get returning element type '{}' to reg {}", elem_type, reg_id);
                                }
                                // Track the element type as the register's struct type
                                // This is useful for struct types, but primitives (Int/Bool/etc.) don't need struct tracking
                                if !is_primitive_type(&elem_type) {
                                    self.reg_struct_types.insert(*reg_id, elem_type.clone());
                                }
                                // Also track as array element struct for propagation
                                self.reg_array_element_struct.insert(*reg_id, elem_type.clone());
                                // NOTE: Vec_get results should NOT be marked as boxed generic!
                                // Vec_get already unboxes the ptr-as-int internally and returns
                                // a direct pointer to the element. Marking it as boxed would cause
                                // the comparison code to try to unbox again, leading to crashes.
                            }
                        }
                    }
                }

                // Handle Map_get: Map.get(key) returns Option<V>
                // Propagate the tracked value type as the Option's inner type
                if is_map_get {
                    let map_arg = args.get(0);
                    let value_type = match map_arg {
                        Some(IRValue::Variable(map_var)) => {
                            self.var_array_element_struct.get(map_var).cloned()
                        }
                        Some(IRValue::Register(map_reg)) => {
                            self.reg_array_element_struct.get(map_reg).cloned()
                        }
                        _ => None
                    };

                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] Map_get on {:?}, value_type={:?}", map_arg, value_type);
                    }

                    if let Some(vtype) = value_type {
                        if let IRValue::Register(reg_id) = r {
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Map_get returning Option with inner type '{}' to reg {}", vtype, reg_id);
                            }
                            // Map.get returns Option<V>, so track the inner type
                            self.reg_option_inner_type.insert(*reg_id, vtype.clone());
                            self.reg_struct_types.insert(*reg_id, "Option".to_string());
                        }
                    }
                }

                self.assign_value(r, ret.clone())?;
                
                // Debug: trace char return values
                if self.trace_options.trace_boxing && ret.is_int_value() && ret.into_int_value().get_type().get_bit_width() == 8 {
                    eprintln!("[BOXING] Assigning i8 return value to {:?}, ret_type={:?}", r, ret.get_type());
                }

                // Propagate return struct type info
                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] emit_call struct type propagation for target={:?}", target);
                }
                let func_name = match target {
                    IRValue::Variable(name) => Some(name),
                    IRValue::Function { name, .. } => Some(name),
                    _ => None,
                };
                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] emit_call func_name={:?}", func_name);
                }
                
                if let Some(name) = func_name {
                    if name == "__ArrayNew" {
                        let elem_struct = args.get(0).and_then(|arg| {
                            if let IRValue::SizeOf(ty) = arg {
                                match ty {
                                    crate::value::IRType::String => Some("String".to_string()),
                                    crate::value::IRType::Struct { name, .. } => Some(name.clone()),
                                    crate::value::IRType::Enum { name, .. } => Some(name.clone()),
                                    crate::value::IRType::Generic(name) => Some(name.clone()),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        });
                        if let Some(elem_struct) = elem_struct {
                            // Track element type so ArrayAccess loads the correct element layout.
                            if let IRValue::Register(reg_id) = r {
                                self.reg_array_element_struct.insert(*reg_id, elem_struct);
                            } else if let IRValue::Variable(var_name) = r {
                                self.var_array_element_struct.insert(var_name.clone(), elem_struct);
                            }
                        }
                    }

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
                        .or_else(|| alt_name.as_ref().and_then(|alt| self.fn_return_struct_types.get(alt).cloned()))
                        // Special case: Some() and None() return Option type
                        .or_else(|| if name == "Some" || name == "None" { Some("Option".to_string()) } else { None });
                    
                    if self.trace_options.trace_boxing && (name.contains("ImportSymbol") || name.contains("getLocation") || name.contains("TypeError")) {
                        eprintln!("[BOXING] Call to '{}', fn_return_struct_types.get = {:?}, alt_name = {:?}, r = {:?}",
                            name, self.fn_return_struct_types.get(name), alt_name.as_ref().and_then(|alt| self.fn_return_struct_types.get(alt)), r);
                    }

                    if let Some(struct_name) = struct_name {
                        if let IRValue::Register(reg_id) = r {
                            // Don't overwrite a more specific type with a generic 'T'
                            // If we already have a concrete type (like Option) from Vec_get handling, keep it
                            // Also don't overwrite if we have an array element type (from Vec.get returning primitives)
                            let existing = self.reg_struct_types.get(reg_id).cloned();
                            let has_elem_type = self.reg_array_element_struct.contains_key(reg_id);
                            let is_generic = is_generic_type_param(&struct_name);
                            if (existing.is_none() && !has_elem_type) || !is_generic {
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] Setting reg {} struct type to '{}' from call to '{}'", reg_id, struct_name, name);
                                }
                                self.reg_struct_types.insert(*reg_id, struct_name.clone());
                                // NOTE: Do NOT mark generic type results as boxed generic here!
                                // Functions like Vec_get already unbox the ptr-as-int internally and
                                // return a direct pointer. Marking results as boxed would cause
                                // the comparison code to try to unbox again, leading to crashes.
                                // If a function truly returns boxed values, the caller should handle it explicitly.
                            } else {
                                if self.trace_options.trace_boxing {
                                    eprintln!("[BOXING] Keeping existing reg {} struct type '{}' (not overwriting with '{}', has_elem_type={})",
                                        reg_id, existing.as_ref().unwrap_or(&String::new()), struct_name, has_elem_type);
                                }
                            }
                        }
                    }
                    
                    // Try to find array element struct return type using both naming conventions
                    let elem_struct = self.fn_return_array_element_struct.get(name).cloned()
                        .or_else(|| alt_name.as_ref().and_then(|alt| self.fn_return_array_element_struct.get(alt).cloned()));
                    if let Some(elem_struct) = elem_struct {
                        if let IRValue::Register(reg_id) = r {
                            let is_generic_name = |type_name: &str| {
                                let base = type_name
                                    .rsplit(&['.', ':'][..])
                                    .find(|part| !part.is_empty())
                                    .unwrap_or(type_name);
                                matches!(
                                    base,
                                    "T" | "K" | "V" | "E" | "T1" | "T2" | "U" | "R" | "A" | "B"
                                )
                            };
                            let is_generic_new = is_generic_name(&elem_struct);
                            let keep_existing = self
                                .reg_array_element_struct
                                .get(reg_id)
                                .map(|existing| !is_generic_name(existing) && is_generic_new)
                                .unwrap_or(false);
                            if !keep_existing {
                                self.reg_array_element_struct.insert(*reg_id, elem_struct.clone());
                            }
                        }
                    }
                    
                    // Special handling for Vec_toArray: propagate element type from Vec to resulting Array
                    if name == "Vec_toArray" {
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] Vec_toArray call detected, args[0] = {:?}", args.get(0));
                        }
                        if let Some(vec_arg) = args.get(0) {
                            // Try to get element type from the Vec variable
                            let elem_type = match vec_arg {
                                IRValue::Variable(var_name) => {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] Vec_toArray checking var_array_element_struct for '{}'", var_name);
                                    }
                                    self.var_array_element_struct.get(var_name).cloned()
                                }
                                IRValue::Register(reg_id) => {
                                    self.reg_array_element_struct.get(reg_id).cloned()
                                }
                                _ => None,
                            };
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Vec_toArray elem_type = {:?}", elem_type);
                            }
                            if let Some(elem_type) = elem_type {
                                if let IRValue::Register(reg_id) = r {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] Vec_toArray propagating element type '{}' to reg {}", elem_type, reg_id);
                                    }
                                    self.reg_array_element_struct.insert(*reg_id, elem_type);
                                }
                            }
                        }
                    }
                    
                    // Special handling for Option/Result unwrap: propagate inner type
                    // Also handle generic type prefixes like E_unwrap, T_unwrap (single uppercase letter)
                    let is_unwrap_call = name == "Option_unwrap" || name == "Option_Unwrap"
                        || name == "Result_unwrap" || name == "unwrap" || name == "Unwrap"
                        || name.ends_with("_unwrap") || name.ends_with("_Unwrap");
                    if is_unwrap_call {
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] unwrap special handling triggered for '{}', args: {:?}", name, args);
                        }
                        if let Some(container_arg) = args.get(0) {
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] unwrap call to '{}' with arg {:?}", name, container_arg);
                            }
                            // Try to get inner type from the Option tracking first, then fall back to array element struct
                            let inner_type = match container_arg {
                                IRValue::Variable(var_name) => {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] unwrap checking var_option_inner_type for '{}', all keys: {:?}",
                                            var_name, self.var_option_inner_type.keys().collect::<Vec<_>>());
                                    }
                                    self.var_option_inner_type.get(var_name).cloned()
                                        .or_else(|| self.var_array_element_struct.get(var_name).cloned())
                                }
                                IRValue::Register(reg_id) => {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] unwrap checking reg_option_inner_type for reg {}", reg_id);
                                    }
                                    self.reg_option_inner_type.get(reg_id).cloned()
                                        .or_else(|| self.reg_array_element_struct.get(reg_id).cloned())
                                }
                                _ => None,
                            };
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] unwrap inner_type = {:?}", inner_type);
                            }
                            if let Some(inner_type) = inner_type {
                                if let IRValue::Register(reg_id) = r {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] unwrap propagating inner type '{}' to reg {}", inner_type, reg_id);
                                    }
                                    self.reg_struct_types.insert(*reg_id, inner_type.clone());

                                    // Check if this is a generic type parameter (single uppercase letter or common generics)
                                    let inner_base = inner_type
                                        .rsplit(&['.', ':'][..])
                                        .find(|part| !part.is_empty())
                                        .unwrap_or(inner_type.as_str());
                                    let is_generic_param = matches!(
                                        inner_base,
                                        "T" | "V" | "K" | "T1" | "T2" | "U" | "R" | "E" | "A" | "B"
                                    );

                                    // Check if this is an enum type
                                    let is_enum_type = self.enum_types.contains_key(&inner_type);

                                    // Unbox values: unwrap() returns ptr to boxed value for generic T
                                    // When T is a primitive (Int/Bool/Char/Float), String, enum, or a generic parameter
                                    // we need to dereference the pointer
                                    let needs_unboxing = is_primitive_type(&inner_type) || inner_type == "String" || is_generic_param || is_enum_type;
                                    if needs_unboxing {
                                        if self.trace_options.trace_boxing {
                                            eprintln!("[BOXING] Unboxing '{}' from unwrap result reg {} (is_generic={}, is_enum={})",
                                                inner_type, reg_id, is_generic_param, is_enum_type);
                                        }

                                        // Get the ptr value that was assigned from the unwrap call
                                        if let Some(ptr_val) = self.reg_values.get(reg_id).copied() {
                                            if ptr_val.is_pointer_value() {
                                                let ptr = ptr_val.into_pointer_value();

                                                if is_enum_type {
                                                    // For enum types: load the i64 value directly (enum tag)
                                                    // Enums are stored as i64 integers, NOT as pointers
                                                    let loaded_i64 = self.builder.build_load(self.i64_t, ptr, "unbox_enum")?;

                                                    if self.trace_options.trace_boxing {
                                                        eprintln!("[BOXING] Unboxed enum '{}': loaded i64={:?}", inner_type, loaded_i64);
                                                    }

                                                    // Update reg_values with the unboxed enum value
                                                    self.reg_values.insert(*reg_id, loaded_i64);

                                                    // Also update slot if exists
                                                    if let Some(slot) = self.reg_slots.get(reg_id).copied() {
                                                        self.builder.build_store(slot, loaded_i64)?;
                                                    }

                                                    // Track the enum type name in reg_struct_types for method dispatch
                                                    self.reg_struct_types.insert(*reg_id, inner_type.clone());
                                                } else if inner_type == "String" {
                                                    // For String: the boxed value is an i64 (ptr-as-int)
                                                    // Load the i64 and convert to pointer
                                                    let loaded_i64 = self.builder.build_load(self.i64_t, ptr, "unbox_gen_int")?;
                                                    let gen_ptr = self.builder.build_int_to_ptr(
                                                        loaded_i64.into_int_value(),
                                                        self.ctx.ptr_type(inkwell::AddressSpace::default()),
                                                        "unbox_gen_ptr"
                                                    )?;

                                                    if self.trace_options.trace_boxing {
                                                        eprintln!("[BOXING] Unboxed String: i64={:?} -> ptr={:?}", loaded_i64, gen_ptr);
                                                    }

                                                    // Update reg_values with the unboxed pointer
                                                    self.reg_values.insert(*reg_id, gen_ptr.into());

                                                    // Also update slot if exists
                                                    if let Some(slot) = self.reg_slots.get(reg_id).copied() {
                                                        self.builder.build_store(slot, gen_ptr)?;
                                                    }
                                                } else if is_generic_param {
                                                    // For generic params: the boxed value is an i64 (ptr-as-int)
                                                    // Load the i64 and convert to pointer for use by downstream operations
                                                    // (e.g., String operations that expect a pointer they can dereference)
                                                    // NOTE: This may cause issues with enum values, but String concatenation
                                                    // is more common than enum toString, so prioritize Strings.
                                                    let loaded_i64 = self.builder.build_load(self.i64_t, ptr, "unbox_gen_int")?;
                                                    let gen_ptr = self.builder.build_int_to_ptr(
                                                        loaded_i64.into_int_value(),
                                                        self.ctx.ptr_type(inkwell::AddressSpace::default()),
                                                        "unbox_gen_ptr"
                                                    )?;

                                                    if self.trace_options.trace_boxing {
                                                        eprintln!("[BOXING] Unboxed generic param '{}': i64={:?} -> ptr={:?}", inner_type, loaded_i64, gen_ptr);
                                                    }

                                                    // Update reg_values with the unboxed pointer
                                                    self.reg_values.insert(*reg_id, gen_ptr.into());

                                                    // Also update slot if exists
                                                    if let Some(slot) = self.reg_slots.get(reg_id).copied() {
                                                        self.builder.build_store(slot, gen_ptr)?;
                                                    }
                                                } else {
                                                    // For primitives: load the value directly
                                                    let llvm_ty: inkwell::types::BasicTypeEnum = match inner_type.as_str() {
                                                        "Int" | "Integer" | "i64" => self.i64_t.into(),
                                                        "Bool" | "Boolean" | "i1" => self.bool_t.into(),
                                                        "Char" | "i8" => self.ctx.i8_type().into(),
                                                        "Float" | "f64" => self.ctx.f64_type().into(),
                                                        _ => self.i64_t.into(),
                                                    };
                                                    let loaded = self.builder.build_load(llvm_ty, ptr, "unbox_prim")?;

                                                    if self.trace_options.trace_boxing {
                                                        eprintln!("[BOXING] Unboxed primitive from ptr {:?} -> {:?}", ptr, loaded);
                                                    }

                                                    // Update reg_values with the unboxed value
                                                    self.reg_values.insert(*reg_id, loaded);

                                                    // Also update slot if exists
                                                    if let Some(slot) = self.reg_slots.get(reg_id).copied() {
                                                        self.builder.build_store(slot, loaded)?;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Track Option inner types when Some() is called
                    if name == "Some" {
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] Some() call detected, args={:?}, result={:?}", args, result);
                        }
                        if let Some(value_arg) = args.get(0) {
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] Some() value_arg={:?}", value_arg);
                            }
                            // Get the type of the value being wrapped in Some
                            // Try multiple sources: struct types, array element types (from Vec.get), and literals
                            let inner_type = match value_arg {
                                IRValue::Variable(var_name) => {
                                    let struct_ty = self.var_struct_types.get(var_name).cloned();
                                    let elem_ty = self.var_array_element_struct.get(var_name).cloned();
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] Some() var '{}' lookup: struct_ty={:?}, elem_ty={:?}", var_name, struct_ty, elem_ty);
                                    }
                                    // Prefer the element type (from Vec.get) if it's a primitive, otherwise use struct type
                                    if elem_ty.is_some() && is_primitive_type(elem_ty.as_ref().unwrap()) {
                                        elem_ty
                                    } else {
                                        struct_ty.or(elem_ty)
                                    }
                                }
                                IRValue::Register(reg_id) => {
                                    self.reg_struct_types.get(reg_id).cloned()
                                        .or_else(|| self.reg_array_element_struct.get(reg_id).cloned())
                                }
                                // For integer literals
                                IRValue::Integer(_) => Some("Int".to_string()),
                                IRValue::Float(_) => Some("Float".to_string()),
                                IRValue::Boolean(_) => Some("Bool".to_string()),
                                IRValue::Char(_) => Some("Char".to_string()),
                                _ => None,
                            };
                            // If no type found from IR value, try to infer from LLVM value
                            let inner_type = inner_type.or_else(|| {
                                if let Ok(val) = self.eval_value(value_arg, fn_map) {
                                    if val.is_int_value() {
                                        let bit_width = val.into_int_value().get_type().get_bit_width();
                                        Some(match bit_width {
                                            64 => "Int",
                                            32 => "Int",
                                            8 => "Char",
                                            1 => "Bool",
                                            _ => "Int",
                                        }.to_string())
                                    } else if val.is_float_value() {
                                        Some("Float".to_string())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            });
                            if let Some(inner_type) = inner_type.clone() {
                                if let IRValue::Register(reg_id) = r {
                                    if self.trace_options.trace_boxing {
                                        eprintln!("[BOXING] Some() tracking inner type '{}' for result reg {}", inner_type, reg_id);
                                    }
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
