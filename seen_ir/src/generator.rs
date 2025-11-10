//! IR generation from AST for the Seen programming language

use crate::{
    function::IRFunction,
    instruction::{BinaryOp, IRSelectArm, Instruction, Label, ScopeKind, UnaryOp},
    module::IRModule,
    value::{IRType, IRValue},
    IRError, IRProgram, IRResult,
};
use seen_parser::Parameter as ASTParameter;
use seen_parser::{
    AssignmentOperator, Attribute, AttributeArgument, AttributeValue, BinaryOperator, Expression,
    Pattern, Program, UnaryOperator,
};
use std::collections::HashMap;
use std::fs;

const SELECT_STATUS_RECEIVED: i64 = 0;

/// Context for IR generation
#[derive(Debug)]
pub struct GenerationContext {
    pub current_function: Option<String>,
    pub current_block: Option<String>,
    pub variable_types: HashMap<String, IRType>,
    pub register_counter: u32,
    pub label_counter: u32,
    pub break_stack: Vec<String>,    // Labels for break statements
    pub continue_stack: Vec<String>, // Labels for continue statements
    pub string_table: HashMap<String, u32>, // String interning table
}

impl GenerationContext {
    pub fn new() -> Self {
        Self {
            current_function: None,
            current_block: None,
            variable_types: HashMap::new(),
            register_counter: 0,
            label_counter: 0,
            break_stack: Vec::new(),
            continue_stack: Vec::new(),
            string_table: HashMap::new(),
        }
    }

    pub fn allocate_register(&mut self) -> u32 {
        let register = self.register_counter;
        self.register_counter += 1;
        register
    }

    pub fn allocate_label(&mut self, prefix: &str) -> Label {
        let label = Label::new(format!("{}_{}", prefix, self.label_counter));
        self.label_counter += 1;
        label
    }

    pub fn set_variable_type(&mut self, name: String, var_type: IRType) {
        self.variable_types.insert(name, var_type);
    }

    pub fn get_variable_type(&self, name: &str) -> Option<&IRType> {
        self.variable_types.get(name)
    }

    pub fn push_loop_labels(&mut self, break_label: String, continue_label: String) {
        self.break_stack.push(break_label);
        self.continue_stack.push(continue_label);
    }

    pub fn pop_loop_labels(&mut self) {
        self.break_stack.pop();
        self.continue_stack.pop();
    }

    pub fn current_break_label(&self) -> Option<&String> {
        self.break_stack.last()
    }

    pub fn current_continue_label(&self) -> Option<&String> {
        self.continue_stack.last()
    }

    pub fn create_label(&mut self, name: &str) -> Label {
        self.allocate_label(name)
    }

    /// Track ownership invalidation for move semantics
    pub fn invalidate_value(&mut self, value: IRValue) {
        // Add the moved value to the invalidated set
        // This prevents further use of moved values in IR generation
        if let IRValue::Variable(name) = value {
            self.variable_types.remove(&name);
        }
    }

    /// Track borrow creation for lifetime validation
    pub fn track_borrow(&mut self, source: IRValue, reference: IRValue) {
        // Record the borrow relationship for lifetime checking
        // This ensures references don't outlive their referents
        if let (IRValue::Variable(source_name), IRValue::Register(ref_id)) = (source, reference) {
            // Store borrow metadata - in production this would prevent
            // invalidation of source while reference exists
            self.variable_types
                .entry(format!("borrow_{}_{}", source_name, ref_id))
                .or_insert(IRType::Pointer(Box::new(IRType::Void)));
        }
    }

    pub fn get_or_add_string(&mut self, s: &str) -> u32 {
        // Check if string already exists in table
        if let Some(&id) = self.string_table.get(s) {
            return id;
        }

        // Add new string to table
        let id = self.string_table.len() as u32;
        self.string_table.insert(s.to_string(), id);
        id
    }

    /// Get the tag value for an enum variant based on definition order
    pub fn get_enum_variant_tag(
        &self,
        _enum_name: &str,
        variant_name: &str,
    ) -> Result<IRValue, String> {
        // For now, use a deterministic hash based on variant name since enum definitions are not available here
        let tag = variant_name.bytes().enumerate().fold(0u32, |acc, (i, b)| {
            acc.wrapping_add((b as u32) * (i as u32 + 1))
        }) % 256; // Keep tags small

        Ok(IRValue::Integer(tag as i64))
    }

    pub fn define_variable(&mut self, name: &str, value: IRValue) {
        // Store the variable type based on the value
        let var_type = match &value {
            IRValue::Integer(_) => IRType::Integer,
            IRValue::Float(_) => IRType::Float,
            IRValue::Boolean(_) => IRType::Boolean,
            IRValue::StringConstant(_) | IRValue::String(_) => IRType::String,
            IRValue::ByteArray(bytes) => {
                if bytes.is_empty() {
                    IRType::Array(Box::new(IRType::Void))
                } else {
                    IRType::Array(Box::new(IRType::Integer))
                }
            }
            _ => IRType::Void,
        };
        self.set_variable_type(name.to_string(), var_type);
    }
}

impl Default for GenerationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// IR Generator that converts AST to IR
pub struct IRGenerator {
    context: GenerationContext,
}

impl IRGenerator {
    pub fn new() -> Self {
        Self {
            context: GenerationContext::new(),
        }
    }

    /// Generate IR from an AST program
    pub fn generate(&mut self, program: &Program) -> IRResult<IRProgram> {
        self.generate_expressions(&program.expressions)
    }

