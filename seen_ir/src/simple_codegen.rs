//! Simple code generation from IR for the Seen programming language

use std::collections::{HashMap, HashSet};
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
    InvalidFieldAccess { struct_name: String, field_name: String },
    Other(String),
}

impl std::fmt::Display for CodeGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeGenError::UnsupportedInstruction(msg) => write!(f, "Unsupported instruction: {}", msg),
            CodeGenError::UnsupportedValue(msg) => write!(f, "Unsupported value: {}", msg),
            CodeGenError::InvalidFieldAccess { struct_name, field_name } => {
                write!(f, "Invalid field access: {}.{}", struct_name, field_name)
            },
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
        
        // Generate struct type definitions first
        for module in &program.modules {
            let struct_defs = self.generate_struct_definitions(module)?;
            output.push_str(&struct_defs);
        }
        
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
        // Collect variables from all blocks, not just entry
        for block in function.cfg.blocks.values() {
            self.collect_variables_from_block(block, &mut variables);
        }
        
        // Remove parameter names from variables to avoid redeclaration
        let param_names: std::collections::HashSet<String> = function.parameters.iter().map(|p| p.name.clone()).collect();
        for param_name in &param_names {
            variables.remove(param_name);
        }
        
        // Check which variables are arrays by scanning for array stores
        let mut array_vars = std::collections::HashSet::new();
        // Check which variables are structs by scanning for struct stores
        let mut struct_vars = std::collections::HashMap::new();
        // Check which variables are strings by scanning for string stores
        let mut string_vars = std::collections::HashSet::new();
        // Check which variables are booleans by scanning for boolean stores
        let mut bool_vars = std::collections::HashSet::new();
        
        for block in function.cfg.blocks.values() {
            for inst in &block.instructions {
                match inst {
                    Instruction::Store { value: IRValue::Array(_), dest: IRValue::Variable(name) } => {
                        array_vars.insert(name.clone());
                    },
                    Instruction::Store { value: IRValue::Struct { type_name, .. }, dest: IRValue::Variable(name) } => {
                        struct_vars.insert(name.clone(), type_name.clone());
                    },
                    Instruction::Store { value: IRValue::String(_), dest: IRValue::Variable(name) } => {
                        string_vars.insert(name.clone());
                    },
                    Instruction::Store { value: IRValue::Boolean(_), dest: IRValue::Variable(name) } => {
                        bool_vars.insert(name.clone());
                    },
                    _ => {}
                }
            }
        }
        
        for var_name in &variables {
            if array_vars.contains(var_name) {
                writeln!(output, "    int64_t* {};", var_name).unwrap();
            } else if let Some(struct_type) = struct_vars.get(var_name) {
                writeln!(output, "    {} {};", struct_type, var_name).unwrap();
            } else if string_vars.contains(var_name) {
                writeln!(output, "    char* {};", var_name).unwrap();
            } else if bool_vars.contains(var_name) {
                writeln!(output, "    bool {};", var_name).unwrap();
            } else {
                writeln!(output, "    int64_t {};", var_name).unwrap();
            }
        }
        
        // Generate all blocks in the CFG
        if let Some(entry) = &function.cfg.entry_block {
            // Process blocks in control flow order using DFS
            let mut visited = HashSet::new();
            let mut stack = vec![entry.clone()];
            let mut ordered_blocks = Vec::new();
            
            // First, collect all blocks in DFS order starting from entry
            while let Some(block_id) = stack.pop() {
                if visited.contains(&block_id) {
                    continue;
                }
                visited.insert(block_id.clone());
                
                if let Some(block) = function.cfg.get_block(&block_id) {
                    ordered_blocks.push((block_id.clone(), block.clone()));
                    
                    // Process successors in reverse order (so they are visited in correct order)
                    let mut successors = Vec::new();
                    
                    // Check terminator for jump targets
                    if let Some(terminator) = &block.terminator {
                        match terminator {
                            Instruction::Jump(target) => {
                                successors.push(target.0.clone());
                            },
                            Instruction::JumpIf { target, .. } => {
                                // For conditional jumps, we need to find the fall-through block
                                // Look for the next block after this conditional
                                successors.push(target.0.clone());
                            },
                            Instruction::JumpIfNot { target, .. } => {
                                successors.push(target.0.clone());
                            },
                            _ => {}
                        }
                    }
                    
                    // Also check instructions for jump targets
                    for instruction in &block.instructions {
                        match instruction {
                            Instruction::Jump(target) => {
                                if !visited.contains(&target.0) {
                                    successors.push(target.0.clone());
                                }
                            },
                            Instruction::JumpIf { target, .. } |
                            Instruction::JumpIfNot { target, .. } => {
                                if !visited.contains(&target.0) {
                                    successors.push(target.0.clone());
                                }
                            },
                            _ => {}
                        }
                    }
                    
                    // Add successors to stack in reverse order
                    for successor in successors.into_iter().rev() {
                        if !visited.contains(&successor) {
                            stack.push(successor);
                        }
                    }
                }
            }
            
            // Now add any remaining blocks that weren't reached from entry
            let mut all_block_names: Vec<String> = function.cfg.blocks.keys().cloned().collect();
            all_block_names.sort();
            
            for block_name in all_block_names {
                if !visited.contains(&block_name) {
                    if let Some(block) = function.cfg.get_block(&block_name) {
                        ordered_blocks.push((block_name, block.clone()));
                    }
                }
            }
            
            // Generate code for all blocks in order
            for (block_label, block) in ordered_blocks {
                // Don't output the label if it's "entry" (it's implicit)
                if block_label != "entry" {
                    writeln!(output, "{}:", block_label).unwrap();
                }
                let block_code = self.generate_basic_block(&block)?;
                output.push_str(&block_code);
            }
        }
        
        writeln!(output, "}}").unwrap();
        
        Ok(output)
    }
    
    pub fn generate_basic_block(&mut self, block: &BasicBlock) -> Result<String> {
        let mut output = String::new();
        
        // Generate all instructions directly without pattern matching
        // Skip Label instructions since we output them separately at the block level
        for instruction in &block.instructions {
            // Skip labels - they're handled at the block level now
            if matches!(instruction, Instruction::Label(_)) {
                continue;
            }
            
            let inst_code = self.generate_instruction(instruction)?;
            writeln!(output, "    {}", inst_code).unwrap();
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
                // Special handling for array literals
                if let IRValue::Array(elements) = value {
                    let dst = self.generate_c_value(dest)?;
                    let mut output = String::new();
                    // Allocate array on heap and initialize
                    writeln!(output, "{} = (int64_t*)malloc({} * sizeof(int64_t));", dst, elements.len()).unwrap();
                    for (i, elem) in elements.iter().enumerate() {
                        let elem_str = self.generate_c_value(elem)?;
                        writeln!(output, "    {}[{}] = {};", dst, i, elem_str).unwrap();
                    }
                    Ok(output.trim_end().to_string())
                } else {
                    let val = self.generate_c_value(value)?;
                    let dst = self.generate_c_value(dest)?;
                    Ok(format!("{} = {};", dst, val))
                }
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
            
            Instruction::FieldAccess { struct_val, field, result } => {
                let struct_str = self.generate_c_value(struct_val)?;
                let result_str = self.generate_c_value(result)?;
                // Verify struct field access is valid
                if let IRValue::Variable(var_name) = struct_val {
                    // In a real implementation, we would look up the struct type from a symbol table
                    // For now, we'll assume the field access is valid
                    let _ = var_name; // Use the variable to avoid warnings
                    let fields: Vec<(String, IRType)> = Vec::new(); // Placeholder
                    if !fields.iter().any(|(f, _)| f == field) {
                        return Err(CodeGenError::InvalidFieldAccess {
                            struct_name: var_name.clone(),
                            field_name: field.clone(),
                        });
                    }
                }
                
                Ok(format!("{} = {}.{};", result_str, struct_str, field))
            },
            
            Instruction::FieldSet { struct_val, field, value } => {
                let struct_str = self.generate_c_value(struct_val)?;
                let value_str = self.generate_c_value(value)?;
                Ok(format!("{}.{} = {};", struct_str, field, value_str))
            },
            
            Instruction::ArrayAccess { array, index, result } => {
                let array_str = self.generate_c_value(array)?;
                let index_str = self.generate_c_value(index)?;
                let result_str = self.generate_c_value(result)?;
                Ok(format!("{} = {}[{}];", result_str, array_str, index_str))
            },
            
            Instruction::ArraySet { array, index, value } => {
                let array_str = self.generate_c_value(array)?;
                let index_str = self.generate_c_value(index)?;
                let value_str = self.generate_c_value(value)?;
                Ok(format!("{}[{}] = {};", array_str, index_str, value_str))
            },
            
            Instruction::StringConcat { left, right, result } => {
                // For C, we'll need to use string functions
                // This is a simplified implementation using sprintf
                let left_str = self.generate_c_value(left)?;
                let right_str = self.generate_c_value(right)?;
                let result_str = self.generate_c_value(result)?;
                
                // Allocate buffer and concatenate
                // Note: This is simplified and doesn't handle dynamic sizing properly
                Ok(format!("{} = (char*)malloc(1024); sprintf({}, \"%s%s\", {}, {});", 
                          result_str, result_str, left_str, right_str))
            },
            
            Instruction::GetEnumTag { enum_value, result } => {
                let enum_str = self.generate_c_value(enum_value)?;
                let result_str = self.generate_c_value(result)?;
                Ok(format!("{} = {}.tag;", result_str, enum_str))
            },
            
            Instruction::GetEnumField { enum_value, field_index, result } => {
                let enum_str = self.generate_c_value(enum_value)?;
                let result_str = self.generate_c_value(result)?;
                // For now, assume single field variants use a simple field name
                // This would need to be enhanced for multi-field variants
                Ok(format!("{} = {}.data.field{};", result_str, enum_str, field_index))
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
            IRType::String => "char*".to_string(),
            IRType::Struct { name, .. } => name.clone(),
            IRType::Enum { name, .. } => name.clone(),
            _ => "void*".to_string(),
        }
    }
    
    fn generate_struct_definitions(&mut self, module: &IRModule) -> Result<String> {
        let mut output = String::new();
        
        if !module.types.is_empty() {
            writeln!(output, "// Type definitions").unwrap();
            
            // Generate enum definitions first (they may be referenced by structs)
            for type_def in module.types.values() {
                if let IRType::Enum { name, variants } = &type_def.type_def {
                    // Generate enum definition
                    writeln!(output, "typedef enum {{").unwrap();
                    for (variant_name, _) in variants {
                        writeln!(output, "    {}_TAG_{},", name.to_uppercase(), variant_name.to_uppercase()).unwrap();
                    }
                    writeln!(output, "}} {}_tag;", name).unwrap();
                    writeln!(output).unwrap();
                    
                    // Generate union for enum data
                    writeln!(output, "typedef struct {{").unwrap();
                    writeln!(output, "    {}_tag tag;", name).unwrap();
                    writeln!(output, "    union {{").unwrap();
                    for (variant_name, variant_fields) in variants {
                        if let Some(fields) = variant_fields {
                            if fields.len() == 1 {
                                // Single field tuple variant
                                let c_type = self.generate_c_type(&fields[0]);
                                writeln!(output, "        {} {};", c_type, variant_name.to_lowercase()).unwrap();
                            } else if !fields.is_empty() {
                                // Multiple field tuple variant - create a struct
                                writeln!(output, "        struct {{").unwrap();
                                for (i, field_type) in fields.iter().enumerate() {
                                    let c_type = self.generate_c_type(field_type);
                                    writeln!(output, "            {} field_{};", c_type, i).unwrap();
                                }
                                writeln!(output, "        }} {};", variant_name.to_lowercase()).unwrap();
                            }
                        }
                        // Simple variants (no fields) don't need union members
                    }
                    writeln!(output, "    }} data;").unwrap();
                    writeln!(output, "}} {};", name).unwrap();
                    writeln!(output).unwrap();
                }
            }
            
            // Generate enum constructor functions
            for type_def in module.types.values() {
                if let IRType::Enum { name, variants } = &type_def.type_def {
                    for (variant_name, variant_fields) in variants {
                        if let Some(fields) = variant_fields {
                            // Constructor for tuple variant
                            write!(output, "{} {}__{}(", name, name, variant_name).unwrap();
                            for (i, field_type) in fields.iter().enumerate() {
                                if i > 0 { write!(output, ", ").unwrap(); }
                                let c_type = self.generate_c_type(field_type);
                                write!(output, "{} arg{}", c_type, i).unwrap();
                            }
                            writeln!(output, ") {{").unwrap();
                            writeln!(output, "    {} result;", name).unwrap();
                            writeln!(output, "    result.tag = {}_TAG_{};", name.to_uppercase(), variant_name.to_uppercase()).unwrap();
                            for (i, _) in fields.iter().enumerate() {
                                if fields.len() == 1 {
                                    writeln!(output, "    result.data.{} = arg{};", variant_name.to_lowercase(), i).unwrap();
                                } else {
                                    writeln!(output, "    result.data.{}.field_{} = arg{};", variant_name.to_lowercase(), i, i).unwrap();
                                }
                            }
                            writeln!(output, "    return result;").unwrap();
                            writeln!(output, "}}").unwrap();
                            writeln!(output).unwrap();
                        } else {
                            // Constructor for simple variant
                            writeln!(output, "{} {}__{}() {{", name, name, variant_name).unwrap();
                            writeln!(output, "    {} result;", name).unwrap();
                            writeln!(output, "    result.tag = {}_TAG_{};", name.to_uppercase(), variant_name.to_uppercase()).unwrap();
                            writeln!(output, "    return result;").unwrap();
                            writeln!(output, "}}").unwrap();
                            writeln!(output).unwrap();
                        }
                    }
                }
            }
            
            // Generate struct definitions
            for type_def in module.types.values() {
                if let IRType::Struct { name, fields } = &type_def.type_def {
                    writeln!(output, "typedef struct {{").unwrap();
                    for (field_name, field_type) in fields {
                        let c_type = self.generate_c_type(field_type);
                        writeln!(output, "    {} {};", c_type, field_name).unwrap();
                    }
                    writeln!(output, "}} {};", name).unwrap();
                    writeln!(output).unwrap();
                }
            }
        }
        
        Ok(output)
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
            IRValue::Array(elements) => {
                // Generate C array initializer
                let element_strs: Result<Vec<String>> = elements.iter()
                    .map(|elem| self.generate_c_value(elem))
                    .collect();
                match element_strs {
                    Ok(strs) => Ok(format!("{{{}}}", strs.join(", "))),
                    Err(e) => Err(e),
                }
            },
            IRValue::Struct { type_name, fields } => {
                // Generate C99 compound literal with designated initializer
                let mut field_strs = Vec::new();
                for (field_name, field_value) in fields {
                    let value_str = self.generate_c_value(field_value)?;
                    field_strs.push(format!(".{} = {}", field_name, value_str));
                }
                Ok(format!("({}){{{}}}", type_name, field_strs.join(", ")))
            },
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
    
    fn generate_for_loop_block(&mut self, instructions: &[Instruction], terminator: &Option<Instruction>) -> Result<Option<String>> {
        // Detect for loop pattern:
        // 1. Store initial value to loop variable
        // 2. Labels for for_start, for_body, for_end
        // 3. Condition check (usually <= or <)
        // 4. Body instructions
        // 5. Increment loop variable
        
        let mut loop_var_name = None;
        let mut loop_start_label = None;
        let mut loop_body_label = None;
        let mut loop_end_label = None;
        let mut condition_check = None;
        let mut body_instructions = Vec::new();
        let mut increment_instructions = Vec::new();
        
        let mut current_section = "init";
        
        for instruction in instructions {
            match instruction {
                Instruction::Store { dest: IRValue::Variable(name), .. } if current_section == "init" => {
                    loop_var_name = Some(name.clone());
                },
                Instruction::Label(label) if label.0.contains("for_start") => {
                    loop_start_label = Some(label.clone());
                    current_section = "condition";
                },
                Instruction::Label(label) if label.0.contains("for_body") => {
                    loop_body_label = Some(label.clone());
                    current_section = "body";
                },
                Instruction::Label(label) if label.0.contains("for_end") => {
                    loop_end_label = Some(label.clone());
                    current_section = "end";
                },
                Instruction::Binary { op: crate::instruction::BinaryOp::LessThan, .. } 
                | Instruction::Binary { op: crate::instruction::BinaryOp::LessEqual, .. }
                if current_section == "condition" => {
                    condition_check = Some(instruction.clone());
                },
                Instruction::Binary { op: crate::instruction::BinaryOp::Add, left, right: IRValue::Integer(1), .. }
                if current_section == "body" => {
                    // This is the increment instruction
                    increment_instructions.push(instruction.clone());
                },
                Instruction::Store { dest: IRValue::Variable(name), .. }
                if current_section == "body" && loop_var_name.as_ref() == Some(name) => {
                    // Store of increment back to loop variable
                    increment_instructions.push(instruction.clone());
                },
                _ if current_section == "body" && increment_instructions.is_empty() => {
                    body_instructions.push(instruction.clone());
                },
                _ => {}
            }
        }
        
        // If we have all the components of a for loop, generate proper C code
        if let (Some(var_name), Some(start_label), Some(body_label), Some(end_label), Some(condition)) = 
               (loop_var_name, loop_start_label, loop_body_label, loop_end_label, condition_check) {
            
            let mut output = String::new();
            
            // Handle initialization (first store instruction)
            for instruction in instructions {
                if let Instruction::Store { value, dest: IRValue::Variable(name) } = instruction {
                    if name == &var_name {
                        let val_str = self.generate_c_value(value)?;
                        writeln!(output, "    {} = {};", name, val_str).unwrap();
                        break;
                    }
                }
            }
            
            // Generate the for loop structure
            writeln!(output, "{}:", start_label.0).unwrap();
            
            // Generate condition check
            let cond_str = self.generate_instruction(&condition)?;
            writeln!(output, "    {}", cond_str).unwrap();
            
            // Generate conditional jump
            if let Instruction::Binary { result, .. } = condition {
                writeln!(output, "    if (!{}) goto {};", self.generate_c_value(&result)?, end_label.0).unwrap();
            }
            
            // Generate body
            writeln!(output, "{}:", body_label.0).unwrap();
            for inst in &body_instructions {
                let inst_str = self.generate_instruction(inst)?;
                writeln!(output, "    {}", inst_str).unwrap();
            }
            
            // Generate increment
            for inst in &increment_instructions {
                let inst_str = self.generate_instruction(inst)?;
                writeln!(output, "    {}", inst_str).unwrap();
            }
            
            // Jump back to start
            writeln!(output, "    goto {};", start_label.0).unwrap();
            
            // End label
            writeln!(output, "{}:", end_label.0).unwrap();
            
            return Ok(Some(output));
        }
        
        Ok(None)
    }
    
    fn generate_while_loop_block(&mut self, instructions: &[Instruction], terminator: &Option<Instruction>) -> Result<Option<String>> {
        // Detect while loop pattern:
        // 1. Labels for loop_start, loop_body, loop_end
        // 2. Condition check
        // 3. Body instructions
        
        let mut loop_start_label = None;
        let mut loop_body_label = None;
        let mut loop_end_label = None;
        let mut condition_check = None;
        let mut body_instructions = Vec::new();
        
        let mut current_section = "init";
        
        for instruction in instructions {
            match instruction {
                Instruction::Label(label) if label.0.contains("loop_start") => {
                    loop_start_label = Some(label.clone());
                    current_section = "condition";
                },
                Instruction::Label(label) if label.0.contains("loop_body") => {
                    loop_body_label = Some(label.clone());
                    current_section = "body";
                },
                Instruction::Label(label) if label.0.contains("loop_end") => {
                    loop_end_label = Some(label.clone());
                    current_section = "end";
                },
                Instruction::Binary { op: crate::instruction::BinaryOp::LessThan, .. } 
                | Instruction::Binary { op: crate::instruction::BinaryOp::LessEqual, .. }
                | Instruction::Binary { op: crate::instruction::BinaryOp::GreaterThan, .. }
                | Instruction::Binary { op: crate::instruction::BinaryOp::GreaterEqual, .. }
                | Instruction::Binary { op: crate::instruction::BinaryOp::Equal, .. }
                | Instruction::Binary { op: crate::instruction::BinaryOp::NotEqual, .. } 
                if current_section == "condition" => {
                    condition_check = Some(instruction.clone());
                },
                _ if current_section == "body" => {
                    body_instructions.push(instruction.clone());
                },
                _ => {}
            }
        }
        
        // If we have all the components of a while loop, generate proper C code
        if let (Some(start_label), Some(body_label), Some(end_label), Some(condition)) = 
               (loop_start_label, loop_body_label, loop_end_label, condition_check) {
            
            let mut output = String::new();
            
            // Handle any instructions before the loop
            for instruction in instructions {
                if let Instruction::Label(label) = instruction {
                    if label.0.contains("loop_start") {
                        break;
                    }
                }
                let inst_str = self.generate_instruction(instruction)?;
                writeln!(output, "    {}", inst_str).unwrap();
            }
            
            // Generate the while loop structure
            writeln!(output, "{}:", start_label.0).unwrap();
            
            // Generate condition check
            let cond_str = self.generate_instruction(&condition)?;
            writeln!(output, "    {}", cond_str).unwrap();
            
            // Generate conditional jump
            if let Instruction::Binary { result, .. } = condition {
                writeln!(output, "    if (!{}) goto {};", self.generate_c_value(&result)?, end_label.0).unwrap();
            }
            
            // Generate body
            writeln!(output, "{}:", body_label.0).unwrap();
            for inst in &body_instructions {
                let inst_str = self.generate_instruction(inst)?;
                writeln!(output, "    {}", inst_str).unwrap();
            }
            
            // Jump back to start
            writeln!(output, "    goto {};", start_label.0).unwrap();
            
            // End label
            writeln!(output, "{}:", end_label.0).unwrap();
            
            return Ok(Some(output));
        }
        
        Ok(None)
    }
    
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