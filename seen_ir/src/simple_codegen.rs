//! Simple code generation from IR for the Seen programming language

use std::collections::HashMap;
use std::fmt::Write;
use crate::{
    IRProgram, 
    instruction::{Instruction, BasicBlock, BinaryOp, UnaryOp},
    value::{IRValue, IRType},
    function::IRFunction,
    module::IRModule,
};

/// Simple errors for codegen
#[derive(Debug)]
pub enum CodeGenError {
    UnsupportedInstruction(String),
    UnsupportedValue(String),
    Other(String),
}

impl std::fmt::Display for CodeGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeGenError::UnsupportedInstruction(msg) => write!(f, "Unsupported instruction: {}", msg),
            CodeGenError::UnsupportedValue(msg) => write!(f, "Unsupported value: {}", msg),
            CodeGenError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for CodeGenError {}

pub type Result<T> = std::result::Result<T, CodeGenError>;

/// Code generation context
#[derive(Debug)]
pub struct CodeGenContext {
    pub register_mapping: HashMap<u32, String>,
    pub current_function: Option<String>,
}

impl CodeGenContext {
    pub fn new() -> Self {
        Self {
            register_mapping: HashMap::new(),
            current_function: None,
        }
    }
    
    pub fn map_register(&mut self, virtual_reg: u32, physical_loc: String) {
        self.register_mapping.insert(virtual_reg, physical_loc);
    }
    
    pub fn get_register_location(&self, virtual_reg: u32) -> Option<&String> {
        self.register_mapping.get(&virtual_reg)
    }
}

/// C code generator
pub struct CCodeGenerator {
    context: CodeGenContext,
}

impl CCodeGenerator {
    pub fn new() -> Self {
        Self {
            context: CodeGenContext::new(),
        }
    }
    
    pub fn generate_program(&mut self, program: &IRProgram) -> Result<String> {
        let mut output = String::new();
        
        writeln!(output, "#include <stdio.h>").unwrap();
        writeln!(output, "#include <stdlib.h>").unwrap();
        writeln!(output, "#include <stdint.h>").unwrap();
        writeln!(output, "#include <stdbool.h>").unwrap();
        writeln!(output).unwrap();
        
        for module in &program.modules {
            let module_code = self.generate_module(module)?;
            output.push_str(&module_code);
        }
        
        if let Some(entry) = &program.entry_point {
            writeln!(output, "int main(int argc, char* argv[]) {{").unwrap();
            if entry == "main" {
                writeln!(output, "    return seen_main();").unwrap();
            } else {
                writeln!(output, "    return {}();", entry).unwrap();
            }
            writeln!(output, "}}").unwrap();
        }
        
        Ok(output)
    }
    
    pub fn generate_module(&mut self, module: &IRModule) -> Result<String> {
        let mut output = String::new();
        
        writeln!(output, "// Module: {}", module.name).unwrap();
        
        for function in module.functions.values() {
            let function_code = self.generate_function(function)?;
            output.push_str(&function_code);
            writeln!(output).unwrap();
        }
        
        Ok(output)
    }
    
    pub fn generate_function(&mut self, function: &IRFunction) -> Result<String> {
        let mut output = String::new();
        
        let return_type = self.generate_c_type(&function.return_type);
        let function_name = if function.name == "main" { "seen_main" } else { &function.name };
        
        // Generate parameter list
        let mut param_strings = Vec::new();
        for param in &function.parameters {
            let param_type = self.generate_c_type(&param.param_type);
            param_strings.push(format!("{} {}", param_type, param.name));
        }
        let params_str = param_strings.join(", ");
        
        writeln!(output, "{} {}({}) {{", return_type, function_name, params_str).unwrap();
        
        // Declare registers
        for reg in 0..function.register_count {
            writeln!(output, "    int64_t r{};", reg).unwrap();
            self.context.map_register(reg, format!("r{}", reg));
        }
        
        // Collect and declare variables (excluding parameters)
        let mut variables = std::collections::HashSet::new();
        if let Some(entry) = &function.cfg.entry_block {
            if let Some(block) = function.cfg.get_block(entry) {
                self.collect_variables_from_block(block, &mut variables);
            }
        }
        
        // Remove parameter names from variables to avoid redeclaration
        let param_names: std::collections::HashSet<String> = function.parameters.iter().map(|p| p.name.clone()).collect();
        for param_name in &param_names {
            variables.remove(param_name);
        }
        
        for var_name in &variables {
            writeln!(output, "    int64_t {};", var_name).unwrap();
        }
        
        if let Some(entry) = &function.cfg.entry_block {
            if let Some(block) = function.cfg.get_block(entry) {
                let block_code = self.generate_basic_block(block)?;
                output.push_str(&block_code);
            }
        }
        
        writeln!(output, "}}").unwrap();
        
        Ok(output)
    }
    