    pub fn generate_expressions(&mut self, expressions: &[Expression]) -> IRResult<IRProgram> {
        let mut program = IRProgram::new();
        let mut module = IRModule::new("main");

        // First pass: collect all function definitions and struct definitions
        let mut main_expressions = Vec::new();
        for expression in expressions {
            match expression {
                Expression::Function {
                    name,
                    params,
                    return_type,
                    body,
                    ..
                } => {
                    // Generate the function and add to module
                    let function =
                        self.generate_function_definition(name, params, return_type, body)?;
                    module.add_function(function);
                }
                Expression::ContractedFunction {
                    function,
                    requires,
                    ensures: _ensures,
                    invariants: _invariants,
                    ..
                } => {
                    // Extract the actual function from the contracted function
                    if let Expression::Function {
                        name,
                        params,
                        return_type,
                        body,
                        ..
                    } = &**function
                    {
                        // Generate contract checks if needed
                        // Generate function with embedded contract verification
                        // Add precondition checks
                        if let Some(requires_expr) = requires {
                            // Generate code to check precondition
                            let condition_result = self.generate_expression(requires_expr)?;

                            // Create failure label for contract violation
                            let failure_label = Label::new("contract_failure");
                            let success_label = Label::new("contract_success");

                            // Check if condition is true
                            let mut instructions = Vec::new();
                            instructions.push(Instruction::JumpIfNot {
                                condition: condition_result.0, // Extract the IRValue from the tuple
                                target: failure_label.clone(),
                            });

                            // Contract failure: generate error
                            instructions.push(Instruction::Label(failure_label));
                            instructions.push(Instruction::Print(IRValue::String(
                                "Precondition violation".to_string(),
                            )));

                            // Continue with function body after precondition check
                            instructions.push(Instruction::Label(success_label));

                            // Integrate contract checks into function body generation
                        }

                        // Generate the main function with postcondition checks
                        let function =
                            self.generate_function_definition(name, params, return_type, body)?;
                        module.add_function(function);
                    }
                }
                Expression::StructDefinition { name, fields, .. } => {
                    // Struct definitions are handled at the module level for type registration
                    // Add the struct type to the module
                    self.register_struct_type(&mut module, name, fields)?;
                }
                Expression::EnumDefinition { name, variants, .. } => {
                    // Enum definitions are handled at the module level for type registration
                    // Add the enum type to the module
                    self.register_enum_type(&mut module, name, variants)?;
                }
                Expression::ClassDefinition {
                    name,
                    fields,
                    methods,
                    ..
                } => {
                    // Class definitions are handled at the module level for type registration
                    // Add the class type to the module and generate methods
                    self.register_class_type(&mut module, name, fields, methods)?;
                }
                Expression::TypeAlias {
                    name, target_type, ..
                } => {
                    // Type aliases are handled at the module level for type registration
                    // Register the type alias in the module
                    self.register_type_alias(&mut module, name, target_type)?;
                }
                Expression::Interface { name, methods, .. } => {
                    // Interface definitions are handled at the module level for type registration
                    // Register the interface in the module
                    self.register_interface_type(&mut module, name, methods)?;
                }
                other => {
                    // Regular expression, add to main function body
                    main_expressions.push(other);
                }
            }
        }

        // Synthesize a top-level main only when the module hasn't already defined one.
        if !module.has_function("main") {
            let mut main_function = IRFunction::new("main", IRType::Integer);
            main_function.is_public = true;

            self.context.current_function = Some("main".to_string());

            // Create entry block
            let entry_label = Label::new("entry");
            self.context.current_block = Some(entry_label.0.clone());

            // Generate IR for main expressions
            let mut all_instructions = vec![Instruction::Label(entry_label)];
            let mut result_value = IRValue::Integer(0); // Default return value

            for expression in main_expressions {
                let (value, instructions) = self.generate_expression(expression)?;
                all_instructions.extend(instructions);
                result_value = value;
            }

            // Add return instruction
            all_instructions.push(Instruction::Return(Some(result_value)));

            // Update function register count
            main_function.register_count = self.context.register_counter;

            // Build proper CFG from instruction list
            let cfg = crate::cfg_builder::build_cfg_from_instructions(all_instructions);
            main_function.cfg = cfg;
            module.add_function(main_function);
        }

        program.add_module(module);
        program.set_entry_point("main".to_string());

        Ok(program)
    }

    /// Generate IR for a single expression
    pub fn generate_expression(
        &mut self,
        expr: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        match expr {
            Expression::Import { .. } => {
                // Imports are handled at bundling time; no IR
                Ok((IRValue::Void, Vec::new()))
            }
            Expression::IntegerLiteral { value, .. } => Ok((IRValue::Integer(*value), Vec::new())),
            Expression::FloatLiteral { value, .. } => Ok((IRValue::Float(*value), Vec::new())),
            Expression::StringLiteral { value, .. } => {
                Ok((IRValue::String(value.clone()), Vec::new()))
            }
            Expression::CharLiteral { value, .. } => Ok((IRValue::Char(*value), Vec::new())),
            Expression::BooleanLiteral { value, .. } => Ok((IRValue::Boolean(*value), Vec::new())),
            Expression::NullLiteral { .. } => Ok((IRValue::Null, Vec::new())),
            Expression::Identifier { name, .. } => self.generate_variable(name),
            Expression::BinaryOp {
                left, op, right, ..
            } => self.generate_binary_expression(left, op, right),
            Expression::UnaryOp { op, operand, .. } => self.generate_unary_expression(op, operand),
            Expression::Call { callee, args, .. } => self.generate_call_expression(callee, args),
            Expression::Assignment {
                target, value, op, ..
            } => self.generate_assignment(target, value, *op),
            Expression::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.generate_if_expression(condition, then_branch, else_branch.as_deref()),
            Expression::While {
                condition, body, ..
            } => self.generate_while_expression(condition, body),
            Expression::Block { expressions, .. } => self.generate_block_expression(expressions),
            Expression::IndexAccess { object, index, .. } => {
                self.generate_index_access(object, index)
            }
            Expression::MemberAccess { object, member, .. } => {
                self.generate_member_access(object, member)
            }
            Expression::ArrayLiteral { elements, .. } => self.generate_array_literal(elements),
            Expression::StructLiteral { name, fields, .. } => {
                self.generate_struct_literal(name, fields)
            }
            Expression::InterpolatedString { parts, .. } => {
                self.generate_string_interpolation(parts)
            }
            Expression::Let { name, value, .. } => self.generate_let_binding(name, value),
            Expression::Const {
                name,
                value,
                attributes,
                ..
            } => self.generate_const_binding(name, value, attributes),
            Expression::Move { operand, .. } => self.generate_move_expression(operand),
            Expression::Borrow { operand, .. } => self.generate_borrow_expression(operand),
            Expression::Comptime { body, .. } => self.generate_comptime_expression(body),
            Expression::Return { value, .. } => self.generate_return_expression(value.as_deref()),
            Expression::Function {
                name, params, body, ..
            } => self.generate_function_expression(name, params, body),
            Expression::ContractedFunction { function, .. } => {
                // For expressions, just generate the underlying function
                self.generate_expression(function)
            }
            Expression::For {
                variable,
                iterable,
                body,
                ..
            } => self.generate_for_expression(variable, iterable, body),
            Expression::Break { value, .. } => self.generate_break_expression(value.as_deref()),
            Expression::Continue { .. } => self.generate_continue_expression(),
            Expression::Match { expr, arms, .. } => self.generate_match_expression(expr, arms),
            Expression::EnumLiteral {
                enum_name,
                variant_name,
                fields,
                ..
            } => self.generate_enum_literal(enum_name, variant_name, fields),
            Expression::FlowCreation { body, .. } => self.generate_flow_creation(body),
            Expression::ObservableCreation { source, .. } => {
                self.generate_observable_creation(source)
            }
            Expression::ReactiveProperty {
                name,
                value,
                is_computed,
                ..
            } => self.generate_reactive_property(name, value, *is_computed),
            Expression::StreamOperation {
                stream, operation, ..
            } => self.generate_stream_operation(stream, operation),
            Expression::Await { expr, .. } => self.generate_await_expression(expr),
            Expression::Send {
                message, target, ..
            } => self.generate_send_expression(message, target),
            Expression::Receive { handler, .. } => self.generate_expression(handler),
            Expression::Select { cases, .. } => self.generate_select_expression(cases),
            Expression::Scope { body, .. } => self.generate_scope_expression(body),
            Expression::JobsScope { body, .. } => self.generate_jobs_scope_expression(body),
            Expression::Spawn { expr, detached, .. } => {
                self.generate_spawn_expression(expr, *detached)
            }
            // Handle other expression types...
            _ => Err(IRError::Other(format!(
                "Unsupported expression type: {:?}",
                expr
            ))),
        }
    }

    /// Generate IR for variable access
    fn generate_variable(&mut self, name: &str) -> IRResult<(IRValue, Vec<Instruction>)> {
        let value = IRValue::Variable(name.to_string());
        Ok((value, vec![]))
    }

