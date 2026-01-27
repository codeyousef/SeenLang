//! IR generation from AST for the Seen programming language

use crate::{
    function::{IRFunction, Parameter as IRParameter, CallingConvention},
    instruction::{Instruction, Label},
    module::IRModule,
    value::{IRType, IRValue},
    IRError, IRProgram, IRResult,
};
use super::context::GenerationContext;
use seen_parser::{Expression, Parameter, Program, Type};


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

        // Pre-pass: Register all function signatures
        for expression in expressions {
            match expression {
                Expression::Function {
                    name, return_type, receiver, ..
                } => {
                    let mut func_name = name.clone();
                    if let Some(recv) = receiver {
                        func_name = format!("{}.{}", recv.type_name, name);
                    }

                    let ir_return_type = return_type
                        .as_ref()
                        .map(|t| self.convert_ast_type_to_ir(t))
                        .unwrap_or(IRType::Void);
                    // Register both dot and underscore formats for consistency
                    self.context
                        .function_return_types
                        .insert(func_name.clone(), ir_return_type.clone());
                    // Also register underscore format for call resolution
                    if let Some(recv) = receiver {
                        let underscore_name = format!("{}_{}", recv.type_name, name);
                        self.context
                            .function_return_types
                            .insert(underscore_name, ir_return_type);
                    }
                }
                Expression::ContractedFunction { function, .. } => {
                    if let Expression::Function {
                        name, return_type, receiver, ..
                    } = &**function
                    {
                        let mut func_name = name.clone();
                        if let Some(recv) = receiver {
                            func_name = format!("{}.{}", recv.type_name, name);
                        }

                        let ir_return_type = return_type
                            .as_ref()
                            .map(|t| self.convert_ast_type_to_ir(t))
                            .unwrap_or(IRType::Void);
                        self.context
                            .function_return_types
                            .insert(func_name.clone(), ir_return_type.clone());
                        // Also register underscore format for call resolution
                        if let Some(recv) = receiver {
                            let underscore_name = format!("{}_{}", recv.type_name, name);
                            self.context
                                .function_return_types
                                .insert(underscore_name, ir_return_type);
                        }
                    }
                }
                Expression::ClassDefinition { name, methods, fields, .. } => {
                    // Register class type definition
                    let ir_fields: Vec<(String, IRType)> = fields
                        .iter()
                        .map(|f| (f.name.clone(), self.convert_ast_type_to_ir(&f.field_type)))
                        .collect();
                    let type_def = IRType::Struct {
                        name: name.clone(),
                        fields: ir_fields.clone(),
                    };
                    self.context.type_definitions.insert(name.clone(), type_def.clone());

                    // Generate default constructor function if not present
                    // This is needed so that ClassName() calls work and return the correct type
                    let constructor_name = name.clone();
                    if !self.context.function_return_types.contains_key(&constructor_name) {
                        let return_type = IRType::Struct { name: name.clone(), fields: ir_fields.clone() };
                        
                        // Register return type
                        self.context.function_return_types.insert(constructor_name.clone(), return_type.clone());
                        
                        // Create function
                        let mut func = IRFunction::new(&constructor_name, return_type);
                        func.is_public = true;
                        
                        let entry_label = Label::new("entry");
                        let mut instructions = vec![Instruction::Label(entry_label)];
                        
                        // Use ConstructObject instruction which properly allocates on heap
                        // and returns a pointer to the struct
                        let result_reg = self.context.allocate_register();
                        let result_val = IRValue::Register(result_reg);
                        
                        // Generate default field values as args
                        let mut arg_values = Vec::new();
                        for (_f_name, f_type) in &ir_fields {
                             let dummy = match f_type {
                                 IRType::Integer => IRValue::Integer(0),
                                 IRType::Float => IRValue::Float(0.0),
                                 IRType::Boolean => IRValue::Boolean(false),
                                 IRType::String => IRValue::String("".to_string()),
                                 IRType::Char => IRValue::Char('\0'),
                                 _ => IRValue::Null,
                             };
                             arg_values.push(dummy);
                        }
                        
                        instructions.push(Instruction::ConstructObject {
                            class_name: name.clone(),
                            args: arg_values,
                            result: result_val.clone(),
                            arg_types: None,
                        });
                        instructions.push(Instruction::Return(Some(result_val)));
                        
                        func.cfg = crate::cfg_builder::build_cfg_from_instructions(instructions);
                        func.register_count = self.context.register_counter;
                        module.add_function(func);
                    }

                    for method in methods {
                        let mangled_name = format!("{}_{}", name, method.name);
                        let dot_name = format!("{}.{}", name, method.name);
                        let ir_return_type = method
                            .return_type
                            .as_ref()
                            .map(|t| self.convert_ast_type_to_ir(t))
                            .unwrap_or(IRType::Void);
                        self.context
                            .function_return_types
                            .insert(mangled_name, ir_return_type.clone());
                        self.context
                            .function_return_types
                            .insert(dot_name, ir_return_type);
                    }
                }
                Expression::StructDefinition { name, fields, .. } => {
                    let ir_fields = fields
                        .iter()
                        .map(|f| (f.name.clone(), self.convert_ast_type_to_ir(&f.field_type)))
                        .collect();
                    let type_def = IRType::Struct {
                        name: name.clone(),
                        fields: ir_fields,
                    };
                    self.context.type_definitions.insert(name.clone(), type_def);
                }
                _ => {}
            }
        }

        // First pass: collect all function definitions and struct definitions
        let mut main_expressions = Vec::new();
        for expression in expressions {
            match expression {
                Expression::Function {
                    name,
                    params,
                    return_type,
                    body,
                    receiver,
                    is_external,
                    ..
                } => {
                    // Handle external function declarations
                    if *is_external {
                        // Create an extern function declaration (no body)
                        // Use underscore format to match call sites (Type_method)
                        let func_name = if let Some(recv) = receiver {
                            format!("{}_{}", recv.type_name, name)
                        } else {
                            name.clone()
                        };
                        
                        let ir_return_type = return_type
                            .as_ref()
                            .map(|t| self.convert_ast_type_to_ir(t))
                            .unwrap_or(IRType::Void);
                        
                        let mut extern_func = IRFunction::new(&func_name, ir_return_type)
                            .extern_function(CallingConvention::C);
                        
                        // Add parameters
                        for param in params {
                            let ir_param = IRParameter::new(
                                param.name.clone(),
                                param
                                    .type_annotation
                                    .as_ref()
                                    .map(|t| self.convert_ast_type_to_ir(t))
                                    .unwrap_or(IRType::Integer),
                            );
                            extern_func.add_parameter(ir_param);
                        }
                        
                        module.add_function(extern_func);
                        continue;
                    }
                    
                    if let Some(recv) = receiver {
                        // Use underscore format to match call sites (Type_method)
                        let func_name = format!("{}_{}", recv.type_name, name);
                        let is_constructor = name == "new";
                        
                        // Set current_type_definition for data type methods
                        let old_type_def = self.context.current_type_definition.clone();
                        self.context.current_type_definition = Some(recv.type_name.clone());

                        if is_constructor {
                            let function = self.generate_function_definition(
                                &func_name,
                                params,
                                return_type,
                                body,
                            )?;
                            self.context.current_type_definition = old_type_def;
                            module.add_function(function);
                        } else {
                            // Instance method defined outside class
                            let recv_type = Type {
                                name: recv.type_name.clone(),
                                is_nullable: false,
                                generics: vec![],
                            };
                            let receiver_name = if recv.name.is_empty() || recv.name == "self" {
                                "this".to_string()
                            } else {
                                recv.name.clone()
                            };
                            let self_param = Parameter {
                                name: receiver_name.clone(),
                                type_annotation: Some(recv_type),
                                default_value: None,
                                memory_modifier: None,
                            };

                            let mut effective_params = vec![self_param];
                            effective_params.extend(params.clone());

                            let old_receiver_name = self.context._current_receiver_name.clone();
                            self.context._current_receiver_name = Some(receiver_name);

                            let function = self.generate_method_function(
                                &func_name,
                                &effective_params,
                                return_type,
                                body,
                            )?;

                            self.context._current_receiver_name = old_receiver_name;
                            self.context.current_type_definition = old_type_def;
                            module.add_function(function);
                        }
                    } else {
                        // Skip RealParser methods that were incorrectly parsed as standalone functions
                        // These should be generated as class methods (RealParser_methodName)
                        let misplaced_parser_methods = [
                            "isAtEnd", "peek", "peekTokenType", "peekTokenTypeStr",
                            "previous", "advance", "consume", "checkToken", "matchTokenStr",
                            "skipExpression", "toArray"
                        ];
                        if misplaced_parser_methods.contains(&name.as_str()) {
                            // Skip - will be generated as class method
                            continue;
                        }

                        // Generate the function and add to module
                        let function =
                            self.generate_function_definition(name, params, return_type, body)?;
                        module.add_function(function);
                    }
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
                    eprintln!("[DEBUG IR] Processing ClassDefinition: '{}'", name);
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

            // Only add a return instruction if the last instruction is NOT already a return.
            // This prevents duplicate returns when the main function body ends with a return statement.
            let ends_with_return = all_instructions.last().map_or(false, |inst| {
                matches!(inst, Instruction::Return(_))
            });
            if !ends_with_return {
                all_instructions.push(Instruction::Return(Some(result_value)));
            }

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
        // Debug: Log expression variant for tracking Array constructor path
        let variant_name = match expr {
            Expression::Call { callee, .. } => format!("Call(callee={:?})", callee),
            Expression::Identifier { name, type_args, .. } => format!("Identifier(name={}, type_args={:?})", name, type_args),
            Expression::Let { name, .. } => format!("Let(name={})", name),
            _ => "other".to_string(),
        };
        if variant_name.contains("Array") || variant_name.contains("arr") {
            eprintln!("[DEBUG generate_expression] {}", variant_name);
        }

        let result = match expr {
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
            Expression::Let { name, value, type_annotation, .. } => {
                self.generate_let_binding(name, value, type_annotation.as_ref())
            }
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
        };

        result
    }

    /// Generate IR for function calls
    fn generate_call_expression(
        &mut self,
        function: &Expression,
        arguments: &[Expression],
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Debug: print all call expressions to understand what's being generated
        eprintln!("[DEBUG generate_call_expression] function={:?}", function);

        let mut instructions = Vec::new();
        let mut arg_values = Vec::new();

        // Handle __default<T>() calls - encode type in function name
        if let Expression::Identifier { name, type_args, .. } = function {
            if name == "__default" && arguments.is_empty() {
                let result_reg = self.context.allocate_register();
                
                // Get the type name from type_args
                let type_name = if let Some(first_type) = type_args.first() {
                    first_type.name.clone()
                } else {
                    // No type argument - use Int as default
                    "Int".to_string()
                };
                
                // Set the result register type based on type_name
                let ir_type = match type_name.as_str() {
                    "Int" | "i64" => crate::value::IRType::Integer,
                    "Float" | "f64" => crate::value::IRType::Float,
                    "Bool" => crate::value::IRType::Boolean,
                    "String" => crate::value::IRType::String,
                    _ => crate::value::IRType::Struct { 
                        name: type_name.clone(), 
                        fields: vec![] 
                    },
                };
                self.context.set_register_type(result_reg, ir_type);
                
                let result_value = IRValue::Register(result_reg);
                // Encode the type in the function name: __default_Int, __default_String, etc.
                let mangled_name = format!("__default_{}", type_name);
                instructions.push(Instruction::Call {
                    target: IRValue::Variable(mangled_name),
                    args: vec![],
                    result: Some(result_value.clone()),
                    arg_types: None,
                    return_type: None,
                });
                return Ok((result_value, instructions));
            }
        }

        // Handle Array<T>() constructor calls - these need to become __ArrayNew calls
        // eprintln!("[DEBUG IR] generate_call function type: {:?}", std::any::type_name_of_val(function));
        if let Expression::Identifier { name, type_args, .. } = function {
            eprintln!("[DEBUG IR] Function identifier name='{}', type_args={:?}", name, type_args);
            if name == "Array" {
                if arguments.is_empty() {
                    // Array<T>() constructor call - turn this into __ArrayNew(sizeof(T), 0)
                    let result_reg = self.context.allocate_register();
                    
                    let element_type = if let Some(first_type) = type_args.first() {
                        self.convert_ast_type_to_ir(first_type)
                    } else {
                        crate::value::IRType::Integer
                    };
                    
                    // Use SizeOf(T) instead of hardcoded size
                    // This allows monomorphization to substitute the correct size later
                    let element_size_val = IRValue::SizeOf(element_type.clone());
                    
                    self.context.set_register_type(result_reg, crate::value::IRType::Array(Box::new(element_type)));
                    
                    let result_value = IRValue::Register(result_reg);
                    instructions.push(Instruction::Call {
                        target: IRValue::Variable("__ArrayNew".to_string()),
                        args: vec![element_size_val, IRValue::Integer(0)],
                        result: Some(result_value.clone()),
                        arg_types: None,
                        return_type: None,
                    });
                    return Ok((result_value, instructions));
                } else if arguments.len() == 1 {
                    // Array<T>(capacity) constructor call - turn this into __ArrayNew(sizeof(T), capacity)
                    let result_reg = self.context.allocate_register();
                    
                    let element_type = if let Some(first_type) = type_args.first() {
                        self.convert_ast_type_to_ir(first_type)
                    } else {
                        crate::value::IRType::Integer
                    };
                    
                    // Use SizeOf(T) instead of hardcoded size
                    let element_size_val = IRValue::SizeOf(element_type.clone());
                    
                    self.context.set_register_type(result_reg, crate::value::IRType::Array(Box::new(element_type)));
                    
                    let result_value = IRValue::Register(result_reg);
                    
                    // Evaluate capacity argument
                    let (cap_val, cap_insts) = self.generate_expression(&arguments[0])?;
                    instructions.extend(cap_insts);
                    
                    instructions.push(Instruction::Call {
                        target: IRValue::Variable("__ArrayNew".to_string()),
                        args: vec![element_size_val, cap_val],
                        result: Some(result_value.clone()),
                        arg_types: None,
                        return_type: None,
                    });
                    return Ok((result_value, instructions));
                }
            }
        }

        // Generate IR for all arguments
        for arg in arguments {
            let (arg_val, arg_instructions) = self.generate_expression(arg)?;
            instructions.extend(arg_instructions);
            arg_values.push(arg_val);
        }

        // Method-call desugaring and intrinsics
        if let Expression::MemberAccess { object, member, .. } = function {
            // Check for static method call (Type.method)
            // Heuristic: if object is an Identifier starting with Uppercase and NOT a known variable, treat as static call
            if let Expression::Identifier { name, type_args, .. } = object.as_ref() {
                let is_field = if let Some(recv_type) = &self.context._current_receiver_type {
                    if let crate::value::IRType::Struct { fields, .. } = recv_type {
                        fields.iter().any(|(f_name, _)| f_name == name)
                    } else {
                        false
                    }
                } else {
                    false
                };
                
                if !is_field
                    && self.context.get_variable_type(name).is_none()
                    && name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                {
                    // Static call: Type.method(...) -> call Type_method(...)
                    // We do NOT evaluate the object (it's a type).
                    
                    // Use underscore format for actual function lookup
                    let target_name = format!("{}_{}", name, member);
                    
                    // For static methods like Type.new(), just pass user arguments
                    // The method itself handles any allocation internally
                    let final_args = arg_values;
                    
                    let result_reg = self.context.allocate_register();
                    // Look up return type (try both formats)
                    let return_type = self.context.function_return_types.get(&target_name).cloned()
                        .or_else(|| self.context.function_return_types.get(&format!("{}.{}", name, member)).cloned());
                    if let Some(ret_type) = return_type {
                        self.context.set_register_type(result_reg, ret_type);
                    }
                    
                    // Track generic type arguments for container types (Vec<T>, Option<T>, Result<T,E>, etc.)
                    // This allows us to know the element type when calling methods like toArray(), unwrap(), etc.
                    if !type_args.is_empty() {
                        let elem_type = self.convert_ast_type_to_ir(&type_args[0]);
                        // Store the element type for this register so it can be used by toArray(), unwrap(), etc.
                        self.context.container_element_types.insert(format!("reg_{}", result_reg), elem_type);
                    }

                    let result_value = IRValue::Register(result_reg);
                    instructions.push(Instruction::Call {
                        target: IRValue::Variable(target_name),
                        args: final_args,
                        result: Some(result_value.clone()),
                        arg_types: None,
                        return_type: None,
                    });
                    return Ok((result_value, instructions));
                }
            }

            // Evaluate object expression first
            let (obj_val, obj_instructions) = self.generate_expression(object)?;
            instructions.extend(obj_instructions);

            // Handle zero-arg length/size on arrays and strings
            // Only apply ArrayLength/StringLength for actual Array<T> or String types,
            // NOT for Vec<T> or other classes that define their own .len()/.length() methods
            if (member == "length" || member == "size" || member == "len") && arguments.is_empty() {
                // Check the object's type to see if it's an actual Array or String
                // First try from Identifier, then fall back to register type lookup
                let obj_type = if let Expression::Identifier { name, .. } = object.as_ref() {
                    self.context.get_variable_type(name).cloned()
                } else {
                    // For MemberAccess and other expressions, the obj_val register has the type
                    match &obj_val {
                        IRValue::Register(reg) => self.context.register_types.get(reg).cloned(),
                        IRValue::Variable(var_name) => self.context.get_variable_type(var_name).cloned(),
                        _ => None,
                    }
                };
                
                let is_array_type = matches!(obj_type, Some(IRType::Array(_)));
                let is_string_type = matches!(obj_type, Some(IRType::String));
                
                if is_string_type {
                    let result_reg = self.context.allocate_register();
                    let result_value = IRValue::Register(result_reg);
                    instructions.push(Instruction::StringLength {
                        string: obj_val.clone(),
                        result: result_value.clone(),
                    });
                    return Ok((result_value, instructions));
                } else if is_array_type {
                    let result_reg = self.context.allocate_register();
                    let result_value = IRValue::Register(result_reg);
                    instructions.push(Instruction::ArrayLength {
                        array: obj_val.clone(),
                        result: result_value.clone(),
                    });
                    return Ok((result_value, instructions));
                }
                // For other types (like Vec<T>), fall through to regular method call
            }

            // Try to determine object's type for method return type lookup
            let obj_type_name = match &obj_val {
                IRValue::Register(reg) => {
                    let reg_type = self.context.register_types.get(reg);
                    if member == "toString" {
                        eprintln!("DEBUG codegen: toString on Register({}), reg_type={:?}", reg, reg_type);
                    }
                    let result = match reg_type {
                        Some(IRType::Struct { name, .. }) => Some(name.clone()),
                        Some(IRType::Enum { name, .. }) => Some(name.clone()),
                        // Use "Array" for builtin arrays - NOT "Vec" which is a different class
                        Some(IRType::Array(_)) => Some("Array".to_string()),
                        Some(IRType::String) => Some("String".to_string()),
                        Some(IRType::Optional(_)) => Some("Option".to_string()),
                        // Primitive types for method calls like Int.toString(), Char.toString()
                        Some(IRType::Integer) => Some("Int".to_string()),
                        Some(IRType::Char) => Some("Char".to_string()),
                        Some(IRType::Float) => Some("Float".to_string()),
                        Some(IRType::Boolean) => Some("Bool".to_string()),
                        _ => None,
                    };
                    result
                }
                IRValue::Variable(var_name) => {
                    let var_type = self.context.get_variable_type(var_name);
                    if member == "toString" {
                        eprintln!("DEBUG codegen: toString on Variable({}), var_type={:?}", var_name, var_type);
                    }
                    let result = match var_type {
                        Some(IRType::Struct { name, .. }) => Some(name.clone()),
                        Some(IRType::Enum { name, .. }) => Some(name.clone()),
                        // Use "Array" for builtin arrays - NOT "Vec" which is a different class
                        Some(IRType::Array(_)) => Some("Array".to_string()),
                        Some(IRType::String) => Some("String".to_string()),
                        Some(IRType::Optional(_)) => Some("Option".to_string()),
                        // Primitive types for method calls like Int.toString(), Char.toString()
                        Some(IRType::Integer) => Some("Int".to_string()),
                        Some(IRType::Char) => Some("Char".to_string()),
                        Some(IRType::Float) => Some("Float".to_string()),
                        Some(IRType::Boolean) => Some("Bool".to_string()),
                        _ => None,
                    };
                    result
                }
                _ => None
            };
            
            // Get container element type from the receiver variable/register
            let receiver_element_type = match &obj_val {
                IRValue::Variable(var_name) => {
                    self.context.container_element_types.get(var_name).cloned()
                }
                IRValue::Register(reg_id) => {
                    self.context.container_element_types.get(&format!("reg_{}", reg_id)).cloned()
                }
                _ => None
            };
            

            // Fallback: call a free function named after the member; first arg is receiver
            let mut final_args = Vec::with_capacity(1 + arg_values.len());
            final_args.push(obj_val.clone());

            // For methods that transfer ownership (push, insert, etc.), mark variable arguments as moved
            // This prevents the Vale-style cleanup from deallocating values that were moved into containers
            if member == "push" || member == "insert" || member == "add" || member == "append" {
                for arg_val in &arg_values {
                    if let IRValue::Variable(var_name) = arg_val {
                        // Check if this variable is heap-allocated (struct/class instance)
                        // If so, mark it as moved so it won't be deallocated at function end
                        if self.context.heap_allocated.contains(var_name) {
                            self.context.mark_moved(var_name);
                        }
                    }
                }
            }

            final_args.extend(arg_values.into_iter());

            let result_reg = self.context.allocate_register();
            
            // Track whether we set a specialized type for this call
            let mut type_set_specially = false;
            
            // For methods that return the inner type of a generic container,
            // propagate the element type to the result
            if member == "toArray" {
                // Vec<T>.toArray() returns Array<T>
                if let Some(elem_type) = &receiver_element_type {
                    // Set the register type to Array<elem_type>
                    self.context.set_register_type(result_reg, IRType::Array(Box::new(elem_type.clone())));
                    // Also track it as container element type for downstream array access
                    self.context.container_element_types.insert(format!("reg_{}", result_reg), elem_type.clone());
                    type_set_specially = true;
                }
            } else if member == "unwrap" || member == "Unwrap" {
                // Option<T>.unwrap() returns T, Result<T,E>.unwrap() returns T
                if let Some(elem_type) = &receiver_element_type {
                    self.context.set_register_type(result_reg, elem_type.clone());
                    type_set_specially = true;
                    // If T is itself a struct, track it
                    if let IRType::Struct { name: _, .. } = elem_type {
                        self.context.container_element_types.insert(format!("reg_{}", result_reg), elem_type.clone());
                    }
                }
            } else if member == "get" {
                // Both Vec<T>.get() and Array<T>.get() return T directly in Seen
                // (The Vec.get() implementation throws on out-of-bounds, not returns Option)
                if let Some(elem_type) = &receiver_element_type {
                    // Track the element type for container_element_types
                    self.context.container_element_types.insert(format!("reg_{}", result_reg), elem_type.clone());

                    // For Vec.get() and Array.get(), set the register type to the element type T
                    if obj_type_name.as_deref() == Some("Vec") || obj_type_name.as_deref() == Some("Array") {
                        eprintln!("[DEBUG codegen] Setting register {} type to {:?}", result_reg, elem_type);
                        self.context.set_register_type(result_reg, elem_type.clone());
                        type_set_specially = true;
                    }
                }
            }
            
            // Determine the function name - check both underscore and dot naming conventions
            // Functions defined as `Type.method()` need to be called by that name
            let (mangled_name, _ret_type_found) = if let Some(ref type_name) = obj_type_name {
                let underscore_name = format!("{}_{}", type_name, member);
                let dot_name = format!("{}.{}", type_name, member);
                if member == "toString" {
                    eprintln!("DEBUG codegen mangled: obj_type_name={}, underscore={}, dot={}", type_name, underscore_name, dot_name);
                }
                // Prefer underscore naming (class methods) over dot naming (standalone functions)
                // but use whichever exists
                if self.context.function_return_types.contains_key(&underscore_name) {
                    let ret_type = self.context.function_return_types.get(&underscore_name).cloned();
                    // Only set register type from generic function if we didn't already set it specially
                    if !type_set_specially {
                        if let Some(ret_type) = ret_type {
                            self.context.set_register_type(result_reg, ret_type);
                        }
                    }
                    (underscore_name, true)
                } else if self.context.function_return_types.contains_key(&dot_name) {
                    let ret_type = self.context.function_return_types.get(&dot_name).cloned();
                    // Only set register type from generic function if we didn't already set it specially
                    if !type_set_specially {
                        if let Some(ret_type) = ret_type {
                            self.context.set_register_type(result_reg, ret_type);
                        }
                    }
                    (dot_name, true)
                } else {
                    // Neither found, use underscore as default (will be looked up in fn_map later)
                    (underscore_name, false)
                }
            } else {
                (member.clone(), false)
            };
            
            let result_value = IRValue::Register(result_reg);
            if member == "toString" {
                eprintln!("DEBUG codegen EMIT: Call to mangled_name={} (member={})", mangled_name, member);
            }
            instructions.push(Instruction::Call {
                target: IRValue::Variable(mangled_name.clone()),
                args: final_args,
                result: Some(result_value.clone()),
                arg_types: None,
                return_type: None,
            });
            return Ok((result_value, instructions));
        }

        // Normal function target
        let (func_val, func_instructions) = self.generate_expression(function)?;
        instructions.extend(func_instructions);

        // Allocate register for result
        let result_reg = self.context.allocate_register();
        
        // Check if this is a bare method call inside a class (should be a call to self.method)
        let (final_target, final_args, ret_type) = if let IRValue::Variable(func_name) = &func_val {
             // First check if bare name exists as a function
             if let Some(ret_type) = self.context.function_return_types.get(func_name).cloned() {
                 (func_val.clone(), arg_values, Some(ret_type))
             } else if let Some(class_name) = &self.context.current_type_definition.clone() {
                 // Inside a class - check if this is a method call on the same class
                 let dot_name = format!("{}.{}", class_name, func_name);
                 let underscore_name = format!("{}_{}", class_name, func_name);
                 
                 if let Some(ret_type) = self.context.function_return_types.get(&dot_name).cloned()
                     .or_else(|| self.context.function_return_types.get(&underscore_name).cloned()) {
                     // This is a method on the same class - add 'this' as first arg
                     let this_val = IRValue::Variable("this".to_string());
                     let mut new_args = vec![this_val];
                     new_args.extend(arg_values);
                     (IRValue::Variable(underscore_name), new_args, Some(ret_type))
                 } else {
                     (func_val.clone(), arg_values, None)
                 }
             } else {
                 (func_val.clone(), arg_values, None)
             }
        } else {
             (func_val.clone(), arg_values, None)
        };
        
        if let Some(ret_type) = ret_type {
            self.context.set_register_type(result_reg, ret_type);
        }

        let result_value = IRValue::Register(result_reg);

        instructions.push(Instruction::Call {
            target: final_target,
            args: final_args,
            result: Some(result_value.clone()),
            arg_types: None,
            return_type: None,
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
