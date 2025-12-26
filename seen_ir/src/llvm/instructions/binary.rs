//! Binary and unary operation handlers for LLVM code generation.
//!
//! This module handles arithmetic, comparison, and logical operations.

use anyhow::{anyhow, Result};
use inkwell::values::BasicValueEnum;

use crate::instruction::BinaryOp;
use crate::llvm_backend::LlvmBackend;
use crate::llvm::string_ops::RuntimeStringOps;
use crate::value::IRValue;

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
        
        // Check if either operand is a float for arithmetic operations
        let is_float_op = l.is_float_value() || r.is_float_value();
        
        match op {
            BinaryOp::Add => {
                if self.is_string_value_ir(left) || self.is_string_value_ir(right) {
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
                if self.is_string_value_ir(left) || self.is_string_value_ir(right) {
                    let lp = self.as_cstr_ptr(l)?;
                    let rp = self.as_cstr_ptr(r)?;
                    let cmp = self.call_strcmp(lp, rp)?;
                    let zero = self.ctx.i32_type().const_zero();
                    Ok(self.builder
                        .build_int_compare(pred, cmp, zero, "strcmp")?
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
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_int_compare(inkwell::IntPredicate::SLT, li, ri, "lt")?
                    .as_basic_value_enum())
            }
            BinaryOp::LessEqual => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_int_compare(inkwell::IntPredicate::SLE, li, ri, "le")?
                    .as_basic_value_enum())
            }
            BinaryOp::GreaterThan => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_int_compare(inkwell::IntPredicate::SGT, li, ri, "gt")?
                    .as_basic_value_enum())
            }
            BinaryOp::GreaterEqual => {
                let li = self.as_i64(l)?;
                let ri = self.as_i64(r)?;
                Ok(self.builder
                    .build_int_compare(inkwell::IntPredicate::SGE, li, ri, "ge")?
                    .as_basic_value_enum())
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_compiles() {
        assert!(true);
    }
}
