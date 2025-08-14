//! IR generation from AST for the Seen programming language

use std::collections::HashMap;
use seen_parser::{Expression, BinaryOperator, UnaryOperator, Program};
use seen_parser::Parameter as ASTParameter;
use crate::{
    IRProgram, IRResult, IRError,
    value::{IRValue, IRType},
    instruction::{Instruction, BasicBlock, Label, BinaryOp, UnaryOp},
    function::{IRFunction, Parameter},
    module::IRModule,
};

/// Context for IR generation
#[derive(Debug)]
pub struct GenerationContext {
    pub current_function: Option<String>,
    pub current_block: Option<String>,
    pub variable_types: HashMap<String, IRType>,
    pub register_counter: u32,
    pub label_counter: u32,
    pub break_stack: Vec<String>, // Labels for break statements
    pub continue_stack: Vec<String>, // Labels for continue statements
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
                Expression::Function { name, params, return_type, body, .. } => {
                    // Generate the function and add to module
                    let function = self.generate_function_definition(name, params, return_type, body)?;
                    module.add_function(function);
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
                other => {
                    // Regular expression, add to main function body
                    main_expressions.push(other);
                }
            }
        }
        
        // Create main function with remaining expressions
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
        
        program.add_module(module);
        program.set_entry_point("main".to_string());
        
        Ok(program)
    }
    
    /// Generate IR for a single expression
    pub fn generate_expression(&mut self, expr: &Expression) -> IRResult<(IRValue, Vec<Instruction>)> {
        match expr {
            Expression::IntegerLiteral { value, .. } => {
                Ok((IRValue::Integer(*value), Vec::new()))
            },
            Expression::FloatLiteral { value, .. } => {
                Ok((IRValue::Float(*value), Vec::new()))
            },
            Expression::StringLiteral { value, .. } => {
                Ok((IRValue::String(value.clone()), Vec::new()))
            },
            Expression::BooleanLiteral { value, .. } => {
                Ok((IRValue::Boolean(*value), Vec::new()))
            },
            Expression::NullLiteral { .. } => {
                Ok((IRValue::Null, Vec::new()))
            },
            Expression::Identifier { name, .. } => self.generate_variable(name),
            Expression::BinaryOp { left, op, right, .. } => {
                self.generate_binary_expression(left, op, right)
            },
            Expression::UnaryOp { op, operand, .. } => {
                self.generate_unary_expression(op, operand)
            },
            Expression::Call { callee, args, .. } => {
                self.generate_call_expression(callee, args)
            },
            Expression::Assignment { target, value, .. } => {
                self.generate_assignment(target, value)
            },
            Expression::If { condition, then_branch, else_branch, .. } => {
                self.generate_if_expression(condition, then_branch, else_branch.as_deref())
            },
            Expression::While { condition, body, .. } => {
                self.generate_while_expression(condition, body)
            },
            Expression::Block { expressions, .. } => {
                self.generate_block_expression(expressions)
            },
            Expression::IndexAccess { object, index, .. } => {
                self.generate_index_access(object, index)
            },
            Expression::MemberAccess { object, member, .. } => {
                self.generate_member_access(object, member)
            },
            Expression::ArrayLiteral { elements, .. } => {
                self.generate_array_literal(elements)
            },
            Expression::StructLiteral { name, fields, .. } => {
                self.generate_struct_literal(name, fields)
            },
            Expression::InterpolatedString { parts, .. } => {
                self.generate_string_interpolation(parts)
            },
            Expression::Let { name, value, .. } => {
                self.generate_let_binding(name, value)
            },
            Expression::Return { value, .. } => {
                self.generate_return_expression(value.as_deref())
            },
            Expression::Function { name, params, body, .. } => {
                self.generate_function_expression(name, params, body)
            },
            Expression::For { variable, iterable, body, .. } => {
                self.generate_for_expression(variable, iterable, body)
            },
            Expression::Break { value, .. } => {
                self.generate_break_expression(value.as_deref())
            },
            Expression::Continue { .. } => {
                self.generate_continue_expression()
            },
            Expression::Match { expr, arms, .. } => {
                self.generate_match_expression(expr, arms)
            },
            Expression::EnumLiteral { enum_name, variant_name, fields, .. } => {
                self.generate_enum_literal(enum_name, variant_name, fields)
            },
            // Handle other expression types...
            _ => Err(IRError::Other(format!("Unsupported expression type: {:?}", expr))),
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
        right: &Expression
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (left_val, mut left_instructions) = self.generate_expression(left)?;
        let (right_val, right_instructions) = self.generate_expression(right)?;
        
        left_instructions.extend(right_instructions);
        
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
            BinaryOperator::InclusiveRange => return Err(IRError::Other("Range operators not yet implemented".to_string())),
            BinaryOperator::ExclusiveRange => return Err(IRError::Other("Range operators not yet implemented".to_string())),
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
        operand: &Expression
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
        arguments: &[Expression]
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        let mut arg_values = Vec::new();
        
        // Generate IR for all arguments
        for arg in arguments {
            let (arg_val, arg_instructions) = self.generate_expression(arg)?;
            instructions.extend(arg_instructions);
            arg_values.push(arg_val);
        }
        
        // Generate function target
        let (func_val, func_instructions) = self.generate_expression(function)?;
        instructions.extend(func_instructions);
        
        // Allocate register for result
        let result_reg = self.context.allocate_register();
        let result_value = IRValue::Register(result_reg);
        
        let call_instruction = Instruction::Call {
            target: func_val,
            args: arg_values,
            result: Some(result_value.clone()),
        };
        
        instructions.push(call_instruction);
        
        Ok((result_value, instructions))
    }
    
    /// Generate IR for assignment
    fn generate_assignment(
        &mut self,
        target: &Expression,
        value: &Expression
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (value_val, mut value_instructions) = self.generate_expression(value)?;
        
        match target {
            Expression::Identifier { name, .. } => {
                let target_val = IRValue::Variable(name.clone());
                
                let store_instruction = Instruction::Store {
                    value: value_val.clone(),
                    dest: target_val,
                };
                
                value_instructions.push(store_instruction);
                Ok((value_val, value_instructions))
            },
            Expression::IndexAccess { object, index, .. } => {
                let (obj_val, obj_instructions) = self.generate_expression(object)?;
                let (idx_val, idx_instructions) = self.generate_expression(index)?;
                
                value_instructions.extend(obj_instructions);
                value_instructions.extend(idx_instructions);
                
                let array_set_instruction = Instruction::ArraySet {
                    array: obj_val,
                    index: idx_val,
                    value: value_val.clone(),
                };
                
                value_instructions.push(array_set_instruction);
                Ok((value_val, value_instructions))
            },
            Expression::MemberAccess { object, member, .. } => {
                let (obj_val, obj_instructions) = self.generate_expression(object)?;
                
                value_instructions.extend(obj_instructions);
                
                let field_set_instruction = Instruction::FieldSet {
                    struct_val: obj_val,
                    field: member.clone(),
                    value: value_val.clone(),
                };
                
                value_instructions.push(field_set_instruction);
                Ok((value_val, value_instructions))
            },
            _ => Err(IRError::Other("Invalid assignment target".to_string())),
        }
    }
    
    /// Generate IR for if expressions
    fn generate_if_expression(
        &mut self,
        condition: &Expression,
        then_branch: &Expression,
        else_branch: Option<&Expression>
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
        body: &Expression
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        
        // For now, only handle range expressions
        match iterable {
            Expression::BinaryOp { left, op, right, .. } => {
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
                        
                        self.context.push_loop_labels(loop_end.0.clone(), loop_start.0.clone());
                        
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
                    _ => Err(IRError::Other("For loops only support range iterables currently".to_string()))
                }
            }
            _ => Err(IRError::Other("For loops only support range iterables currently".to_string()))
        }
    }
    
    /// Generate IR for break expressions
    fn generate_break_expression(
        &mut self,
        value: Option<&Expression>
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
            Ok((IRValue::Void, vec![Instruction::Jump(Label::new(continue_label.clone()))]))
        } else {
            Err(IRError::Other("Continue outside of loop".to_string()))
        }
    }
    
    /// Generate IR for while loops
    fn generate_while_expression(
        &mut self,
        condition: &Expression,
        body: &Expression
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        
        let loop_start = self.context.allocate_label("loop_start");
        let loop_body = self.context.allocate_label("loop_body");
        let loop_end = self.context.allocate_label("loop_end");
        
        // Push loop labels for break/continue
        self.context.push_loop_labels(loop_end.0.clone(), loop_start.0.clone());
        
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
        expressions: &[Expression]
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
        index: &Expression
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
        member: &str
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
        elements: &[Expression]
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
        fields: &[(String, Expression)]
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
        parts: &[seen_parser::InterpolationPart]
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
                },
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
        value: &Expression
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let (value_val, mut instructions) = self.generate_expression(value)?;
        
        // Store the variable mapping
        let var_val = IRValue::Variable(name.to_string());
        
        instructions.push(Instruction::Store {
            value: value_val.clone(),
            dest: var_val,
        });
        
        // Let expressions return the bound value
        Ok((value_val, instructions))
    }
    
    /// Generate IR for return expressions
    fn generate_return_expression(
        &mut self,
        value: Option<&Expression>
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
        body: &Expression
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
            function.parameters.push(crate::function::Parameter::new(param.name.clone(), param_type));
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
            self.context.set_variable_type(param.name.clone(), param_type);
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
    
    /// Generate IR for function expressions (now deprecated - use generate_function_definition)
    fn generate_function_expression(
        &mut self,
        name: &str,
        _params: &[ASTParameter],
        _body: &Expression
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Function expressions should not occur in the main generation flow anymore
        // They're handled at the module level
        Ok((IRValue::Variable(format!("function_{}", name)), Vec::new()))
    }
    
    /// Generate IR for match expressions
    fn generate_match_expression(
        &mut self,
        expr: &Expression,
        arms: &[seen_parser::MatchArm]
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
                },
                seen_parser::Pattern::Wildcard => {
                    // Wildcard always matches, jump directly to this arm
                    instructions.push(Instruction::Jump(arm_label.clone()));
                    break; // No need to check further patterns after wildcard
                },
                seen_parser::Pattern::Identifier(_name) => {
                    // Identifier pattern always matches (binds the value to the identifier)
                    // For now, treat as wildcard - TODO: implement variable binding
                    instructions.push(Instruction::Jump(arm_label.clone()));
                    break;
                },
                seen_parser::Pattern::Enum { variant, fields, .. } => {
                    // For enum patterns, we need to check the enum tag
                    // For now, implement basic enum variant matching without field destructuring
                    // TODO: Implement proper enum tag checking and field extraction
                    
                    // Generate a placeholder comparison - this needs proper enum tag checking
                    let cmp_reg = self.context.allocate_register();
                    let cmp_result = IRValue::Register(cmp_reg);
                    
                    // This is a placeholder - in a real implementation we'd:
                    // 1. Extract the tag from the enum value
                    // 2. Compare it with the expected variant tag
                    // 3. If fields.len() > 0, extract and bind field values
                    
                    instructions.push(Instruction::Binary {
                        op: crate::instruction::BinaryOp::Equal,
                        left: match_val.clone(),
                        right: IRValue::StringConstant(0), // Placeholder
                        result: cmp_result.clone(),
                    });
                    
                    instructions.push(Instruction::JumpIf {
                        condition: cmp_result,
                        target: arm_label.clone(),
                    });
                },
                _ => return Err(IRError::Other("Complex patterns not yet implemented".to_string())),
            }
        }
        
        // If no patterns matched and no wildcard, jump to end (this shouldn't happen with exhaustive patterns)
        if !arms.iter().any(|arm| matches!(arm.pattern, seen_parser::Pattern::Wildcard)) {
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
        fields: &[seen_parser::StructField]
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
        variants: &[seen_parser::EnumVariant]
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

    /// Generate IR for enum literal construction
    fn generate_enum_literal(
        &mut self, 
        enum_name: &str,
        variant_name: &str,
        fields: &[Expression]
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        let mut instructions = Vec::new();
        
        // Generate IR for the field values
        let mut field_values = Vec::new();
        for field in fields {
            let (value, field_instructions) = self.generate_expression(field)?;
            instructions.extend(field_instructions);
            field_values.push(value);
        }
        
        // For now, we'll represent enum literals as function calls to constructor functions
        // In a more complete implementation, we would create proper enum constructor IR
        let result_register = self.context.allocate_register();
        let result_value = IRValue::Register(result_register);
        
        // Create constructor call instruction
        // This is a placeholder - in a full implementation we would have dedicated enum construction
        let constructor_name = format!("{}::{}", enum_name, variant_name);
        let call_instruction = Instruction::Call {
            target: IRValue::GlobalVariable(constructor_name),
            args: field_values,
            result: Some(result_value.clone()),
        };
        
        instructions.push(call_instruction);
        
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
        let literal = Expression::IntegerLiteral { value: 42, pos: seen_parser::Position::new(1, 1, 0) };
        
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
            left: Box::new(Expression::IntegerLiteral { value: 5, pos: seen_parser::Position::new(1, 1, 0) }),
            op: BinaryOperator::Add,
            right: Box::new(Expression::IntegerLiteral { value: 3, pos: seen_parser::Position::new(1, 1, 0) }),
            pos: seen_parser::Position::new(1, 1, 0),
        };
        
        let result = generator.generate_expression(&expr);
        assert!(result.is_ok());
        
        let (value, instructions) = result.unwrap();
        assert!(matches!(value, IRValue::Register(_)));
        assert_eq!(instructions.len(), 1);
        
        if let Instruction::Binary { op, left, right, result: _ } = &instructions[0] {
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
        let expressions = vec![
            Expression::IntegerLiteral { value: 42, pos: seen_parser::Position::new(1, 1, 0) }
        ];
        
        let result = generator.generate_expressions(&expressions);
        assert!(result.is_ok());
        
        let program = result.unwrap();
        assert!(!program.modules.is_empty());
        assert_eq!(program.entry_point, Some("main".to_string()));
    }
}