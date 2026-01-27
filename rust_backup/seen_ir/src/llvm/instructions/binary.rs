//! Binary and unary operation handlers for LLVM code generation.
//!
//! This module handles arithmetic, comparison, and logical operations.

use anyhow::{anyhow, Result};
use inkwell::values::{BasicValue, BasicValueEnum};
use inkwell::types::BasicTypeEnum;

use crate::instruction::{BinaryOp, UnaryOp};
use crate::llvm_backend::LlvmBackend;
use crate::llvm::string_ops::RuntimeStringOps;
use crate::llvm::type_cast::TypeCastOps;
use crate::llvm::c_library::CLibraryOps;
use crate::value::IRValue;

/// Check if an LLVM value is a string struct type `{i64, ptr}` or a pointer to one
fn is_llvm_string_struct<'ctx>(backend: &LlvmBackend<'ctx>, val: BasicValueEnum<'ctx>, trace: bool) -> bool {
    if val.is_struct_value() {
        let str_ty = backend.ty_string();
        let val_ty = val.into_struct_value().get_type();
        let is_match = val_ty == str_ty;
        if trace {
            eprintln!("[TRACE binary] is_llvm_string_struct: struct_value, str_ty={:?}, val_ty={:?}, match={}", str_ty, val_ty, is_match);
        }
        return is_match;
    }
    false
}

/// Try to load a string struct from a pointer value
/// Returns Some(string_struct) if the pointer points to a string struct, None otherwise
///
/// For boxed generic parameters (ptr to i64 containing string ptr), this function
/// first loads the i64, converts to ptr, then loads the String struct.
/// Also handles generic type markers (T, K, V, E) which use ptr-as-int representation.
fn try_load_string_from_ptr<'ctx>(backend: &mut LlvmBackend<'ctx>, val: BasicValueEnum<'ctx>, trace: bool, ir_val: Option<&IRValue>) -> Option<BasicValueEnum<'ctx>> {
    if !val.is_pointer_value() {
        return None;
    }
    let ptr = val.into_pointer_value();
    let str_ty = backend.ty_string();

    // Check if this is a boxed generic PARAMETER or a register from a generic field access
    // VARIABLES (function parameters) use the ptr -> i64 -> ptr -> String representation
    // REGISTERS can also be boxed generic when loaded from a field of generic type (T, K, V, E)
    // Also check var_struct_types/reg_struct_types for generic type markers (T, K, V, E, Option)
    // which indicate the value might be a ptr-as-int String
    let is_boxed_generic = match ir_val {
        Some(IRValue::Variable(name)) => {
            backend.var_is_boxed_generic.contains(name) ||
            backend.var_struct_types.get(name).map(|t| {
                t == "T" || t == "K" || t == "V" || t == "E" || t == "Option"
            }).unwrap_or(false)
        }
        Some(IRValue::Register(reg_id)) => {
            backend.reg_is_boxed_generic.contains(reg_id) ||
            backend.reg_struct_types.get(reg_id).map(|t| {
                t == "T" || t == "K" || t == "V" || t == "E" || t == "Option"
            }).unwrap_or(false)
        }
        _ => false,
    };

    if is_boxed_generic {
        // Boxed generic: ptr -> i64 -> ptr -> String
        // Load the i64 value (which is the actual String ptr as int)
        if let Ok(i64_val) = backend.builder.build_load(backend.i64_t, ptr, "load_boxed_str_ptr") {
            let i64_val = i64_val.into_int_value();
            // Convert i64 to pointer
            if let Ok(str_ptr) = backend.builder.build_int_to_ptr(
                i64_val,
                backend.ctx.ptr_type(inkwell::AddressSpace::from(0u16)),
                "boxed_str_ptr"
            ) {
                // Now load the String struct from this pointer
                match backend.builder.build_load(str_ty, str_ptr, "maybe_str_load") {
                    Ok(loaded) => {
                        if trace {
                            eprintln!("[TRACE binary] try_load_string_from_ptr: loaded {:?} from boxed generic", loaded.get_type());
                        }
                        return Some(loaded);
                    }
                    Err(_) => return None
                }
            }
        }
        return None;
    }

    // Direct pointer to String struct
    match backend.builder.build_load(str_ty, ptr, "maybe_str_load") {
        Ok(loaded) => {
            if trace {
                eprintln!("[TRACE binary] try_load_string_from_ptr: loaded {:?} from ptr", loaded.get_type());
            }
            Some(loaded)
        }
        Err(_) => None
    }
}

