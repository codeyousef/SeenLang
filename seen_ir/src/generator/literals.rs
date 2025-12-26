//! Literal value generation for the IR generator.
//!
//! Handles conversion of AST literal expressions to IR values.

use crate::{instruction::Instruction, value::IRValue, IRResult};
use seen_parser::Expression;

use super::IRGenerator;

impl IRGenerator {
    /// Generate IR for variable access
    pub(crate) fn generate_variable(&mut self, name: &str) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Check if variable is a local variable or argument
        if self.context.get_variable_type(name).is_some() {
            // If name is "self" but "this" also exists (and likely is the canonical one), prefer "this"
            let var_name = if name == "self" && self.context.get_variable_type("this").is_some() {
                "this"
            } else {
                name
            };
            let value = IRValue::Variable(var_name.to_string());
            return Ok((value, vec![]));
        }

        // Check for implicit field access on 'self'
        let field_info = if let Some(recv_type) = &self.context._current_receiver_type {
            if let crate::value::IRType::Struct { name: _struct_name, fields } = recv_type {
                // Check if 'name' is a field
                fields.iter().find(|(f_name, _)| f_name == name).map(|(_, f_type)| f_type.clone())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(field_type) = field_info {
            // It is a field! Generate self.name
            let self_name = self.context._current_receiver_name.clone().unwrap_or_else(|| "self".to_string());
            let self_var = IRValue::Variable(self_name);
            
            let result_reg = self.context.allocate_register();
            let result_val = IRValue::Register(result_reg);
            
            self.context.set_register_type(result_reg, field_type);
            
            let instructions = vec![Instruction::FieldAccess {
                struct_val: self_var,
                field: name.to_string(),
                result: result_val.clone(),
            }];
            
            return Ok((result_val, instructions));
        }

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
