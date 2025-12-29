//! Statement generation for the IR generator.
//!
//! Handles let bindings, const bindings, assignments, and return statements.

use crate::{
    instruction::{BinaryOp, Instruction},
    value::IRValue,
    IRError, IRResult,
};
use seen_parser::{AssignmentOperator, Attribute, AttributeArgument, AttributeValue, Expression};
use std::fs;

use super::IRGenerator;

impl IRGenerator {
    // ==================== DRY Helpers ====================

    /// Core binding generation logic shared by let and const.
    /// Generates IR to store a value in a named variable.
    fn generate_binding_core(
        &mut self,
        name: &str,
        value_val: IRValue,
        instructions: &mut Vec<Instruction>,
    ) {
        let var_val = IRValue::Variable(name.to_string());
        instructions.push(Instruction::Store {
            value: value_val.clone(),
            dest: var_val,
        });
        self.context.define_variable(name, value_val);
    }

    /// Load embedded bytes from an attribute
    fn load_embed_bytes(&self, attr: &Attribute) -> IRResult<Vec<u8>> {
        let path = attr
            .args
            .iter()
            .find_map(|arg| match arg {
                AttributeArgument::Named { name, value } if name == "path" => {
                    if let AttributeValue::String(path) = value {
                        Some(path.clone())
                    } else {
                        None
                    }
                }
                AttributeArgument::Positional(AttributeValue::String(path)) => Some(path.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                IRError::Other("embed attribute requires a string `path` argument".to_string())
            })?;

        fs::read(&path).map_err(|err| {
            IRError::Other(format!("Failed to read embedded asset '{}': {}", path, err))
        })
    }

    // ==================== Public Generation Methods ====================

    /// Generate IR for let bindings
    pub(crate) fn generate_let_binding(
        &mut self,
        name: &str,
        value: &Expression,
        type_annotation: Option<&seen_parser::ast::Type>,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (value_val, mut instructions) = self.generate_expression(value)?;
        
        // If there's an explicit type annotation, use that type for the variable
        if let Some(ty) = type_annotation {
            let ir_type = self.convert_ast_type_to_ir(ty);
            self.context.set_variable_type(name.to_string(), ir_type.clone());
            
            // Also register the struct type name for field access tracking
            if let crate::value::IRType::Struct { name: struct_name, .. } = &ir_type {
                // Track this in local_variables for the LLVM backend
                if !self.context.local_variables.iter().any(|lv| lv.name == name) {
                    let local = crate::function::LocalVariable::new(name, ir_type);
                    self.context.local_variables.push(local);
                }
            }
        }
        
        self.generate_binding_core(name, value_val.clone(), &mut instructions);
        Ok((value_val, instructions))
    }

    /// Generate IR for const bindings
    pub(crate) fn generate_const_binding(
        &mut self,
        name: &str,
        value: &Expression,
        attributes: &[Attribute],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Handle embed attribute specially
        if let Some(embed_attr) = attributes.iter().find(|attr| attr.name == "embed") {
            let bytes = self.load_embed_bytes(embed_attr)?;
            let embed_value = IRValue::ByteArray(bytes);
            let mut instructions = Vec::new();
            self.generate_binding_core(name, embed_value.clone(), &mut instructions);
            return Ok((embed_value, instructions));
        }

        // For constants without embed attributes, use shared binding logic
        let (value_val, mut instructions) = self.generate_expression(value)?;
        self.generate_binding_core(name, value_val.clone(), &mut instructions);
        Ok((value_val, instructions))
    }

    /// Generate IR for assignment
    pub(crate) fn generate_assignment(
        &mut self,
        target: &Expression,
        value: &Expression,
        op: AssignmentOperator,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (rhs_val, mut instructions) = self.generate_expression(value)?;

        match target {
            Expression::Identifier { name, .. } => {
                // Check if this identifier is actually a field of the current receiver (implicit field access)
                let is_implicit_field = if let Some(recv_type) = &self.context._current_receiver_type {
                    if let crate::value::IRType::Struct { fields, .. } = recv_type {
                        fields.iter().any(|(f_name, _)| f_name == name)
                    } else {
                        false
                    }
                } else {
                    false
                };
                
                let mut assigned_value = rhs_val.clone();

                if !matches!(op, AssignmentOperator::Assign) {
                    let ir_op = match op {
                        AssignmentOperator::AddAssign => BinaryOp::Add,
                        AssignmentOperator::SubAssign => BinaryOp::Subtract,
                        AssignmentOperator::MulAssign => BinaryOp::Multiply,
                        AssignmentOperator::DivAssign => BinaryOp::Divide,
                        AssignmentOperator::ModAssign => BinaryOp::Modulo,
                        AssignmentOperator::Assign => unreachable!(),
                    };

                    let result_reg = self.context.allocate_register();
                    let result_val = IRValue::Register(result_reg);

                    // For implicit fields, use field access to get the current value
                    let left_val = if is_implicit_field {
                        let self_name = self.context._current_receiver_name.clone().unwrap_or_else(|| "self".to_string());
                        let field_reg = self.context.allocate_register();
                        let field_val = IRValue::Register(field_reg);
                        instructions.push(Instruction::FieldAccess {
                            struct_val: IRValue::Variable(self_name),
                            field: name.clone(),
                            result: field_val.clone(),
                            struct_type: None,
                            field_type: None,
                        });
                        field_val
                    } else {
                        IRValue::Variable(name.clone())
                    };

                    instructions.push(Instruction::Binary {
                        op: ir_op,
                        left: left_val,
                        right: rhs_val,
                        result: result_val.clone(),
                    });

                    assigned_value = result_val;
                }

                // Generate the appropriate store/field-set instruction
                if is_implicit_field {
                    let self_name = self.context._current_receiver_name.clone().unwrap_or_else(|| "self".to_string());
                    instructions.push(Instruction::FieldSet {
                        struct_val: IRValue::Variable(self_name),
                        field: name.clone(),
                        value: assigned_value.clone(),
                        struct_type: None,
                        field_type: None,
                    });
                } else {
                    instructions.push(Instruction::Store {
                        value: assigned_value.clone(),
                        dest: IRValue::Variable(name.clone()),
                    });
                }

                Ok((assigned_value, instructions))
            }
            Expression::IndexAccess { object, index, .. } => {
                if !matches!(op, AssignmentOperator::Assign) {
                    return Err(IRError::Other(
                        "Compound assignment not supported for index targets".to_string(),
                    ));
                }

                let (obj_val, obj_instructions) = self.generate_expression(object)?;
                let (idx_val, idx_instructions) = self.generate_expression(index)?;

                instructions.extend(obj_instructions);
                instructions.extend(idx_instructions);

                instructions.push(Instruction::ArraySet {
                    array: obj_val,
                    index: idx_val,
                    value: rhs_val.clone(),
                    element_type: None,
                });

                Ok((rhs_val, instructions))
            }
            Expression::MemberAccess { object, member, .. } => {
                if !matches!(op, AssignmentOperator::Assign) {
                    return Err(IRError::Other(
                        "Compound assignment not supported for member targets".to_string(),
                    ));
                }

                let (obj_val, obj_instructions) = self.generate_expression(object)?;
                instructions.extend(obj_instructions);

                instructions.push(Instruction::FieldSet {
                    struct_val: obj_val,
                    field: member.clone(),
                    value: rhs_val.clone(),
                    struct_type: None,
                    field_type: None,
                });

                Ok((rhs_val, instructions))
            }
            _ => Err(IRError::Other("Invalid assignment target".to_string())),
        }
    }

    /// Generate IR for return expressions
    pub(crate) fn generate_return_expression(
        &mut self,
        value: Option<&Expression>,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        if let Some(expr) = value {
            let (ret_val, expr_instructions) = self.generate_expression(expr)?;
            instructions.extend(expr_instructions);
            instructions.push(Instruction::Return(Some(ret_val.clone())));
            Ok((ret_val, instructions))
        } else {
            instructions.push(Instruction::Return(None));
            Ok((IRValue::Void, instructions))
        }
    }

    /// Generate IR for move expressions
    pub(crate) fn generate_move_expression(
        &mut self,
        operand: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (source_value, mut instructions) = self.generate_expression(operand)?;

        let dest_register = self.context.allocate_register();
        let dest_value = IRValue::Register(dest_register);

        let move_instruction = Instruction::Move {
            source: source_value.clone(),
            dest: dest_value.clone(),
        };
        instructions.push(move_instruction);

        // Track ownership transfer - source is now invalidated
        self.context.invalidate_value(source_value);

        Ok((dest_value, instructions))
    }

    /// Generate IR for borrow expressions
    pub(crate) fn generate_borrow_expression(
        &mut self,
        operand: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (source_value, mut instructions) = self.generate_expression(operand)?;

        let ref_register = self.context.allocate_register();
        let ref_value = IRValue::Register(ref_register);

        let borrow_instruction = Instruction::Load {
            source: IRValue::AddressOf(Box::new(source_value.clone())),
            dest: ref_value.clone(),
        };
        instructions.push(borrow_instruction);

        // Track borrow in IR metadata for lifetime validation
        self.context.track_borrow(source_value, ref_value.clone());

        Ok((ref_value, instructions))
    }

    /// Generate IR for comptime expressions
    pub(crate) fn generate_comptime_expression(
        &mut self,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Attempt compile-time evaluation of the expression
        match self.evaluate_at_compile_time(body) {
            Ok(constant_value) => {
                // Expression was successfully evaluated at compile time
                Ok((constant_value, Vec::new()))
            }
            Err(_) => {
                // Fall back to runtime evaluation
                let (value, mut instructions) = self.generate_expression(body)?;

                let comptime_register = self.context.allocate_register();
                let comptime_result = IRValue::Register(comptime_register);

                let comptime_instruction = Instruction::Load {
                    source: value,
                    dest: comptime_result.clone(),
                };
                instructions.push(comptime_instruction);

                Ok((comptime_result, instructions))
            }
        }
    }

    /// Evaluate an expression at compile time if possible
    pub(crate) fn evaluate_at_compile_time(&self, expr: &Expression) -> Result<IRValue, String> {
        match expr {
            Expression::IntegerLiteral { value, .. } => Ok(IRValue::Integer(*value)),
            Expression::FloatLiteral { value, .. } => Ok(IRValue::Float(*value)),
            Expression::BooleanLiteral { value, .. } => Ok(IRValue::Boolean(*value)),
            Expression::CharLiteral { value, .. } => Ok(IRValue::Char(*value)),
            Expression::StringLiteral { value, .. } => Ok(IRValue::String(value.clone())),

            Expression::BinaryOp {
                left, right, op, ..
            } => {
                let left_val = self.evaluate_at_compile_time(left)?;
                let right_val = self.evaluate_at_compile_time(right)?;
                self.evaluate_binary_operation(&left_val, &right_val, op)
            }

            Expression::UnaryOp { operand, op, .. } => {
                let operand_val = self.evaluate_at_compile_time(operand)?;
                self.evaluate_unary_operation(&operand_val, op)
            }

            _ => Err("Expression cannot be evaluated at compile time".to_string()),
        }
    }
}
