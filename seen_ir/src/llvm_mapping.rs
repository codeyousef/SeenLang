//! Mapping between AST operators and LLVM operations

use inkwell::builder::Builder;
use inkwell::values::BasicValueEnum;
use seen_parser::ast::{BinaryOperator, UnaryOperator};
use crate::error::CodeGenError;

pub fn map_binary_operator<'ctx, F>(
    operator: &BinaryOperator,
    left: BasicValueEnum<'ctx>,
    right: BasicValueEnum<'ctx>,
    builder: &Builder<'ctx>,
    error_fn: F,
) -> Result<BasicValueEnum<'ctx>, CodeGenError>
where
    F: Fn(String) -> CodeGenError,
{
    match operator {
        BinaryOperator::Add => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_add(
                    left.into_int_value(),
                    right.into_int_value(),
                    "add_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int add: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_add(
                    left.into_float_value(),
                    right.into_float_value(),
                    "fadd_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float add: {:?}", e)))?.into())
            } else {
                Err(error_fn("Addition requires matching types".to_string()))
            }
        },
        BinaryOperator::Subtract => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_sub(
                    left.into_int_value(),
                    right.into_int_value(),
                    "sub_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int sub: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_sub(
                    left.into_float_value(),
                    right.into_float_value(),
                    "fsub_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float sub: {:?}", e)))?.into())
            } else {
                Err(error_fn("Subtraction requires matching types".to_string()))
            }
        },
        BinaryOperator::Multiply => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_mul(
                    left.into_int_value(),
                    right.into_int_value(),
                    "mul_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int mul: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_mul(
                    left.into_float_value(),
                    right.into_float_value(),
                    "fmul_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float mul: {:?}", e)))?.into())
            } else {
                Err(error_fn("Multiplication requires matching types".to_string()))
            }
        },
        BinaryOperator::Divide => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_signed_div(
                    left.into_int_value(),
                    right.into_int_value(),
                    "div_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int div: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_div(
                    left.into_float_value(),
                    right.into_float_value(),
                    "fdiv_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float div: {:?}", e)))?.into())
            } else {
                Err(error_fn("Division requires matching types".to_string()))
            }
        },
        BinaryOperator::Modulo => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_signed_rem(
                    left.into_int_value(),
                    right.into_int_value(),
                    "mod_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int rem: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_rem(
                    left.into_float_value(),
                    right.into_float_value(),
                    "fmod_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float rem: {:?}", e)))?.into())
            } else {
                Err(error_fn("Modulo requires matching types".to_string()))
            }
        },
        BinaryOperator::Equal => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_compare(
                    inkwell::IntPredicate::EQ,
                    left.into_int_value(),
                    right.into_int_value(),
                    "eq_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int compare: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_compare(
                    inkwell::FloatPredicate::OEQ,
                    left.into_float_value(),
                    right.into_float_value(),
                    "feq_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float compare: {:?}", e)))?.into())
            } else {
                Err(error_fn("Equality requires matching types".to_string()))
            }
        },
        BinaryOperator::NotEqual => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    left.into_int_value(),
                    right.into_int_value(),
                    "ne_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int compare: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_compare(
                    inkwell::FloatPredicate::ONE,
                    left.into_float_value(),
                    right.into_float_value(),
                    "fne_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float compare: {:?}", e)))?.into())
            } else {
                Err(error_fn("Inequality requires matching types".to_string()))
            }
        },
        BinaryOperator::LessThan => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_compare(
                    inkwell::IntPredicate::SLT,
                    left.into_int_value(),
                    right.into_int_value(),
                    "lt_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int compare: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_compare(
                    inkwell::FloatPredicate::OLT,
                    left.into_float_value(),
                    right.into_float_value(),
                    "flt_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float compare: {:?}", e)))?.into())
            } else {
                Err(error_fn("Less than requires matching types".to_string()))
            }
        },
        BinaryOperator::LessEqual => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_compare(
                    inkwell::IntPredicate::SLE,
                    left.into_int_value(),
                    right.into_int_value(),
                    "le_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int compare: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_compare(
                    inkwell::FloatPredicate::OLE,
                    left.into_float_value(),
                    right.into_float_value(),
                    "fle_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float compare: {:?}", e)))?.into())
            } else {
                Err(error_fn("Less equal requires matching types".to_string()))
            }
        },
        BinaryOperator::GreaterThan => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_compare(
                    inkwell::IntPredicate::SGT,
                    left.into_int_value(),
                    right.into_int_value(),
                    "gt_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int compare: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_compare(
                    inkwell::FloatPredicate::OGT,
                    left.into_float_value(),
                    right.into_float_value(),
                    "fgt_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float compare: {:?}", e)))?.into())
            } else {
                Err(error_fn("Greater than requires matching types".to_string()))
            }
        },
        BinaryOperator::GreaterEqual => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_int_compare(
                    inkwell::IntPredicate::SGE,
                    left.into_int_value(),
                    right.into_int_value(),
                    "ge_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int compare: {:?}", e)))?.into())
            } else if left.is_float_value() && right.is_float_value() {
                Ok(builder.build_float_compare(
                    inkwell::FloatPredicate::OGE,
                    left.into_float_value(),
                    right.into_float_value(),
                    "fge_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float compare: {:?}", e)))?.into())
            } else {
                Err(error_fn("Greater equal requires matching types".to_string()))
            }
        },
        BinaryOperator::And => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_and(
                    left.into_int_value(),
                    right.into_int_value(),
                    "and_tmp",
                ).map_err(|e| error_fn(format!("Failed to build and: {:?}", e)))?.into())
            } else {
                Err(error_fn("Logical and requires boolean values".to_string()))
            }
        },
        BinaryOperator::Or => {
            if left.is_int_value() && right.is_int_value() {
                Ok(builder.build_or(
                    left.into_int_value(),
                    right.into_int_value(),
                    "or_tmp",
                ).map_err(|e| error_fn(format!("Failed to build or: {:?}", e)))?.into())
            } else {
                Err(error_fn("Logical or requires boolean values".to_string()))
            }
        },
    }
}

pub fn map_unary_operator<'ctx, F>(
    operator: &UnaryOperator,
    operand: BasicValueEnum<'ctx>,
    builder: &Builder<'ctx>,
    error_fn: F,
) -> Result<BasicValueEnum<'ctx>, CodeGenError>
where
    F: Fn(String) -> CodeGenError,
{
    match operator {
        UnaryOperator::Minus => {
            if operand.is_int_value() {
                Ok(builder.build_int_neg(
                    operand.into_int_value(),
                    "neg_tmp",
                ).map_err(|e| error_fn(format!("Failed to build int neg: {:?}", e)))?.into())
            } else if operand.is_float_value() {
                Ok(builder.build_float_neg(
                    operand.into_float_value(),
                    "fneg_tmp",
                ).map_err(|e| error_fn(format!("Failed to build float neg: {:?}", e)))?.into())
            } else {
                Err(error_fn("Negation requires numeric type".to_string()))
            }
        },
        UnaryOperator::Not => {
            if operand.is_int_value() {
                Ok(builder.build_not(
                    operand.into_int_value(),
                    "not_tmp",
                ).map_err(|e| error_fn(format!("Failed to build not: {:?}", e)))?.into())
            } else {
                Err(error_fn("Logical not requires boolean type".to_string()))
            }
        },
    }
}