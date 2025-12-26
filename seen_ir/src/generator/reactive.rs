//! Reactive programming constructs for the IR generator.
//!
//! Handles flow, observable, reactive property, and stream operations.

use crate::{instruction::Instruction, value::IRValue, IRResult};
use seen_parser::Expression;

use super::IRGenerator;

impl IRGenerator {
    /// Generate IR for flow creation
    pub(crate) fn generate_flow_creation(
        &mut self,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        // Create flow object - simplified implementation
        let flow_register = self.context.allocate_register();
        let result = IRValue::Register(flow_register);

        // Generate body as a closure/function
        let (body_val, body_instructions) = self.generate_expression(body)?;
        instructions.extend(body_instructions);

        // Create flow constructor call
        let flow_constructor = IRValue::GlobalVariable("Flow::new".to_string());
        let call = Instruction::Call {
            target: flow_constructor,
            args: vec![body_val],
            result: Some(result.clone()),
        };
        instructions.push(call);

        Ok((result, instructions))
    }

    /// Generate IR for observable creation
    pub(crate) fn generate_observable_creation(
        &mut self,
        source: &seen_parser::ast::ObservableSource,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        let observable_register = self.context.allocate_register();
        let result = IRValue::Register(observable_register);

        match source {
            seen_parser::ast::ObservableSource::Range { start, end, step } => {
                let (start_val, start_instructions) = self.generate_expression(start)?;
                let (end_val, end_instructions) = self.generate_expression(end)?;
                instructions.extend(start_instructions);
                instructions.extend(end_instructions);

                let mut args = vec![start_val, end_val];
                if let Some(step_expr) = step {
                    let (step_val, step_instructions) = self.generate_expression(step_expr)?;
                    instructions.extend(step_instructions);
                    args.push(step_val);
                }

                let constructor = IRValue::GlobalVariable("Observable::Range".to_string());
                let call = Instruction::Call {
                    target: constructor,
                    args,
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::ObservableSource::FromArray(array_expr) => {
                let (array_val, array_instructions) = self.generate_expression(array_expr)?;
                instructions.extend(array_instructions);

                let constructor = IRValue::GlobalVariable("Observable::FromArray".to_string());
                let call = Instruction::Call {
                    target: constructor,
                    args: vec![array_val],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::ObservableSource::FromEvent(event_name) => {
                let event_string = IRValue::String(event_name.clone());
                let constructor = IRValue::GlobalVariable("Observable::FromEvent".to_string());
                let call = Instruction::Call {
                    target: constructor,
                    args: vec![event_string],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::ObservableSource::Interval(duration) => {
                let duration_val = IRValue::Integer(*duration as i64);
                let constructor = IRValue::GlobalVariable("Observable::Interval".to_string());
                let call = Instruction::Call {
                    target: constructor,
                    args: vec![duration_val],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
        }

        Ok((result, instructions))
    }

    /// Generate IR for reactive property
    pub(crate) fn generate_reactive_property(
        &mut self,
        name: &str,
        value: &Expression,
        is_computed: bool,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        let (value_val, value_instructions) = self.generate_expression(value)?;
        instructions.extend(value_instructions);

        let prop_register = self.context.allocate_register();
        let result = IRValue::Register(prop_register);

        let constructor = if is_computed {
            "ReactiveProperty::Computed"
        } else {
            "ReactiveProperty::new"
        };

        let call = Instruction::Call {
            target: IRValue::GlobalVariable(constructor.to_string()),
            args: vec![IRValue::String(name.to_string()), value_val],
            result: Some(result.clone()),
        };
        instructions.push(call);

        Ok((result, instructions))
    }

    /// Generate IR for stream operation
    pub(crate) fn generate_stream_operation(
        &mut self,
        stream: &Expression,
        operation: &seen_parser::ast::StreamOp,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        let (stream_val, stream_instructions) = self.generate_expression(stream)?;
        instructions.extend(stream_instructions);

        let result_register = self.context.allocate_register();
        let result = IRValue::Register(result_register);

        match operation {
            seen_parser::ast::StreamOp::Map(lambda) => {
                let (lambda_val, lambda_instructions) = self.generate_expression(lambda)?;
                instructions.extend(lambda_instructions);

                let call = Instruction::Call {
                    target: IRValue::GlobalVariable("Stream::map".to_string()),
                    args: vec![stream_val, lambda_val],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::StreamOp::Filter(predicate) => {
                let (pred_val, pred_instructions) = self.generate_expression(predicate)?;
                instructions.extend(pred_instructions);

                let call = Instruction::Call {
                    target: IRValue::GlobalVariable("Stream::filter".to_string()),
                    args: vec![stream_val, pred_val],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::StreamOp::Throttle(duration) => {
                let call = Instruction::Call {
                    target: IRValue::GlobalVariable("Stream::throttle".to_string()),
                    args: vec![stream_val, IRValue::Integer(*duration as i64)],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::StreamOp::Debounce(duration) => {
                let call = Instruction::Call {
                    target: IRValue::GlobalVariable("Stream::debounce".to_string()),
                    args: vec![stream_val, IRValue::Integer(*duration as i64)],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::StreamOp::Take(count) => {
                let call = Instruction::Call {
                    target: IRValue::GlobalVariable("Stream::take".to_string()),
                    args: vec![stream_val, IRValue::Integer(*count as i64)],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::StreamOp::Skip(count) => {
                let call = Instruction::Call {
                    target: IRValue::GlobalVariable("Stream::skip".to_string()),
                    args: vec![stream_val, IRValue::Integer(*count as i64)],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
            seen_parser::ast::StreamOp::Distinct => {
                let call = Instruction::Call {
                    target: IRValue::GlobalVariable("Stream::distinct".to_string()),
                    args: vec![stream_val],
                    result: Some(result.clone()),
                };
                instructions.push(call);
            }
        }

        Ok((result, instructions))
    }
}