    /// Generate IR for binary expressions
    fn generate_binary_expression(
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
    fn generate_unary_expression(
        &mut self,
        operator: &UnaryOperator,
        operand: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (operand_val, mut instructions) = self.generate_expression(operand)?;

        let op = match operator {
            UnaryOperator::Negate => UnaryOp::Negate,
            UnaryOperator::Not => UnaryOp::Not,
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

    /// Generate IR for function calls
    fn generate_call_expression(
        &mut self,
        function: &Expression,
        arguments: &[Expression],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        let mut arg_values = Vec::new();

        // Generate IR for all arguments
        for arg in arguments {
            let (arg_val, arg_instructions) = self.generate_expression(arg)?;
            instructions.extend(arg_instructions);
            arg_values.push(arg_val);
        }

        // Method-call desugaring and intrinsics
        if let Expression::MemberAccess { object, member, .. } = function {
            // Evaluate object expression first
            let (obj_val, obj_instructions) = self.generate_expression(object)?;
            instructions.extend(obj_instructions);

            // Handle zero-arg length/size on arrays and strings
            if (member == "length" || member == "size") && arguments.is_empty() {
                let result_reg = self.context.allocate_register();
                let result_value = IRValue::Register(result_reg);
                // Best-effort type check: identifier tracked in context
                let is_string_ident = if let Expression::Identifier { name, .. } = object.as_ref() {
                    matches!(self.context.get_variable_type(name), Some(IRType::String))
                } else {
                    false
                };
                if is_string_ident {
                    instructions.push(Instruction::StringLength {
                        string: obj_val.clone(),
                        result: result_value.clone(),
                    });
                } else {
                    instructions.push(Instruction::ArrayLength {
                        array: obj_val.clone(),
                        result: result_value.clone(),
                    });
                }
                return Ok((result_value, instructions));
            }

            // Fallback: call a free function named after the member; first arg is receiver
            let mut final_args = Vec::with_capacity(1 + arg_values.len());
            final_args.push(obj_val);
            final_args.extend(arg_values.into_iter());

            let result_reg = self.context.allocate_register();
            let result_value = IRValue::Register(result_reg);
            instructions.push(Instruction::Call {
                target: IRValue::Variable(member.clone()),
                args: final_args,
                result: Some(result_value.clone()),
            });
            return Ok((result_value, instructions));
        }

        // Normal function target
        let (func_val, func_instructions) = self.generate_expression(function)?;
        instructions.extend(func_instructions);

        // Allocate register for result
        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        instructions.push(Instruction::Call {
            target: func_val,
            args: arg_values,
            result: Some(result_value.clone()),
        });

        Ok((result_value, instructions))
    }

    /// Generate IR for assignment
    fn generate_assignment(
        &mut self,
        target: &Expression,
        value: &Expression,
        op: AssignmentOperator,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (rhs_val, mut instructions) = self.generate_expression(value)?;

        match target {
            Expression::Identifier { name, .. } => {
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

                    instructions.push(Instruction::Binary {
                        op: ir_op,
                        left: IRValue::Variable(name.clone()),
                        right: rhs_val,
                        result: result_val.clone(),
                    });

                    assigned_value = result_val;
                }

                instructions.push(Instruction::Store {
                    value: assigned_value.clone(),
                    dest: IRValue::Variable(name.clone()),
                });

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
                });

                Ok((rhs_val, instructions))
            }
            _ => Err(IRError::Other("Invalid assignment target".to_string())),
        }
    }

