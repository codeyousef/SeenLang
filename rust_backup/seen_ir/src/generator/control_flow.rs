//! Control flow generation for the IR generator.
//!
//! Handles if, while, for, break, continue, and match expressions.

use crate::{
    instruction::{BinaryOp, Instruction, Label},
    value::IRValue,
    IRError, IRResult,
};
use seen_parser::{BinaryOperator, Expression, ForBinding, Pattern};

use super::IRGenerator;

impl IRGenerator {
    // ==================== DRY Helpers ====================

    /// Generate the core range-based for loop structure.
    /// `inclusive` determines whether to use LessEqual (inclusive) or LessThan (exclusive).
    fn generate_range_loop(
        &mut self,
        variable: &str,
        start_val: IRValue,
        end_val: IRValue,
        body: &Expression,
        inclusive: bool,
        instructions: &mut Vec<Instruction>,
    ) -> IRResult<()> {
        // Initialize loop variable
        let loop_var = IRValue::Variable(variable.to_string());
        instructions.push(Instruction::Store {
            value: start_val,
            dest: loop_var.clone(),
        });

        // Allocate labels
        let loop_start = self.context.allocate_label("for_start");
        let loop_body_label = self.context.allocate_label("for_body");
        let loop_end = self.context.allocate_label("for_end");

        self.context
            .push_loop_labels(loop_end.0.clone(), loop_start.0.clone());

        instructions.push(Instruction::Label(loop_start.clone()));

        // Check loop condition
        let cond_reg = self.context.allocate_register();
        let cond_result = IRValue::Register(cond_reg);
        let comparison_op = if inclusive {
            BinaryOp::LessEqual
        } else {
            BinaryOp::LessThan
        };

        instructions.push(Instruction::Binary {
            op: comparison_op,
            left: loop_var.clone(),
            right: end_val,
            result: cond_result.clone(),
        });

        instructions.push(Instruction::JumpIfNot {
            condition: cond_result,
            target: loop_end.clone(),
        });

        // Loop body
        instructions.push(Instruction::Label(loop_body_label));
        let (_, body_instrs) = self.generate_expression(body)?;
        instructions.extend(body_instrs);

        // Increment loop variable
        let inc_reg = self.context.allocate_register();
        let inc_result = IRValue::Register(inc_reg);
        instructions.push(Instruction::Binary {
            op: BinaryOp::Add,
            left: loop_var.clone(),
            right: IRValue::Integer(1),
            result: inc_result.clone(),
        });
        instructions.push(Instruction::Store {
            value: inc_result,
            dest: loop_var,
        });

        // Jump back to start
        instructions.push(Instruction::Jump(loop_start));
        instructions.push(Instruction::Label(loop_end));

        self.context.pop_loop_labels();
        Ok(())
    }

    // ==================== Public Generation Methods ====================

    /// Generate IR for if expressions
    pub(crate) fn generate_if_expression(
        &mut self,
        condition: &Expression,
        then_branch: &Expression,
        else_branch: Option<&Expression>,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (cond_val, mut instructions) = self.generate_expression(condition)?;

        let then_label = self.context.allocate_label("then");
        let else_label = self.context.allocate_label("else");
        let end_label = self.context.allocate_label("if_end");

        // Jump to then or else based on condition
        instructions.push(Instruction::JumpIf {
            condition: cond_val,
            target: then_label.clone(),
        });
        instructions.push(Instruction::Jump(else_label.clone()));

        // Then block
        instructions.push(Instruction::Label(then_label));
        let (then_val, then_instructions) = self.generate_expression(then_branch)?;
        instructions.extend(then_instructions);

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        instructions.push(Instruction::Move {
            source: then_val,
            dest: result_value.clone(),
        });
        instructions.push(Instruction::Jump(end_label.clone()));

        // Else block
        instructions.push(Instruction::Label(else_label));
        if let Some(else_expr) = else_branch {
            let (else_val, else_instructions) = self.generate_expression(else_expr)?;
            instructions.extend(else_instructions);
            instructions.push(Instruction::Move {
                source: else_val,
                dest: result_value.clone(),
            });
        } else {
            // No else branch, use unit value
            instructions.push(Instruction::Move {
                source: IRValue::Void,
                dest: result_value.clone(),
            });
        }

        instructions.push(Instruction::Label(end_label));

        Ok((result_value, instructions))
    }

