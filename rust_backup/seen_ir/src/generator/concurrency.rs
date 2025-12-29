//! Concurrency primitives for the IR generator.
//!
//! Handles async/await, channels, spawn, scope, and select expressions.

use crate::{
    instruction::{BinaryOp, IRSelectArm, Instruction, ScopeKind},
    value::IRValue,
    IRError, IRResult,
};
use seen_parser::{Expression, Pattern};

use super::IRGenerator;

/// Status code for successful channel receive in select
const SELECT_STATUS_RECEIVED: i64 = 0;

impl IRGenerator {
    // ==================== DRY Helpers ====================

    /// Emit a runtime function call with no result (fire-and-forget).
    fn emit_runtime_call_void(&self, name: &str, args: Vec<IRValue>) -> Instruction {
        Instruction::Call {
            target: IRValue::Function {
                name: name.to_string(),
                parameters: Vec::new(),
            },
            args,
            result: None,
            arg_types: None,
            return_type: None,
        }
    }

    /// Emit a runtime function call that returns a result.
    fn emit_runtime_call_with_result(&mut self, name: &str, args: Vec<IRValue>) -> (IRValue, Instruction) {
        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);
        let call = Instruction::Call {
            target: IRValue::Function {
                name: name.to_string(),
                parameters: Vec::new(),
            },
            args,
            result: Some(result_value.clone()),
            arg_types: None,
            return_type: None,
        };
        (result_value, call)
    }

    /// Generate a scope runtime call (__scope_push / __scope_pop).
    fn scope_runtime_call(&self, name: &str, kind: ScopeKind) -> Instruction {
        let kind_arg = match kind {
            ScopeKind::Task => 0,
            ScopeKind::Jobs => 1,
        };
        self.emit_runtime_call_void(name, vec![IRValue::Integer(kind_arg)])
    }

    // ==================== Public Generation Methods ====================

    /// Generate IR for await expressions
    pub(crate) fn generate_await_expression(
        &mut self,
        awaited: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (value, mut instructions) = self.generate_expression(awaited)?;
        let (result_value, call) = self.emit_runtime_call_with_result("__await", vec![value]);
        instructions.push(call);
        Ok((result_value, instructions))
    }

    /// Generate IR for select expressions (channel multiplexing)
    pub(crate) fn generate_select_expression(
        &mut self,
        cases: &[seen_parser::ast::SelectCase],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        if cases.is_empty() {
            return Err(IRError::Other(
                "Select expression must include at least one case".to_string(),
            ));
        }

        let mut instructions = Vec::new();
        let mut arms = Vec::with_capacity(cases.len());
        let mut lowered_cases = Vec::with_capacity(cases.len());

        for case in cases {
            let (channel_value, channel_instrs) = self.generate_expression(&case.channel)?;
            instructions.extend(channel_instrs);

            arms.push(IRSelectArm {
                channel: channel_value,
            });
            lowered_cases.push((case.pattern.clone(), (*case.handler).clone()));
        }

        // Optimize single wildcard case
        if lowered_cases.len() == 1 && matches!(lowered_cases[0].0, Pattern::Wildcard) {
            let channel = arms[0].channel.clone();
            instructions.push(Instruction::Call {
                target: IRValue::Function {
                    name: "seen_channel_recv".to_string(),
                    parameters: Vec::new(),
                },
                args: vec![channel],
                result: None,
                arg_types: None,
                return_type: None,
            });
            let (_, handler_expr) = lowered_cases.into_iter().next().unwrap();
            let (handler_value, handler_instrs) = self.generate_expression(&handler_expr)?;
            instructions.extend(handler_instrs);
            return Ok((handler_value, instructions));
        }

        // Allocate result registers
        let payload_reg = self.context.allocate_register();
        let payload_value = IRValue::Register(payload_reg);
        let index_reg = self.context.allocate_register();
        let index_value = IRValue::Register(index_reg);
        let status_reg = self.context.allocate_register();
        let status_value = IRValue::Register(status_reg);

        instructions.push(Instruction::ChannelSelect {
            cases: arms,
            payload_result: payload_value.clone(),
            index_result: index_value.clone(),
            status_result: status_value.clone(),
        });

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);
        instructions.push(Instruction::Move {
            source: IRValue::Integer(0),
            dest: result_value.clone(),
        });

        let end_label = self.context.allocate_label("select_end");

        // Generate case handlers
        for (case_index, (pattern, handler)) in lowered_cases.into_iter().enumerate() {
            let skip_label = self
                .context
                .allocate_label(&format!("select_skip_{}", case_index));

            // Check if this case index matches
            let idx_cmp_reg = self.context.allocate_register();
            let idx_cmp_value = IRValue::Register(idx_cmp_reg);
            instructions.push(Instruction::Binary {
                op: BinaryOp::Equal,
                left: index_value.clone(),
                right: IRValue::Integer(case_index as i64),
                result: idx_cmp_value.clone(),
            });

            // Check if receive was successful
            let status_cmp_reg = self.context.allocate_register();
            let status_cmp_value = IRValue::Register(status_cmp_reg);
            instructions.push(Instruction::Binary {
                op: BinaryOp::Equal,
                left: status_value.clone(),
                right: IRValue::Integer(SELECT_STATUS_RECEIVED),
                result: status_cmp_value.clone(),
            });

            // Combined condition
            let cond_reg = self.context.allocate_register();
            let cond_value = IRValue::Register(cond_reg);
            instructions.push(Instruction::Binary {
                op: BinaryOp::And,
                left: idx_cmp_value,
                right: status_cmp_value,
                result: cond_value.clone(),
            });

            instructions.push(Instruction::JumpIfNot {
                condition: cond_value,
                target: skip_label.clone(),
            });

            // Handle pattern binding
            match pattern {
                Pattern::Wildcard => {}
                Pattern::Identifier(name) => {
                    let binding_reg = self.context.allocate_register();
                    let binding_value = IRValue::Register(binding_reg);
                    instructions.push(Instruction::Move {
                        source: payload_value.clone(),
                        dest: binding_value.clone(),
                    });
                    self.context.define_variable(&name, binding_value);
                }
                other => {
                    return Err(IRError::Other(format!(
                        "select pattern {:?} is not yet supported for LLVM lowering",
                        other
                    )));
                }
            }

            // Generate handler
            let (handler_value, handler_instrs) = self.generate_expression(&handler)?;
            instructions.extend(handler_instrs);
            instructions.push(Instruction::Move {
                source: handler_value,
                dest: result_value.clone(),
            });
            instructions.push(Instruction::Jump(end_label.clone()));
            instructions.push(Instruction::Label(skip_label));
        }

        instructions.push(Instruction::Label(end_label));
        Ok((result_value, instructions))
    }

    /// Generate IR for scope expressions (task scope)
    pub(crate) fn generate_scope_expression(
        &mut self,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        self.generate_scope_with_kind(body, ScopeKind::Task)
    }

    /// Generate IR for jobs scope expressions
    pub(crate) fn generate_jobs_scope_expression(
        &mut self,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        self.generate_scope_with_kind(body, ScopeKind::Jobs)
    }

    /// Generate IR for scope with a specific kind
    fn generate_scope_with_kind(
        &mut self,
        body: &Expression,
        kind: ScopeKind,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        instructions.push(self.scope_runtime_call("__scope_push", kind));

        let (body_value, body_insts) = self.generate_expression(body)?;
        instructions.extend(body_insts);

        instructions.push(self.scope_runtime_call("__scope_pop", kind));
        Ok((body_value, instructions))
    }

    /// Generate IR for spawn expressions
    pub(crate) fn generate_spawn_expression(
        &mut self,
        expr: &Expression,
        detached: bool,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (_body_value, mut instructions) = self.generate_expression(expr)?;

        let runtime_name = if detached { "__spawn_detached" } else { "__spawn_task" };
        let (result_value, call) = self.emit_runtime_call_with_result(runtime_name, Vec::new());
        instructions.push(call);

        Ok((result_value, instructions))
    }

    /// Generate IR for send expressions (channel send)
    pub(crate) fn generate_send_expression(
        &mut self,
        message: &Expression,
        target: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (msg_val, mut instructions) = self.generate_expression(message)?;
        let (target_val, target_instrs) = self.generate_expression(target)?;
        instructions.extend(target_instrs);

        let (result_value, call) = self.emit_runtime_call_with_result(
            "seen_channel_send",
            vec![target_val, msg_val],
        );
        instructions.push(call);

        Ok((result_value, instructions))
    }
}