/// Try to load a string from a generic register that holds an i64 (ptr-as-int).
/// When Vec.get returns a value with generic type T, the LLVM value is an i64,
/// not a pointer. This function detects that case and loads the String struct.
fn try_load_string_from_generic_i64<'ctx>(backend: &mut LlvmBackend<'ctx>, val: BasicValueEnum<'ctx>, trace: bool, ir_val: &IRValue) -> Option<BasicValueEnum<'ctx>> {
    // Handle both registers and variables with generic types
    let is_generic = match ir_val {
        IRValue::Register(id) => {
            backend.reg_struct_types.get(id).map(|t| {
                t == "T" || t == "K" || t == "V" || t == "E" || t == "String"
            }).unwrap_or(false)
        }
        IRValue::Variable(name) => {
            backend.var_struct_types.get(name).map(|t| {
                t == "T" || t == "K" || t == "V" || t == "E" || t == "String"
            }).unwrap_or(false)
        }
        _ => false,
    };

    if !is_generic {
        return None;
    }

    // The value should be an i64 (ptr-as-int representation)
    if !val.is_int_value() {
        return None;
    }

    let i64_val = val.into_int_value();
    if i64_val.get_type().get_bit_width() != 64 {
        return None;
    }

    // Convert i64 to pointer
    let str_ptr = match backend.builder.build_int_to_ptr(
        i64_val,
        backend.ctx.ptr_type(inkwell::AddressSpace::from(0u16)),
        "generic_str_ptr"
    ) {
        Ok(p) => p,
        Err(_) => return None,
    };

    // Load the String struct
    let str_ty = backend.ty_string();
    match backend.builder.build_load(str_ty, str_ptr, "generic_str_load") {
        Ok(loaded) => {
            if trace {
                eprintln!("[TRACE binary] try_load_string_from_generic_i64: loaded String struct {:?}", loaded.get_type());
            }
            Some(loaded)
        }
        Err(_) => None,
    }
}

/// Trait for binary operation emission.
pub trait BinaryOps<'ctx> {
    /// Emit a binary operation and return the result value.
    fn emit_binary_op(
        &mut self,
        op: &BinaryOp,
        left: &IRValue,
        right: &IRValue,
        fn_map: &indexmap::IndexMap<String, inkwell::values::FunctionValue<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>>;
}

