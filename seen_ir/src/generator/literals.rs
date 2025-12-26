//! Literal value generation for the IR generator.
//!
//! Handles conversion of AST literal expressions to IR values.

use crate::{instruction::Instruction, value::IRValue, IRResult};
use seen_parser::Expression;

use super::IRGenerator;

impl IRGenerator {
    /// Generate IR for variable access
    pub(crate) fn generate_variable(&mut self, name: &str) -> IRResult<(IRValue, Vec<Instruction>)> {
        let value = IRValue::Variable(name.to_string());
        Ok((value, vec![]))
    }

    /// Generate IR for integer literal
    #[allow(dead_code)]
    pub(crate) fn generate_integer_literal(&self, value: i64) -> IRResult<(IRValue, Vec<Instruction>)> {
        Ok((IRValue::Integer(value), Vec::new()))
    }

    /// Generate IR for float literal
    #[allow(dead_code)]
    pub(crate) fn generate_float_literal(&self, value: f64) -> IRResult<(IRValue, Vec<Instruction>)> {
        Ok((IRValue::Float(value), Vec::new()))
    }

    /// Generate IR for string literal
    #[allow(dead_code)]
    pub(crate) fn generate_string_literal(&self, value: &str) -> IRResult<(IRValue, Vec<Instruction>)> {
        Ok((IRValue::String(value.to_string()), Vec::new()))
    }

    /// Generate IR for char literal
    #[allow(dead_code)]
    pub(crate) fn generate_char_literal(&self, value: char) -> IRResult<(IRValue, Vec<Instruction>)> {
        Ok((IRValue::Char(value), Vec::new()))
    }

    /// Generate IR for boolean literal
    #[allow(dead_code)]
    pub(crate) fn generate_boolean_literal(&self, value: bool) -> IRResult<(IRValue, Vec<Instruction>)> {
        Ok((IRValue::Boolean(value), Vec::new()))
    }

    /// Generate IR for null literal
    #[allow(dead_code)]
    pub(crate) fn generate_null_literal(&self) -> IRResult<(IRValue, Vec<Instruction>)> {
        Ok((IRValue::Null, Vec::new()))
    }

    /// Generate IR for enum literals
    pub(crate) fn generate_enum_literal(
        &mut self,
        enum_name: &str,
        variant_name: &str,
        fields: &[Expression],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        // Generate field values
        let mut field_values = Vec::new();
        for field_expr in fields {
            let (field_val, field_instructions) = self.generate_expression(field_expr)?;
            instructions.extend(field_instructions);
            field_values.push(field_val);
        }

        // Create enum value with variant name and fields
        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        instructions.push(Instruction::ConstructEnum {
            enum_name: enum_name.to_string(),
            variant_name: variant_name.to_string(),
            fields: field_values,
            result: result_value.clone(),
        });

        Ok((result_value, instructions))
    }
}
