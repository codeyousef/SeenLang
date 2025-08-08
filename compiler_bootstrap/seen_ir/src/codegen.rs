//! Code generation with LLVM backend

use crate::ir::*;
use seen_common::SeenResult;

/// Code generator with LLVM backend
pub struct CodeGenerator {
    module_name: String,
    debug_info_enabled: bool,
    calling_convention: String,
    target_triple: String,
    optimization_level: u32,
    string_constants: std::collections::HashMap<String, usize>,
}

impl CodeGenerator {
    pub fn new(module_name: String) -> Self {
        Self {
            module_name,
            debug_info_enabled: false,
            calling_convention: "fastcc".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            optimization_level: 2,
            string_constants: std::collections::HashMap::new(),
        }
    }
    
    /// Enable or disable debug information generation
    pub fn enable_debug_info(&mut self, enabled: bool) {
        self.debug_info_enabled = enabled;
    }
    
    /// Set calling convention (C, fastcc, etc.)
    pub fn set_calling_convention(&mut self, convention: &str) {
        self.calling_convention = convention.to_string();
    }
    
    /// Set target triple for cross-compilation
    pub fn set_target_triple(&mut self, triple: &str) {
        self.target_triple = triple.to_string();
    }
    
    /// Set optimization level (0-3)
    pub fn set_optimization_level(&mut self, level: u32) {
        self.optimization_level = level.clamp(0, 3);
    }
    
    /// Generate LLVM IR from module
    pub fn generate_llvm_ir(&mut self, module: &Module) -> SeenResult<String> {
        // Pre-calculate approximate size to avoid reallocations
        let estimated_size = module.functions.iter()
            .map(|f| f.blocks.iter().map(|b| b.instructions.len() * 50).sum::<usize>())
            .sum::<usize>() + 1000; // 50 chars per instruction estimate + headers
        
        let mut llvm_ir = String::with_capacity(estimated_size);
        
        // Module header with target information
        use std::fmt::Write;
        let _ = write!(&mut llvm_ir, "target triple = \"{}\"\n", self.target_triple);
        let _ = write!(&mut llvm_ir, "; Module: {}\n\n", module.name);
        
        // Debug info metadata if enabled
        if self.debug_info_enabled {
            llvm_ir.push_str(&self.generate_debug_metadata());
        }
        
        // Generate functions
        for function in &module.functions {
            llvm_ir.push_str(&self.generate_function_ir(function)?);
            llvm_ir.push('\n');
        }
        
        // Debug info declarations if enabled
        if self.debug_info_enabled {
            llvm_ir.push_str(&self.generate_debug_declarations());
        }
        
        Ok(llvm_ir)
    }
    
    /// Generate LLVM IR for a function
    fn generate_function_ir(&mut self, function: &Function) -> SeenResult<String> {
        // Pre-allocate with estimated size
        let estimated_size = function.blocks.iter()
            .map(|b| b.instructions.len() * 50)
            .sum::<usize>() + 200;
        let mut ir = String::with_capacity(estimated_size);
        
        use std::fmt::Write;
        
        // Function signature with proper linkage and calling convention
        let return_type = if function.name == "main" { "i32" } else { "i32" };
        
        // Determine linkage - C ABI functions should be external
        let linkage = if function.name == "main" { 
            "" 
        } else if self.calling_convention == "C" { 
            "" // External linkage for C ABI functions
        } else { 
            "internal" 
        };
        
        // Build function signature with proper spacing
        if linkage.is_empty() {
            let _ = write!(&mut ir, "define {} @{}(", return_type, function.name);
        } else {
            let _ = write!(&mut ir, "define {} {} @{}(", linkage, return_type, function.name);
        }
        
        // Parameters
        for (i, param) in function.params.iter().enumerate() {
            if i > 0 { ir.push_str(", "); }
            let _ = write!(&mut ir, "i32 %{}", param);
        }
        
        // Add attributes for main function
        if function.name == "main" {
            ir.push_str(") nounwind {\n");
        } else {
            let calling_conv = if self.calling_convention == "C" { "" } else { "fastcc" };
            let _ = write!(&mut ir, ") {} {{\n", calling_conv);
        }
        
        // Generate basic blocks
        for block in &function.blocks {
            ir.push_str(&self.generate_basic_block_ir(block)?);
        }
        
        ir.push_str("}\n");
        Ok(ir)
    }
    
    /// Generate LLVM IR for a basic block
    fn generate_basic_block_ir(&mut self, block: &BasicBlock) -> SeenResult<String> {
        let mut ir = String::with_capacity(block.instructions.len() * 50 + 50);
        use std::fmt::Write;
        
        // Block label
        let _ = write!(&mut ir, "{}:\n", block.label);
        
        // Generate instructions
        for instruction in &block.instructions {
            ir.push_str("  ");
            ir.push_str(&self.generate_instruction_ir(instruction)?);
            ir.push('\n');
        }
        
        Ok(ir)
    }
    