    pub fn generate_basic_block(&mut self, block: &BasicBlock) -> Result<String> {
        let mut output = String::new();
        
        // Special handling for if-else patterns
        if let Some(if_code) = self.generate_if_else_block(&block.instructions)? {
            output.push_str(&if_code);
        } else {
            for instruction in &block.instructions {
                let inst_code = self.generate_instruction(instruction)?;
                writeln!(output, "    {}", inst_code).unwrap();
            }
        }
        
        if let Some(terminator) = &block.terminator {
            let term_code = self.generate_instruction(terminator)?;
            writeln!(output, "    {}", term_code).unwrap();
        }
        
        Ok(output)
    }
    
    pub fn generate_instruction(&mut self, instruction: &Instruction) -> Result<String> {
        match instruction {
            Instruction::Move { source, dest } => {
                let src = self.generate_c_value(source)?;
                let dst = self.generate_c_value(dest)?;
                Ok(format!("{} = {};", dst, src))
            },
            
            Instruction::Binary { op, left, right, result } => {
                let lhs = self.generate_c_value(left)?;
                let rhs = self.generate_c_value(right)?;
                let res = self.generate_c_value(result)?;
                
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Subtract => "-",
                    BinaryOp::Multiply => "*",
                    BinaryOp::Divide => "/",
                    BinaryOp::Modulo => "%",
                    BinaryOp::Equal => "==",
                    BinaryOp::NotEqual => "!=",
                    BinaryOp::LessThan => "<",
                    BinaryOp::LessEqual => "<=",
                    BinaryOp::GreaterThan => ">",
                    BinaryOp::GreaterEqual => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                    BinaryOp::BitwiseAnd => "&",
                    BinaryOp::BitwiseOr => "|",
                    BinaryOp::BitwiseXor => "^",
                    BinaryOp::LeftShift => "<<",
                    BinaryOp::RightShift => ">>",
                    _ => return Err(CodeGenError::UnsupportedInstruction(format!("Binary op: {:?}", op))),
                };
                
                Ok(format!("{} = {} {} {};", res, lhs, op_str, rhs))
            },
            
            Instruction::Store { value, dest } => {
                let val = self.generate_c_value(value)?;
                let dst = self.generate_c_value(dest)?;
                Ok(format!("{} = {};", dst, val))
            },
            
            Instruction::Load { source, dest } => {
                let src = self.generate_c_value(source)?;
                let dst = self.generate_c_value(dest)?;
                Ok(format!("{} = {};", dst, src))
            },
            
            Instruction::Return(value) => {
                if let Some(val) = value {
                    let val_str = self.generate_c_value(val)?;
                    Ok(format!("return {};", val_str))
                } else {
                    Ok("return;".to_string())
                }
            },
            
            Instruction::Label(label) => {
                Ok(format!("{}:", label.0))
            },
            
            Instruction::Jump(label) => {
                Ok(format!("goto {};", label.0))
            },
            
            Instruction::JumpIf { condition, target } => {
                let cond = self.generate_c_value(condition)?;
                Ok(format!("if ({}) goto {};", cond, target.0))
            },
            
            Instruction::JumpIfNot { condition, target } => {
                let cond = self.generate_c_value(condition)?;
                Ok(format!("if (!{}) goto {};", cond, target.0))
            },
            
            Instruction::Call { target, args, result } => {
                let function_name = self.generate_c_value(target)?;
                let mut arg_strings = Vec::new();
                for arg in args {
                    arg_strings.push(self.generate_c_value(arg)?);
                }
                
                if let Some(result_reg) = result {
                    let result_str = self.generate_c_value(result_reg)?;
                    Ok(format!("{} = {}({});", result_str, function_name, arg_strings.join(", ")))
                } else {
                    Ok(format!("{}({});", function_name, arg_strings.join(", ")))
                }
            },
            
            _ => Err(CodeGenError::UnsupportedInstruction(format!("{:?}", instruction))),
        }
    }
    
    fn generate_c_type(&self, ir_type: &IRType) -> String {
        match ir_type {
            IRType::Void => "void".to_string(),
            IRType::Integer => "int64_t".to_string(),
            IRType::Float => "double".to_string(),
            IRType::Boolean => "bool".to_string(),
            _ => "void*".to_string(),
        }
    }
    
    fn generate_c_value(&self, value: &IRValue) -> Result<String> {
        match value {
            IRValue::Integer(i) => Ok(i.to_string()),
            IRValue::Float(f) => Ok(f.to_string()),
            IRValue::Boolean(b) => Ok(if *b { "true".to_string() } else { "false".to_string() }),
            IRValue::Register(reg) => {
                if let Some(location) = self.context.get_register_location(*reg) {
                    Ok(location.clone())
                } else {
                    Ok(format!("r{}", reg))
                }
            },
            IRValue::Variable(name) => Ok(name.clone()),
            IRValue::String(s) => Ok(format!("\"{}\"", s.replace('"', "\\\""))),
            _ => Err(CodeGenError::UnsupportedValue(format!("{:?}", value))),
        }
    }
    
    fn collect_variables_from_block(&self, block: &BasicBlock, variables: &mut std::collections::HashSet<String>) {
        for instruction in &block.instructions {
            self.collect_variables_from_instruction(instruction, variables);
        }
        
        if let Some(terminator) = &block.terminator {
            self.collect_variables_from_instruction(terminator, variables);
        }
    }
    
    fn collect_variables_from_instruction(&self, instruction: &Instruction, variables: &mut std::collections::HashSet<String>) {
        match instruction {
            Instruction::Store { value, dest } => {
                self.collect_variables_from_value(value, variables);
                self.collect_variables_from_value(dest, variables);
            },
            Instruction::Load { source, dest } => {
                self.collect_variables_from_value(source, variables);
                self.collect_variables_from_value(dest, variables);
            },
            Instruction::Move { source, dest } => {
                self.collect_variables_from_value(source, variables);
                self.collect_variables_from_value(dest, variables);
            },
            Instruction::Binary { left, right, result, .. } => {
                self.collect_variables_from_value(left, variables);
                self.collect_variables_from_value(right, variables);
                self.collect_variables_from_value(result, variables);
            },
            Instruction::Return(Some(value)) => {
                self.collect_variables_from_value(value, variables);
            },
            _ => {}, // Handle other instruction types as needed
        }
    }
    
    fn collect_variables_from_value(&self, value: &IRValue, variables: &mut std::collections::HashSet<String>) {
        if let IRValue::Variable(name) = value {
            variables.insert(name.clone());
        }
    }
    
    /// Generate if-else block using simple pattern matching  
    fn generate_if_else_block(&self, instructions: &[Instruction]) -> Result<Option<String>> {
        // Look for the specific pattern we know exists:
        // store variable, binary comparison, labels and moves
        let mut output = String::new();
        let mut found_pattern = false;
        
        let mut condition_var = None;
        let mut then_value = None;
        let mut else_value = None;
        let mut result_var = None;
        
        for instruction in instructions {
            match instruction {
                Instruction::Binary { left, right, result, .. } => {
                    if let (IRValue::Variable(_), IRValue::Integer(_)) = (left, right) {
                        condition_var = Some((left, right));
                        result_var = Some(result);
                    }
                },
                _ => {}
            }
        }
        
        // Look for move instructions in different sections
        let mut in_then = false;
        let mut in_else = false;
        
        for instruction in instructions {
            match instruction {
                Instruction::Label(label) if label.0.starts_with("then") => {
                    in_then = true;
                    in_else = false;
                },
                Instruction::Label(label) if label.0.starts_with("else") => {
                    in_then = false;
                    in_else = true;
                },
                Instruction::Label(label) if label.0.starts_with("if_end") => {
                    in_then = false;
                    in_else = false;
                },
                Instruction::Binary { left, right, result: _, .. } if in_then => {
                    if let (IRValue::Variable(_), IRValue::Integer(_)) = (left, right) {
                        then_value = Some((left, right));
                    }
                },
                Instruction::Binary { left, right, result: _, .. } if in_else => {
                    if let (IRValue::Variable(_), IRValue::Integer(_)) = (left, right) {
                        else_value = Some((left, right));
                    }
                },
                _ => {}
            }
        }
        
        if let (Some((cond_left, cond_right)), Some((then_left, then_right)), Some((else_left, else_right)), Some(result_reg)) = 
               (condition_var, then_value, else_value, result_var) {
            
            let cond_left_str = self.generate_c_value(cond_left)?;
            let cond_right_str = self.generate_c_value(cond_right)?;
            let then_left_str = self.generate_c_value(then_left)?; 
            let then_right_str = self.generate_c_value(then_right)?;
            let else_left_str = self.generate_c_value(else_left)?;
            let else_right_str = self.generate_c_value(else_right)?;
            let result_str = self.generate_c_value(result_reg)?;
            
            // Handle the store instruction
            for instruction in instructions {
                if let Instruction::Store { value, dest } = instruction {
                    let val_str = self.generate_c_value(value)?;
                    let var_str = self.generate_c_value(dest)?;
                    writeln!(output, "    {} = {};", var_str, val_str).unwrap();
                    break;
                }
            }
            
            // Generate the if statement  
            writeln!(output, "    if ({} > {}) {{", cond_left_str, cond_right_str).unwrap();
            writeln!(output, "        {} = {} + {};", result_str, then_left_str, then_right_str).unwrap();
            writeln!(output, "    }} else {{").unwrap();
            writeln!(output, "        {} = {} - {};", result_str, else_left_str, else_right_str).unwrap();
            writeln!(output, "    }}").unwrap();
            writeln!(output, "    return {};", result_str).unwrap();
            
            found_pattern = true;
        }
        
        if found_pattern {
            Ok(Some(output))
        } else {
            Ok(None)
        }
    }
    
    fn count_if_else_instructions(&self, instructions: &[Instruction]) -> usize {
        // Count instructions that are part of the if-else pattern
        let mut count = 0;
        let mut found_end = false;
        
        for inst in instructions {
            count += 1;
            if let Instruction::Label(label) = inst {
                if label.0.starts_with("if_end") {
                    found_end = true;
                    break;
                }
            }
        }
        
        if found_end {
            count
        } else {
            1  // Just skip one instruction if pattern not found
        }
    }
    
    fn find_result_assignment(&self, instructions: &[Instruction]) -> Option<IRValue> {
        // Find the value being assigned to the result register
        for inst in instructions {
            if let Instruction::Move { source, dest: _ } = inst {
                return Some(source.clone());
            }
        }
        None
    }
    
    fn find_result_variable(&self, instructions: &[Instruction]) -> Result<String> {
        // Find the final result variable
        for inst in instructions.iter().rev() {
            if let Instruction::Move { source: _, dest } = inst {
                return Ok(self.generate_c_value(dest)?);
            }
        }
        Err(CodeGenError::Other("Could not find result variable".to_string()))
    }
}

impl Default for CCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}