impl<'ctx> BinaryOps<'ctx> for LlvmBackend<'ctx> {
    fn emit_binary_op(
        &mut self,
        op: &BinaryOp,
        left: &IRValue,
        right: &IRValue,
        fn_map: &indexmap::IndexMap<String, inkwell::values::FunctionValue<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let l = self.eval_value(left, fn_map)?;
        let r = self.eval_value(right, fn_map)?;
        let trace = self.trace_options.trace_values;
        
        // Check if either operand is a float for arithmetic operations
        let is_float_op = l.is_float_value() || r.is_float_value();
        
        match op {
            BinaryOp::Add => {
                // Check both IR-level string info AND LLVM-level struct type for generics
                let l_is_str = self.is_string_value_ir(left) || is_llvm_string_struct(self, l.clone(), trace);
                let r_is_str = self.is_string_value_ir(right) || is_llvm_string_struct(self, r.clone(), trace);
                if l_is_str || r_is_str {
                    let l_str = self.ensure_string(l.clone(), left)?;
                    let r_str = self.ensure_string(r.clone(), right)?;
                    self.runtime_concat(l_str, r_str)
                } else if is_float_op {
                    let lf = self.as_f64(l)?;
                    let rf = self.as_f64(r)?;
                    Ok(self.builder
                        .build_float_add(lf, rf, "fadd")?
                        .as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder
                        .build_int_add(li, ri, "add")?
                        .as_basic_value_enum())
                }
            }
            BinaryOp::Subtract => {
                if is_float_op {
                    let lf = self.as_f64(l)?;
                    let rf = self.as_f64(r)?;
                    Ok(self.builder
                        .build_float_sub(lf, rf, "fsub")?
                        .as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder
                        .build_int_sub(li, ri, "sub")?
                        .as_basic_value_enum())
                }
            }
            BinaryOp::Multiply => {
                if is_float_op {
                    let lf = self.as_f64(l)?;
                    let rf = self.as_f64(r)?;
                    Ok(self.builder
                        .build_float_mul(lf, rf, "fmul")?
                        .as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder
                        .build_int_mul(li, ri, "mul")?
                        .as_basic_value_enum())
                }
            }
            BinaryOp::Divide => {
                if is_float_op {
                    let lf = self.as_f64(l)?;
                    let rf = self.as_f64(r)?;
                    Ok(self.builder
                        .build_float_div(lf, rf, "fdiv")?
                        .as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder
                        .build_int_signed_div(li, ri, "div")?
                        .as_basic_value_enum())
                }
            }
            BinaryOp::Modulo => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_int_signed_rem(li, ri, "mod")?
                    .as_basic_value_enum())
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let pred = match op {
                    BinaryOp::Equal => inkwell::IntPredicate::EQ,
                    _ => inkwell::IntPredicate::NE,
                };

                // Handle null comparison specially
                // For Option<T> types, we compare hasValue field (first field) to check if it has a value
                // For other types, we do pointer comparison
                let left_is_null = matches!(left, IRValue::Null);
                let right_is_null = matches!(right, IRValue::Null);
                
                if left_is_null || right_is_null {
                    // Determine which side is the non-null value and check if it's an Option
                    let (non_null_val, non_null_ir, non_null_llvm) = if left_is_null {
                        (right, r.clone(), l.clone())
                    } else {
                        (left, l.clone(), r.clone())
                    };
                    
                    // Check if the non-null value is an Option type
                    let is_option = match non_null_val {
                        IRValue::Register(reg_id) => {
                            self.reg_struct_types.get(reg_id)
                                .map(|s| s.starts_with("Option"))
                                .unwrap_or(false)
                        }
                        IRValue::Variable(name) => {
                            self.var_struct_types.get(name)
                                .map(|s| s.starts_with("Option"))
                                .unwrap_or(false)
                        }
                        _ => false,
                    };
                    
                    if trace {
                        eprintln!("[TRACE binary] null comparison: left_is_null={}, right_is_null={}, is_option={}", 
                            left_is_null, right_is_null, is_option);
                    }
                    
                    if is_option {
                        // For Option, compare hasValue field to check if it has a value
                        // Option layout: { i1 hasValue, i64 value }
                        // hasValue is at index 0
                        let option_ptr = if non_null_ir.is_pointer_value() {
                            non_null_ir.into_pointer_value()
                        } else if non_null_ir.is_int_value() {
                            // i64 (ptr-as-int) - convert to pointer
                            let iv = non_null_ir.into_int_value();
                            self.builder.build_int_to_ptr(iv, self.i8_ptr_t, "option_ptr")?
                        } else {
                            return Err(anyhow!("Expected pointer or i64 for Option null comparison"));
                        };
                        
                        // Option struct type: { i1, i64 }
                        let option_ty = self.ctx.struct_type(&[self.bool_t.into(), self.i64_t.into()], false);
                        
                        // Get pointer to hasValue field (index 0)
                        let has_value_ptr = self.builder.build_struct_gep(
                            option_ty,
                            option_ptr,
                            0,
                            "has_value_ptr"
                        )?;
                        
                        // Load hasValue (i1/bool)
                        let has_value = self.builder.build_load(self.bool_t, has_value_ptr, "has_value")?;
                        let has_value_i1 = has_value.into_int_value();
                        
                        if trace {
                            eprintln!("[TRACE binary] Option null comparison: loaded hasValue, comparing with pred {:?}", pred);
                        }
                        
                        // For "option == null", we want hasValue == false (0)
                        // For "option != null", we want hasValue == true (1)
                        // Since pred is EQ for ==, NE for !=:
                        // - "option == null" means "hasValue == 0" -> compare hasValue to 0 with EQ
                        // - "option != null" means "hasValue != 0" -> compare hasValue to 0 with NE
                        let zero = self.bool_t.const_zero();
                        return Ok(self.builder
                            .build_int_compare(pred, has_value_i1, zero, "option_null_cmp")?
                            .as_basic_value_enum());
                    }
                    
                    // For non-Option types, use pointer comparison for null checks
                    // Both sides should be pointers (or castable to pointers)
                    let lp = if l.is_pointer_value() {
                        l.into_pointer_value()
                    } else if l.is_int_value() {
                        // i64 (could be ptr-as-int) - treat as pointer
                        let iv = l.into_int_value();
                        self.builder.build_int_to_ptr(iv, self.i8_ptr_t, "null_cmp_i2p")?
                    } else {
                        // Fallback: treat as null pointer if it's a null literal
                        self.i8_ptr_t.const_null()
                    };
                    let rp = if r.is_pointer_value() {
                        r.into_pointer_value()
                    } else if r.is_int_value() {
                        let iv = r.into_int_value();
                        self.builder.build_int_to_ptr(iv, self.i8_ptr_t, "null_cmp_i2p")?
                    } else {
                        self.i8_ptr_t.const_null()
                    };
                    let li = self.builder.build_ptr_to_int(lp, self.i64_t, "l_ptr2i")?;
                    let ri = self.builder.build_ptr_to_int(rp, self.i64_t, "r_ptr2i")?;
                    return Ok(self.builder
                        .build_int_compare(pred, li, ri, "null_cmp")?
                        .as_basic_value_enum());
                }
                
                // Check IR-level string detection
                let l_is_ir_str = self.is_string_value_ir(left);
                let r_is_ir_str = self.is_string_value_ir(right);

                // Check LLVM-level string struct types for generics
                let l_is_llvm_str = is_llvm_string_struct(self, l.clone(), trace);
                let r_is_llvm_str = is_llvm_string_struct(self, r.clone(), trace);

                // Check if either side is a literal Int/Integer value
                // We should NOT try to load as String if comparing against a literal Int
                let l_is_literal_int = matches!(left, IRValue::Integer(_));
                let r_is_literal_int = matches!(right, IRValue::Integer(_));

                // Check if either side has a "String" type hint in its struct_types or is tracked as String
                let l_has_string_hint = match left {
                    IRValue::Variable(name) => self.var_struct_types.get(name).map(|t| t == "String").unwrap_or(false)
                        || self.var_is_string.contains(name),
                    IRValue::Register(id) => self.reg_struct_types.get(id).map(|t| t == "String").unwrap_or(false),
                    _ => false,
                };
                let r_has_string_hint = match right {
                    IRValue::Variable(name) => self.var_struct_types.get(name).map(|t| t == "String").unwrap_or(false)
                        || self.var_is_string.contains(name),
                    IRValue::Register(id) => self.reg_struct_types.get(id).map(|t| t == "String").unwrap_or(false),
                    _ => false,
                };

                // Check if either side has a generic type (T, K, V, E)
                let l_is_generic = match left {
                    IRValue::Variable(name) => self.var_struct_types.get(name).map(|t| {
                        t == "T" || t == "K" || t == "V" || t == "E"
                    }).unwrap_or(false),
                    IRValue::Register(id) => self.reg_struct_types.get(id).map(|t| {
                        t == "T" || t == "K" || t == "V" || t == "E"
                    }).unwrap_or(false),
                    _ => false,
                };
                let r_is_generic = match right {
                    IRValue::Variable(name) => self.var_struct_types.get(name).map(|t| {
                        t == "T" || t == "K" || t == "V" || t == "E"
                    }).unwrap_or(false),
                    IRValue::Register(id) => self.reg_struct_types.get(id).map(|t| {
                        t == "T" || t == "K" || t == "V" || t == "E"
                    }).unwrap_or(false),
                    _ => false,
                };

                // We might be doing a String comparison if:
                // - One side is already known to be String (IR or LLVM level)
                // - One side has explicit "String" type hint
                // - Both sides are generic types (might be comparing String generics)
                // BUT NOT if one side is a literal Int (then it's definitely Int comparison)
                let might_be_string_comparison =
                    !l_is_literal_int && !r_is_literal_int &&
                    (l_is_ir_str || r_is_ir_str || l_is_llvm_str || r_is_llvm_str ||
                     l_has_string_hint || r_has_string_hint ||
                     (l_is_generic && r_is_generic));

                // For generic registers holding i64 (ptr-as-int from Vec.get), convert to String
                // This must be checked BEFORE pointer check since these are i64 values, not pointers
                let (l_final, l_loaded_str) = if !l_is_ir_str && !l_is_llvm_str && might_be_string_comparison {
                    // First try: generic i64 (from Vec.get with type T)
                    if let Some(loaded) = try_load_string_from_generic_i64(self, l.clone(), trace, left) {
                        if is_llvm_string_struct(self, loaded.clone(), trace) {
                            (loaded, true)
                        } else {
                            (l.clone(), false)
                        }
                    // Second try: pointer to boxed generic
                    } else if l.is_pointer_value() {
                        if let Some(loaded) = try_load_string_from_ptr(self, l.clone(), trace, Some(left)) {
                            if is_llvm_string_struct(self, loaded.clone(), trace) {
                                (loaded, true)
                            } else {
                                (l.clone(), false)
                            }
                        } else {
                            (l.clone(), false)
                        }
                    } else {
                        (l.clone(), false)
                    }
                } else {
                    (l.clone(), false)
                };

                let (r_final, r_loaded_str) = if !r_is_ir_str && !r_is_llvm_str && might_be_string_comparison {
                    // First try: generic i64 (from Vec.get with type T)
                    if let Some(loaded) = try_load_string_from_generic_i64(self, r.clone(), trace, right) {
                        if is_llvm_string_struct(self, loaded.clone(), trace) {
                            (loaded, true)
                        } else {
                            (r.clone(), false)
                        }
                    // Second try: pointer to boxed generic
                    } else if r.is_pointer_value() {
                        if let Some(loaded) = try_load_string_from_ptr(self, r.clone(), trace, Some(right)) {
                            if is_llvm_string_struct(self, loaded.clone(), trace) {
                                (loaded, true)
                            } else {
                                (r.clone(), false)
                            }
                        } else {
                            (r.clone(), false)
                        }
                    } else {
                        (r.clone(), false)
                    }
                } else {
                    (r.clone(), false)
                };
                
                let left_is_str = l_is_ir_str || l_is_llvm_str || l_loaded_str;
                let right_is_str = r_is_ir_str || r_is_llvm_str || r_loaded_str;
                
                if trace {
                    eprintln!("[TRACE binary] Equal/NotEqual: left={:?} l_type={:?} l_is_llvm_str={} l_loaded_str={} left_is_str={}", 
                             left, l.get_type(), l_is_llvm_str, l_loaded_str, left_is_str);
                    eprintln!("[TRACE binary] Equal/NotEqual: right={:?} r_type={:?} r_is_llvm_str={} r_loaded_str={} right_is_str={}", 
                             right, r.get_type(), r_is_llvm_str, r_loaded_str, right_is_str);
                }
                
                // Use the potentially-loaded values for comparison
                let l = l_final;
                let r = r_final;
                
                // Check both IR literal Char AND i8 LLVM values (from variables holding chars)
                let left_is_char = matches!(left, IRValue::Char(_)) || 
                    (l.is_int_value() && l.into_int_value().get_type().get_bit_width() == 8);
                let right_is_char = matches!(right, IRValue::Char(_)) ||
                    (r.is_int_value() && r.into_int_value().get_type().get_bit_width() == 8);
                if left_is_str || right_is_str || left_is_char || right_is_char {
                    // Convert char values to string structs before comparison
                    let lp = if left_is_char {
                        self.char_to_cstr_ptr(l)?
                    } else {
                        self.as_cstr_ptr(l)?
                    };
                    let rp = if right_is_char {
                        self.char_to_cstr_ptr(r)?
                    } else {
                        self.as_cstr_ptr(r)?
                    };
                    let cmp = self.call_strcmp(lp, rp)?;
                    let zero = self.ctx.i32_type().const_zero();
                    Ok(self.builder
                        .build_int_compare(pred, cmp, zero, "strcmp")?
                        .as_basic_value_enum())
                } else if l.is_pointer_value() && r.is_pointer_value() {
                    // Pointer comparison (reference equality)
                    // This handles null checks (ptr != null) and class reference equality
                    let lp = l.into_pointer_value();
                    let rp = r.into_pointer_value();
                    let li = self.builder.build_ptr_to_int(lp, self.i64_t, "l_ptr2i")?;
                    let ri = self.builder.build_ptr_to_int(rp, self.i64_t, "r_ptr2i")?;
                    Ok(self.builder
                        .build_int_compare(pred, li, ri, "ptr_cmp")?
                        .as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder
                        .build_int_compare(pred, li, ri, "icmp")?
                        .as_basic_value_enum())
                }
            }
            BinaryOp::LessThan => {
                // Check IR-level AND LLVM-level string struct types for generics
                let left_is_str = self.is_string_value_ir(left) || is_llvm_string_struct(self, l.clone(), trace);
                let right_is_str = self.is_string_value_ir(right) || is_llvm_string_struct(self, r.clone(), trace);
                let left_is_char = matches!(left, IRValue::Char(_)) || 
                    (l.is_int_value() && l.into_int_value().get_type().get_bit_width() == 8);
                let right_is_char = matches!(right, IRValue::Char(_)) ||
                    (r.is_int_value() && r.into_int_value().get_type().get_bit_width() == 8);
                if left_is_str || right_is_str || left_is_char || right_is_char {
                    let lp = if left_is_char { self.char_to_cstr_ptr(l)? } else { self.as_cstr_ptr(l)? };
                    let rp = if right_is_char { self.char_to_cstr_ptr(r)? } else { self.as_cstr_ptr(r)? };
                    let cmp = self.call_strcmp(lp, rp)?;
                    let zero = self.ctx.i32_type().const_zero();
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SLT, cmp, zero, "strcmp_lt")?.as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SLT, li, ri, "lt")?.as_basic_value_enum())
                }
            }
            BinaryOp::LessEqual => {
                // Check IR-level AND LLVM-level string struct types for generics
                let left_is_str = self.is_string_value_ir(left) || is_llvm_string_struct(self, l.clone(), trace);
                let right_is_str = self.is_string_value_ir(right) || is_llvm_string_struct(self, r.clone(), trace);
                let left_is_char = matches!(left, IRValue::Char(_)) || 
                    (l.is_int_value() && l.into_int_value().get_type().get_bit_width() == 8);
                let right_is_char = matches!(right, IRValue::Char(_)) ||
                    (r.is_int_value() && r.into_int_value().get_type().get_bit_width() == 8);
                if left_is_str || right_is_str || left_is_char || right_is_char {
                    let lp = if left_is_char { self.char_to_cstr_ptr(l)? } else { self.as_cstr_ptr(l)? };
                    let rp = if right_is_char { self.char_to_cstr_ptr(r)? } else { self.as_cstr_ptr(r)? };
                    let cmp = self.call_strcmp(lp, rp)?;
                    let zero = self.ctx.i32_type().const_zero();
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SLE, cmp, zero, "strcmp_le")?.as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SLE, li, ri, "le")?.as_basic_value_enum())
                }
            }
            BinaryOp::GreaterThan => {
                // Check IR-level AND LLVM-level string struct types for generics
                let left_is_str = self.is_string_value_ir(left) || is_llvm_string_struct(self, l.clone(), trace);
                let right_is_str = self.is_string_value_ir(right) || is_llvm_string_struct(self, r.clone(), trace);
                let left_is_char = matches!(left, IRValue::Char(_)) || 
                    (l.is_int_value() && l.into_int_value().get_type().get_bit_width() == 8);
                let right_is_char = matches!(right, IRValue::Char(_)) ||
                    (r.is_int_value() && r.into_int_value().get_type().get_bit_width() == 8);
                if left_is_str || right_is_str || left_is_char || right_is_char {
                    let lp = if left_is_char { self.char_to_cstr_ptr(l)? } else { self.as_cstr_ptr(l)? };
                    let rp = if right_is_char { self.char_to_cstr_ptr(r)? } else { self.as_cstr_ptr(r)? };
                    let cmp = self.call_strcmp(lp, rp)?;
                    let zero = self.ctx.i32_type().const_zero();
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SGT, cmp, zero, "strcmp_gt")?.as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SGT, li, ri, "gt")?.as_basic_value_enum())
                }
            }
            BinaryOp::GreaterEqual => {
                // Check IR-level AND LLVM-level string struct types for generics
                let left_is_str = self.is_string_value_ir(left) || is_llvm_string_struct(self, l.clone(), trace);
                let right_is_str = self.is_string_value_ir(right) || is_llvm_string_struct(self, r.clone(), trace);
                let left_is_char = matches!(left, IRValue::Char(_)) || 
                    (l.is_int_value() && l.into_int_value().get_type().get_bit_width() == 8);
                let right_is_char = matches!(right, IRValue::Char(_)) ||
                    (r.is_int_value() && r.into_int_value().get_type().get_bit_width() == 8);
                if left_is_str || right_is_str || left_is_char || right_is_char {
                    let lp = if left_is_char { self.char_to_cstr_ptr(l)? } else { self.as_cstr_ptr(l)? };
                    let rp = if right_is_char { self.char_to_cstr_ptr(r)? } else { self.as_cstr_ptr(r)? };
                    let cmp = self.call_strcmp(lp, rp)?;
                    let zero = self.ctx.i32_type().const_zero();
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SGE, cmp, zero, "strcmp_ge")?.as_basic_value_enum())
                } else {
                    let li = self.as_i64(l)?;
                    let ri = self.as_i64(r)?;
                    Ok(self.builder.build_int_compare(inkwell::IntPredicate::SGE, li, ri, "ge")?.as_basic_value_enum())
                }
            }
            BinaryOp::And => {
                let li = self.as_bool(l)?;
                let ri = self.as_bool(r)?;
                Ok(self.builder.build_and(li, ri, "and")?.as_basic_value_enum())
            }
            BinaryOp::Or => {
                let li = self.as_bool(l)?;
                let ri = self.as_bool(r)?;
                Ok(self.builder.build_or(li, ri, "or")?.as_basic_value_enum())
            }
            BinaryOp::BitwiseAnd => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_and(li, ri, "band")?
                    .as_basic_value_enum())
            }
            BinaryOp::BitwiseOr => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder.build_or(li, ri, "bor")?.as_basic_value_enum())
            }
            BinaryOp::BitwiseXor => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_xor(li, ri, "bxor")?
                    .as_basic_value_enum())
            }
            BinaryOp::LeftShift => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_left_shift(li, ri, "shl")?
                    .as_basic_value_enum())
            }
            BinaryOp::RightShift => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_right_shift(li, ri, true, "shr")?
                    .as_basic_value_enum())
            }
        }
    }
}

