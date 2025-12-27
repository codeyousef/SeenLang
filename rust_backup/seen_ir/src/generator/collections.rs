//! Collection operations for the IR generator.
//!
//! Handles array/struct literals, index access, member access, and string interpolation.

use crate::{instruction::Instruction, value::{IRValue, IRType}, IRResult};
use indexmap::IndexMap;
use seen_parser::Expression;

use super::IRGenerator;

impl IRGenerator {
    /// Generate IR for array indexing
    pub(crate) fn generate_index_access(
        &mut self,
        object: &Expression,
        index: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (obj_val, mut obj_instructions) = self.generate_expression(object)?;
        let (idx_val, idx_instructions) = self.generate_expression(index)?;

        obj_instructions.extend(idx_instructions);

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        let access_instruction = Instruction::ArrayAccess {
            array: obj_val,
            index: idx_val,
            result: result_value.clone(),
        };

        obj_instructions.push(access_instruction);

        Ok((result_value, obj_instructions))
    }

    /// Generate IR for member access
    pub(crate) fn generate_member_access(
        &mut self,
        object: &Expression,
        member: &str,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (obj_val, mut obj_instructions) = self.generate_expression(object)?;

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        // Try to determine the field type from the object's struct type
        // First, get the struct type name of the object
        let obj_type_name = match &obj_val {
            IRValue::Register(reg) => {
                match self.context.register_types.get(reg) {
                    Some(IRType::Struct { name, .. }) => Some(name.clone()),
                    _ => None,
                }
            }
            IRValue::Variable(var_name) => {
                match self.context.get_variable_type(var_name) {
                    Some(IRType::Struct { name, .. }) => Some(name.clone()),
                    _ => None,
                }
            }
            _ => None
        };
        
        eprintln!("DEBUG IR: generate_member_access obj_val={:?}, member={}, obj_type_name={:?}", 
            obj_val, member, obj_type_name);

        // If we know the struct type, look up the field type and set it on the result register
        if let Some(type_name) = obj_type_name {
            // Clone what we need from type_definitions to avoid borrow conflicts
            let field_type_opt = if let Some(type_def) = self.context.type_definitions.get(&type_name) {
                if let IRType::Struct { fields, .. } = type_def {
                    fields.iter()
                        .find(|(field_name, _)| field_name == member)
                        .map(|(_, field_type)| field_type.clone())
                } else {
                    None
                }
            } else {
                None
            };
            
            if let Some(field_type) = field_type_opt {
                self.context.set_register_type(result_reg, field_type.clone());
                
                // Track container element types for Vec<T>, Option<T>, etc.
                // This allows Vec.get() and Option.unwrap() to resolve inner types
                match &field_type {
                    IRType::Array(inner) => {
                        // For Vec<T>, track the element type T
                        self.context.container_element_types.insert(
                            format!("reg_{}", result_reg),
                            (**inner).clone()
                        );
                        eprintln!("DEBUG IR: Field access {}.{} - tracking Vec element type {:?}", 
                            type_name, member, inner);
                    }
                    IRType::Optional(inner) => {
                        // For Option<T>, track the inner type T
                        self.context.container_element_types.insert(
                            format!("reg_{}", result_reg),
                            (**inner).clone()
                        );
                    }
                    _ => {}
                }
            }
        }

        let access_instruction = Instruction::FieldAccess {
            struct_val: obj_val,
            field: member.to_string(),
            result: result_value.clone(),
        };

        obj_instructions.push(access_instruction);

        Ok((result_value, obj_instructions))
    }

    /// Generate IR for array literals
    pub(crate) fn generate_array_literal(
        &mut self,
        elements: &[Expression],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        let mut element_values = Vec::new();

        // Generate IR for all elements
        for element in elements {
            let (elem_val, elem_instructions) = self.generate_expression(element)?;
            instructions.extend(elem_instructions);
            element_values.push(elem_val);
        }

        let result_value = IRValue::Array(element_values);
        Ok((result_value, instructions))
    }

    /// Generate IR for struct literals
    pub(crate) fn generate_struct_literal(
        &mut self,
        name: &str,
        fields: &[(String, Expression)],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        let mut field_values: IndexMap<String, IRValue> = IndexMap::new();

        // Generate IR for all field values
        for (field_name, field_expr) in fields {
            let (field_val, field_instructions) = self.generate_expression(field_expr)?;
            instructions.extend(field_instructions);
            field_values.insert(field_name.clone(), field_val);
        }

        let result_value = IRValue::Struct {
            type_name: name.to_string(),
            fields: field_values,
        };

        Ok((result_value, instructions))
    }

    /// Generate IR for string interpolation
    pub(crate) fn generate_string_interpolation(
        &mut self,
        parts: &[seen_parser::InterpolationPart],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        let result_reg = self.context.allocate_register();
        let mut result_value = IRValue::Register(result_reg);

        // Initialize with empty string
        instructions.push(Instruction::Move {
            source: IRValue::String(String::new()),
            dest: result_value.clone(),
        });

        for part in parts {
            match &part.kind {
                seen_parser::InterpolationKind::Text(text) => {
                    let text_value = IRValue::String(text.clone());
                    let new_reg = self.context.allocate_register();
                    let new_result = IRValue::Register(new_reg);

                    instructions.push(Instruction::StringConcat {
                        left: result_value.clone(),
                        right: text_value,
                        result: new_result.clone(),
                    });

                    result_value = new_result;
                }
                seen_parser::InterpolationKind::Expression(expr) => {
                    let (expr_val, expr_instructions) = self.generate_expression(expr)?;
                    instructions.extend(expr_instructions);

                    let new_reg = self.context.allocate_register();
                    let new_result = IRValue::Register(new_reg);

                    instructions.push(Instruction::StringConcat {
                        left: result_value.clone(),
                        right: expr_val,
                        result: new_result.clone(),
                    });

                    result_value = new_result;
                }
            }
        }

        Ok((result_value, instructions))
    }
}
