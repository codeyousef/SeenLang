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
use inkwell::values::{BasicValueEnum, FunctionValue, InstructionValue, PointerValue, BasicMetadataValueEnum};
use inkwell::AddressSpace;
use inkwell::types::{BasicTypeEnum, BasicMetadataTypeEnum};
use inkwell::IntPredicate;
use inkwell::basic_block::BasicBlock as LlvmBasicBlock;

use crate::instruction::Instruction;
use crate::value::IRValue;
use crate::llvm_backend::LlvmBackend;
use crate::ir::IRType;

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
        // Handle Vec methods specially to convert float<->i64
        let func_name = match target {
            IRValue::Variable(name) => Some(name.clone()),
            IRValue::Function { name, .. } => Some(name.clone()),
            _ => None,
        };
        
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

        // Handle known intrinsics
        if let IRValue::Variable(name) = target {
            // Normalize method names using helper from instructions module
            let base_normalized = normalize_method_name(name);
            
            // Handle Result/File method aliases using helper
            let result_method = get_result_method_alias(base_normalized);
            
            // If this is a Result/File method, call the actual Result method
            if let Some(method) = result_method {
                let result_fn_name = format!("Result_{}", method);
                let func_opt = fn_map.get(&result_fn_name).copied()
                    .or_else(|| self.module.get_function(&result_fn_name));
                if let Some(func) = func_opt {
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
                                // Function expects pointer
                                if val.is_pointer_value() {
                                    val
                                } else if val.is_struct_value() {
                                    // Spill struct to stack and pass pointer
                                    let tmp = self.alloca_for_type(val.get_type().as_basic_type_enum(), "result_arg")?;
                                    self.builder.build_store(tmp, val)?;
                                    tmp.as_basic_value_enum()
                                } else if val.is_int_value() {
                                    // Treat int as pointer (int-to-ptr)
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
                "__default" => {
                    // Return 0 (i64) as default value
                    if let Some(r) = result {
                        self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
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
                        let elem_size = self.i64_t.const_int(8, false);
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
                "push" | "Vec_push" | "List_push" | "Array_push" => {
                    // push(array, value) - append value to dynamic array
                    if args.len() == 2 {
                        let arr_val = self.eval_value(&args[0], fn_map)?;
                        let value = self.eval_value(&args[1], fn_map)?;
                        
                        let arr_ptr = if arr_val.is_pointer_value() {
                            arr_val.into_pointer_value()
                        } else if arr_val.is_struct_value() {
                            // Vec struct value - spill to stack and use pointer
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

                        let elem_byte_size = if value.is_struct_value() {
                            if value.into_struct_value().get_type() == self.ty_string() {
                                16
                            } else {
                                16
                            }
                        } else {
                            8
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
                            // Pointer value (struct pointer)
                            let ptr_ptr_ty = self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16));
                            let data_ptr_ptr = self.builder.build_pointer_cast(data_ptr, ptr_ptr_ty, "data_ptr_ptr")?;
                            let elem_ptr = unsafe {
                                self.builder.build_gep(
                                    self.i8_ptr_t,
                                    data_ptr_ptr,
                                    &[len],
                                    "elem_ptr"
                                )?
                            };
                            self.builder.build_store(elem_ptr, value.into_pointer_value())?;
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
                    // charAt(string, index) -> char (returned as single-char String)
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
                        let char_val = self.builder.build_load(self.ctx.i8_type(), char_ptr, "char_val")?;
                        if let Some(r) = result {
                            self.assign_value(r, char_val)?;
                        }
                        return Ok(());
                    }
                }
                "toInt" => {
                    // toInt(char) -> int - convert Char (i8) to Int (i64)
                    if args.len() == 1 {
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
                            self.assign_value(r, bufp.as_basic_value_enum())?;
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
                            self.assign_value(r, bufp.as_basic_value_enum())?;
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
                "size" | "length" | "len" if args.len() == 1 => {
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
                "push" | "List_push" if args.len() == 2 => {
                    // Try to call Vec_push if it exists
                    if let Some(vec_push) = fn_map.get("Vec_push").copied()
                        .or_else(|| self.module.get_function("Vec_push")) {
                        let arr_val = self.eval_value(&args[0], fn_map)?;
                        let item_val = self.eval_value(&args[1], fn_map)?;
                        
                        // Convert args to pointers if needed
                        let arr_ptr = if arr_val.is_pointer_value() {
                            arr_val
                        } else if arr_val.is_struct_value() {
                            let tmp = self.alloca_for_type(arr_val.get_type().as_basic_type_enum(), "push_arr")?;
                            self.builder.build_store(tmp, arr_val)?;
                            tmp.as_basic_value_enum()
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
                "__ReadFile" => {
                    // FILE* f = fopen(path, "rb"); if !f return ""
                    let fnty = self.i8_ptr_t; // FILE* opaque as i8*
                    let fopen = self.declare_c_fn(
                        "fopen",
                        fnty.into(),
                        &[self.i8_ptr_t.into(), self.i8_ptr_t.into()],
                        false,
                    );
                    let fseek = self.declare_c_fn(
                        "fseek",
                        self.ctx.i32_type().into(),
                        &[fnty.into(), self.i64_t.into(), self.ctx.i32_type().into()],
                        false,
                    );
                    let ftell = self.declare_c_fn(
                        "ftell",
                        self.i64_t.into(),
                        &[fnty.into()],
                        false,
                    );
                    let rewindf = self.declare_c_void_fn("rewind", &[fnty.into()], false);
                    let fread = self.declare_c_fn(
                        "fread",
                        self.i64_t.into(),
                        &[
                            self.i8_ptr_t.into(),
                            self.i64_t.into(),
                            self.i64_t.into(),
                            fnty.into(),
                        ],
                        false,
                    );
                    let fclose = self.declare_c_fn(
                        "fclose",
                        self.ctx.i32_type().into(),
                        &[fnty.into()],
                        false,
                    );
                    let path_v = self.eval_value(&args[0], fn_map)?;
                    let path = self.as_cstr_ptr(path_v)?;
                    let mode = self.builder.build_global_string_ptr("rb", "rb")?;
                    let f = self.builder.build_call(
                        fopen,
                        &[path.into(), mode.as_pointer_value().into()],
                        "fopen",
                    )?;
                    let fval = f.try_as_basic_value().left().unwrap();
                    let is_null = self
                        .builder
                        .build_is_null(fval.into_pointer_value(), "isnull")?;
                    let fnv = self.current_fn.unwrap();
                    let then_bb = self.ctx.append_basic_block(fnv, "rf_null");
                    let cont_bb = self.ctx.append_basic_block(fnv, "rf_cont");
                    let done_bb = self.ctx.append_basic_block(fnv, "rf_done");
                    self.builder
                        .build_conditional_branch(is_null, then_bb, cont_bb)?;
                    let mut rf_then_val: Option<(
                        BasicValueEnum<'ctx>,
                        LlvmBasicBlock<'ctx>,
                    )> = None;
                    let mut rf_cont_val: Option<(
                        BasicValueEnum<'ctx>,
                        LlvmBasicBlock<'ctx>,
                    )> = None;
                    self.builder.position_at_end(then_bb);
                    let empty = self.builder.build_global_string_ptr("", "empty")?;
                    let empty_ptr = empty.as_pointer_value();
                    let empty_len = self.i64_t.const_zero();
                    let mut empty_str = self.ty_string().get_undef();
                    empty_str = self.builder.build_insert_value(empty_str, empty_len, 0, "empty_len")?.into_struct_value();
                    empty_str = self.builder.build_insert_value(empty_str, empty_ptr, 1, "empty_ptr")?.into_struct_value();
                    rf_then_val = Some((empty_str.as_basic_value_enum(), then_bb));
                    self.builder.build_unconditional_branch(done_bb)?;
                    self.builder.position_at_end(cont_bb);
                    // size
                    let seek_end = self.ctx.i32_type().const_int(2, false);
                    self.builder.build_call(
                        fseek,
                        &[fval.into(), self.i64_t.const_zero().into(), seek_end.into()],
                        "fseek_end",
                    )?;
                    let sz = self.builder.build_call(ftell, &[fval.into()], "ftell")?;
                    let szv = sz.try_as_basic_value().left().unwrap().into_int_value();
                    let _ = self.builder.build_call(rewindf, &[fval.into()], "rewind")?;
                    let malloc = self.get_malloc();
                    let one = self.i64_t.const_int(1, false);
                    let total = self.builder.build_int_add(szv, one, "tot")?;
                    let buf =
                        self.builder
                            .build_call(malloc, &[total.into()], "malloc_rf")?;
                    let bufp = buf
                        .try_as_basic_value()
                        .left()
                        .unwrap()
                        .into_pointer_value();
                    let _rd = self.builder.build_call(
                        fread,
                        &[bufp.into(), one.into(), szv.into(), fval.into()],
                        "fread",
                    )?;
                    let endp = unsafe {
                        self.builder
                            .build_gep(self.ctx.i8_type(), bufp, &[szv], "end")?
                    };
                    self.builder
                        .build_store(endp, self.ctx.i8_type().const_zero())?;
                    let _ = self.builder.build_call(fclose, &[fval.into()], "fclose")?;
                    
                    let mut str_val = self.ty_string().get_undef();
                    str_val = self.builder.build_insert_value(str_val, szv, 0, "str_len")?.into_struct_value();
                    str_val = self.builder.build_insert_value(str_val, bufp, 1, "str_ptr")?.into_struct_value();
                    
                    rf_cont_val = Some((str_val.as_basic_value_enum(), cont_bb));
                    self.builder.build_unconditional_branch(done_bb)?;
                    self.builder.position_at_end(done_bb);
                    if let Some(r) = result {
                        let phi = self.builder.build_phi(self.ty_string(), "rf_value")?;
                        if let Some((val, bb)) = rf_then_val {
                            phi.add_incoming(&[(&val, bb)]);
                        }
                        if let Some((val, bb)) = rf_cont_val {
                            phi.add_incoming(&[(&val, bb)]);
                        }
                        self.assign_value(r, phi.as_basic_value())?;
                    }
                    return Ok(());
                }
                "__WriteFile" => {
                    let fnty = self.i8_ptr_t;
                    let fopen = self.declare_c_fn(
                        "fopen",
                        fnty.into(),
                        &[self.i8_ptr_t.into(), self.i8_ptr_t.into()],
                        false,
                    );
                    let fwrite = self.declare_c_fn(
                        "fwrite",
                        self.i64_t.into(),
                        &[
                            self.i8_ptr_t.into(),
                            self.i64_t.into(),
                            self.i64_t.into(),
                            fnty.into(),
                        ],
                        false,
                    );
                    let fclose = self.declare_c_fn(
                        "fclose",
                        self.ctx.i32_type().into(),
                        &[fnty.into()],
                        false,
                    );
                    let strlen = self.get_strlen();
                    let path_v = self.eval_value(&args[0], fn_map)?;
                    let path = self.as_cstr_ptr(path_v)?;
                    let content_v = self.eval_value(&args[1], fn_map)?;
                    let content = self.as_cstr_ptr(content_v)?;
                    let mode = self.builder.build_global_string_ptr("wb", "wb")?;
                    let f = self.builder.build_call(
                        fopen,
                        &[path.into(), mode.as_pointer_value().into()],
                        "fopen_w",
                    )?;
                    let fval = f.try_as_basic_value().left().unwrap();
                    let is_null = self
                        .builder
                        .build_is_null(fval.into_pointer_value(), "isnullw")?;
                    let len =
                        self.builder
                            .build_call(strlen, &[content.into()], "strlen")?;
                    let lenv = len.try_as_basic_value().left().unwrap().into_int_value();
                    let one = self.i64_t.const_int(1, false);
                    let _wr = self.builder.build_call(
                        fwrite,
                        &[content.into(), one.into(), lenv.into(), fval.into()],
                        "fwrite",
                    )?;
                    let _ = self
                        .builder
                        .build_call(fclose, &[fval.into()], "fclose_w")?;
                    if let Some(r) = result {
                        self.assign_value(
                            r,
                            self.bool_t.const_int(1, false).as_basic_value_enum(),
                        )?;
                    }
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

        // Normal call by name
        let f_opt = match target {
            IRValue::Variable(name) => fn_map.get(name).cloned(),
            IRValue::Function { name, .. } => fn_map.get(name).cloned(),
            _ => None,
        };
        let f = match f_opt {
            Some(func) => func,
            None => {
                // Try to auto-declare external runtime functions starting with __
                if let IRValue::Variable(name) = target {
                    if name.starts_with("__") {
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
                        if is_collection {
                            // Spill Int to stack
                            let iv = v.into_int_value();
                            let tmp = self.alloca_for_type(self.i64_t.into(), "int_spill")?;
                            self.builder.build_store(tmp, iv)?;
                            call_args.push(tmp.into());
                            pushed = true;
                        } else {
                            // Standard Int -> Ptr cast (e.g. null check or address calculation)
                            let ptr = self.builder.build_int_to_ptr(
                                v.into_int_value(), 
                                expected_ty.into_pointer_type(), 
                                "arg_cast"
                            )?;
                            call_args.push(ptr.into());
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
                } else {
                    // Default to i64 zero
                    call_args.push(self.i64_t.const_zero().into());
                }
            }
            extra_args_needed -= 1;
        }
        
        let call = self.builder.build_call(f, &call_args, "call")?;
        // Debug: check the return type of the call
        let target_name = match target {
            IRValue::Variable(name) | IRValue::Function { name, .. } => name.as_str(),
            _ => "unknown",
        };
        if target_name == "SeenLexer_new" {
    //                     println!("DEBUG: Call to {} returned {:?}", target_name, call.try_as_basic_value().left().map(|v| v.get_type()));
    //                     println!("DEBUG:   Function f return type: {:?}", f.get_type().get_return_type());
    //                     println!("DEBUG:   result register: {:?}", result);
        }
        if let Some(r) = result {
            if let Some(ret) = call.try_as_basic_value().left() {
                // For Vec_get from float Vec, DON'T bitcast here - keep as i64
                // but mark the register as containing float bits
                if is_vec_get && is_float_vec_call {
                    if let IRValue::Register(reg_id) = r {
                        self.reg_is_float.insert(*reg_id);
                    }
                }
                self.assign_value(r, ret)?;

                // Propagate return struct type info
                let func_name = match target {
                    IRValue::Variable(name) => Some(name),
                    IRValue::Function { name, .. } => Some(name),
                    _ => None,
                };
                
                if let Some(name) = func_name {
                    if let Some(struct_name) = self.fn_return_struct_types.get(name) {
                        if let IRValue::Register(reg_id) = r {
                            self.reg_struct_types.insert(*reg_id, struct_name.clone());
                        }
                    }
                    
                    // Propagate return array element struct type info
                    if let Some(elem_struct) = self.fn_return_array_element_struct.get(name) {
                        if let IRValue::Register(reg_id) = r {
                            self.reg_array_element_struct.insert(*reg_id, elem_struct.clone());
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
            "String" | "Array" | "Vec" | "List" | "Map" | "Result" | "Option" | "File" => {
                &name[idx + 1..]
            }
            _ => name,
        }
    } else {
        name
    }
}

/// Helper to map Result/File method aliases to their actual implementation names
pub fn get_result_method_alias(name: &str) -> Option<&'static str> {
    match name {
        "isOkay" => Some("isOk"),
        "isError" => Some("isErr"),
        "getValue" => Some("unwrap"),
        "getError" => Some("unwrapErr"),
        "getValueOrDefault" => Some("unwrapOr"),
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