    /// Generate IR for while loops
    pub(crate) fn generate_while_expression(
        &mut self,
        condition: &Expression,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        let loop_start = self.context.allocate_label("loop_start");
        let loop_body = self.context.allocate_label("loop_body");
        let loop_end = self.context.allocate_label("loop_end");

        // Push loop labels for break/continue
        self.context
            .push_loop_labels(loop_end.0.clone(), loop_start.0.clone());

        instructions.push(Instruction::Label(loop_start.clone()));

        // Generate condition
        let (cond_val, cond_instructions) = self.generate_expression(condition)?;
        instructions.extend(cond_instructions);

        // Jump to end if condition is false, otherwise continue to body
        instructions.push(Instruction::JumpIfNot {
            condition: cond_val,
            target: loop_end.clone(),
        });

        // Generate body
        instructions.push(Instruction::Label(loop_body));
        let (_, body_instructions) = self.generate_expression(body)?;
        instructions.extend(body_instructions);

        // Jump back to start
        instructions.push(Instruction::Jump(loop_start));

        instructions.push(Instruction::Label(loop_end.clone()));

        // Pop loop labels
        self.context.pop_loop_labels();

        // While loops return unit value
        Ok((IRValue::Void, instructions))
    }

    /// Generate IR for for loops
    pub(crate) fn generate_for_expression(
        &mut self,
        binding: &ForBinding,
        iterable: &Expression,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        let variable = match binding {
            ForBinding::Identifier(name) => name,
            ForBinding::Tuple(_) => {
                // Tuple destructuring in for loops isn't supported in the bootstrap IR path.
                return Ok((IRValue::Void, Vec::new()));
            }
        };

        // Handle range expressions (binary operator syntax) and range() function calls
        match iterable {
            Expression::Call { callee, args, .. } => {
                // Check if this is a range() function call
                if let Expression::Identifier { name, .. } = callee.as_ref() {
                    if name == "range" && args.len() == 2 {
                        // Extract start and end arguments from range(start, end)
                        let (start_val, start_instructions) = self.generate_expression(&args[0])?;
                        let (end_val, end_instructions) = self.generate_expression(&args[1])?;

                        instructions.extend(start_instructions);
                        instructions.extend(end_instructions);

                        // Generate range loop (range() is exclusive)
                        self.generate_range_loop(
                            variable,
                            start_val,
                            end_val,
                            body,
                            false, // range() is exclusive
                            &mut instructions,
                        )?;

                        Ok((IRValue::Void, instructions))
                    } else {
                        // Fallback: unsupported iterable in bootstrap mode – skip loop body
                        return Ok((IRValue::Void, Vec::new()));
                    }
                } else {
                    // Fallback: unsupported iterable in bootstrap mode – skip loop body
                    return Ok((IRValue::Void, Vec::new()));
                }
            }
            Expression::BinaryOp {
                left, op, right, ..
            } => {
                match op {
                    BinaryOperator::InclusiveRange | BinaryOperator::ExclusiveRange => {
                        // Get range bounds
                        let (start_val, start_instructions) = self.generate_expression(left)?;
                        let (end_val, end_instructions) = self.generate_expression(right)?;

                        instructions.extend(start_instructions);
                        instructions.extend(end_instructions);

                        // Generate range loop (inclusive for ..=, exclusive for ..)
                        self.generate_range_loop(
                            variable,
                            start_val,
                            end_val,
                            body,
                            matches!(op, BinaryOperator::InclusiveRange),
                            &mut instructions,
                        )?;

                        Ok((IRValue::Void, instructions))
                    }
                    _ => Ok((IRValue::Void, Vec::new())),
                }
            }
            _ => Ok((IRValue::Void, Vec::new())),
        }
    }

    /// Generate IR for break expressions
    pub(crate) fn generate_break_expression(
        &mut self,
        value: Option<&Expression>,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        let result_value = if let Some(expr) = value {
            let (val, expr_instructions) = self.generate_expression(expr)?;
            instructions.extend(expr_instructions);
            val
        } else {
            IRValue::Void
        };

        if let Some(break_label) = self.context.current_break_label() {
            instructions.push(Instruction::Jump(Label::new(break_label.clone())));
        } else {
            return Err(IRError::Other("Break outside of loop".to_string()));
        }

        Ok((result_value, instructions))
    }

    /// Generate IR for continue expressions
    pub(crate) fn generate_continue_expression(&mut self) -> IRResult<(IRValue, Vec<Instruction>)> {
        if let Some(continue_label) = self.context.current_continue_label() {
            Ok((
                IRValue::Void,
                vec![Instruction::Jump(Label::new(continue_label.clone()))],
            ))
        } else {
            Err(IRError::Other("Continue outside of loop".to_string()))
        }
    }

