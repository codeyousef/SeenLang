//! Binary and unary operation generation for the IR generator.
//!
//! Handles arithmetic, logical, and bitwise operations.

use crate::{
    instruction::{BinaryOp, Instruction, UnaryOp},
    value::{IRType, IRValue},
    IRError, IRResult,
};
use seen_parser::{BinaryOperator, Expression, UnaryOperator};

use super::IRGenerator;

impl IRGenerator {
    /// Generate IR for binary expressions
    pub(crate) fn generate_binary_expression(
        &mut self,
        left: &Expression,
        operator: &BinaryOperator,
        right: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (left_val, mut left_instructions) = self.generate_expression(left)?;
        let (right_val, right_instructions) = self.generate_expression(right)?;

        left_instructions.extend(right_instructions);

        // Helper: detect string-typed expressions (basic cases)
        let is_string_expr = |expr: &Expression| -> bool {
            match expr {
                Expression::StringLiteral { .. } | Expression::InterpolatedString { .. } => true,
                Expression::Identifier { name, .. } => self
                    .context
                    .get_variable_type(name)
                    .map(|t| matches!(t, IRType::String))
                    .unwrap_or(false),
                _ => false,
            }
        };

        // Special-case string concatenation for '+'
        if matches!(operator, BinaryOperator::Add)
            && (is_string_expr(left) || is_string_expr(right))
        {
            let result_reg = self.context.allocate_register();
            let result_value = IRValue::Register(result_reg);
            left_instructions.push(Instruction::StringConcat {
                left: left_val,
                right: right_val,
                result: result_value.clone(),
            });
            return Ok((result_value, left_instructions));
        }

        let op = match operator {
            BinaryOperator::Add => BinaryOp::Add,
            BinaryOperator::Subtract => BinaryOp::Subtract,
            BinaryOperator::Multiply => BinaryOp::Multiply,
            BinaryOperator::Divide => BinaryOp::Divide,
            BinaryOperator::Modulo => BinaryOp::Modulo,
            BinaryOperator::Equal => BinaryOp::Equal,
            BinaryOperator::NotEqual => BinaryOp::NotEqual,
            BinaryOperator::Less => BinaryOp::LessThan,
            BinaryOperator::LessEqual => BinaryOp::LessEqual,
            BinaryOperator::Greater => BinaryOp::GreaterThan,
            BinaryOperator::GreaterEqual => BinaryOp::GreaterEqual,
            BinaryOperator::And => BinaryOp::And,
            BinaryOperator::Or => BinaryOp::Or,
            BinaryOperator::BitwiseAnd => BinaryOp::BitwiseAnd,
            BinaryOperator::BitwiseOr => BinaryOp::BitwiseOr,
            BinaryOperator::BitwiseXor => BinaryOp::BitwiseXor,
            BinaryOperator::LeftShift => BinaryOp::LeftShift,
            BinaryOperator::RightShift => BinaryOp::RightShift,
            BinaryOperator::InclusiveRange => {
                return Err(IRError::Other(
                    "Range operators not yet implemented".to_string(),
                ))
            }
            BinaryOperator::ExclusiveRange => {
                return Err(IRError::Other(
                    "Range operators not yet implemented".to_string(),
                ))
            }
        };

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        let binary_instruction = Instruction::Binary {
            op,
            left: left_val,
            right: right_val,
            result: result_value.clone(),
        };

        left_instructions.push(binary_instruction);

        Ok((result_value, left_instructions))
    }

    /// Generate IR for unary expressions
    pub(crate) fn generate_unary_expression(
        &mut self,
        operator: &UnaryOperator,
        operand: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (operand_val, mut instructions) = self.generate_expression(operand)?;

        let op = match operator {
            UnaryOperator::Negate => UnaryOp::Negate,
            UnaryOperator::Not => UnaryOp::Not,
            UnaryOperator::BitwiseNot => UnaryOp::BitwiseNot,
        };

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        let unary_instruction = Instruction::Unary {
            op,
            operand: operand_val,
            result: result_value.clone(),
        };

        instructions.push(unary_instruction);

        Ok((result_value, instructions))
    }

    /// Evaluate binary operations at compile time
    pub(crate) fn evaluate_binary_operation(
        &self,
        left: &IRValue,
        right: &IRValue,
        op: &BinaryOperator,
    ) -> Result<IRValue, String> {
        match (left, right, op) {
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::Add) => {
                Ok(IRValue::Integer(a + b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::Subtract) => {
                Ok(IRValue::Integer(a - b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::Multiply) => {
                Ok(IRValue::Integer(a * b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::Divide) => {
                if *b == 0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(IRValue::Integer(a / b))
                }
            }
            (IRValue::Float(a), IRValue::Float(b), BinaryOperator::Add) => {
                Ok(IRValue::Float(a + b))
            }
            (IRValue::Float(a), IRValue::Float(b), BinaryOperator::Subtract) => {
                Ok(IRValue::Float(a - b))
            }
            (IRValue::Float(a), IRValue::Float(b), BinaryOperator::Multiply) => {
                Ok(IRValue::Float(a * b))
            }
            (IRValue::Float(a), IRValue::Float(b), BinaryOperator::Divide) => {
                Ok(IRValue::Float(a / b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::Equal) => {
                Ok(IRValue::Boolean(a == b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::NotEqual) => {
                Ok(IRValue::Boolean(a != b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::Less) => {
                Ok(IRValue::Boolean(a < b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::Greater) => {
                Ok(IRValue::Boolean(a > b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::LessEqual) => {
                Ok(IRValue::Boolean(a <= b))
            }
            (IRValue::Integer(a), IRValue::Integer(b), BinaryOperator::GreaterEqual) => {
                Ok(IRValue::Boolean(a >= b))
            }
            _ => Err(format!(
                "Cannot evaluate binary operation: {:?} {:?} {:?}",
                left, op, right
            )),
        }
    }

    /// Evaluate unary operations at compile time
    pub(crate) fn evaluate_unary_operation(
        &self,
        operand: &IRValue,
        op: &UnaryOperator,
    ) -> Result<IRValue, String> {
        match (operand, op) {
            (IRValue::Integer(a), UnaryOperator::Negate) => Ok(IRValue::Integer(-a)),
            (IRValue::Float(a), UnaryOperator::Negate) => Ok(IRValue::Float(-a)),
            (IRValue::Boolean(a), UnaryOperator::Not) => Ok(IRValue::Boolean(!a)),
            _ => Err(format!(
                "Cannot evaluate unary operation: {:?} {:?}",
                op, operand
            )),
        }
    }
}