/// Trait for unary operation emission.
pub trait UnaryOps<'ctx> {
    fn emit_unary_op(
        &mut self,
        op: &UnaryOp,
        val: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>>;
}

impl<'ctx> UnaryOps<'ctx> for LlvmBackend<'ctx> {
    fn emit_unary_op(
        &mut self,
        op: &UnaryOp,
        val: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        match op {
            UnaryOp::Negate => {
                if val.is_float_value() {
                    let f = val.into_float_value();
                    Ok(self.builder.build_float_neg(f, "fneg")?.as_basic_value_enum())
                } else {
                    let i = self.as_i64(val)?;
                    Ok(self.builder.build_int_neg(i, "neg")?.as_basic_value_enum())
                }
            }
            UnaryOp::Not => {
                let i = self.as_i64(val)?;
                let zero = self.i64_t.const_zero();
                let cmp = self.builder.build_int_compare(inkwell::IntPredicate::EQ, i, zero, "not")?;
                Ok(self.builder.build_int_z_extend(cmp, self.i64_t, "not_ext")?.as_basic_value_enum())
            }
            UnaryOp::BitwiseNot => {
                let i = self.as_i64(val)?;
                Ok(self.builder.build_not(i, "bnot")?.as_basic_value_enum())
            }
            UnaryOp::Reference | UnaryOp::Dereference => {
                Err(anyhow!("Reference/Dereference not supported in emit_unary_op"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_compiles() {
        assert!(true);
    }
}