    /// Generate IR for match expressions
    pub(crate) fn generate_match_expression(
        &mut self,
        expr: &Expression,
        arms: &[seen_parser::MatchArm],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (match_val, mut instructions) = self.generate_expression(expr)?;

        if arms.is_empty() {
            return Ok((IRValue::Void, instructions));
        }

        // Allocate result register
        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        // Generate labels for each arm and the end
        let mut arm_labels = Vec::new();
        for i in 0..arms.len() {
            arm_labels.push(self.context.allocate_label(&format!("match_arm_{}", i)));
        }
        let end_label = self.context.allocate_label("match_end");

        // Generate pattern matching logic (check each pattern in sequence)
        for (i, arm) in arms.iter().enumerate() {
            let arm_label = &arm_labels[i];

            match &arm.pattern {
                Pattern::Literal(literal) => {
                    // Generate comparison for literal pattern
                    let (literal_val, literal_instructions) = self.generate_expression(literal)?;
                    instructions.extend(literal_instructions);

                    let cmp_reg = self.context.allocate_register();
                    let cmp_result = IRValue::Register(cmp_reg);

                    instructions.push(Instruction::Binary {
                        op: BinaryOp::Equal,
                        left: match_val.clone(),
                        right: literal_val,
                        result: cmp_result.clone(),
                    });

                    // If this pattern matches, jump to its arm
                    instructions.push(Instruction::JumpIf {
                        condition: cmp_result,
                        target: arm_label.clone(),
                    });
                }
                Pattern::Wildcard => {
                    // Wildcard always matches, jump directly to this arm
                    instructions.push(Instruction::Jump(arm_label.clone()));
                    break;
                }
                Pattern::Identifier(name) => {
                    // Identifier pattern always matches and binds the value
                    let binding_register = self.context.allocate_register();
                    let binding_value = IRValue::Register(binding_register);

                    instructions.push(Instruction::Move {
                        source: match_val.clone(),
                        dest: binding_value.clone(),
                    });

                    self.context.define_variable(name, binding_value);

                    instructions.push(Instruction::Jump(arm_label.clone()));
                    break;
                }
                Pattern::Enum {
                    enum_name,
                    variant,
                    fields,
                } => {
                    // Extract the tag from the enum value
                    let tag_reg = self.context.allocate_register();
                    let tag_val = IRValue::Register(tag_reg);

                    instructions.push(Instruction::GetEnumTag {
                        enum_value: match_val.clone(),
                        result: tag_val.clone(),
                    });

                    // Compare with the expected variant tag
                    let variant_tag = self
                        .context
                        .get_enum_variant_tag(enum_name, variant)
                        .map_err(|e| IRError::Other(e))?;

                    let cmp_reg = self.context.allocate_register();
                    let cmp_result = IRValue::Register(cmp_reg);

                    instructions.push(Instruction::Binary {
                        op: BinaryOp::Equal,
                        left: tag_val,
                        right: variant_tag,
                        result: cmp_result.clone(),
                    });

                    if !fields.is_empty() {
                        let skip_label = self.context.create_label("skip_extract");

                        instructions.push(Instruction::JumpIfNot {
                            condition: cmp_result.clone(),
                            target: skip_label.clone(),
                        });

                        // Extract fields and bind to variables
                        for (i, field_pattern) in fields.iter().enumerate() {
                            if let Pattern::Identifier(name) = &**field_pattern {
                                let field_reg = self.context.allocate_register();
                                let field_val = IRValue::Register(field_reg);

                                instructions.push(Instruction::GetEnumField {
                                    enum_value: match_val.clone(),
                                    field_index: i as u32,
                                    result: field_val.clone(),
                                });

                                self.context.define_variable(name, field_val);
                            }
                        }

                        instructions.push(Instruction::Jump(arm_label.clone()));
                        instructions.push(Instruction::Label(skip_label));
                    } else {
                        instructions.push(Instruction::JumpIf {
                            condition: cmp_result,
                            target: arm_label.clone(),
                        });
                    }
                }
                _ => {
                    return Err(IRError::Other(
                        "Complex patterns not yet implemented".to_string(),
                    ))
                }
            }
        }

        // If no patterns matched, jump to end
        if !arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Wildcard))
        {
            instructions.push(Instruction::Jump(end_label.clone()));
        }

        // Generate code for each arm
        for (i, arm) in arms.iter().enumerate() {
            let arm_label = &arm_labels[i];
            instructions.push(Instruction::Label(arm_label.clone()));

            let (arm_val, arm_instructions) = self.generate_expression(&arm.body)?;
            instructions.extend(arm_instructions);

            instructions.push(Instruction::Move {
                source: arm_val,
                dest: result_value.clone(),
            });

            instructions.push(Instruction::Jump(end_label.clone()));
        }

        instructions.push(Instruction::Label(end_label));

        Ok((result_value, instructions))
    }

    /// Generate IR for block expressions
    pub(crate) fn generate_block_expression(
        &mut self,
        expressions: &[Expression],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        let mut result_value = IRValue::Void;

        for expr in expressions {
            let (val, expr_instructions) = self.generate_expression(expr)?;
            instructions.extend(expr_instructions);
            result_value = val;
        }

        Ok((result_value, instructions))
    }
}
