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
fn try_load_string_from_ptr<'ctx>(backend: &mut LlvmBackend<'ctx>, val: BasicValueEnum<'ctx>, trace: bool) -> Option<BasicValueEnum<'ctx>> {
    if !val.is_pointer_value() {
        return None;
    }
    let ptr = val.into_pointer_value();
    let str_ty = backend.ty_string();
    
    // Try to load as string struct
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
                
                // Check IR-level string detection
                let l_is_ir_str = self.is_string_value_ir(left);
                let r_is_ir_str = self.is_string_value_ir(right);
                
                // Check LLVM-level string struct types for generics
                let l_is_llvm_str = is_llvm_string_struct(self, l.clone(), trace);
                let r_is_llvm_str = is_llvm_string_struct(self, r.clone(), trace);
                
                // For pointer values that might point to string structs (boxed generics),
                // try to load and check if they're strings
                let (l_final, l_loaded_str) = if !l_is_ir_str && !l_is_llvm_str && l.is_pointer_value() {
                    if let Some(loaded) = try_load_string_from_ptr(self, l.clone(), trace) {
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
                };
                
                let (r_final, r_loaded_str) = if !r_is_ir_str && !r_is_llvm_str && r.is_pointer_value() {
                    if let Some(loaded) = try_load_string_from_ptr(self, r.clone(), trace) {
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
