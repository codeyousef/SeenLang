use inkwell::values::{BasicValueEnum, InstructionValue, PointerValue};
use seen_parser::ast::{BinaryOperator, UnaryOperator};
use crate::error::{CodeGenError, Result};

/// Maps Seen language binary operators to LLVM IR instructions
pub fn map_binary_operator<'ctx, F>(
    operator: &BinaryOperator,
    left: BasicValueEnum<'ctx>,
    right: BasicValueEnum<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    error_fn: F,
) -> Result<BasicValueEnum<'ctx>>
where
    F: FnOnce(String) -> CodeGenError,
{
    match operator {
        BinaryOperator::Add => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_add(left_int, right_int, "addtmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_add(left_float, right_float, "addtmp").into())
            } else {
                Err(error_fn(format!("Cannot add values of this type")))
            }
        },
        BinaryOperator::Subtract => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_sub(left_int, right_int, "subtmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_sub(left_float, right_float, "subtmp").into())
            } else {
                Err(error_fn(format!("Cannot subtract values of this type")))
            }
        },
        BinaryOperator::Multiply => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_mul(left_int, right_int, "multmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_mul(left_float, right_float, "multmp").into())
            } else {
                Err(error_fn(format!("Cannot multiply values of this type")))
            }
        },
        BinaryOperator::Divide => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                // Signed division for integers
                Ok(builder.build_int_signed_div(left_int, right_int, "divtmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_div(left_float, right_float, "divtmp").into())
            } else {
                Err(error_fn(format!("Cannot divide values of this type")))
            }
        },
        BinaryOperator::Modulo => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                // Signed remainder for integers
                Ok(builder.build_int_signed_rem(left_int, right_int, "modtmp").into())
            } else {
                Err(error_fn(format!("Modulo operation only supported for integers")))
            }
        },
        BinaryOperator::Equal => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_compare(inkwell::IntPredicate::EQ, left_int, right_int, "eqtmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_compare(inkwell::FloatPredicate::OEQ, left_float, right_float, "eqtmp").into())
            } else {
                Err(error_fn(format!("Cannot compare values of this type")))
            }
        },
        BinaryOperator::NotEqual => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_compare(inkwell::IntPredicate::NE, left_int, right_int, "netmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_compare(inkwell::FloatPredicate::ONE, left_float, right_float, "netmp").into())
            } else {
                Err(error_fn(format!("Cannot compare values of this type")))
            }
        },
        BinaryOperator::LessThan => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_compare(inkwell::IntPredicate::SLT, left_int, right_int, "lttmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_compare(inkwell::FloatPredicate::OLT, left_float, right_float, "lttmp").into())
            } else {
                Err(error_fn(format!("Cannot compare values of this type")))
            }
        },
        BinaryOperator::GreaterThan => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_compare(inkwell::IntPredicate::SGT, left_int, right_int, "gttmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_compare(inkwell::FloatPredicate::OGT, left_float, right_float, "gttmp").into())
            } else {
                Err(error_fn(format!("Cannot compare values of this type")))
            }
        },
        BinaryOperator::LessEqual => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_compare(inkwell::IntPredicate::SLE, left_int, right_int, "letmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_compare(inkwell::FloatPredicate::OLE, left_float, right_float, "letmp").into())
            } else {
                Err(error_fn(format!("Cannot compare values of this type")))
            }
        },
        BinaryOperator::GreaterEqual => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_int_compare(inkwell::IntPredicate::SGE, left_int, right_int, "getmp").into())
            } else if left.is_float_value() {
                let left_float = left.into_float_value();
                let right_float = right.into_float_value();
                Ok(builder.build_float_compare(inkwell::FloatPredicate::OGE, left_float, right_float, "getmp").into())
            } else {
                Err(error_fn(format!("Cannot compare values of this type")))
            }
        },
        BinaryOperator::And => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_and(left_int, right_int, "andtmp").into())
            } else {
                Err(error_fn(format!("Cannot perform logical AND on values of this type")))
            }
        },
        BinaryOperator::Or => {
            if left.is_int_value() {
                let left_int = left.into_int_value();
                let right_int = right.into_int_value();
                Ok(builder.build_or(left_int, right_int, "ortmp").into())
            } else {
                Err(error_fn(format!("Cannot perform logical OR on values of this type")))
            }
        },
    }
}

/// Maps Seen language unary operators to LLVM IR instructions
pub fn map_unary_operator<'ctx, F>(
    operator: &UnaryOperator,
    operand: BasicValueEnum<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    error_fn: F,
) -> Result<BasicValueEnum<'ctx>>
where
    F: FnOnce(String) -> CodeGenError,
{
    match operator {
        UnaryOperator::Negate => {
            if operand.is_int_value() {
                let operand_int = operand.into_int_value();
                Ok(builder.build_int_neg(operand_int, "negtmp").into())
            } else if operand.is_float_value() {
                let operand_float = operand.into_float_value();
                Ok(builder.build_float_neg(operand_float, "negtmp").into())
            } else {
                Err(error_fn(format!("Cannot negate values of this type")))
            }
        },
        UnaryOperator::Not => {
            if operand.is_int_value() {
                let operand_int = operand.into_int_value();
                Ok(builder.build_not(operand_int, "nottmp").into())
            } else {
                Err(error_fn(format!("Cannot apply logical NOT to values of this type")))
            }
        },
        UnaryOperator::Plus => {
            // Unary plus is a no-op, just return the operand
            Ok(operand)
        },
    }
}