    /// Generate LLVM IR for an instruction
    fn generate_instruction_ir(&mut self, instruction: &Instruction) -> SeenResult<String> {
        use std::fmt::Write;
        let mut ir = String::with_capacity(80);
        
        match instruction {
            Instruction::Load { dest, src } => {
                let _ = write!(&mut ir, "%{} = load i32, i32* %{}, align 4", dest, src);
            }
            Instruction::Store { dest, src } => {
                let _ = write!(&mut ir, "store i32 %{}, i32* %{}, align 4", src, dest);
            }
            Instruction::Call { dest, func, args } => {
                if let Some(dest_reg) = dest {
                    let _ = write!(&mut ir, "%{} = call i32 @{}(", dest_reg, func);
                } else {
                    let _ = write!(&mut ir, "call void @{}(", func);
                }
                
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { ir.push_str(", "); }
                    ir.push_str(&self.format_value(arg));
                }
                ir.push(')');
            }
            Instruction::Return { value } => {
                if let Some(val) = value {
                    let _ = write!(&mut ir, "ret i32 {}", self.format_value(val));
                } else {
                    ir.push_str("ret void");
                }
            }
            Instruction::Binary { dest, op, left, right } => {
                let op_str = match op {
                    crate::BinaryOp::Add => "add",
                    crate::BinaryOp::Sub => "sub",
                    crate::BinaryOp::Mul => "mul",
                    crate::BinaryOp::Div => "sdiv",
                    crate::BinaryOp::Mod => "srem",
                    crate::BinaryOp::And => "and",
                    crate::BinaryOp::Or => "or",
                    crate::BinaryOp::Xor => "xor",
                };
                let _ = write!(&mut ir, "%{} = {} i32 {}, {}", 
                    dest, op_str, self.format_value(left), self.format_value(right));
            }
            Instruction::Compare { dest, op, left, right } => {
                let cond = match op {
                    crate::CompareOp::Eq => "eq",
                    crate::CompareOp::Ne => "ne",
                    crate::CompareOp::Lt => "slt",
                    crate::CompareOp::Le => "sle",
                    crate::CompareOp::Gt => "sgt",
                    crate::CompareOp::Ge => "sge",
                };
                let _ = write!(&mut ir, "%{} = icmp {} i32 {}, {}", 
                    dest, cond, self.format_value(left), self.format_value(right));
            }
            Instruction::Branch { condition, true_label, false_label } => {
                if let Some(cond) = condition {
                    if let Some(false_l) = false_label {
                        let _ = write!(&mut ir, "br i1 %{}, label %{}, label %{}", 
                            cond, true_label, false_l);
                    } else {
                        let _ = write!(&mut ir, "br label %{}", true_label);
                    }
                } else {
                    let _ = write!(&mut ir, "br label %{}", true_label);
                }
            }
            Instruction::Phi { dest, values } => {
                let _ = write!(&mut ir, "%{} = phi i32 ", dest);
                for (i, (val, label)) in values.iter().enumerate() {
                    if i > 0 { ir.push_str(", "); }
                    let _ = write!(&mut ir, "[{}, %{}]", self.format_value(val), label);
                }
            }
            Instruction::Alloca { dest, ty } => {
                let type_str = self.format_type(ty);
                let _ = write!(&mut ir, "%{} = alloca {}, align 4", dest, type_str);
            }
            Instruction::Nop => {
                ir.push_str("; nop");
            }
        }
        
        Ok(ir)
    }
    
    /// Format a value for LLVM IR
    fn format_value(&mut self, value: &crate::Value) -> String {
        match value {
            crate::Value::Register(reg) => format!("%{}", reg),
            crate::Value::Integer(val) => val.to_string(),
            crate::Value::Float(val) => format!("{:.6}", val),
            crate::Value::Boolean(val) => if *val { "1" } else { "0" }.to_string(),
            crate::Value::String(s) => {
                // Generate a string constant reference
                // Strings are stored as global constants
                let str_id = self.string_constants.len();
                self.string_constants.insert(s.clone(), str_id);
                format!("@.str.{}", str_id)
            }
        }
    }
    
    /// Format a type for LLVM IR
    fn format_type(&self, ty: &crate::Type) -> String {
        match ty {
            crate::Type::I32 => "i32".to_string(),
            crate::Type::I64 => "i64".to_string(),
            crate::Type::F32 => "float".to_string(),
            crate::Type::F64 => "double".to_string(),
            crate::Type::Bool => "i1".to_string(),
            crate::Type::Ptr(inner) => format!("{}*", self.format_type(inner)),
            crate::Type::Void => "void".to_string(),
        }
    }
    
    /// Generate debug metadata
    fn generate_debug_metadata(&self) -> String {
        use std::fmt::Write;
        let mut result = String::with_capacity(700);
        let _ = write!(&mut result, r#"!llvm.dbg.cu = !{{!0}}
!llvm.module.flags = !{{!1, !2}}

!0 = !DICompileUnit(language: DW_LANG_C99, file: !3, producer: "Seen Compiler", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, subprograms: !4)
!1 = !{{i32 2, !"Dwarf Version", i32 4}}
!2 = !{{i32 2, !"Debug Info Version", i32 3}}
!3 = !DIFile(filename: "{}.seen", directory: "/tmp")
!4 = !{{!5}}
!5 = !DISubprogram(name: "main", scope: !3, file: !3, line: 1, type: !6, isLocal: false, isDefinition: true, scopeLine: 1, isOptimized: false, unit: !0)
!6 = !DISubroutineType(types: !7)
!7 = !{{null}}
!8 = !DILocation(line: 1, column: 1, scope: !5)

"#, self.module_name);
        result
    }
    
    /// Generate debug declarations
    fn generate_debug_declarations(&self) -> String {
        r#"declare void @llvm.dbg.declare(metadata, metadata, metadata)

"#.to_string()
    }
    
    /// Legacy generate method for compatibility
    pub fn generate(&mut self) -> SeenResult<String> {
        // Create a simple test module
        let test_module = Module {
            name: self.module_name.clone(),
            functions: vec![Function {
                name: "main".to_string(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        Instruction::Return { value: Some(crate::Value::Integer(0)) },
                    ],
                }],
            }],
        };
        
        self.generate_llvm_ir(&test_module)
    }
}