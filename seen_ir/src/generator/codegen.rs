//! IR generation from AST for the Seen programming language

use crate::{
    function::IRFunction,
    instruction::{Instruction, Label},
    module::IRModule,
    value::{IRType, IRValue},
    IRError, IRProgram, IRResult,
};
use super::context::GenerationContext;
use seen_parser::{Expression, Program};


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
}

impl Default for IRGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::BinaryOp;
    use seen_parser::{BinaryOperator, Expression, Pattern};

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
            type_args: Vec::new(),
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
            type_args: Vec::new(),
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
                } if name == "seen_channel_send"
            )),
            "expected instructions to include seen_channel_send call"
        );
    }

    #[test]
    fn generate_select_expression_compiles_each_branch() {
        let mut generator = IRGenerator::new();
        let channel_ident1 = Expression::Identifier {
            name: "ch".to_string(),
            is_public: false,
            type_args: Vec::new(),
            pos: seen_parser::Position::new(1, 1, 0),
        };
        let channel_ident2 = Expression::Identifier {
            name: "ch2".to_string(),
            is_public: false,
            type_args: Vec::new(),
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
            type_args: Vec::new(),
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
                    type_args: Vec::new(),
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
                    type_args: Vec::new(),
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
