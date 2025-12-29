use anyhow::{anyhow, Result};
use inkwell::values::BasicValue;
use inkwell::values::{BasicValueEnum, FunctionValue};
use inkwell::AddressSpace;
use inkwell::types::BasicType;
use indexmap::IndexMap;

use crate::value::{IRType, IRValue};
use crate::llvm_backend::LlvmBackend;
use crate::llvm::type_cast::TypeCastOps;
use crate::llvm::type_builders::TypeBuilders;

type HashMap<K, V> = IndexMap<K, V>;

pub trait AggregateOps<'ctx> {
    fn emit_array_length(
        &mut self,
        array: &IRValue,
        result: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;

    fn emit_array_access(
        &mut self,
        array: &IRValue,
        index: &IRValue,
        result: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;

    fn emit_array_set(
        &mut self,
        array: &IRValue,
        index: &IRValue,
        value: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;

    fn emit_field_access(
        &mut self,
        struct_val: &IRValue,
        field: &str,
        result: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;

    fn emit_field_set(
        &mut self,
        struct_val: &IRValue,
        field: &str,
        value: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;
}

impl<'ctx> AggregateOps<'ctx> for LlvmBackend<'ctx> {
    fn emit_array_length(
        &mut self,
        array: &IRValue,
        result: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        // Dynamic arrays with layout { i64 len, i64 capacity, data... }
        // Length is always at offset 0
        let arr_v = self.eval_value(array, fn_map)?;
        let res = if let IRValue::Array(values) = array {
            self.i64_t
                .const_int(values.len() as u64, false)
                .as_basic_value_enum()
        } else if arr_v.is_struct_value() {
            let sv = arr_v.into_struct_value();
            let len = self.builder.build_extract_value(sv, 0, "alen").unwrap();
            len.as_basic_value_enum()
        } else if arr_v.is_pointer_value() || arr_v.is_int_value() {
            let arr_ptr = if arr_v.is_pointer_value() {
                arr_v.into_pointer_value()
            } else {
                self.builder
                    .build_int_to_ptr(
                        arr_v.into_int_value(),
                        self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                        "arr_len_ptr",
                    )
                    .map_err(|e| anyhow!("{e:?}"))?
            };
            // Cast to i64* and load the first i64 (length)
            let len_ptr = self.builder.build_pointer_cast(
                arr_ptr,
                self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                "len_ptr"
            )?;
            let len = self.builder.build_load(self.i64_t, len_ptr, "len")?;
            len.as_basic_value_enum()
        } else {
            self.i64_t.const_int(0, false).as_basic_value_enum()
        };
        self.assign_value(result, res)?;
        Ok(())
    }

    fn emit_array_access(
        &mut self,
        array: &IRValue,
        index: &IRValue,
        result: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        let arr_v = self.eval_value(array, fn_map)?;
        
        // Debug: print array access type info
        if let IRValue::Variable(var_name) = array {
            if var_name.contains("rawArgs") {
                eprintln!("DEBUG: ArrayAccess on '{}', element_struct_type: {:?}", var_name, self.var_array_element_struct.get(var_name));
            }
        }
        // Check if this is a string variable (for string indexing)
        let is_string = if let IRValue::Variable(var_name) = array {
            self.var_is_string.contains(var_name)
        } else {
            false
        };
        
        // Check if this is an integer array
        let is_int_array = match array {
            IRValue::Variable(var_name) => self.var_is_int_array.contains(var_name),
            IRValue::Register(reg_id) => self.reg_is_int_array.contains(reg_id),
            _ => false,
        };
        
        // Check if this is an array of structs
        let element_struct_type = match array {
            IRValue::Variable(var_name) => self.var_array_element_struct.get(var_name).cloned(),
            IRValue::Register(reg_id) => self.reg_array_element_struct.get(reg_id).cloned(),
            _ => None
        };
        
        if let IRValue::Array(vs) = array {
            let idx_bv = self.eval_value(index, fn_map)?;
            let idx_val = self.as_usize(idx_bv)? as usize;
            if idx_val >= vs.len() {
                return Err(anyhow!("Array index OOB"));
            }
            let elem = self.eval_value(&vs[idx_val], fn_map)?;
            self.assign_value(result, elem)?;
        } else if is_string {
            // String indexing
            let str_ptr = self.to_string_ptr(arr_v)?;

            let idx_bv = self.eval_value(index, fn_map)?;
            let idx_iv = self.as_i64(idx_bv)?;

            // GEP to the character at index
            let char_ptr = unsafe {
                self.builder.build_gep(
                    self.ctx.i8_type(),
                    str_ptr,
                    &[idx_iv],
                    "char_ptr",
                )?
            };

            // Load the character as i8
            let char_val = self.builder.build_load(self.ctx.i8_type(), char_ptr, "char_val")?.into_int_value();
            // let char_i64 = self.builder.build_int_z_extend(char_val, self.i64_t, "char_i64")?;

            self.assign_value(result, char_val.as_basic_value_enum())?;
        } else if arr_v.is_pointer_value() || arr_v.is_int_value() {
            // Dynamic array with layout { i64 len, i64 cap, i8* data_ptr }
            // Data pointer is at offset 16
            let arr_ptr = if arr_v.is_pointer_value() {
                arr_v.into_pointer_value()
            } else {
                self.builder
                    .build_int_to_ptr(
                        arr_v.into_int_value(),
                        self.i8_ptr_t,
                        "arr_int_to_ptr",
                    )
                    .map_err(|e| anyhow!("{e:?}"))?
            };
            
            // Load data pointer from offset 16
            let data_ptr_ptr = unsafe {
                self.builder.build_gep(
                    self.i64_t,
                    self.builder.build_pointer_cast(arr_ptr, self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)), "cast")?,
                    &[self.i64_t.const_int(2, false)],
                    "data_ptr_ptr"
                )?
            };
            let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                data_ptr_ptr,
                self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                "data_ptr_ptr_casted"
            )?;
            let data_ptr = self.builder.build_load(self.i8_ptr_t, data_ptr_ptr_casted, "data_ptr")?.into_pointer_value();
            
            let idx_bv = self.eval_value(index, fn_map)?;
            let idx_iv = self.as_i64(idx_bv)?;

            // BOUNDS CHECK (using DRY helper)
            let len = self.get_array_len(arr_ptr)?;
            self.build_bounds_check(idx_iv, len)?;
            
            // Check if we're accessing a struct array
            if let Some(ref struct_type_name) = element_struct_type {
                eprintln!("DEBUG ArrayAccess struct array: element_struct_type={}", struct_type_name);
                // Check if this is a class type (heap-allocated, stored as pointer)
                let is_class = self.class_types.contains(struct_type_name);
                
                // Handle generic type parameters (T, E, K, V, etc.)
                // These are unresolved at codegen time, so we need to use a fallback strategy
                let is_generic_param = struct_type_name.len() == 1 && struct_type_name.chars().next().map_or(false, |c| c.is_uppercase());
                
                // Resolve the LLVM struct type, handling the built-in String explicitly
                let llvm_struct_ty = if struct_type_name == "String" {
                    Some(self.ty_string())
                } else if is_generic_param {
                    // For generic type parameters, try String first as it's the most common case
                    // that breaks when loaded as i64 (since String is 16 bytes)
                    // Note: This is a heuristic - ideally we'd track the actual instantiated type
                    Some(self.ty_string())
                } else {
                    self.struct_types.get(struct_type_name).map(|(ty, _)| *ty)
                };

                if let Some(llvm_struct_ty) = llvm_struct_ty {
                    // For classes: elements are stored as i64 (pointer-as-int)
                    // For structs: elements are stored inline
                    // For generic params (T), assume struct storage (safer default)
                    if is_class && !is_generic_param {
                        // Class arrays store pointers-as-i64
                        let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                        let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
                        
                        let elem_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                data_i64_ptr,
                                &[idx_iv],
                                "class_elem_ptr",
                            )?
                        };
                        
                        // Load the pointer-as-i64
                        let ptr_as_int = self.builder.build_load(self.i64_t, elem_ptr, "class_ptr_int")?.into_int_value();
                        
                        // Convert to struct pointer
                        let struct_ptr_ty = llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16));
                        let struct_ptr = self.builder.build_int_to_ptr(ptr_as_int, struct_ptr_ty, "class_struct_ptr")?;
                        
                        // Load the actual struct value from the pointer
                        let struct_val = self.builder.build_load(llvm_struct_ty, struct_ptr, "class_struct_load")?;
                        
                        // Assign the struct value to result
                        self.assign_value(result, struct_val)?;
                        
                        // Track struct type for subsequent field access
                        if let IRValue::Register(reg_id) = result {
                            self.reg_struct_types.insert(*reg_id, struct_type_name.clone());
                        }
                        return Ok(());
                    }
                    
                    // Struct arrays store structs directly (inline), not pointers
                    // Cast data_ptr to pointer-to-struct
                    let struct_ptr_ty = llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16));
                    let data_struct_ptr = self.builder.build_pointer_cast(data_ptr, struct_ptr_ty, "data_struct_ptr")?;
                    
                    // GEP to the element (struct at index i)
                    let elem_ptr = unsafe {
                        self.builder.build_gep(
                            llvm_struct_ty,
                            data_struct_ptr,
                            &[idx_iv],
                            "struct_elem_ptr",
                        )?
                    };
                    
                    // Load the actual struct value directly
                    let struct_val = self.builder.build_load(llvm_struct_ty, elem_ptr, "struct_load")?;
                    
                    // Assign the struct value to result
                    self.assign_value(result, struct_val)?;
                    
                    // Track struct type for subsequent field access
                    if let IRValue::Register(reg_id) = result {
                        self.reg_struct_types.insert(*reg_id, struct_type_name.clone());
                    }
                    return Ok(());
                }
            }
            
            // Check if we're accessing an integer array
            if is_int_array {
                let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
                
                let elem_ptr = unsafe {
                    self.builder.build_gep(
                        self.i64_t,
                        data_i64_ptr,
                        &[idx_iv],
                        "int_elem_ptr",
                    )?
                };
                let elem = self.builder.build_load(self.i64_t, elem_ptr, "int_elem")?;
                self.assign_value(result, elem.as_basic_value_enum())?;
                return Ok(());
            }
            
            // Check if we're accessing a string array (parts: [string])
            if let Some(struct_type_name) = element_struct_type {
                if struct_type_name == "String" {
                    let llvm_struct_ty = self.ty_string();
                    let struct_ptr_ty = llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16));
                    let data_struct_ptr = self.builder.build_pointer_cast(data_ptr, struct_ptr_ty, "data_struct_ptr")?;
                    
                    let elem_ptr = unsafe {
                        self.builder.build_gep(
                            llvm_struct_ty,
                            data_struct_ptr,
                            &[idx_iv],
                            "struct_elem_ptr",
                        )?
                    };
                    
                    let struct_val = self.builder.build_load(llvm_struct_ty, elem_ptr, "struct_load")?;
                    self.assign_value(result, struct_val)?;
                    
                    if let IRValue::Register(reg_id) = result {
                        self.reg_struct_types.insert(*reg_id, "String".to_string());
                    }
                    return Ok(());
                }
            }
            
            // Default: treat as i64 array (works for int, pointer-as-int, and generic values)
            eprintln!("DEBUG ArrayAccess DEFAULT: array={:?}", array);
            let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
            let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
            
            let elem_ptr = unsafe {
                self.builder.build_gep(
                    self.i64_t,
                    data_i64_ptr,
                    &[idx_iv],
                    "elem_ptr",
                )?
            };
            let elem = self.builder.build_load(self.i64_t, elem_ptr, "elem")?;
            self.assign_value(result, elem.as_basic_value_enum())?;
        } else if arr_v.is_struct_value() {
            // Handle struct value - this is an array struct loaded by value { len, cap, data }
            // Extract the data pointer from the struct (field 2)
            // Note: in opaque pointer mode, field 2 is a ptr type, not i64
            let arr_struct = arr_v.into_struct_value();
            let data_ptr = self.builder.build_extract_value(arr_struct, 2, "data_ptr")?
                .into_pointer_value();
            
            let idx_bv = self.eval_value(index, fn_map)?;
            let idx_iv = self.as_i64(idx_bv)?;
            
            // Check if this is a struct array  
            if let Some(ref struct_type_name) = element_struct_type {
                let llvm_struct_ty = if struct_type_name == "String" {
                    Some(self.ty_string())
                } else {
                    self.struct_types.get(struct_type_name).map(|(ty, _)| *ty)
                };
                
                if let Some(llvm_struct_ty) = llvm_struct_ty {
                    let struct_ptr_ty = llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16));
                    let data_struct_ptr = self.builder.build_pointer_cast(data_ptr, struct_ptr_ty, "data_struct_ptr")?;
                    
                    let elem_ptr = unsafe {
                        self.builder.build_gep(
                            llvm_struct_ty,
                            data_struct_ptr,
                            &[idx_iv],
                            "struct_elem_ptr",
                        )?
                    };
                    
                    let struct_val = self.builder.build_load(llvm_struct_ty, elem_ptr, "struct_load")?;
                    self.assign_value(result, struct_val)?;
                    
                    if let IRValue::Register(reg_id) = result {
                        self.reg_struct_types.insert(*reg_id, struct_type_name.clone());
                    }
                    return Ok(());
                }
            }
            
            // Default: treat as i64 array
            let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
            let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
            
            let elem_ptr = unsafe {
                self.builder.build_gep(
                    self.i64_t,
                    data_i64_ptr,
                    &[idx_iv],
                    "elem_ptr",
                )?
            };
            let elem = self.builder.build_load(self.i64_t, elem_ptr, "elem")?;
            self.assign_value(result, elem.as_basic_value_enum())?;
        } else {
            eprintln!("ERROR ArrayAccess: array={:?}, arr_v type={:?}", array, arr_v.get_type());
            return Err(anyhow!("Unsupported array access value"));
        }
        Ok(())
    }

    fn emit_array_set(
        &mut self,
        array: &IRValue,
        index: &IRValue,
        value: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        // Dynamic array with layout { i64 len, i64 cap, data[...] }
        // Data starts at offset 16 (2 * sizeof(i64))
        let arr_v = self.eval_value(array, fn_map)?;
        let val_v = self.eval_value(value, fn_map)?;
        
        // Check if this is an integer array
        let is_int_array = match array {
            IRValue::Variable(var_name) => self.var_is_int_array.contains(var_name),
            IRValue::Register(reg_id) => self.reg_is_int_array.contains(reg_id),
            _ => false,
        };
        
        // Check if this is a struct array
        let element_struct_type = if let IRValue::Variable(var_name) = array {
            self.var_array_element_struct.get(var_name).cloned()
        } else {
            None
        };
        
        // Get data pointer and array length - handle struct value (array loaded by value) or pointer/int
        let (data_ptr, len) = if arr_v.is_struct_value() {
            // Array struct loaded by value { len, cap, data_ptr }
            let arr_struct = arr_v.into_struct_value();
            let len_val = self.builder.build_extract_value(arr_struct, 0, "len")?
                .into_int_value();
            let data_ptr = self.builder.build_extract_value(arr_struct, 2, "data_ptr")?
                .into_pointer_value();
            (data_ptr, len_val)
        } else {
            let arr_ptr = if arr_v.is_pointer_value() {
                arr_v.into_pointer_value()
            } else {
                self.builder
                    .build_int_to_ptr(
                        arr_v.into_int_value(),
                        self.i8_ptr_t,
                        "arr_set_ptr",
                    )
                    .map_err(|e| anyhow!("{e:?}"))?
            };
            
            // Get length from array pointer
            let len = self.get_array_len(arr_ptr)?;
            
            // Get pointer to data section (offset 16)
            let data_ptr_ptr = unsafe {
                self.builder.build_gep(
                    self.i64_t,
                    self.builder.build_pointer_cast(arr_ptr, self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)), "cast")?,
                    &[self.i64_t.const_int(2, false)],
                    "data_ptr_ptr"
                )?
            };
            let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                data_ptr_ptr,
                self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                "data_ptr_ptr_casted"
            )?;
            let data_ptr = self.builder.build_load(self.i8_ptr_t, data_ptr_ptr_casted, "data_ptr")?.into_pointer_value();
            (data_ptr, len)
        };
        
        let idx_bv = self.eval_value(index, fn_map)?;
        let idx_iv = self.as_i64(idx_bv)?;

        // BOUNDS CHECK (using DRY helper)
        self.build_bounds_check(idx_iv, len)?;
        
        // Check if we're setting a struct array element
        if let Some(ref struct_type_name) = element_struct_type {
            let llvm_struct_ty = if struct_type_name == "String" {
                Some(self.ty_string())
            } else {
                self.struct_types.get(struct_type_name).map(|(ty, _)| *ty)
            };

            if let Some(llvm_struct_ty) = llvm_struct_ty {
                // Struct arrays store structs directly (inline)
                let struct_ptr_ty = llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16));
                let data_struct_ptr = self.builder.build_pointer_cast(data_ptr, struct_ptr_ty, "data_struct_ptr")?;
                
                // GEP to the element (struct at index i)
                let elem_ptr = unsafe {
                    self.builder.build_gep(
                        llvm_struct_ty,
                        data_struct_ptr,
                        &[idx_iv],
                        "struct_elem_ptr",
                    )?
                };
                
                // Get the struct value to store
                let struct_to_store = if val_v.is_struct_value() {
                    val_v.into_struct_value()
                } else if val_v.is_pointer_value() {
                    // Load struct from pointer
                    self.builder.build_load(llvm_struct_ty, val_v.into_pointer_value(), "load_struct")?.into_struct_value()
                } else {
                    return Err(anyhow!("ArraySet struct: expected struct value"));
                };
                
                // Store struct directly
                self.builder.build_store(elem_ptr, struct_to_store)?;
                return Ok(());
            }
        }
        
        // Check if we're setting an integer array element
        if is_int_array {
            let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
            let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
            
            let elem_ptr = unsafe {
                self.builder.build_gep(
                    self.i64_t,
                    data_i64_ptr,
                    &[idx_iv],
                    "int_elem_ptr",
                )?
            };
            
            // Store value (convert to i64 if needed)
            let i64_val = if val_v.is_int_value() {
                val_v.into_int_value()
            } else if val_v.is_float_value() {
                self.builder.build_float_to_signed_int(
                    val_v.into_float_value(),
                    self.i64_t,
                    "f2i"
                )?
            } else if val_v.is_pointer_value() {
                self.builder.build_ptr_to_int(
                    val_v.into_pointer_value(),
                    self.i64_t,
                    "ptr2i"
                )?
            } else {
                return Err(anyhow!("ArraySet: unsupported value type"));
            };
            self.builder.build_store(elem_ptr, i64_val)?;
            return Ok(());
        }
        
        // Default: treat as f64 array
        let f64_ptr_ty = self.ctx.f64_type().ptr_type(inkwell::AddressSpace::from(0u16));
        let data_f64_ptr = self.builder.build_pointer_cast(data_ptr, f64_ptr_ty, "data_f64_ptr")?;
        
        let elem_ptr = unsafe {
            self.builder.build_gep(
                self.ctx.f64_type(),
                data_f64_ptr,
                &[idx_iv],
                "elem_ptr",
            )?
        };
        
        // Store value (convert to f64 if needed)
        let f64_val = if val_v.is_float_value() {
            val_v.into_float_value()
        } else if val_v.is_int_value() {
            self.builder.build_signed_int_to_float(
                val_v.into_int_value(),
                self.ctx.f64_type(),
                "i2f"
            )?
        } else if val_v.is_pointer_value() {
            let int_val = self.builder.build_ptr_to_int(
                val_v.into_pointer_value(),
                self.i64_t,
                "ptr2i"
            )?;
            self.builder.build_signed_int_to_float(
                int_val,
                self.ctx.f64_type(),
                "i2f"
            )?
        } else {
            return Err(anyhow!("ArraySet: unsupported value type"));
        };
        self.builder.build_store(elem_ptr, f64_val)?;
        Ok(())
    }

    fn emit_field_access(
        &mut self,
        struct_val: &IRValue,
        field: &str,
        result: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        let cur_fn = self.current_fn.map(|f| f.get_name().to_string_lossy().into_owned()).unwrap_or_else(|| "unknown".to_string());
        if field == "totalCapacity" || field == "capacity" {
            eprintln!("DEBUG emit_field_access: fn={}, struct_val={:?}, field={}", cur_fn, struct_val, field);
        }
        
        // Check if this is an enum variant access (EnumName.VariantName)
        if let IRValue::Variable(enum_name) = struct_val {
            if let Some(variants) = self.enum_types.get(enum_name).cloned() {
                // This is an enum variant access
                if let Some(variant_idx) = variants.iter().position(|v| v == field) {
                    // Enum variants are represented as integers (0, 1, 2, ...)
                    let variant_value = self.i64_t.const_int(variant_idx as u64, false);
                    self.assign_value(result, variant_value.into())?;
                    return Ok(());
                } else {
                    return Err(anyhow!("Unknown enum variant '{}' in enum '{}'", field, enum_name));
                }
            }
        }
        
        let sv = self.eval_value(struct_val, fn_map)?;
        
        // Try to determine the struct type from the variable name or register
        let struct_type_name = match struct_val {
            IRValue::Variable(var_name) => {
                self.var_struct_types.get(var_name).cloned()
            }
            IRValue::Register(reg_id) => {
                self.reg_struct_types.get(reg_id).cloned()
            }
            _ => None
        };
        
        // Check if we have a registered struct type
        if field == "totalCapacity" {
            eprintln!("DEBUG emit_field_access: struct_type_name={:?}, struct_types contains Vec = {}", 
                struct_type_name, self.struct_types.contains_key("Vec"));
        }
        if let Some(type_name) = &struct_type_name {
            if let Some((llvm_struct_ty, field_names)) = self.struct_types.get(type_name).cloned() {
                // Find field index
                let field_idx = field_names.iter().position(|n| n == field)
                    .ok_or_else(|| {
                        let available = field_names.join(", ");
                        eprintln!("ERROR: Field '{}' not found in struct '{}' (struct_val={:?}). Available: [{}]", 
                            field, type_name, struct_val, available);
                        anyhow!(
                            "Field '{}' not found in struct '{}'. Available fields: [{}]. \
                             This may indicate type confusion - check that the variable is the correct type.",
                            field, type_name, available
                        )
                    })?;
                
                let ptr = if sv.is_pointer_value() {
                    sv.into_pointer_value()
                } else if sv.is_int_value() {
                    self.builder
                        .build_int_to_ptr(
                            sv.into_int_value(),
                            llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "struct_field_ptr",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?
                } else if sv.is_struct_value() {
                    let tmp = self.alloca_for_type(
                        sv.into_struct_value().get_type().as_basic_type_enum(),
                        "struct_field_stack",
                    )?;
                    self.builder.build_store(tmp, sv)?;
                    tmp
                } else {
                    return Err(anyhow!("Unsupported field access value for {:?}", struct_val));
                };
                
                // Cast to struct pointer to ensure GEP works correctly
                let struct_ptr = self.builder.build_pointer_cast(
                    ptr,
                    llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                    "struct_ptr_cast"
                ).map_err(|e| anyhow!("{e:?}"))?;

                if field == "totalCapacity" || field == "capacity" {
                    let cur_fn = self.current_fn.map(|f| f.get_name().to_string_lossy().into_owned()).unwrap_or_else(|| "unknown".to_string());
                    eprintln!("DEBUG emit_field_access: fn={}, type={}, field_idx={}, struct_ptr={:?}", cur_fn, type_name, field_idx, struct_ptr);
                }

                let gep = self.builder.build_struct_gep(llvm_struct_ty, struct_ptr, field_idx as u32, &format!("field_{}", field))?;
                let field_ty = llvm_struct_ty.get_field_types()[field_idx];
                let loaded = self.builder.build_load(field_ty, gep, &format!("load_{}", field))?;
                self.assign_value(result, loaded.as_basic_value_enum())?;
                
                // Check if the field is an array of structs and record it for the result register
                if let Some(fields) = self.struct_definitions.get(type_name) {
                    if let Some((_, field_type)) = fields.iter().find(|(n, _)| n == field) {
                        // Track struct fields for nested field access
                        if let IRType::Struct { name: field_struct_name, .. } = field_type {
                            if let IRValue::Register(reg_id) = result {
                                eprintln!("DEBUG: FieldAccess {}.{} result Register({}) -> struct type '{}'", type_name, field, reg_id, field_struct_name);
                                self.reg_struct_types.insert(*reg_id, field_struct_name.clone());
                                
                                // Special handling for known container types with generic parameters
                                // StringHashMap.entries is Vec<Option<HashEntry>> - element type is Option<HashEntry>
                                if type_name == "StringHashMap" && field == "entries" {
                                    eprintln!("DEBUG: FieldAccess StringHashMap.entries - tracking Option<HashEntry> element type");
                                    self.reg_array_element_struct.insert(*reg_id, "Option".to_string());
                                    self.reg_option_inner_type.insert(*reg_id, "HashEntry".to_string());
                                }
                            }
                        }
                        
                        if let IRType::Array(inner) = field_type {
                            if let IRType::Struct { name: inner_struct_name, .. } = &**inner {
                                if let IRValue::Register(reg_id) = result {
                                    self.reg_array_element_struct.insert(*reg_id, inner_struct_name.clone());
                                }
                            }
                            // Track String arrays explicitly
                            if matches!(inner.as_ref(), IRType::String) {
                                if let IRValue::Register(reg_id) = result {
                                    self.reg_array_element_struct.insert(*reg_id, "String".to_string());
                                }
                            }
                            // Track integer and char arrays for proper array access
                            if matches!(inner.as_ref(), IRType::Integer | IRType::Char) {
                                if let IRValue::Register(reg_id) = result {
                                    self.reg_is_int_array.insert(*reg_id);
                                }
                            }
                        }
                    }
                } else {
                    eprintln!("DEBUG: FieldAccess {}.{} - no struct_definitions found for type", type_name, field);
                }

                return Ok(());
            } else {
                // Struct type name is known but not registered - this is a bug
                return Err(anyhow!(
                    "Struct type '{}' is referenced but not registered in module.types. \
                     This indicates the type definition is missing from the IR. \
                     Ensure ClassDefinition or StructDefinition is processed before use.",
                    type_name
                ));
            }
        }
        
        // Fallback: Support CommandResult{ success: i1, output: i8* }
        let ty = self.ty_cmd_result();
        let ptr = if sv.is_pointer_value() {
            sv.into_pointer_value()
        } else if sv.is_int_value() {
            self.builder
                .build_int_to_ptr(
                    sv.into_int_value(),
                    ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                    "cmd_field_ptr",
                )
                .map_err(|e| anyhow!("{e:?}"))?
        } else if sv.is_struct_value() {
            let tmp = self.alloca_for_type(
                sv.into_struct_value().get_type().as_basic_type_enum(),
                "cmd_field_stack",
            )?;
            self.builder.build_store(tmp, sv)?;
            self.builder
                .build_pointer_cast(
                    tmp,
                    ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                    "cmd_field_ptr_stack",
                )
                .map_err(|e| anyhow!("{e:?}"))?
        } else {
            return Err(anyhow!(format!(
                "Unsupported field access value for {:?}",
                struct_val
            )));
        };
        let idx = match field {
            "success" => 0u32,
            "output" => 1u32,
            _ => 0u32,
        };
        let gep = self.builder.build_struct_gep(ty, ptr, idx, "fld")?;
        let loaded = if idx == 0 {
            self.builder.build_load(self.bool_t, gep, "succ")?
        } else {
            self.builder.build_load(self.i8_ptr_t, gep, "out")?
        };
        self.assign_value(result, loaded.as_basic_value_enum())?;
        Ok(())
    }

    fn emit_field_set(
        &mut self,
        struct_val: &IRValue,
        field: &str,
        value: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        let sv = self.eval_value(struct_val, fn_map)?;
        let val = self.eval_value(value, fn_map)?;
        
        // Try to determine the struct type from the variable name or register
        let struct_type_name = match struct_val {
            IRValue::Variable(var_name) => {
                let result = self.var_struct_types.get(var_name).cloned();
                if result.is_none() && (var_name == "this" || var_name == "self") {
                    eprintln!("DEBUG FieldSet: '{}' not found in var_struct_types (setting field '{}'). Available: {:?}", var_name, field, self.var_struct_types.keys().collect::<Vec<_>>());
                    eprintln!("DEBUG FieldSet: var_slots available: {:?}", self.var_slots.keys().collect::<Vec<_>>());
                }
                result
            }
            IRValue::Register(reg_id) => {
                self.reg_struct_types.get(reg_id).cloned()
            }
            _ => None
        };
        
        // Check if we have a registered struct type
        if let Some(type_name) = struct_type_name {
            if let Some((llvm_struct_ty, field_names)) = self.struct_types.get(&type_name).cloned() {
                // Find field index
                let field_idx = field_names.iter().position(|n| n == field)
                    .ok_or_else(|| {
                        eprintln!("DEBUG: FieldSet: Field '{}' not found in struct '{}'. Available fields: {:?}", field, type_name, field_names);
                        anyhow!("Field '{}' not found in struct '{}'", field, type_name)
                    })?;
                
                let ptr = if let IRValue::Variable(name) = struct_val {
                     if let Some(slot) = self.var_slots.get(name).copied() {
                         let slot_ty = self.var_slot_types.get(name).unwrap();
                         if slot_ty.is_struct_type() {
                             slot
                         } else if slot_ty.is_pointer_type() {
                             self.builder.build_load(self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)), slot, "load_ptr")?.into_pointer_value()
                         } else if slot_ty.is_int_type() {
                             // Class type stored as i64 (pointer-to-int) - load and convert
                             let ptr_int = self.builder.build_load(self.i64_t, slot, "load_class_ptr")?.into_int_value();
                             self.builder.build_int_to_ptr(ptr_int, llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)), "class_ptr")?
                         } else {
                             return Err(anyhow!("Variable {} has unexpected type for FieldSet", name));
                         }
                     } else {
                         if sv.is_pointer_value() {
                             sv.into_pointer_value()
                         } else {
                             return Err(anyhow!("Unsupported field set value for {:?}", struct_val));
                         }
                     }
                } else if let IRValue::Register(id) = struct_val {
                     if let Some(slot) = self.reg_slots.get(id).copied() {
                         let slot_ty = self.reg_slot_types.get(id).unwrap();
                         if slot_ty.is_struct_type() {
                             slot
                         } else if slot_ty.is_pointer_type() {
                             self.builder.build_load(self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)), slot, "load_ptr")?.into_pointer_value()
                         } else if slot_ty.is_int_type() {
                             // Class type stored as i64 (pointer-to-int) - load and convert
                             let ptr_int = self.builder.build_load(self.i64_t, slot, "load_class_ptr")?.into_int_value();
                             self.builder.build_int_to_ptr(ptr_int, llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)), "class_ptr")?
                         } else {
                             return Err(anyhow!("Register {} has unexpected type for FieldSet", id));
                         }
                     } else {
                         if sv.is_pointer_value() {
                             sv.into_pointer_value()
                         } else {
                             return Err(anyhow!("Unsupported field set value for {:?}", struct_val));
                         }
                     }
                } else {
                    if sv.is_pointer_value() {
                        sv.into_pointer_value()
                    } else if sv.is_int_value() {
                        self.builder
                            .build_int_to_ptr(
                                sv.into_int_value(),
                                llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                                "struct_field_set_ptr",
                            )
                            .map_err(|e| anyhow!("{e:?}"))?
                    } else {
                        return Err(anyhow!("Unsupported field set value for {:?}", struct_val));
                    }
                };
                
                let gep = self.builder.build_struct_gep(llvm_struct_ty, ptr, field_idx as u32, &format!("field_set_{}", field))?;
                
                // Convert value to the correct type if needed
                let field_ty = llvm_struct_ty.get_field_types()[field_idx];
                let store_val = if field_ty.is_int_type() {
                    if val.is_int_value() {
                        val
                    } else if val.is_float_value() {
                        self.builder.build_float_to_signed_int(
                            val.into_float_value(),
                            field_ty.into_int_type(),
                            "f2i_field"
                        )?.as_basic_value_enum()
                    } else if val.is_pointer_value() {
                        self.builder.build_ptr_to_int(
                            val.into_pointer_value(),
                            field_ty.into_int_type(),
                            "ptr2i_field"
                        )?.as_basic_value_enum()
                    } else {
                        val
                    }
                } else if field_ty.is_float_type() {
                    if val.is_float_value() {
                        val
                    } else if val.is_int_value() {
                        self.builder.build_signed_int_to_float(
                            val.into_int_value(),
                            field_ty.into_float_type(),
                            "i2f_field"
                        )?.as_basic_value_enum()
                    } else {
                        val
                    }
                } else {
                    val
                };
                
                self.builder.build_store(gep, store_val)?;
                return Ok(());
            }
        }
        
        Err(anyhow!("FieldSet: struct type not found for {:?}", struct_val))
    }
}
