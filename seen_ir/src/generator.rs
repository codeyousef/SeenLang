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
        
        // First pass: collect all function definitions
        let mut main_expressions = Vec::new();
        for expression in expressions {
            match expression {
                Expression::Function { name, params, return_type, body, .. } => {
                    // Generate the function and add to module
                    let function = self.generate_function_definition(name, params, return_type, body)?;
                    module.add_function(function);
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
        let mut entry_block = BasicBlock::new(entry_label.clone());
        self.context.current_block = Some(entry_label.0.clone());
        
        // Generate IR for main expressions
        let mut result_value = IRValue::Integer(0); // Default return value
        
        for expression in main_expressions {
            let (value, instructions) = self.generate_expression(expression)?;
            
            // Add all instructions to the current block
            for instruction in instructions {
                entry_block.add_instruction(instruction);
            }
            
            result_value = value;
        }
        
        // Add return instruction
        entry_block.add_instruction(Instruction::Return(Some(result_value)));
        
        // Update function register count
        main_function.register_count = self.context.register_counter;
        
        main_function.add_block(entry_block);
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
            Expression::StructLiteral { fields, .. } => {
                self.generate_struct_literal(fields)
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
        
        // Jump to body if condition is true, otherwise exit
        instructions.push(Instruction::JumpIf {
            condition: cond_val,
            target: loop_body.clone(),
        });
        instructions.push(Instruction::Jump(loop_end.clone()));
        
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
            type_name: "Unknown".to_string(), // Would need type information
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
        let mut entry_block = BasicBlock::new(entry_label.clone());
        self.context.current_block = Some(entry_label.0.clone());
        
        // Generate function body
        let (result_value, instructions) = self.generate_expression(body)?;
        
        // Add instructions to entry block
        for instruction in instructions {
            entry_block.add_instruction(instruction);
        }
        
        // Add return instruction
        entry_block.add_instruction(Instruction::Return(Some(result_value)));
        
        // Update function register count
        function.register_count = self.context.register_counter;
        
        // Add block to function
        function.add_block(entry_block);
        
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
        
        let result = generator.generate(&expressions);
        assert!(result.is_ok());
        
        let program = result.unwrap();
        assert!(!program.modules.is_empty());
        assert_eq!(program.entry_point, Some("main".to_string()));
    }
}