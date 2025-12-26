//! IR generation from AST for the Seen programming language

use crate::{
    function::IRFunction,
    instruction::{BinaryOp, IRSelectArm, Instruction, Label, ScopeKind},
    module::IRModule,
    value::{IRType, IRValue},
    IRError, IRProgram, IRResult,
};
use indexmap::IndexMap;
use super::context::GenerationContext;
use seen_parser::Parameter as ASTParameter;
use seen_parser::{Expression, Pattern, Program};

const SELECT_STATUS_RECEIVED: i64 = 0;


/// IR Generator that converts AST to IR
pub struct IRGenerator {
    pub(crate) context: GenerationContext,
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
                Expression::Import {
                    module_path,
                    symbols,
                    ..
                } => {
                    // Record imported types for downstream reference resolution
                    let module_path_str = module_path.join(".");
                    self.register_import_types(&mut module, &module_path_str, symbols)?;
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
                    self.generate_class_methods(&mut module, name, methods)?;
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
            Expression::Import {
                ..
            } => Ok((IRValue::Void, Vec::new())),
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
                binding,
                iterable,
                body,
                ..
            } => self.generate_for_expression(binding, iterable, body),
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
    pub(crate) fn generate_method_function(
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
    #[allow(dead_code)]
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

    /// Convert AST type to IR type

    /// Register a struct type for use in the IR





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
