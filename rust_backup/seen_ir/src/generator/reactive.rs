//! Reactive programming constructs for the IR generator.
//!
//! Handles flow, observable, reactive property, and stream operations.

use crate::{instruction::Instruction, value::IRValue, IRResult};
use seen_parser::Expression;

use super::IRGenerator;

impl IRGenerator {
    // ==================== DRY Helpers ====================

    /// Emit a call to a global function, allocating a result register.
    /// Returns (result_value, instruction).
    fn emit_global_call(&mut self, function_name: &str, args: Vec<IRValue>) -> (IRValue, Instruction) {
        let result_reg = self.context.allocate_register();
        let result = IRValue::Register(result_reg);
        let call = Instruction::Call {
            target: IRValue::GlobalVariable(function_name.to_string()),
            args,
            result: Some(result.clone()),
            arg_types: None,
            return_type: None,
        };
        (result, call)
    }

    /// Emit a stream operation that takes an expression argument (e.g., map, filter).
    fn emit_stream_op_with_expr(
        &mut self,
        stream_val: IRValue,
        method: &str,
        expr: &Expression,
        instructions: &mut Vec<Instruction>,
    ) -> IRResult<IRValue> {
        let (expr_val, expr_instrs) = self.generate_expression(expr)?;
        instructions.extend(expr_instrs);
        let (result, call) = self.emit_global_call(method, vec![stream_val, expr_val]);
        instructions.push(call);
        Ok(result)
    }

    /// Emit a stream operation that takes an integer argument (e.g., throttle, take).
    fn emit_stream_op_with_int(
        &mut self,
        stream_val: IRValue,
        method: &str,
        value: i64,
        instructions: &mut Vec<Instruction>,
    ) -> IRValue {
        let (result, call) = self.emit_global_call(method, vec![stream_val, IRValue::Integer(value)]);
        instructions.push(call);
        result
    }

    /// Emit a stream operation with no additional arguments (e.g., distinct).
    fn emit_stream_op_nullary(
        &mut self,
        stream_val: IRValue,
        method: &str,
        instructions: &mut Vec<Instruction>,
    ) -> IRValue {
        let (result, call) = self.emit_global_call(method, vec![stream_val]);
        instructions.push(call);
        result
    }

    // ==================== Public Generation Methods ====================

    /// Generate IR for flow creation
    pub(crate) fn generate_flow_creation(
        &mut self,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        // Generate body as a closure/function
        let (body_val, body_instructions) = self.generate_expression(body)?;
        instructions.extend(body_instructions);

        // Create flow constructor call
        let (result, call) = self.emit_global_call("Flow::new", vec![body_val]);
        instructions.push(call);

        Ok((result, instructions))
    }

    /// Generate IR for observable creation
    pub(crate) fn generate_observable_creation(
        &mut self,
        source: &seen_parser::ast::ObservableSource,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        let (result, call) = match source {
            seen_parser::ast::ObservableSource::Range { start, end, step } => {
                let (start_val, start_instrs) = self.generate_expression(start)?;
                let (end_val, end_instrs) = self.generate_expression(end)?;
                instructions.extend(start_instrs);
                instructions.extend(end_instrs);

                let mut args = vec![start_val, end_val];
                if let Some(step_expr) = step {
                    let (step_val, step_instrs) = self.generate_expression(step_expr)?;
                    instructions.extend(step_instrs);
                    args.push(step_val);
                }
                self.emit_global_call("Observable::Range", args)
            }
            seen_parser::ast::ObservableSource::FromArray(array_expr) => {
                let (array_val, array_instrs) = self.generate_expression(array_expr)?;
                instructions.extend(array_instrs);
                self.emit_global_call("Observable::FromArray", vec![array_val])
            }
            seen_parser::ast::ObservableSource::FromEvent(event_name) => {
                self.emit_global_call("Observable::FromEvent", vec![IRValue::String(event_name.clone())])
            }
            seen_parser::ast::ObservableSource::Interval(duration) => {
                self.emit_global_call("Observable::Interval", vec![IRValue::Integer(*duration as i64)])
            }
        };

        instructions.push(call);
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

        let (value_val, value_instrs) = self.generate_expression(value)?;
        instructions.extend(value_instrs);

        let constructor = if is_computed {
            "ReactiveProperty::Computed"
        } else {
            "ReactiveProperty::new"
        };

        let (result, call) = self.emit_global_call(
            constructor,
            vec![IRValue::String(name.to_string()), value_val],
        );
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

        let (stream_val, stream_instrs) = self.generate_expression(stream)?;
        instructions.extend(stream_instrs);

        let result = match operation {
            seen_parser::ast::StreamOp::Map(lambda) => {
                self.emit_stream_op_with_expr(stream_val, "Stream::map", lambda, &mut instructions)?
            }
            seen_parser::ast::StreamOp::Filter(predicate) => {
                self.emit_stream_op_with_expr(stream_val, "Stream::filter", predicate, &mut instructions)?
            }
            seen_parser::ast::StreamOp::Throttle(duration) => {
                self.emit_stream_op_with_int(stream_val, "Stream::throttle", *duration as i64, &mut instructions)
            }
            seen_parser::ast::StreamOp::Debounce(duration) => {
                self.emit_stream_op_with_int(stream_val, "Stream::debounce", *duration as i64, &mut instructions)
            }
            seen_parser::ast::StreamOp::Take(count) => {
                self.emit_stream_op_with_int(stream_val, "Stream::take", *count as i64, &mut instructions)
            }
            seen_parser::ast::StreamOp::Skip(count) => {
                self.emit_stream_op_with_int(stream_val, "Stream::skip", *count as i64, &mut instructions)
            }
            seen_parser::ast::StreamOp::Distinct => {
                self.emit_stream_op_nullary(stream_val, "Stream::distinct", &mut instructions)
            }
        };

        Ok((result, instructions))
    }
}