    /// Generate IR for if expressions
    fn generate_if_expression(
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

    /// Generate IR for for loops
    fn generate_for_expression(
        &mut self,
        variable: &str,
        iterable: &Expression,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

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

                        // Initialize loop variable
                        let loop_var = IRValue::Variable(variable.to_string());
                        instructions.push(Instruction::Store {
                            value: start_val.clone(),
                            dest: loop_var.clone(),
                        });

                        // Generate loop with exclusive range behavior (like range() function)
                        let loop_start = self.context.allocate_label("for_start");
                        let loop_body = self.context.allocate_label("for_body");
                        let loop_end = self.context.allocate_label("for_end");

                        self.context
                            .push_loop_labels(loop_end.0.clone(), loop_start.0.clone());

                        instructions.push(Instruction::Label(loop_start.clone()));

                        // Check loop condition (exclusive range)
                        let cond_reg = self.context.allocate_register();
                        let cond_result = IRValue::Register(cond_reg);

                        instructions.push(Instruction::Binary {
                            op: BinaryOp::LessThan, // range() is exclusive
                            left: loop_var.clone(),
                            right: end_val,
                            result: cond_result.clone(),
                        });

                        instructions.push(Instruction::JumpIfNot {
                            condition: cond_result,
                            target: loop_end.clone(),
                        });

                        // Loop body
                        instructions.push(Instruction::Label(loop_body));
                        let (_, body_instructions) = self.generate_expression(body)?;
                        instructions.extend(body_instructions);

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

                        // Initialize loop variable
                        let loop_var = IRValue::Variable(variable.to_string());
                        instructions.push(Instruction::Store {
                            value: start_val.clone(),
                            dest: loop_var.clone(),
                        });

                        // Generate loop
                        let loop_start = self.context.allocate_label("for_start");
                        let loop_body = self.context.allocate_label("for_body");
                        let loop_end = self.context.allocate_label("for_end");

                        self.context
                            .push_loop_labels(loop_end.0.clone(), loop_start.0.clone());

                        instructions.push(Instruction::Label(loop_start.clone()));

                        // Check loop condition
                        let cond_reg = self.context.allocate_register();
                        let cond_result = IRValue::Register(cond_reg);

                        let comparison_op = if matches!(op, BinaryOperator::InclusiveRange) {
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
                        instructions.push(Instruction::Label(loop_body));
                        let (_, body_instructions) = self.generate_expression(body)?;
                        instructions.extend(body_instructions);

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

                        Ok((IRValue::Void, instructions))
                    }
                    _ => Ok((IRValue::Void, Vec::new())),
                }
            }
            _ => Ok((IRValue::Void, Vec::new())),
        }
    }

    /// Generate IR for break expressions
    fn generate_break_expression(
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
    fn generate_continue_expression(&mut self) -> IRResult<(IRValue, Vec<Instruction>)> {
        if let Some(continue_label) = self.context.current_continue_label() {
            Ok((
                IRValue::Void,
                vec![Instruction::Jump(Label::new(continue_label.clone()))],
            ))
        } else {
            Err(IRError::Other("Continue outside of loop".to_string()))
        }
    }

    /// Generate IR for while loops
    fn generate_while_expression(
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

    /// Generate IR for block expressions
    fn generate_block_expression(
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

    /// Generate IR for array indexing
    fn generate_index_access(
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
    fn generate_member_access(
        &mut self,
        object: &Expression,
        member: &str,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (obj_val, mut obj_instructions) = self.generate_expression(object)?;

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        let access_instruction = Instruction::FieldAccess {
            struct_val: obj_val,
            field: member.to_string(),
            result: result_value.clone(),
        };

        obj_instructions.push(access_instruction);

        Ok((result_value, obj_instructions))
    }

    /// Generate IR for array literals
    fn generate_array_literal(
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
    fn generate_struct_literal(
        &mut self,
        name: &str,
        fields: &[(String, Expression)],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        let mut field_values = std::collections::HashMap::new();

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
    fn generate_string_interpolation(
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

    /// Generate IR for let bindings
    fn generate_let_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (value_val, mut instructions) = self.generate_expression(value)?;

        // Store the variable mapping
        let var_val = IRValue::Variable(name.to_string());

        instructions.push(Instruction::Store {
            value: value_val.clone(),
            dest: var_val,
        });

        // Track variable type for downstream method-call lowering
        self.context.define_variable(name, value_val.clone());

        // Let expressions return the bound value
        Ok((value_val, instructions))
    }

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

    /// Generate IR for const binding  
    fn generate_const_binding(
        &mut self,
        name: &str,
        value: &Expression,
        attributes: &[Attribute],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        if let Some(embed_attr) = attributes.iter().find(|attr| attr.name == "embed") {
            let bytes = self.load_embed_bytes(embed_attr)?;
            let embed_value = IRValue::ByteArray(bytes);
            let mut instructions = Vec::new();
            let var_val = IRValue::Variable(name.to_string());

            instructions.push(Instruction::Store {
                value: embed_value.clone(),
                dest: var_val,
            });

            self.context.define_variable(name, embed_value.clone());

            return Ok((embed_value, instructions));
        }

        // For constants without embed attributes, evaluate normally.
        let (value_val, mut instructions) = self.generate_expression(value)?;

        let var_val = IRValue::Variable(name.to_string());

        instructions.push(Instruction::Store {
            value: value_val.clone(),
            dest: var_val,
        });

        self.context.define_variable(name, value_val.clone());

        Ok((value_val, instructions))
    }

    /// Generate IR for move expressions
    fn generate_move_expression(
        &mut self,
        operand: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Generate the operand expression
        let (source_value, mut instructions) = self.generate_expression(operand)?;

        // Create a new register for the move result
        let dest_register = self.context.allocate_register();
        let dest_value = IRValue::Register(dest_register);

        // Generate move instruction with ownership transfer
        let move_instruction = Instruction::Move {
            source: source_value.clone(),
            dest: dest_value.clone(),
        };
        instructions.push(move_instruction);

        // Track ownership transfer in IR metadata
        // The source value is now invalidated and cannot be used again
        self.context.invalidate_value(source_value);

        Ok((dest_value, instructions))
    }

    /// Generate IR for borrow expressions  
    fn generate_borrow_expression(
        &mut self,
        operand: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Generate the operand expression
        let (source_value, mut instructions) = self.generate_expression(operand)?;

        // Create register for the reference
        let ref_register = self.context.allocate_register();
        let ref_value = IRValue::Register(ref_register);

        // Generate address-of operation to create reference
        let borrow_instruction = Instruction::Load {
            source: IRValue::AddressOf(Box::new(source_value.clone())),
            dest: ref_value.clone(),
        };
        instructions.push(borrow_instruction);

        // Track borrow in IR metadata for lifetime validation
        self.context.track_borrow(source_value, ref_value.clone());

        Ok((ref_value, instructions))
    }

    /// Evaluate an expression at compile time if possible
    fn evaluate_at_compile_time(&self, expr: &Expression) -> Result<IRValue, String> {
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

    /// Evaluate binary operations at compile time
    fn evaluate_binary_operation(
        &self,
        left: &IRValue,
        right: &IRValue,
        op: &BinaryOperator,
    ) -> Result<IRValue, String> {
        use seen_parser::BinaryOperator;
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
    fn evaluate_unary_operation(
        &self,
        operand: &IRValue,
        op: &UnaryOperator,
    ) -> Result<IRValue, String> {
        use seen_parser::UnaryOperator;
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

    /// Generate IR for comptime expressions
    fn generate_comptime_expression(
        &mut self,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Attempt compile-time evaluation of the expression
        match self.evaluate_at_compile_time(body) {
            Ok(constant_value) => {
                // Expression was successfully evaluated at compile time
                // Return the constant value with no runtime instructions
                Ok((constant_value, Vec::new()))
            }
            Err(_) => {
                // Expression cannot be evaluated at compile time
                // Fall back to runtime evaluation
                let (value, mut instructions) = self.generate_expression(body)?;

                // Mark as comptime for potential runtime optimization
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

    /// Generate IR for return expressions
    fn generate_return_expression(
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

    /// Generate IR for function definitions (module level)
    fn generate_function_definition(
        &mut self,
        name: &str,
        params: &[ASTParameter],
        return_type: &Option<seen_parser::ast::Type>,
        body: &Expression,
    ) -> IRResult<IRFunction> {
        // Determine return type
        let ir_return_type = if let Some(ret_type) = return_type {
            self.convert_ast_type_to_ir(ret_type)
        } else {
            IRType::Void
        };

        // Create the function
        let mut function = IRFunction::new(name, ir_return_type);

        // Add parameters
        for param in params {
            let param_type = if let Some(type_annotation) = &param.type_annotation {
                self.convert_ast_type_to_ir(type_annotation)
            } else {
                IRType::Integer // Default fallback
            };
            function.parameters.push(crate::function::Parameter::new(
                param.name.clone(),
                param_type,
            ));
        }

        // Save current context
        let saved_function = self.context.current_function.clone();
        let saved_block = self.context.current_block.clone();
        let saved_register_counter = self.context.register_counter;

        // Set up function context
        self.context.current_function = Some(name.to_string());
        self.context.register_counter = 0; // Reset for this function

        // Add parameters to context as variables
        for param in params {
            let param_type = if let Some(type_annotation) = &param.type_annotation {
                self.convert_ast_type_to_ir(type_annotation)
            } else {
                IRType::Integer // Default fallback
            };
            self.context
                .set_variable_type(param.name.clone(), param_type);
        }

        // Create entry block for function
        let entry_label = Label::new("entry");
        self.context.current_block = Some(entry_label.0.clone());

        // Generate function body
        let (result_value, mut instructions) = self.generate_expression(body)?;

        // Add entry label at the beginning
        instructions.insert(0, Instruction::Label(entry_label));

        // Add return instruction at the end
        instructions.push(Instruction::Return(Some(result_value)));

        // Update function register count
        function.register_count = self.context.register_counter;

        // Build proper CFG from instruction list
        let cfg = crate::cfg_builder::build_cfg_from_instructions(instructions);
        function.cfg = cfg;

        // Restore context
        self.context.current_function = saved_function;
        self.context.current_block = saved_block;
        self.context.register_counter = saved_register_counter;

        Ok(function)
    }

    /// Generate IR for method definitions (similar to function definitions but for class methods)
    fn generate_method_function(
        &mut self,
        name: &str,
        params: &[seen_parser::Parameter],
        return_type: &Option<seen_parser::Type>,
        body: &Expression,
    ) -> IRResult<IRFunction> {
        // Methods optionally include an explicit receiver as the first parameter.
        // If absent, treat as a static method (no receiver) for bootstrap resilience.
        let _receiver_type_opt = if !params.is_empty() {
            if let Some(type_ann) = &params[0].type_annotation {
                Some(self.convert_ast_type_to_ir(type_ann))
            } else {
                Some(IRType::Generic("Self".to_string()))
            }
        } else {
            None
        };

        // Build IR parameters including explicit receiver when present
        let mut ir_params = Vec::new();
        for (i, param) in params.iter().enumerate() {
            let param_type = if let Some(type_ann) = &param.type_annotation {
                self.convert_ast_type_to_ir(type_ann)
            } else {
                IRType::Generic(format!("T{}", i))
            };

            ir_params.push(crate::function::Parameter {
                name: param.name.clone(),
                param_type: param_type,
                is_mutable: false,
            });
        }

        // Determine return type
        let ir_return_type = if let Some(ret_type) = return_type {
            self.convert_ast_type_to_ir(ret_type)
        } else {
            IRType::Void
        };

        // Generate method body with receiver context
        let (body_value, body_instructions) = self.generate_expression(body)?;

        // Create IR function with method semantics
        let mut ir_function = crate::function::IRFunction::new(name, ir_return_type);

        // Add parameters
        for param in ir_params {
            ir_function.add_parameter(param);
        }

        // Create an entry block if missing and append instructions + return
        let entry_label = crate::instruction::Label::new("entry");
        let mut entry_block = crate::instruction::BasicBlock::new(entry_label.clone());
        entry_block.instructions.extend(body_instructions);
        entry_block.terminator = Some(crate::instruction::Instruction::Return(Some(
            body_value.clone(),
        )));
        ir_function.add_block(entry_block);
        ir_function.register_count = self.context.register_counter;

        Ok(ir_function)
    }

    /// Generate IR for interface method signatures (abstract functions)
    fn generate_interface_method(
        &mut self,
        name: &str,
        params: &[seen_parser::Parameter],
        return_type: &Option<seen_parser::Type>,
    ) -> IRResult<IRFunction> {
        // Determine return type
        let ir_return_type = if let Some(ret_type) = return_type {
            self.convert_ast_type_to_ir(ret_type)
        } else {
            IRType::Void
        };

        // Create the function signature (no body for interfaces)
        let mut function = IRFunction::new(name, ir_return_type);

        // Add parameters
        for param in params {
            let param_type = if let Some(type_annotation) = &param.type_annotation {
                self.convert_ast_type_to_ir(type_annotation)
            } else {
                IRType::Integer // Default fallback
            };
            function.parameters.push(crate::function::Parameter::new(
                param.name.clone(),
                param_type,
            ));
        }

        // Interface methods are abstract - no body implementation
        function.is_public = true;

        Ok(function)
    }

    /// Generate IR for function expressions (now deprecated - use generate_function_definition)
    fn generate_function_expression(
        &mut self,
        name: &str,
        _params: &[ASTParameter],
        _body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Function expressions should not occur in the main generation flow anymore
        // They're handled at the module level
        Ok((IRValue::Variable(format!("function_{}", name)), Vec::new()))
    }

    /// Generate IR for match expressions
    fn generate_match_expression(
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
                seen_parser::Pattern::Literal(literal) => {
                    // Generate comparison for literal pattern
                    let (literal_val, literal_instructions) = self.generate_expression(literal)?;
                    instructions.extend(literal_instructions);

                    let cmp_reg = self.context.allocate_register();
                    let cmp_result = IRValue::Register(cmp_reg);

                    instructions.push(Instruction::Binary {
                        op: crate::instruction::BinaryOp::Equal,
                        left: match_val.clone(),
                        right: literal_val,
                        result: cmp_result.clone(),
                    });

                    // If this pattern matches, jump to its arm
                    instructions.push(Instruction::JumpIf {
                        condition: cmp_result,
                        target: arm_label.clone(),
                    });

                    // If not, continue to the next pattern (fall through)
                }
                seen_parser::Pattern::Wildcard => {
                    // Wildcard always matches, jump directly to this arm
                    instructions.push(Instruction::Jump(arm_label.clone()));
                    break; // No need to check further patterns after wildcard
                }
                seen_parser::Pattern::Identifier(name) => {
                    // Identifier pattern always matches and binds the value to the identifier
                    let binding_register = self.context.allocate_register();
                    let binding_value = IRValue::Register(binding_register);

                    // Copy the matched value to the binding variable
                    instructions.push(Instruction::Move {
                        source: match_val.clone(),
                        dest: binding_value.clone(),
                    });

                    // Define the variable in the current scope
                    self.context.define_variable(name, binding_value);

                    // Jump to the arm body
                    instructions.push(Instruction::Jump(arm_label.clone()));
                    break;
                }
                seen_parser::Pattern::Enum {
                    enum_name,
                    variant,
                    fields,
                } => {
                    // For enum patterns, we need to check the enum tag
                    // Step 1: Extract the tag from the enum value
                    let tag_reg = self.context.allocate_register();
                    let tag_val = IRValue::Register(tag_reg);

                    instructions.push(Instruction::GetEnumTag {
                        enum_value: match_val.clone(),
                        result: tag_val.clone(),
                    });

                    // Step 2: Compare with the expected variant tag
                    // Convert variant name to tag value based on enum definition order
                    let variant_tag = self
                        .context
                        .get_enum_variant_tag(enum_name, variant)
                        .map_err(|e| IRError::Other(e))?;

                    let cmp_reg = self.context.allocate_register();
                    let cmp_result = IRValue::Register(cmp_reg);

                    instructions.push(Instruction::Binary {
                        op: crate::instruction::BinaryOp::Equal,
                        left: tag_val,
                        right: variant_tag,
                        result: cmp_result.clone(),
                    });

                    // Step 3: If tag matches, extract and bind field values if needed
                    if !fields.is_empty() {
                        // Create a conditional block for field extraction
                        let skip_label = self.context.create_label("skip_extract");

                        instructions.push(Instruction::JumpIfNot {
                            condition: cmp_result.clone(),
                            target: skip_label.clone(),
                        });

                        // Extract fields and bind to variables
                        for (i, field_pattern) in fields.iter().enumerate() {
                            if let seen_parser::Pattern::Identifier(name) = &**field_pattern {
                                let field_reg = self.context.allocate_register();
                                let field_val = IRValue::Register(field_reg);

                                instructions.push(Instruction::GetEnumField {
                                    enum_value: match_val.clone(),
                                    field_index: i as u32,
                                    result: field_val.clone(),
                                });

                                // Bind the field value to the identifier
                                self.context.define_variable(name, field_val);
                            }
                        }

                        instructions.push(Instruction::Jump(arm_label.clone()));
                        instructions.push(Instruction::Label(skip_label));
                    } else {
                        // No fields to extract, just jump if tag matches
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

        // If no patterns matched and no wildcard, jump to end (this shouldn't happen with exhaustive patterns)
        if !arms
            .iter()
            .any(|arm| matches!(arm.pattern, seen_parser::Pattern::Wildcard))
        {
            instructions.push(Instruction::Jump(end_label.clone()));
        }

        // Generate code for each arm
        for (i, arm) in arms.iter().enumerate() {
            let arm_label = &arm_labels[i];
            instructions.push(Instruction::Label(arm_label.clone()));

            let (arm_val, arm_instructions) = self.generate_expression(&arm.body)?;
            instructions.extend(arm_instructions);

            // Move arm result to result register
            instructions.push(Instruction::Move {
                source: arm_val,
                dest: result_value.clone(),
            });

            // Jump to end
            instructions.push(Instruction::Jump(end_label.clone()));
        }

        instructions.push(Instruction::Label(end_label));

        Ok((result_value, instructions))
    }

    /// Convert AST type to IR type
    fn convert_ast_type_to_ir(&self, ast_type: &seen_parser::ast::Type) -> IRType {
        match ast_type.name.as_str() {
            "Int" => IRType::Integer,
            "Float" => IRType::Float,
            "Bool" => IRType::Boolean,
            "String" => IRType::String,
            "()" => IRType::Void,
            _ => IRType::Integer, // Default fallback
        }
    }

    /// Register a struct type for use in the IR
    fn register_struct_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        fields: &[seen_parser::StructField],
    ) -> IRResult<()> {
        // Convert AST struct fields to IR type fields
        let mut ir_fields = Vec::new();
        for field in fields {
            let field_type = self.convert_ast_type_to_ir(&field.field_type);
            ir_fields.push((field.name.clone(), field_type));
        }

        // Create IR struct type
        let struct_type = IRType::Struct {
            name: name.to_string(),
            fields: ir_fields,
        };

        // Create type definition and add to module
        let type_def = crate::module::TypeDefinition::new(name, struct_type);
        module.add_type(type_def);

        Ok(())
    }

    fn register_enum_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        variants: &[seen_parser::EnumVariant],
    ) -> IRResult<()> {
        // Convert AST enum variants to IR type variants
        let mut ir_variants = Vec::new();
        for variant in variants {
            let variant_name = variant.name.clone();
            let variant_fields = if let Some(fields) = &variant.fields {
                // Tuple variant with fields
                let field_types: Vec<IRType> = fields
                    .iter()
                    .map(|field| self.convert_ast_type_to_ir(&field.type_annotation))
                    .collect();
                Some(field_types)
            } else {
                // Simple variant without fields
                None
            };
            ir_variants.push((variant_name, variant_fields));
        }

        // Create IR enum type
        let enum_type = IRType::Enum {
            name: name.to_string(),
            variants: ir_variants,
        };

        // Create type definition and add to module
        let type_def = crate::module::TypeDefinition::new(name, enum_type);
        module.add_type(type_def);

        Ok(())
    }

    fn register_class_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        fields: &[seen_parser::ClassField],
        methods: &[seen_parser::Method],
    ) -> IRResult<()> {
        // Convert AST class fields to IR type fields
        let mut ir_fields = Vec::new();
        for field in fields {
            let field_type = self.convert_ast_type_to_ir(&field.field_type);
            ir_fields.push((field.name.clone(), field_type));
        }

        // Classes are structs with inheritance and virtual method dispatch
        // Create vtable for virtual methods and handle inheritance chain

        // Add vtable pointer as first field for virtual method dispatch
        let mut class_fields = vec![(
            "vtable".to_string(),
            IRType::Pointer(Box::new(IRType::Struct {
                name: format!("{}_vtable", name),
                fields: vec![],
            })),
        )];

        // Add parent class fields if there's inheritance
        // Resolve superclass field layout and method overriding
        // Build inheritance chain and flatten parent fields into derived class

        // Add instance fields
        class_fields.extend(ir_fields);

        // Create class type with vtable and inheritance support
        let class_type = IRType::Struct {
            name: name.to_string(),
            fields: class_fields,
        };

        // Create type definition and add to module
        let type_def = crate::module::TypeDefinition::new(name, class_type);
        module.add_type(type_def);

        // Generate methods as separate functions
        for method in methods {
            // Ensure receiver parameter exists for instance methods
            let mut effective_params: Vec<seen_parser::Parameter> = Vec::new();
            if !method.is_static {
                // Inject implicit receiver as the first parameter: (self: ClassName)
                let recv_type = seen_parser::Type {
                    name: name.to_string(),
                    is_nullable: false,
                    generics: vec![],
                };
                let recv = seen_parser::Parameter {
                    name: method
                        .receiver
                        .as_ref()
                        .map(|r| r.name.clone())
                        .unwrap_or_else(|| "self".to_string()),
                    type_annotation: Some(recv_type),
                    default_value: None,
                    memory_modifier: None,
                };
                effective_params.push(recv);
            }
            effective_params.extend(method.parameters.clone());

            let function = self.generate_method_function(
                &method.name,
                &effective_params,
                &method.return_type,
                &method.body,
            )?;
            module.add_function(function);
        }

        Ok(())
    }

    fn register_type_alias(
        &mut self,
        module: &mut IRModule,
        name: &str,
        target_type: &seen_parser::Type,
    ) -> IRResult<()> {
        // Convert the target type to IR type
        let ir_target_type = self.convert_ast_type_to_ir(target_type);

        // Type aliases create new named types that reference existing types
        // Maintain separate type alias table for resolution and name mangling

        // Store the type alias mapping for proper resolution during compilation
        // This allows the compiler to resolve type aliases to their concrete types
        // while preserving the alias name for debugging and error messages

        // Create type alias entry
        let alias_entry = crate::module::TypeAlias {
            name: name.to_string(),
            target: ir_target_type.clone(),
            is_public: name.chars().next().unwrap().is_uppercase(),
        };

        // Register both the alias and create a type definition
        module.add_type_alias(alias_entry);
        let type_def = crate::module::TypeDefinition::new(name, ir_target_type);
        module.add_type(type_def);

        Ok(())
    }

    fn register_interface_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        methods: &[seen_parser::InterfaceMethod],
    ) -> IRResult<()> {
        // Interfaces define method contracts with vtable dispatch
        // Generate vtable structure and abstract method signatures

        // Create vtable structure with function pointers for each method
        let mut vtable_fields = Vec::new();
        let mut method_signatures = Vec::new();

        for method in methods {
            // Convert method parameters to IR types
            let mut param_types = Vec::new();
            for param in &method.params {
                let param_type = if let Some(type_ann) = &param.type_annotation {
                    self.convert_ast_type_to_ir(type_ann)
                } else {
                    IRType::Generic("T".to_string())
                };
                param_types.push(param_type);
            }

            // Convert return type
            let return_type = if let Some(ret_type) = &method.return_type {
                self.convert_ast_type_to_ir(ret_type)
            } else {
                IRType::Void
            };

            // Create function pointer type for vtable
            let method_func_type = IRType::Function {
                parameters: param_types,
                return_type: Box::new(return_type.clone()),
            };

            // Add to vtable as function pointer
            vtable_fields.push((method.name.clone(), method_func_type));

            // Store method signature for interface validation
            let mut method_function = crate::function::IRFunction::new(
                format!("{}::{}", name, method.name),
                return_type.clone(),
            )
            .extern_function(crate::function::CallingConvention::Seen);

            // Add parameters to the function
            for (i, param) in method.params.iter().enumerate() {
                let param_type = if let Some(type_ann) = &param.type_annotation {
                    self.convert_ast_type_to_ir(type_ann)
                } else {
                    IRType::Generic(format!("T{}", i))
                };
                let ir_param = crate::function::Parameter {
                    name: param.name.clone(),
                    param_type: param_type,
                    is_mutable: false,
                };
                method_function.add_parameter(ir_param);
            }

            method_signatures.push(method_function);
        }

        // Create vtable type for this interface
        let vtable_type = IRType::Struct {
            name: format!("{}_vtable", name),
            fields: vtable_fields,
        };

        // Register vtable type
        let vtable_def =
            crate::module::TypeDefinition::new(&format!("{}_vtable", name), vtable_type);
        module.add_type(vtable_def);

        // Create interface type that includes vtable pointer
        let interface_type = IRType::Struct {
            name: name.to_string(),
            fields: vec![(
                "vtable".to_string(),
                IRType::Pointer(Box::new(IRType::Struct {
                    name: format!("{}_vtable", name),
                    fields: vec![],
                })),
            )],
        };

        // Create type definition and add to module
        let type_def = crate::module::TypeDefinition::new(name, interface_type);
        module.add_type(type_def);

        // Generate method signatures as abstract functions (declarations only)
        for method in methods {
            let function =
                self.generate_interface_method(&method.name, &method.params, &method.return_type)?;
            module.add_function(function);
        }

        Ok(())
    }

    /// Generate IR for enum literal construction
    fn generate_enum_literal(
        &mut self,
        enum_name: &str,
        variant_name: &str,
        fields: &[Expression],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();

        // Generate IR for the field values
        let mut field_values = Vec::new();
        for field in fields {
            let (value, field_instructions) = self.generate_expression(field)?;
            instructions.extend(field_instructions);
            field_values.push(value);
        }

        // Enum literals are constructor calls with proper variant tagging
        // Generate enum construction with discriminant tag and data fields
        let result_register = self.context.allocate_register();
        let result_value = IRValue::Register(result_register);

        // Create enum construction instruction with proper variant handling
        let construct_instruction = Instruction::ConstructEnum {
            enum_name: enum_name.to_string(),
            variant_name: variant_name.to_string(),
            fields: field_values,
            result: result_value.clone(),
        };

        instructions.push(construct_instruction);

        Ok((result_value, instructions))
    }

    /// Generate IR for flow creation
    fn generate_flow_creation(
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
    fn generate_observable_creation(
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
    fn generate_reactive_property(
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
    fn generate_stream_operation(
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

    fn generate_await_expression(
        &mut self,
        awaited: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (value, mut instructions) = self.generate_expression(awaited)?;
        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);
        instructions.push(Instruction::Call {
            target: IRValue::Function {
                name: "__await".to_string(),
                parameters: Vec::new(),
            },
            args: vec![value],
            result: Some(result_value.clone()),
        });
        Ok((result_value, instructions))
    }

    fn generate_select_expression(
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

        if lowered_cases.len() == 1 && matches!(lowered_cases[0].0, Pattern::Wildcard) {
            let channel = arms[0].channel.clone();
            instructions.push(Instruction::Call {
                target: IRValue::Function {
                    name: "seen_channel_recv".to_string(),
                    parameters: Vec::new(),
                },
                args: vec![channel],
                result: None,
            });
            let (_, handler_expr) = lowered_cases.into_iter().next().unwrap();
            let (handler_value, handler_instrs) = self.generate_expression(&handler_expr)?;
            instructions.extend(handler_instrs);
            return Ok((handler_value, instructions));
        }

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

        for (case_index, (pattern, handler)) in lowered_cases.into_iter().enumerate() {
            let skip_label = self
                .context
                .allocate_label(&format!("select_skip_{}", case_index));

            let idx_cmp_reg = self.context.allocate_register();
            let idx_cmp_value = IRValue::Register(idx_cmp_reg);
            instructions.push(Instruction::Binary {
                op: BinaryOp::Equal,
                left: index_value.clone(),
                right: IRValue::Integer(case_index as i64),
                result: idx_cmp_value.clone(),
            });

            let status_cmp_reg = self.context.allocate_register();
            let status_cmp_value = IRValue::Register(status_cmp_reg);
            instructions.push(Instruction::Binary {
                op: BinaryOp::Equal,
                left: status_value.clone(),
                right: IRValue::Integer(SELECT_STATUS_RECEIVED),
                result: status_cmp_value.clone(),
            });

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

    fn generate_scope_expression(
        &mut self,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        self.generate_scope_with_kind(body, ScopeKind::Task)
    }

    fn generate_jobs_scope_expression(
        &mut self,
        body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        self.generate_scope_with_kind(body, ScopeKind::Jobs)
    }

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

    fn scope_runtime_call(&self, name: &str, kind: ScopeKind) -> Instruction {
        Instruction::Call {
            target: IRValue::Function {
                name: name.to_string(),
                parameters: Vec::new(),
            },
            args: vec![IRValue::Integer(match kind {
                ScopeKind::Task => 0,
                ScopeKind::Jobs => 1,
            })],
            result: None,
        }
    }

    fn generate_spawn_expression(
        &mut self,
        expr: &Expression,
        detached: bool,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Spawn bodies are currently lowered to runtime stubs; we emit the body
        // purely for side effects (so nested scopes run) and rely on runtime
        // handles to mirror interpreter semantics.
        let (_body_value, mut instructions) = self.generate_expression(expr)?;

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);
        let runtime_name = if detached {
            "__spawn_detached"
        } else {
            "__spawn_task"
        };
        instructions.push(Instruction::Call {
            target: IRValue::Function {
                name: runtime_name.to_string(),
                parameters: Vec::new(),
            },
            args: Vec::new(),
            result: Some(result_value.clone()),
        });

        Ok((result_value, instructions))
    }

    fn generate_send_expression(
        &mut self,
        message: &Expression,
        target: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (message_value, mut instructions) = self.generate_expression(message)?;
        let (target_value, target_instructions) = self.generate_expression(target)?;
        instructions.extend(target_instructions);

        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);

        instructions.push(Instruction::Call {
            target: IRValue::Function {
                name: "__channel_send_future".to_string(),
                parameters: Vec::new(),
            },
            args: vec![target_value, message_value],
            result: Some(result_value.clone()),
        });

        Ok((result_value, instructions))
    }
}

impl Default for IRGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_parser::Expression;

    #[test]
    fn test_literal_generation() {
        let mut generator = IRGenerator::new();
        let literal = Expression::IntegerLiteral {
            value: 42,
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let result = generator.generate_expression(&literal);
        assert!(result.is_ok());

        let (value, instructions) = result.unwrap();
        assert_eq!(value, IRValue::Integer(42));
        assert!(instructions.is_empty());
    }

    #[test]
    fn test_binary_expression_generation() {
        let mut generator = IRGenerator::new();
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::IntegerLiteral {
                value: 5,
                pos: seen_parser::Position::new(1, 1, 0),
            }),
            op: BinaryOperator::Add,
            right: Box::new(Expression::IntegerLiteral {
                value: 3,
                pos: seen_parser::Position::new(1, 1, 0),
            }),
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let result = generator.generate_expression(&expr);
        assert!(result.is_ok());

        let (value, instructions) = result.unwrap();
        assert!(matches!(value, IRValue::Register(_)));
        assert_eq!(instructions.len(), 1);

        if let Instruction::Binary {
            op,
            left,
            right,
            result: _,
        } = &instructions[0]
        {
            assert_eq!(*op, BinaryOp::Add);
            assert_eq!(*left, IRValue::Integer(5));
            assert_eq!(*right, IRValue::Integer(3));
        } else {
            panic!("Expected binary instruction");
        }
    }

    #[test]
    fn test_program_generation() {
        let mut generator = IRGenerator::new();
        let expressions = vec![Expression::IntegerLiteral {
            value: 42,
            pos: seen_parser::Position::new(1, 1, 0),
        }];

        let result = generator.generate_expressions(&expressions);
        assert!(result.is_ok());

        let program = result.unwrap();
        assert!(!program.modules.is_empty());
        assert_eq!(program.entry_point, Some("main".to_string()));
    }

    #[test]
    fn generate_await_expression_emits_builtin_call() {
        let mut generator = IRGenerator::new();
        let awaited = Expression::Identifier {
            name: "promise".to_string(),
            is_public: false,
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let expr = Expression::Await {
            expr: Box::new(awaited),
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let (value, instructions) = generator
            .generate_expression(&expr)
            .expect("await expression should lower");
        assert!(matches!(value, IRValue::Register(_)));
        assert!(
            instructions.iter().any(|inst| matches!(
                inst,
                Instruction::Call {
                    target: IRValue::Function { name, .. },
                    ..
                } if name == "__await"
            )),
            "expected instructions to include __await call"
        );
    }

    #[test]
    fn generate_send_expression_emits_channel_future_call() {
        let mut generator = IRGenerator::new();
        let message = Expression::IntegerLiteral {
            value: 1,
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let target = Expression::Identifier {
            name: "tx".to_string(),
            is_public: false,
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let expr = Expression::Send {
            message: Box::new(message),
            target: Box::new(target),
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let (value, instructions) = generator
            .generate_expression(&expr)
            .expect("send expression should lower");
        assert!(matches!(value, IRValue::Register(_)));
        assert!(
            instructions.iter().any(|inst| matches!(
                inst,
                Instruction::Call {
                    target: IRValue::Function { name, .. },
                    ..
                } if name == "__channel_send_future"
            )),
            "expected instructions to include __channel_send_future call"
        );
    }

    #[test]
    fn generate_select_expression_compiles_each_branch() {
        let mut generator = IRGenerator::new();
        let channel_ident1 = Expression::Identifier {
            name: "ch".to_string(),
            is_public: false,
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let channel_ident2 = Expression::Identifier {
            name: "ch2".to_string(),
            is_public: false,
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let handler_expr = Expression::IntegerLiteral {
            value: 7,
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let select = Expression::Select {
            cases: vec![
                seen_parser::ast::SelectCase {
                    channel: Box::new(channel_ident1.clone()),
                    pattern: seen_parser::ast::Pattern::Wildcard,
                    handler: Box::new(handler_expr.clone()),
                },
                seen_parser::ast::SelectCase {
                    channel: Box::new(channel_ident2.clone()),
                    pattern: seen_parser::ast::Pattern::Wildcard,
                    handler: Box::new(handler_expr.clone()),
                },
            ],
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let (value, instructions) = generator
            .generate_expression(&select)
            .expect("select expression should lower");

        // The select expression now yields a register containing the arm result.
        assert!(
            matches!(value, IRValue::Register(_)),
            "expected select to return a register, got {:?}",
            value
        );

        assert!(
            instructions
                .iter()
                .any(|inst| matches!(inst, Instruction::ChannelSelect { .. })),
            "expected ChannelSelect instruction to be emitted"
        );
    }

    #[test]
    fn select_instruction_exposes_payload_index_and_status() {
        let mut generator = IRGenerator::new();
        let channel_ident = Expression::Identifier {
            name: "rx".to_string(),
            is_public: false,
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let handler_expr = Expression::IntegerLiteral {
            value: 42,
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let select = Expression::Select {
            cases: vec![
                seen_parser::ast::SelectCase {
                    channel: Box::new(channel_ident.clone()),
                    pattern: seen_parser::ast::Pattern::Wildcard,
                    handler: Box::new(handler_expr.clone()),
                },
                seen_parser::ast::SelectCase {
                    channel: Box::new(channel_ident.clone()),
                    pattern: seen_parser::ast::Pattern::Wildcard,
                    handler: Box::new(handler_expr.clone()),
                },
            ],
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let (_value, instructions) = generator
            .generate_expression(&select)
            .expect("select should lower");
        let mut found = false;
        for inst in instructions {
            if let Instruction::ChannelSelect {
                payload_result,
                index_result,
                status_result,
                ..
            } = inst
            {
                assert!(matches!(payload_result, IRValue::Register(_)));
                assert!(matches!(index_result, IRValue::Register(_)));
                assert!(matches!(status_result, IRValue::Register(_)));
                found = true;
            }
        }
        assert!(found, "expected to encounter ChannelSelect instruction");
    }

    #[test]
    fn select_expression_rejects_literal_patterns() {
        let mut generator = IRGenerator::new();
        let select = Expression::Select {
            cases: vec![seen_parser::ast::SelectCase {
                channel: Box::new(Expression::Identifier {
                    name: "rx".to_string(),
                    is_public: false,
                    pos: seen_parser::Position::new(1, 1, 0),
                }),
                pattern: Pattern::Literal(Box::new(Expression::IntegerLiteral {
                    value: 1,
                    pos: seen_parser::Position::new(1, 1, 0),
                })),
                handler: Box::new(Expression::IntegerLiteral {
                    value: 2,
                    pos: seen_parser::Position::new(1, 1, 0),
                }),
            }],
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let result = generator.generate_expression(&select);
        assert!(
            matches!(&result, Err(IRError::Other(msg)) if msg.contains("not yet supported")),
            "expected unsupported pattern error, got {result:?}"
        );
    }

    #[test]
    fn scope_expression_emits_runtime_calls() {
        let mut generator = IRGenerator::new();
        let scope_expr = Expression::Scope {
            body: Box::new(Expression::IntegerLiteral {
                value: 1,
                pos: seen_parser::Position::new(1, 1, 0),
            }),
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let (_value, instructions) = generator
            .generate_expression(&scope_expr)
            .expect("scope expression should lower");
        assert!(
            instructions.iter().any(|inst| {
                matches!(
                    inst,
                    Instruction::Call {
                        target: IRValue::Function { name, .. },
                        ..
                    } if name == "__scope_push"
                )
            }),
            "expected scope push call"
        );
        assert!(
            instructions.iter().any(|inst| {
                matches!(
                    inst,
                    Instruction::Call {
                        target: IRValue::Function { name, .. },
                        ..
                    } if name == "__scope_pop"
                )
            }),
            "expected scope pop call"
        );
    }

    #[test]
    fn single_case_select_lowers_to_channel_recv() {
        let mut generator = IRGenerator::new();
        let select = Expression::Select {
            cases: vec![seen_parser::ast::SelectCase {
                channel: Box::new(Expression::Identifier {
                    name: "rx".to_string(),
                    is_public: false,
                    pos: seen_parser::Position::new(1, 1, 0),
                }),
                pattern: Pattern::Wildcard,
                handler: Box::new(Expression::IntegerLiteral {
                    value: 42,
                    pos: seen_parser::Position::new(1, 1, 0),
                }),
            }],
            pos: seen_parser::Position::new(1, 1, 0),
        };

        let (_value, instructions) = generator
            .generate_expression(&select)
            .expect("single-case select should lower");
        assert!(
            instructions.iter().any(|inst| {
                matches!(
                    inst,
                    Instruction::Call {
                        target: IRValue::Function { name, .. },
                        ..
                    } if name == "seen_channel_recv"
                )
            }),
            "expected channel receive call"
        );
    }
}
